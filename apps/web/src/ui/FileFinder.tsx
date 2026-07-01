/**
 * fff-backed quick-open dialog. It shares the command palette primitive shell
 * while owning file-specific behavior: native indexed search and frecency tracking.
 */
import { useEffect, useMemo, useRef, useState, useTransition } from "react";
import type { FileFinderItem, FileFinderLocation } from "../platform/fileFinder";
import type { AppRuntime } from "../runtime";
import { FileIcon } from "../workbench/fileIcons";
import { CommandDialog } from "@/ui/command";
import { VirtualCommand, type VirtualCommandRow } from "@/ui/virtual-command";
import { cn } from "@/lib/utils";
import { FilePreview } from "./FilePreviewPanel";
import type { PreviewTarget } from "./filePreview";

const SEARCH_LIMIT = 60;
const SEARCH_DEBOUNCE_MS = 90;
const ROW_HEIGHT = 36;

interface FileRow extends VirtualCommandRow {
  readonly item: FileFinderItem;
}

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

const locationPosition = (
  location: FileFinderLocation | undefined,
): { line: number; character: number } => {
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
      setSelectedValue("");
      setSearchMeta({ totalMatched: 0, totalFiles: 0 });
      setError(null);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    if (root !== runtime.workspaceRoot) {
      setItems([]);
      setSelectedValue("");
      setSearchMeta({ totalMatched: 0, totalFiles: 0 });
      setError(null);
      setLoading(false);
      return;
    }
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
    // Debounce: each fff fuzzy search scores the whole index (hundreds of ms on
    // a large workspace), so firing one per keystroke queued a storm of searches
    // whose results re-rendered in a burst — the UI freeze. Coalesce to one
    // search per pause; the trailing keystroke owns the result (searchSeq drops
    // any stragglers).
    const timer = window.setTimeout(() => {
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
            // A fresh search owns the initial selection. Preserving the previous
            // selected value makes the highlight stick to an older result that is
            // still present lower in the new list (e.g. query "main.c" ranking
            // main.c first while the highlight remains on main.ts).
            setSelectedValue(result.items[0]?.relativePath ?? "");
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
    }, query.length === 0 ? 0 : SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
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

  const canPreview = useMemo(() => runtime.fileSystem()?.readTextRange !== undefined, [runtime]);
  const focusLine =
    searchMeta.location !== undefined ? locationPosition(searchMeta.location).line : undefined;
  const selectedItem = items.find((item) => item.relativePath === selectedValue);
  const selectedPath =
    root !== null && selectedItem !== undefined ? joinRoot(root, selectedItem.relativePath) : null;
  const selectedSize = selectedItem?.size ?? 0;
  const previewTarget = useMemo<PreviewTarget | null>(
    () =>
      selectedPath === null
        ? null
        : {
            path: selectedPath,
            size: selectedSize,
            ...(focusLine !== undefined ? { focusLine } : {}),
          },
    [selectedPath, selectedSize, focusLine],
  );

  const rows = useMemo<ReadonlyArray<FileRow>>(
    () => items.map((item) => ({ id: item.relativePath, item })),
    [items],
  );

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Find File"
      description="Search files in the current workspace"
      className={cn("command-palette-dialog", canPreview && "file-finder-dialog--preview")}
    >
      <VirtualCommand<FileRow>
        className="command-palette command-root file-finder"
        bodyClassName={cn("file-finder__body", canPreview && "file-finder__body--with-preview")}
        listColClassName="file-finder__list-col"
        listClassName="command-palette__list file-finder__list command-list"
        inputClassName="command-palette__input command-input"
        inputRef={inputRef}
        placeholder={root === null ? "Open a folder to find files…" : "Find files…"}
        query={query}
        onQueryChange={setQuery}
        rows={rows}
        rowHeight={ROW_HEIGHT}
        value={selectedValue}
        onValueChange={setSelectedValue}
        onChoose={(row) => openSelected(row.item)}
        belowInput={<div className="file-finder__meta">{error ?? status}</div>}
        empty={
          <div className="command-palette__empty command-empty" data-slot="command-empty">
            {root === null ? "No folder open." : error === null ? "No files found." : error}
          </div>
        }
        aside={canPreview ? <FilePreview target={previewTarget} runtime={runtime} /> : null}
        renderRow={({ item }, { selected }) => (
          <div
            className="command-palette__item command-item file-finder__item"
            data-slot="command-item"
            data-selected={selected ? "true" : undefined}
          >
            <FileIcon fileName={item.fileName} />
            <span className="file-finder__item-main">
              <span className="command-palette__item-title file-finder__name">{item.fileName}</span>
              <span className="file-finder__path">{dirname(item.relativePath)}</span>
            </span>
            {item.gitStatus !== "clean" && <span className="file-finder__git">{item.gitStatus}</span>}
          </div>
        )}
      />
    </CommandDialog>
  );
}
