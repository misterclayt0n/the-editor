/**
 * Global string-search as a picker source: live, paged grep over the fff port,
 * grouped by file (header rows), match rows as styled segments, and a context
 * preview. Enter jumps to a match; Cmd/Ctrl+Enter materializes the full result
 * set into a multibuffer (`searchProject`); Alt+M cycles plain/regex/fuzzy and
 * refreshes the open picker in place.
 *
 * Result rows are plain text + match highlights only. Per-row syntax
 * highlighting loaded a tree-sitter grammar synchronously on the first search
 * of each language → a cold-start UI freeze; the preview pane still
 * highlights, where one file's cost is acceptable.
 */

import { Effect } from "effect";
import { useEffect, useMemo, useState } from "react";
import type { PickerItem, PickerRequest } from "@the-editor/picker";
import type { FileFinderGrepMatch, FileFinderGrepMode } from "../platform/fileFinder";
import type { AppRuntime } from "../runtime";
import { FileIcon } from "../workbench/fileIcons";
import {
  jumpTarget,
  matchKey,
  previewLines,
  styleSegments,
  trimLeading,
  type StyledSegment,
} from "../ui/grepResults";
import {
  loadFilePreview,
  type PreviewReader,
  type PreviewState,
  type PreviewTarget,
} from "../ui/filePreview";
import { highlightPreview } from "../ui/previewHighlight";

const SEARCH_LIMIT = 100;
const CONTEXT_LINES = 12;
const SEARCH_DEBOUNCE_MS = 90;
const PREVIEW_DEBOUNCE_MS = 40;
const PREVIEW_VISIBLE_BEFORE = 12;
const PREVIEW_VISIBLE_AFTER = 14;
const ROW_HEIGHT = 28;
const GREP_MODES: readonly FileFinderGrepMode[] = ["plain", "regex", "fuzzy"];

const joinRoot = (root: string, relativePath: string): string =>
  root === "/" ? `/${relativePath}` : `${root}/${relativePath}`;

const extensionOf = (path: string): string => {
  const slash = path.lastIndexOf("/");
  const dot = path.lastIndexOf(".");
  return dot > slash + 1 ? path.slice(dot + 1).toLowerCase() : "";
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

const textFromSpans = (spans: readonly { readonly text: string }[]): string =>
  spans.map((span) => span.text).join("");

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

function LoadedPreview({
  state,
  match,
}: {
  readonly state: PreviewState;
  readonly match: FileFinderGrepMatch;
}) {
  if (state.status !== "ready" || state.focusLine === undefined)
    return <ContextPreview match={match} />;

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
                " "
              ) : (
                <Segments segments={styleSegments(text, matched ? match.matchRanges : [], spans)} />
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

type SearchEntry =
  | { readonly kind: "header"; readonly fileName: string; readonly directory: string; readonly count: number }
  | { readonly kind: "match"; readonly match: FileFinderGrepMatch };

export function globalSearchRequest(runtime: AppRuntime): PickerRequest<SearchEntry> {
  const root = runtime.workspaceRoot;
  const finder = runtime.fileFinderPort();
  // Mutable per-open state: Alt+M cycles the mode and refreshes the picker in
  // place — the source re-reads it on the next query pass.
  let mode: FileFinderGrepMode = GREP_MODES[0]!;

  const buildItems = (
    matches: ReadonlyArray<FileFinderGrepMatch>,
  ): ReadonlyArray<PickerItem<SearchEntry>> => {
    // Group by file: one non-selectable header row per file, then its matches
    // (uniform row height keeps the window math exact).
    const groups = new Map<string, FileFinderGrepMatch[]>();
    for (const match of matches) {
      groups.set(match.relativePath, [...(groups.get(match.relativePath) ?? []), match]);
    }
    const items: PickerItem<SearchEntry>[] = [];
    for (const [relativePath, group] of groups) {
      const fileName = group[0]!.fileName;
      items.push({
        id: `h:${relativePath}`,
        header: true,
        label: fileName,
        data: {
          kind: "header",
          fileName,
          directory:
            relativePath === fileName ? "" : relativePath.slice(0, -(fileName.length + 1)),
          count: group.length,
        },
      });
      for (const match of group) {
        const row = trimLeading(match.lineContent, match.matchRanges);
        items.push({
          id: matchKey(match),
          label: row.text,
          segments: styleSegments(row.text, row.ranges, null).map((seg) => ({
            text: seg.text,
            ...(seg.match ? { match: true } : {}),
          })),
          data: { kind: "match", match },
        });
      }
    }
    return items;
  };

  return {
    title: "Find in Files",
    rowHeight: ROW_HEIGHT,
    chrome: {
      dialogClassName: "global-search-dialog",
      rootClassName: "global-search",
      bodyClassName: "global-search__body",
      listColClassName: "global-search__list-col",
      listClassName: "global-search__list",
    },
    source: {
      id: "the-editor/global-search",
      placeholder: root === null ? "Open a folder to search…" : "Search in files…",
      debounceMs: SEARCH_DEBOUNCE_MS,
      preRanked: true, // grep matched already; re-ranking would tear file groups
      query: async (query) => {
        const trimmed = query.trim();
        if (root === null) return { items: [], note: "No folder open" };
        if (finder === undefined) {
          return { items: [], note: "Project search is unavailable in this host." };
        }
        if (trimmed.length === 0) return { items: [] };
        const result = await finder.grep({
          root,
          query: trimmed,
          limit: SEARCH_LIMIT,
          mode,
          beforeContext: CONTEXT_LINES,
          afterContext: CONTEXT_LINES,
        });
        return {
          items: buildItems(result.items),
          note:
            result.regexFallbackError ??
            `${result.totalMatched.toLocaleString()} matches · ${result.totalFilesSearched.toLocaleString()} files`,
        };
      },
    },
    emptyText: (query) =>
      root === null
        ? "Open a folder to search."
        : query.trim().length === 0
          ? "Type to search file contents."
          : undefined,
    onChoose: (item) => {
      if (item.data.kind !== "match" || root === null) return;
      const match = item.data.match;
      void runtime.jumpToLocation(joinRoot(root, match.relativePath), jumpTarget(match));
    },
    actions: [
      {
        keys: "mod+enter",
        label: "⌘↵ multibuffer",
        run: (_item, query) => {
          const trimmed = query.trim();
          if (trimmed.length === 0 || root === null || finder === undefined) return;
          void runtime.searchProject(trimmed, { mode });
        },
      },
      {
        keys: "alt+m",
        label: () => `⌥M ${mode}`,
        closes: false,
        run: () => {
          mode = GREP_MODES[(GREP_MODES.indexOf(mode) + 1) % GREP_MODES.length]!;
          runtime.pickerController.refresh();
        },
      },
    ],
    renderItem: (item, { selected }) =>
      item.data.kind === "header" ? (
        <div className="global-search__group-row">
          <span className="global-search__group-heading">
            <FileIcon fileName={item.data.fileName} />
            <span className="global-search__group-file">{item.data.fileName}</span>
            {item.data.directory.length > 0 && (
              <span className="global-search__group-directory">{item.data.directory}</span>
            )}
            <span className="global-search__group-count">{item.data.count}</span>
          </span>
        </div>
      ) : (
        <div
          className="command-palette__item command-item global-search__row"
          data-slot="command-item"
          data-selected={selected ? "true" : undefined}
        >
          <span className="global-search__line-number">{item.data.match.lineNumber}</span>
          <span className="global-search__code">
            <Segments
              segments={(item.segments ?? []).map((seg) => ({
                text: seg.text,
                match: seg.match === true,
              }))}
            />
          </span>
        </div>
      ),
    renderPreview: (item) => (
      <PreviewPanel
        match={item?.data.kind === "match" ? item.data.match : undefined}
        root={root}
        runtime={runtime}
      />
    ),
  };
}
