/**
 * fff-backed quick-open dialog. It deliberately shares the command palette's
 * primitive shell while owning file-specific behavior: native indexed search,
 * frecency tracking, and detached syntax-highlighted previews.
 */

import { useEffect, useMemo, useRef, useState, useTransition } from "react";
import { Effect } from "effect";
import type { FileFinderItem, FileFinderLocation } from "../platform/fileFinder";
import type { AppRuntime } from "../runtime";
import { highlightSnippet, type SnippetSpan } from "../editor/snippetHighlight";
import { FileIcon } from "../workbench/fileIcons";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/ui/command";

const SEARCH_LIMIT = 60;
const SEARCH_DEBOUNCE_MS = 110;
const PREVIEW_DEBOUNCE_MS = 0;
const MAX_PREVIEW_BYTES = 180_000;
const MAX_PREVIEW_CHARS = 12_000;
const MAX_PREVIEW_LINES = 220;

type PreviewState =
  | { readonly kind: "idle" }
  | { readonly kind: "loading"; readonly path: string }
  | { readonly kind: "ready"; readonly path: string; readonly lines: readonly PreviewLine[]; readonly truncated: boolean }
  | { readonly kind: "empty"; readonly message: string }
  | { readonly kind: "error"; readonly message: string };

type PreviewLine = { readonly number: number; readonly spans: readonly SnippetSpan[] };

type SearchMeta = {
  readonly totalMatched: number;
  readonly totalFiles: number;
  readonly location?: FileFinderLocation;
};

export interface FileFinderProps {
  readonly open: boolean;
  readonly runtime: AppRuntime;
  readonly onOpenChange: (open: boolean) => void;
}

const joinRoot = (root: string, relativePath: string): string =>
  root === "/" ? `/${relativePath}` : `${root}/${relativePath}`;

const dirname = (relativePath: string): string => {
  const index = relativePath.lastIndexOf("/");
  return index === -1 ? "" : relativePath.slice(0, index);
};

const languageTag = (relativePath: string): string => {
  const name = relativePath.split("/").pop() ?? relativePath;
  const index = name.lastIndexOf(".");
  return index === -1 ? name.toLowerCase() : name.slice(index + 1).toLowerCase();
};

const formatBytes = (bytes: number): string => {
  if (bytes < 1_024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${Math.round(bytes / 1_024)} KB`;
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
};

const spansToLines = (spans: readonly SnippetSpan[]): readonly PreviewLine[] => {
  const lines: SnippetSpan[][] = [[]];
  for (const span of spans) {
    const parts = span.text.split("\n");
    for (let index = 0; index < parts.length; index++) {
      if (index > 0) lines.push([]);
      const text = parts[index];
      if (text !== undefined && text.length > 0) {
        lines[lines.length - 1]!.push({ text, ...(span.color !== undefined ? { color: span.color } : {}) });
      }
    }
  }
  return lines.map((line, index) => ({ number: index + 1, spans: line }));
};

const plainLines = (text: string): readonly PreviewLine[] =>
  text.split("\n").map((line, index) => ({ number: index + 1, spans: line.length > 0 ? [{ text: line }] : [] }));

const trimPreviewContent = (content: string): { readonly code: string; readonly truncated: boolean } => {
  let end = Math.min(content.length, MAX_PREVIEW_CHARS);
  let lineCount = 0;
  for (let index = 0; index < end; index++) {
    if (content.charCodeAt(index) === 10) {
      lineCount++;
      if (lineCount >= MAX_PREVIEW_LINES) {
        end = index;
        break;
      }
    }
  }
  return { code: content.slice(0, end), truncated: end < content.length };
};

const schedulePreviewWork = (work: () => void): (() => void) => {
  const id = window.requestAnimationFrame(work);
  return () => window.cancelAnimationFrame(id);
};

const locationPosition = (location: FileFinderLocation | undefined): { line: number; character: number } => {
  if (location === undefined) return { line: 0, character: 0 };
  if (location.type === "line") return { line: Math.max(0, location.line - 1), character: 0 };
  if (location.type === "position") {
    return { line: Math.max(0, location.line - 1), character: Math.max(0, location.col - 1) };
  }
  return {
    line: Math.max(0, location.start.line - 1),
    character: Math.max(0, location.start.col - 1),
  };
};

function currentFileRelative(runtime: AppRuntime, root: string): string | undefined {
  const path = runtime.focusedPane()?.fileInfo()?.path;
  if (path === undefined) return undefined;
  const prefix = root === "/" ? "/" : `${root}/`;
  return path.startsWith(prefix) ? path.slice(prefix.length) : undefined;
}

export function FileFinder({ open, runtime, onOpenChange }: FileFinderProps) {
  const [root, setRoot] = useState<string | null>(runtime.workspaceRoot);
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<readonly FileFinderItem[]>([]);
  const [selectedValue, setSelectedValue] = useState("");
  const [searchMeta, setSearchMeta] = useState<SearchMeta>({ totalMatched: 0, totalFiles: 0 });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewState>({ kind: "idle" });
  const [, startResultTransition] = useTransition();
  const inputRef = useRef<HTMLInputElement>(null);
  const searchSeq = useRef(0);
  const previewSeq = useRef(0);

  useEffect(() => runtime.subscribeWorkspaceRoot(setRoot), [runtime]);

  useEffect(() => {
    if (!open) return;
    const id = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setItems([]);
      setSelectedValue("");
      setSearchMeta({ totalMatched: 0, totalFiles: 0 });
      setError(null);
      setPreview({ kind: "idle" });
    }
  }, [open]);

  useEffect(() => {
    if (open) previewSeq.current++;
  }, [open, query]);

  useEffect(() => {
    if (!open) return;
    const seq = ++searchSeq.current;
    if (root === null) {
      setItems([]);
      setSelectedValue("");
      setSearchMeta({ totalMatched: 0, totalFiles: 0 });
      setError(null);
      setLoading(false);
      return;
    }
    const finder = runtime.fileFinderPort();
    if (finder === undefined) {
      setItems([]);
      setSelectedValue("");
      setSearchMeta({ totalMatched: 0, totalFiles: 0 });
      setError("File finder is unavailable in this host.");
      setLoading(false);
      return;
    }
    const currentFile = currentFileRelative(runtime, root);
    const timeout = window.setTimeout(() => {
      setLoading(true);
      finder
        .search({
          root,
          query,
          limit: SEARCH_LIMIT,
          ...(currentFile !== undefined ? { currentFile } : {}),
        })
        .then((result) => {
          if (searchSeq.current !== seq) return;
          startResultTransition(() => {
            setItems(result.items);
            setSearchMeta({
              totalMatched: result.totalMatched,
              totalFiles: result.totalFiles,
              ...(result.location !== undefined ? { location: result.location } : {}),
            });
            setError(null);
            setSelectedValue((current) =>
              result.items.some((item) => item.relativePath === current)
                ? current
                : (result.items[0]?.relativePath ?? ""),
            );
          });
        })
        .catch((caught) => {
          if (searchSeq.current !== seq) return;
          startResultTransition(() => {
            setItems([]);
            setSelectedValue("");
            setSearchMeta({ totalMatched: 0, totalFiles: 0 });
            setError(caught instanceof Error ? caught.message : String(caught));
          });
        })
        .finally(() => {
          if (searchSeq.current === seq) setLoading(false);
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timeout);
  }, [open, query, root, runtime]);

  const selected = useMemo(
    () => items.find((item) => item.relativePath === selectedValue),
    [items, selectedValue],
  );

  useEffect(() => {
    if (!open || root === null || selected === undefined) {
      if (open) setPreview({ kind: "idle" });
      return;
    }
    const seq = ++previewSeq.current;
    if (loading) return;

    let cancelPreviewWork: (() => void) | undefined;
    const timeout = window.setTimeout(() => {
      const absolutePath = joinRoot(root, selected.relativePath);
      if (selected.size > MAX_PREVIEW_BYTES) {
        setPreview({ kind: "empty", message: `Preview skipped · ${formatBytes(selected.size)}` });
        return;
      }
      const fileSystem = runtime.fileSystem();
      if (fileSystem === undefined) {
        setPreview({ kind: "empty", message: "Preview unavailable in this host." });
        return;
      }
      setPreview({ kind: "loading", path: selected.relativePath });
      Effect.runPromise(fileSystem.readFile(absolutePath))
        .then((content) => {
          if (previewSeq.current !== seq) return;
          if (content.includes("\u0000")) {
            setPreview({ kind: "empty", message: "Binary file preview skipped." });
            return;
          }
          const { code, truncated } = trimPreviewContent(content);
          cancelPreviewWork = schedulePreviewWork(() => {
            if (previewSeq.current !== seq) return;
            const spans = highlightSnippet(code, languageTag(selected.relativePath));
            setPreview({
              kind: "ready",
              path: selected.relativePath,
              lines: spans === null ? plainLines(code) : spansToLines(spans),
              truncated,
            });
          });
        })
        .catch((caught) => {
          if (previewSeq.current !== seq) return;
          setPreview({ kind: "error", message: caught instanceof Error ? caught.message : String(caught) });
        });
    }, PREVIEW_DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timeout);
      cancelPreviewWork?.();
    };
  }, [loading, open, query, root, runtime, selected]);

  const openSelected = (item: FileFinderItem) => {
    if (root === null) return;
    const finder = runtime.fileFinderPort();
    void finder?.track({ root, query, relativePath: item.relativePath });
    onOpenChange(false);
    const target = joinRoot(root, item.relativePath);
    const position = locationPosition(searchMeta.location);
    void runtime.jumpToLocation(target, position);
  };

  const status =
    root === null
      ? "No folder open"
      : loading
        ? "Indexing…"
        : `${searchMeta.totalMatched.toLocaleString()} matches · ${searchMeta.totalFiles.toLocaleString()} files`;

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Find File"
      description="Search files in the current workspace"
      className="file-finder-dialog command-palette-dialog"
    >
      <Command
        className="command-palette file-finder"
        value={selectedValue}
        onValueChange={setSelectedValue}
        shouldFilter={false}
        loop
      >
        <CommandInput
          ref={inputRef}
          className="command-palette__input"
          placeholder={root === null ? "Open a folder to find files…" : "Find files…"}
          value={query}
          onValueChange={setQuery}
        />
        <div className="file-finder__body">
          <div className="file-finder__results">
            <div className="file-finder__meta">{error ?? status}</div>
            <CommandList className="command-palette__list file-finder__list">
              <CommandEmpty className="command-palette__empty">
                {root === null ? "No folder open." : error === null ? "No files found." : error}
              </CommandEmpty>
              <CommandGroup className="command-palette__group" heading="Files">
                {items.map((item) => (
                  <CommandItem
                    className="command-palette__item file-finder__item"
                    key={item.relativePath}
                    value={item.relativePath}
                    onSelect={() => openSelected(item)}
                  >
                    <FileIcon fileName={item.fileName} />
                    <span className="file-finder__item-main">
                      <span className="command-palette__item-title file-finder__name">{item.fileName}</span>
                      <span className="file-finder__path">{dirname(item.relativePath)}</span>
                    </span>
                    {item.gitStatus !== "clean" && <span className="file-finder__git">{item.gitStatus}</span>}
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </div>
          <FilePreview preview={preview} selected={selected} />
        </div>
      </Command>
    </CommandDialog>
  );
}

function FilePreview({ preview, selected }: { readonly preview: PreviewState; readonly selected: FileFinderItem | undefined }) {
  const title = selected?.relativePath ?? "Preview";
  return (
    <aside className="file-finder__preview" aria-label="File preview">
      <div className="file-finder__preview-head">
        <span className="file-finder__preview-title">{title}</span>
        {selected !== undefined && <span className="file-finder__preview-size">{formatBytes(selected.size)}</span>}
      </div>
      {preview.kind === "ready" ? (
        <pre className="file-finder__code" aria-label={`Preview of ${preview.path}`}>
          {preview.lines.map((line) => (
            <span className="file-finder__code-line" key={line.number}>
              <span className="file-finder__line-number">{line.number}</span>
              <span className="file-finder__line-text">
                {line.spans.map((span, index) => (
                  <span key={index} style={span.color === undefined ? undefined : { color: span.color }}>
                    {span.text}
                  </span>
                ))}
              </span>
            </span>
          ))}
          {preview.truncated && <span className="file-finder__truncated">Preview truncated</span>}
        </pre>
      ) : (
        <div className="file-finder__preview-empty">
          {preview.kind === "loading"
            ? "Loading preview…"
            : preview.kind === "empty" || preview.kind === "error"
              ? preview.message
              : "Select a file to preview."}
        </div>
      )}
    </aside>
  );
}
