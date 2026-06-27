/**
 * Global string-search picker: live, paged grep results with a context preview
 * built from each match's own surrounding lines (no file reads). Enter jumps to
 * a match; Cmd/Ctrl+Enter materializes the full result set into a multibuffer
 * (the existing `searchProject` path). The two coexist: browse here, expand to
 * a multibuffer when you want to navigate the whole set.
 */

import { Effect } from "effect";
import { type KeyboardEvent, useEffect, useMemo, useRef, useState, useTransition } from "react";
import { highlightToSpans, type CodeSpan } from "../editor/highlightSpans";
import type { FileFinderGrepMatch, FileFinderGrepMode } from "../platform/fileFinder";
import type { AppRuntime } from "../runtime";
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
import {
  jumpTarget,
  matchKey,
  previewLines,
  styleSegments,
  trimLeading,
  type MatchRange,
  type StyledSegment,
} from "./grepResults";
import { loadFilePreview, type PreviewReader, type PreviewState, type PreviewTarget } from "./filePreview";
import { highlightPreview } from "./previewHighlight";

const SEARCH_LIMIT = 100;
const CONTEXT_LINES = 12;
const SEARCH_DEBOUNCE_MS = 90;
const PREVIEW_DEBOUNCE_MS = 40;
const PREVIEW_VISIBLE_BEFORE = 12;
const PREVIEW_VISIBLE_AFTER = 14;
const MAX_INLINE_HIGHLIGHT_CHARS = 20_000;
const GREP_MODES: readonly FileFinderGrepMode[] = ["plain", "regex", "fuzzy"];

type GrepMeta = {
  readonly totalMatched: number;
  readonly totalFilesSearched: number;
  readonly regexFallbackError?: string;
};

export interface GlobalSearchProps {
  readonly open: boolean;
  readonly runtime: AppRuntime;
  readonly onOpenChange: (open: boolean) => void;
}

const joinRoot = (root: string, relativePath: string): string =>
  root === "/" ? `/${relativePath}` : `${root}/${relativePath}`;

const extensionOf = (path: string): string => {
  const slash = path.lastIndexOf("/");
  const dot = path.lastIndexOf(".");
  return dot > slash + 1 ? path.slice(dot + 1).toLowerCase() : "";
};

const syntaxSpans = (code: string, path: string): CodeSpan[] | null => {
  const ext = extensionOf(path);
  if (ext.length === 0) return null;
  return highlightToSpans(code, ext, { maxChars: MAX_INLINE_HIGHLIGHT_CHARS, budgetMs: 6 });
};

type GrepRow = {
  readonly key: string;
  readonly match: FileFinderGrepMatch;
  readonly text: string;
  readonly ranges: readonly MatchRange[];
  readonly spans: readonly CodeSpan[] | null;
};

type GrepFileGroup = {
  readonly relativePath: string;
  readonly fileName: string;
  readonly directory: string;
  readonly rows: readonly GrepRow[];
};

const groupRows = (rows: readonly GrepRow[]): GrepFileGroup[] => {
  const groups = new Map<string, { fileName: string; rows: GrepRow[] }>();
  for (const row of rows) {
    const group = groups.get(row.match.relativePath);
    if (group === undefined) {
      groups.set(row.match.relativePath, { fileName: row.match.fileName, rows: [row] });
    } else {
      group.rows.push(row);
    }
  }
  return [...groups.entries()].map(([relativePath, group]) => ({
    relativePath,
    fileName: group.fileName,
    directory:
      relativePath === group.fileName ? "" : relativePath.slice(0, -(group.fileName.length + 1)),
    rows: group.rows,
  }));
};

const Segments = ({ segments }: { readonly segments: readonly StyledSegment[] }) => (
  <>
    {segments.map((seg, i) => {
      const style = seg.color !== undefined ? { color: seg.color } : undefined;
      return seg.match ? (
        <mark key={i} className="global-search__hit" style={style}>
          {seg.text}
        </mark>
      ) : (
        <span key={i} style={style}>
          {seg.text}
        </span>
      );
    })}
  </>
);

const textFromSpans = (spans: readonly CodeSpan[]): string => spans.map((span) => span.text).join("");

function useGlobalSearchPreview(target: PreviewTarget | null, runtime: AppRuntime): PreviewState {
  const [state, setState] = useState<PreviewState>({ status: "empty" });

  useEffect(() => {
    if (target === null) {
      setState({ status: "empty" });
      return;
    }
    const read = runtime.fileSystem()?.readTextRange;
    if (read === undefined) {
      setState({ status: "error", message: "Preview is unavailable in this host." });
      return;
    }
    let cancelled = false;
    const reader: PreviewReader = (path, options) => Effect.runPromise(read(path, options));
    setState({ status: "loading" });
    const timer = window.setTimeout(() => {
      void loadFilePreview(target, { read: reader, highlight: highlightPreview }).then((next) => {
        if (!cancelled) setState(next);
      });
    }, PREVIEW_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [target, runtime]);

  return state;
}

function ContextPreview({ match }: { readonly match: FileFinderGrepMatch }) {
  const lines = useMemo(() => {
    const preview = previewLines(match);
    const code = preview.map((line) => line.text).join("\n");
    const highlighted = highlightPreview(code, extensionOf(match.relativePath));
    return preview.map((line, i) => ({
      ...line,
      spans: highlighted?.[i] ?? null,
    }));
  }, [match]);

  return (
    <div className="global-search__preview-code">
      {lines.map((line, i) => (
        <div
          key={i}
          className={
            line.match ? "global-search__pline global-search__pline--match" : "global-search__pline"
          }
        >
          <span className="global-search__pln">{line.number}</span>
          <span className="global-search__ptext">
            <Segments segments={styleSegments(line.text, line.ranges, line.spans)} />
          </span>
        </div>
      ))}
    </div>
  );
}

function LoadedPreview({ state, match }: { readonly state: PreviewState; readonly match: FileFinderGrepMatch }) {
  if (state.status !== "ready" || state.focusLine === undefined) return <ContextPreview match={match} />;

  const focusLine = state.focusLine;
  const start = Math.max(0, focusLine - PREVIEW_VISIBLE_BEFORE);
  const end = Math.min(state.lines.length, focusLine + PREVIEW_VISIBLE_AFTER + 1);
  const visible = state.lines.slice(start, end);

  return (
    <div className="global-search__preview-code">
      {visible.map((spans, i) => {
        const lineIndex = start + i;
        const matched = lineIndex === focusLine;
        const text = textFromSpans(spans);
        return (
          <div
            key={lineIndex}
            className={
              matched ? "global-search__pline global-search__pline--match" : "global-search__pline"
            }
          >
            <span className="global-search__pln">{lineIndex + 1}</span>
            <span className="global-search__ptext">
              {spans.length === 0 ? (
                "\u00A0"
              ) : (
                <Segments
                  segments={styleSegments(text, matched ? match.matchRanges : [], spans)}
                />
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function PreviewPanel({
  match,
  root,
  runtime,
}: {
  readonly match: FileFinderGrepMatch | undefined;
  readonly root: string | null;
  readonly runtime: AppRuntime;
}) {
  const target = useMemo<PreviewTarget | null>(
    () =>
      match === undefined || root === null
        ? null
        : {
            path: joinRoot(root, match.relativePath),
            size: match.size,
            focusLine: Math.max(0, match.lineNumber - 1),
          },
    [match, root],
  );
  const state = useGlobalSearchPreview(target, runtime);

  if (match === undefined) return <div className="global-search__preview" />;
  return (
    <div className="global-search__preview">
      <div className="global-search__preview-header">
        {match.relativePath}:{match.lineNumber}
      </div>
      {state.status === "skipped" || state.status === "error" ? (
        <ContextPreview match={match} />
      ) : (
        <LoadedPreview state={state} match={match} />
      )}
    </div>
  );
}

export function GlobalSearch({ open, runtime, onOpenChange }: GlobalSearchProps) {
  const [root, setRoot] = useState<string | null>(runtime.workspaceRoot);
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<FileFinderGrepMode>("plain");
  const [items, setItems] = useState<readonly FileFinderGrepMatch[]>([]);
  const [selectedKey, setSelectedKey] = useState("");
  const [meta, setMeta] = useState<GrepMeta>({ totalMatched: 0, totalFilesSearched: 0 });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [, startResultTransition] = useTransition();
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setRoot(runtime.workspaceRoot);
    return runtime.subscribeWorkspaceRoot(setRoot);
  }, [runtime]);

  useEffect(() => {
    if (!open) return;
    const id = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setItems([]);
      setSelectedKey("");
      setMeta({ totalMatched: 0, totalFilesSearched: 0 });
      setError(null);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const finder = runtime.fileFinderPort();
    const trimmed = query.trim();
    if (root === null || finder === undefined) {
      setItems([]);
      setSelectedKey("");
      setMeta({ totalMatched: 0, totalFilesSearched: 0 });
      setError(finder === undefined ? "Project search is unavailable in this host." : null);
      setLoading(false);
      return;
    }
    if (trimmed.length === 0) {
      setItems([]);
      setSelectedKey("");
      setMeta({ totalMatched: 0, totalFilesSearched: 0 });
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    const timer = window.setTimeout(() => {
      finder
        .grep({
          root,
          query: trimmed,
          limit: SEARCH_LIMIT,
          mode,
          beforeContext: CONTEXT_LINES,
          afterContext: CONTEXT_LINES,
        })
        .then((result) => {
          if (cancelled) return;
          startResultTransition(() => {
            setItems(result.items);
            setMeta({
              totalMatched: result.totalMatched,
              totalFilesSearched: result.totalFilesSearched,
              ...(result.regexFallbackError !== undefined
                ? { regexFallbackError: result.regexFallbackError }
                : {}),
            });
            setError(null);
            setSelectedKey(result.items[0] !== undefined ? matchKey(result.items[0]) : "");
          });
        })
        .catch((caught) => {
          if (cancelled) return;
          startResultTransition(() => {
            setItems([]);
            setSelectedKey("");
            setMeta({ totalMatched: 0, totalFilesSearched: 0 });
            setError(caught instanceof Error ? caught.message : String(caught));
          });
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [open, query, mode, root, runtime]);

  const rows = useMemo(
    () =>
      items.map((item): GrepRow => {
        const row = trimLeading(item.lineContent, item.matchRanges);
        return {
          key: matchKey(item),
          match: item,
          text: row.text,
          ranges: row.ranges,
          spans: syntaxSpans(row.text, item.relativePath),
        };
      }),
    [items],
  );

  const groupedRows = useMemo(() => groupRows(rows), [rows]);

  const selected = useMemo(
    () => rows.find((row) => row.key === selectedKey)?.match,
    [rows, selectedKey],
  );

  const openMatch = (match: FileFinderGrepMatch) => {
    if (root === null) return;
    onOpenChange(false);
    void runtime.jumpToLocation(joinRoot(root, match.relativePath), jumpTarget(match));
  };

  const spawnMultiBuffer = () => {
    const trimmed = query.trim();
    if (trimmed.length === 0 || root === null || runtime.fileFinderPort() === undefined) return;
    onOpenChange(false);
    void runtime.searchProject(trimmed, { mode });
  };

  const cycleMode = () =>
    setMode((prev) => GREP_MODES[(GREP_MODES.indexOf(prev) + 1) % GREP_MODES.length]!);

  const handleKeyDown = (event: KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      spawnMultiBuffer();
    } else if (event.altKey && event.code === "KeyM") {
      event.preventDefault();
      cycleMode();
    }
  };

  const status =
    root === null
      ? "No folder open"
      : loading
        ? "Searching…"
        : (meta.regexFallbackError ??
          `${meta.totalMatched.toLocaleString()} matches · ${meta.totalFilesSearched.toLocaleString()} files`);

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Find in Files"
      description="Search file contents across the workspace"
      className="command-palette-dialog global-search-dialog"
    >
      <Command
        className="command-palette global-search"
        value={selectedKey}
        onValueChange={setSelectedKey}
        shouldFilter={false}
        onKeyDown={handleKeyDown}
        loop
      >
        <CommandInput
          ref={inputRef}
          className="command-palette__input"
          placeholder={root === null ? "Open a folder to search…" : "Search in files…"}
          value={query}
          onValueChange={setQuery}
        />
        <div className="global-search__body">
          <div className="global-search__list-col">
            <CommandList className="command-palette__list global-search__list">
              <CommandEmpty className="command-palette__empty">
                {root === null
                  ? "Open a folder to search."
                  : query.trim().length === 0
                    ? "Type to search file contents."
                    : (error ?? "No matches.")}
              </CommandEmpty>
              {groupedRows.map((group) => (
                <CommandGroup
                  className="global-search__group"
                  heading={
                    <span className="global-search__group-heading">
                      <FileIcon fileName={group.fileName} />
                      <span className="global-search__group-file">{group.fileName}</span>
                      {group.directory.length > 0 && (
                        <span className="global-search__group-directory">{group.directory}</span>
                      )}
                      <span className="global-search__group-count">{group.rows.length}</span>
                    </span>
                  }
                  key={group.relativePath}
                >
                  {group.rows.map((row) => (
                    <CommandItem
                      className="command-palette__item global-search__row"
                      key={row.key}
                      value={row.key}
                      onSelect={() => openMatch(row.match)}
                    >
                      <span className="global-search__line-number">{row.match.lineNumber}</span>
                      <span className="global-search__code">
                        <Segments segments={styleSegments(row.text, row.ranges, row.spans)} />
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              ))}
            </CommandList>
            <div className="global-search__footer">
              <span className="global-search__status">{error ?? status}</span>
              <span className="global-search__hints">
                <button type="button" className="global-search__mode" onClick={cycleMode}>
                  ⌥M {mode}
                </button>
                <span className="global-search__hint">↵ open</span>
                <span className="global-search__hint">⌘↵ multibuffer</span>
              </span>
            </div>
          </div>
          <PreviewPanel match={selected} root={root} runtime={runtime} />
        </div>
      </Command>
    </CommandDialog>
  );
}
