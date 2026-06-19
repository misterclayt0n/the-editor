/**
 * Diff view panel content: @pierre/diffs mounted into a workbench panel.
 *
 * The panel renders the workspace's working-tree diff against HEAD as a single
 * global view. It is data-first: whatever patch `VcsManager.patch()` returns is
 * parsed and displayed. Changes to the workspace root or VCS status refresh the
 * view, and the editor's active theme drives the diff viewer's light/dark mode.
 */

import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { Fragment, jsx, jsxs } from "react/jsx-runtime";
import { createRoot, type Root } from "react-dom/client";
import { Effect, Fiber, Stream } from "effect";
import { DiffIcon } from "lucide-react";
import type { ResolvedTheme } from "@the-editor/theme";
import { CodeView, type CodeViewDiffItem, type CodeViewItem, type CodeViewProps } from "@pierre/diffs/react";
import { parsePatchFiles } from "@pierre/diffs";
import type { AppRuntime } from "../runtime";
import type {
  DiffViewHunkSeparators,
  DiffViewIndicators,
  DiffViewLayout,
  DiffViewLineDiff,
  DiffViewSettings,
  DiffViewSettingsPatch,
} from "../product/diffView";
import type { PanelRenderer } from "../workbench/panelRenderers";
import { FileHeader } from "../ui/FileHeader";

export const DIFF_VIEW_TITLE = "Changes";

type DiffCodeViewOptions = NonNullable<CodeViewProps<undefined>["options"]>;

const DIFF_VIEW_UNSAFE_CSS = `
:host {
  --diffs-light-bg: var(--bg-editor);
  --diffs-dark-bg: var(--bg-editor);
  --diffs-light: var(--fg);
  --diffs-dark: var(--fg);
  --diffs-bg: var(--bg-editor);
  --diffs-fg: var(--fg);
  --diffs-bg-context-override: color-mix(in srgb, var(--fg) 4%, var(--bg-editor));
  --diffs-bg-context-gutter-override: color-mix(in srgb, var(--fg) 6%, var(--bg-editor));
  --diffs-bg-buffer-override: color-mix(in srgb, var(--fg) 8%, var(--bg-editor));
  --diffs-bg-separator-override: color-mix(in srgb, var(--fg) 7%, var(--bg-editor));
  --diffs-bg-hover-override: color-mix(in srgb, var(--fg) 8%, var(--bg-editor));
  --diffs-fg-number-override: var(--fg-dim);
  --diffs-addition-color-override: color-mix(in srgb, #2ec27e 78%, var(--accent));
  --diffs-deletion-color-override: color-mix(in srgb, #ff6b6b 82%, var(--accent));
  --diffs-modified-color-override: var(--accent);
  --diffs-bg-addition-override: color-mix(in srgb, var(--diffs-addition-base) 18%, var(--bg-editor));
  --diffs-bg-deletion-override: color-mix(in srgb, var(--diffs-deletion-base) 18%, var(--bg-editor));
  --diffs-bg-addition-emphasis-override: color-mix(in srgb, var(--diffs-addition-base) 34%, transparent);
  --diffs-bg-deletion-emphasis-override: color-mix(in srgb, var(--diffs-deletion-base) 34%, transparent);
  --diffs-bg-selection-override: var(--selection);
  --diffs-font-family: "SF Mono", Menlo, Monaco, "Cascadia Code", monospace;
  --diffs-header-font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  background: var(--bg-editor);
  color: var(--fg);
}

[data-diffs-header] {
  container-type: scroll-state;
  container-name: diff-file-header;
}

@container diff-file-header scroll-state(stuck: top) {
  [data-diffs-header]::after {
    position: absolute;
    right: 0;
    bottom: -1px;
    left: 0;
    height: 1px;
    content: "";
    background: color-mix(in srgb, var(--fg) 18%, transparent);
  }
}


.wb-file-icon {
  flex: none;
  width: 15px;
  height: 15px;
}

.file-header,
.mb-header {
  position: relative;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  box-sizing: border-box;
  height: 100%;
  min-height: 34px;
  min-width: 0;
  margin: 0;
  padding-inline: 16px;
  border-block: 1px solid color-mix(in srgb, var(--fg) 12%, transparent);
  border-inline: 0;
  border-radius: 0;
  background: color-mix(in srgb, var(--fg) 4%, var(--bg-editor));
  color: var(--fg);
  box-shadow: none;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  font-size: 13px;
  line-height: 20px;
  user-select: none;
  overflow: hidden;
  z-index: 1;
}


.diff-view-file-header .mb-header-content,
.diff-view-file-header .mb-header-metadata {
  display: flex;
  align-items: center;
  min-width: 0;
  white-space: nowrap;
}

.diff-view-file-header .mb-header-content {
  flex: 1 1 auto;
  gap: 8px;
  overflow: hidden;
}

.diff-view-file-header .mb-header-metadata {
  flex: none;
  gap: 6px;
  margin-left: auto;
}

.diff-view-file-header .mb-header-path {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.diff-view-file-header .mb-header-name,
.diff-view-file-header .mb-header-dir {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diff-view-file-header .mb-header-name {
  flex: 0 1 auto;
  font-weight: 600;
}

.diff-view-file-header .mb-header-dir {
  flex: 1 1 auto;
  opacity: 0.62;
}

.diff-view-header-prev {
  display: inline-flex;
  flex: 0 2 auto;
  align-items: center;
  gap: 6px;
  min-width: 28px;
  overflow: hidden;
  color: var(--fg-dim);
}

.diff-view-header-prev bdi {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.diff-view-header-prev-arrow {
  flex: none;
  opacity: 0.75;
}

.diff-view-file-header .mb-header-path {
  cursor: default;
}

.diff-view-header-chip {
  flex: none;
  padding: 1px 7px;
  border: 1px solid color-mix(in srgb, var(--fg) 11%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, var(--fg) 5%, transparent);
  color: var(--fg-dim);
  font-size: 11px;
  line-height: 18px;
}

.diff-view-header-stat {
  flex: none;
  font-family: "SF Mono", Menlo, Monaco, "Cascadia Code", monospace;
  font-size: 11.5px;
  line-height: 18px;
}

.diff-view-header-stat--additions {
  color: var(--diffs-addition-base);
}

.diff-view-header-stat--deletions {
  color: var(--diffs-deletion-base);
}
`;

const DIFF_VIEW_CUSTOM_SEPARATOR_CSS = `
[data-separator=line-info] [data-separator-content],
[data-separator=line-info] [data-expand-button] {
  background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 20%, var(--bg-editor)), var(--diffs-bg-separator));
  color: var(--fg);
}

[data-separator=line-info] [data-separator-content] {
  border: 1px solid color-mix(in srgb, var(--accent) 28%, transparent);
}
`;

const LAYOUT_OPTIONS: ReadonlyArray<readonly [DiffViewLayout, string]> = [
  ["split", "Split"],
  ["stacked", "Stacked"],
];

const INDICATOR_OPTIONS: ReadonlyArray<readonly [DiffViewIndicators, string]> = [
  ["bars", "Bars"],
  ["classic", "Classic"],
  ["none", "None"],
];

const LINE_DIFF_OPTIONS: ReadonlyArray<readonly [DiffViewLineDiff, string]> = [
  ["word-alt", "Word Alt"],
  ["word", "Word"],
  ["char", "Char"],
  ["none", "None"],
];

const HUNK_SEPARATOR_OPTIONS: ReadonlyArray<readonly [DiffViewHunkSeparators, string]> = [
  ["line-info", "Line"],
  ["line-info-basic", "Basic"],
  ["metadata", "Metadata"],
  ["simple", "Simple"],
  ["custom-css", "Custom"],
];

function patchToItems(patch: string): CodeViewItem<undefined>[] {
  const parsedPatches = parsePatchFiles(patch);
  const items: CodeViewDiffItem<undefined>[] = [];
  let idCounter = 0;
  for (const parsed of parsedPatches) {
    for (const fileDiff of parsed.files) {
      items.push({
        id: `${fileDiff.name}:${idCounter++}`,
        type: "diff",
        fileDiff,
      });
    }
  }
  return items;
}

function useActiveThemeAppearance(runtime: AppRuntime): ResolvedTheme["appearance"] | null {
  const [appearance, setAppearance] = useState<ResolvedTheme["appearance"] | null>(() => {
    const initial = Effect.runSync(runtime.themes.active.pipe(Effect.option));
    return initial._tag === "Some" ? initial.value.appearance : null;
  });

  useEffect(() => {
    let cancelled = false;
    const fiber = Effect.runFork(
      Stream.runForEach(runtime.themes.changes, (theme) =>
        Effect.sync(() => {
          if (!cancelled) setAppearance(theme.appearance);
        }),
      ),
    );
    return () => {
      cancelled = true;
      Effect.runFork(Fiber.interrupt(fiber));
    };
  }, [runtime]);

  return appearance;
}

function emptyState(note: string): ReactNode {
  return jsx("div", {
    className: "diff-view-body diff-view-empty",
    tabIndex: -1,
    children: jsx("span", { className: "diff-view-empty-note", children: note }),
  });
}

function useDiffViewSettings(runtime: AppRuntime): DiffViewSettings {
  return useSyncExternalStore(
    (listener) => runtime.diffViewPreferences.subscribe(listener),
    () => runtime.diffViewPreferences.current(),
    () => runtime.diffViewPreferences.current(),
  );
}

function diffCodeViewOptions(
  settings: DiffViewSettings,
  appearance: ResolvedTheme["appearance"] | null,
): DiffCodeViewOptions {
  const hunkSeparators = settings.hunkSeparators === "custom-css" ? "line-info" : settings.hunkSeparators;
  return {
    diffStyle: settings.layout === "split" ? "split" : "unified",
    stickyHeaders: true,
    diffIndicators: settings.indicators,
    disableBackground: !settings.backgrounds,
    overflow: settings.wrapping ? "wrap" : "scroll",
    disableLineNumbers: !settings.lineNumbers,
    lineDiffType: settings.lineDiff,
    hunkSeparators,
    themeType: appearance ?? ("system" as const),
    unsafeCSS:
      settings.hunkSeparators === "custom-css"
        ? `${DIFF_VIEW_UNSAFE_CSS}\n${DIFF_VIEW_CUSTOM_SEPARATOR_CSS}`
        : DIFF_VIEW_UNSAFE_CSS,
    disableVirtualizationBuffers: false,
  };
}


function changeTypeLabel(type: CodeViewDiffItem<undefined>["fileDiff"]["type"]): string {
  switch (type) {
    case "new":
      return "Added";
    case "deleted":
      return "Deleted";
    case "rename-pure":
      return "Renamed";
    case "rename-changed":
      return "Renamed + modified";
    case "change":
      return "Modified";
  }
}

function renderDiffHeader(item: CodeViewItem<undefined>): ReactNode {
  if (item.type !== "diff") return null;
  const { fileDiff } = item;
  let additions = 0;
  let deletions = 0;
  for (const hunk of fileDiff.hunks) {
    additions += hunk.additionLines;
    deletions += hunk.deletionLines;
  }

  return jsx(FileHeader, {
    displayPath: fileDiff.name,
    ...(fileDiff.prevName !== undefined ? { previousDisplayPath: fileDiff.prevName } : {}),
    className: "diff-view-file-header",
    metadata: jsxs(Fragment, {
      children: [
        jsx("span", {
          className: "diff-view-header-chip",
          children: changeTypeLabel(fileDiff.type),
        }),
        jsx("span", {
          className: "diff-view-header-stat diff-view-header-stat--additions",
          children: `+${additions}`,
        }),
        jsx("span", {
          className: "diff-view-header-stat diff-view-header-stat--deletions",
          children: `−${deletions}`,
        }),
      ],
    }),
    showOpenButton: false,
    dataDiffsHeader: true,
  });
}

function renderSegmentedControl<T extends string>(
  label: string,
  options: ReadonlyArray<readonly [T, string]>,
  value: T,
  onSelect: (value: T) => void,
): ReactNode {
  return jsxs("div", {
    className: "diff-view-toolbar-group",
    children: [
      jsx("span", { className: "diff-view-toolbar-label", children: label }),
      jsx("div", {
        className: "diff-view-segmented",
        role: "group",
        "aria-label": label,
        children: options.map(([option, optionLabel]) =>
          jsx(
            "button",
            {
              type: "button",
              className: "diff-view-segment",
              "aria-pressed": option === value,
              "data-active": option === value ? "" : undefined,
              onClick: () => onSelect(option),
              children: optionLabel,
            },
            option,
          ),
        ),
      }),
    ],
  });
}

function renderToolbarToggle(label: string, active: boolean, onToggle: () => void): ReactNode {
  return jsx("button", {
    type: "button",
    className: "diff-view-toggle",
    "aria-pressed": active,
    "data-active": active ? "" : undefined,
    onClick: onToggle,
    children: label,
  });
}

function DiffViewToolbar({
  settings,
  update,
}: {
  readonly settings: DiffViewSettings;
  readonly update: (patch: DiffViewSettingsPatch) => void;
}) {
  return jsxs("div", {
    className: "diff-view-toolbar",
    role: "toolbar",
    "aria-label": "Diff display controls",
    children: [
      renderSegmentedControl("Layout", LAYOUT_OPTIONS, settings.layout, (layout) => update({ layout })),
      renderSegmentedControl("Indicators", INDICATOR_OPTIONS, settings.indicators, (indicators) =>
        update({ indicators }),
      ),
      renderSegmentedControl("Line diff", LINE_DIFF_OPTIONS, settings.lineDiff, (lineDiff) =>
        update({ lineDiff }),
      ),
      renderSegmentedControl(
        "Hunks",
        HUNK_SEPARATOR_OPTIONS,
        settings.hunkSeparators,
        (hunkSeparators) => update({ hunkSeparators }),
      ),
      jsxs("div", {
        className: "diff-view-toolbar-group diff-view-toolbar-group--toggles",
        children: [
          renderToolbarToggle("Backgrounds", settings.backgrounds, () =>
            update({ backgrounds: !settings.backgrounds }),
          ),
          renderToolbarToggle("Wrap", settings.wrapping, () => update({ wrapping: !settings.wrapping })),
          renderToolbarToggle("Line numbers", settings.lineNumbers, () =>
            update({ lineNumbers: !settings.lineNumbers }),
          ),
        ],
      }),
    ],
  });
}


function DiffViewPanel({ runtime }: { readonly runtime: AppRuntime }) {
  const [items, setItems] = useState<CodeViewItem<undefined>[]>([]);
  const [loading, setLoading] = useState(true);
  const [noRepo, setNoRepo] = useState(false);
  const appearance = useActiveThemeAppearance(runtime);
  const settings = useDiffViewSettings(runtime);
  const refreshGeneration = useRef(0);

  const options = useMemo(() => diffCodeViewOptions(settings, appearance), [appearance, settings]);
  const setCodeViewContainer = useCallback((element: HTMLDivElement | null) => {
    if (element !== null) element.tabIndex = -1;
  }, []);


  const refresh = async () => {
    const generation = ++refreshGeneration.current;
    if (runtime.workspaceRoot === null || runtime.vcs === undefined) {
      if (generation !== refreshGeneration.current) return;
      setItems([]);
      setNoRepo(true);
      setLoading(false);
      return;
    }
    setLoading(true);
    const patch = await runtime.vcs.patch();
    if (generation !== refreshGeneration.current) return;
    if (patch === null || patch.length === 0) {
      setItems([]);
      setNoRepo(patch === null);
    } else {
      setItems(patchToItems(patch));
      setNoRepo(false);
    }
    setLoading(false);
  };

  useEffect(() => {
    void refresh();
    const cleanups: Array<() => void> = [];
    cleanups.push(runtime.subscribeWorkspaceRoot(() => void refresh()));
    if (runtime.vcs !== undefined) {
      cleanups.push(runtime.vcs.subscribe(() => void refresh()));
    }
    return () => {
      refreshGeneration.current++;
      for (const cleanup of cleanups) cleanup();
    };
  }, [runtime]);

  const body =
    loading && items.length === 0
      ? emptyState("Loading changes…")
      : noRepo
        ? emptyState("No version control repository")
        : items.length === 0
          ? emptyState("No changes")
          : jsx("div", {
              className: "diff-view-body",
              tabIndex: -1,
              children: jsx(CodeView, {
                className: "diff-view-code",
                items,
                options,
                renderCustomHeader: renderDiffHeader,
                containerRef: setCodeViewContainer,
              }),
            });

  return jsxs("div", {
    className: "diff-view-panel",
    children: [
      settings.controlsVisible
        ? jsx(DiffViewToolbar, {
            settings,
            update: (patch: DiffViewSettingsPatch) => runtime.diffViewPreferences.update(patch),
          })
        : null,
      body,
    ],
  });
}

export function createDiffViewRenderer(runtime: AppRuntime): PanelRenderer {
  return {
    title: () => DIFF_VIEW_TITLE,
    icon: () => jsx(DiffIcon, { className: "wb-tab-renderer-icon", "aria-hidden": "true" }),
    mount: (element) => {
      element.classList.add("diff-view-host");
      const root: Root = createRoot(element);
      root.render(jsx(DiffViewPanel, { runtime }));

      return {
        dispose: () => {
          root.unmount();
          element.classList.remove("diff-view-host");
        },
        focus: () => {
          const focusTarget =
            element.querySelector<HTMLElement>(".diff-view-code") ??
            element.querySelector<HTMLElement>(".diff-view-body, .diff-view-empty");
          focusTarget?.focus();
          return document.activeElement === focusTarget;
        },
      };
    },
  };
}
