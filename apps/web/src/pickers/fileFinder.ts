/**
 * The fff-backed file source for the picker: native indexed search + frecency,
 * exactly what the bespoke FileFinder dialog did, expressed as a
 * `PickerRequest`. Host-owned because it reaches the platform ports
 * (FileFinderPort, jumpToLocation); the rendering is entirely the generic
 * picker host's.
 */

import type { PickerRequest } from "@the-editor/picker";
import type { FileFinderItem, FileFinderLocation } from "../platform/fileFinder";
import type { AppRuntime } from "../runtime";

const SEARCH_LIMIT = 60;
const SEARCH_DEBOUNCE_MS = 90;
const ROW_HEIGHT = 36;

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

export function fileFinderRequest(runtime: AppRuntime): PickerRequest<FileFinderItem> {
  const root = runtime.workspaceRoot;
  const finder = runtime.fileFinderPort();

  // Per-search metadata the source's answer can't hang on individual items:
  // the trailing `file.ts:42` location suffix applies to the query, not a row.
  // The closure carries the latest one for onChoose/preview; staleness is the
  // controller's problem (it drops superseded answers before we read this).
  let lastLocation: FileFinderLocation | undefined;
  let lastQuery = "";

  return {
    title: "Find File",
    rowHeight: ROW_HEIGHT,
    source: {
      id: "the-editor/files",
      placeholder: root === null ? "Open a folder to find files…" : "Find files…",
      debounceMs: SEARCH_DEBOUNCE_MS,
      preRanked: true, // fff owns fuzzy + frecency ranking
      query: async (query) => {
        if (root === null) return { items: [], note: "No folder open" };
        if (finder === undefined) {
          return { items: [], note: "File finder is unavailable in this host." };
        }
        const currentFile = currentFileRelative(runtime, root);
        const result = await finder.search({
          root,
          query,
          limit: SEARCH_LIMIT,
          ...(currentFile !== undefined ? { currentFile } : {}),
        });
        lastLocation = result.location;
        lastQuery = query;
        return {
          items: result.items.map((item) => ({
            id: item.relativePath,
            label: item.fileName,
            detail: dirname(item.relativePath),
            icon: { kind: "file" as const, fileName: item.fileName },
            ...(item.gitStatus !== "clean" ? { badge: item.gitStatus } : {}),
            data: item,
          })),
          note: `${result.totalMatched.toLocaleString()} matches · ${result.totalFiles.toLocaleString()} files`,
        };
      },
    },
    preview: (item) => {
      if (root === null) return null;
      const focusLine =
        lastLocation !== undefined ? locationPosition(lastLocation).line : undefined;
      return {
        kind: "file",
        path: joinRoot(root, item.data.relativePath),
        size: item.data.size,
        ...(focusLine !== undefined ? { focusLine } : {}),
      };
    },
    onChoose: (item) => {
      if (root === null) return;
      void finder?.track({ root, query: lastQuery, relativePath: item.data.relativePath });
      const target = joinRoot(root, item.data.relativePath);
      void runtime.jumpToLocation(target, locationPosition(lastLocation));
    },
  };
}
