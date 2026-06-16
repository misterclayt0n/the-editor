/**
 * fff-backed quick-open dialog. It shares the command palette primitive shell
 * while owning file-specific behavior: native indexed search and frecency tracking.
 */
import { useEffect, useRef, useState, useTransition } from "react";
import type { FileFinderItem, FileFinderLocation } from "../platform/fileFinder";
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

const SEARCH_LIMIT = 60;

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
  const [, startResultTransition] = useTransition();
  const inputRef = useRef<HTMLInputElement>(null);
  const searchSeq = useRef(0);

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
    }
  }, [open]);

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
  }, [open, query, root, runtime]);

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
      className="command-palette-dialog"
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
      </Command>
    </CommandDialog>
  );
}
