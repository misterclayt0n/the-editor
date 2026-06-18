/**
 * Diff view panel content: @pierre/diffs mounted into a workbench panel.
 *
 * The panel renders the workspace's working-tree diff against HEAD as a single
 * global view. It is data-first: whatever patch `VcsManager.patch()` returns is
 * parsed and displayed. Changes to the workspace root or VCS status refresh the
 * view, and the editor's active theme drives the diff viewer's light/dark mode.
 */

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { jsx } from "react/jsx-runtime";
import { createRoot, type Root } from "react-dom/client";
import { Effect, Fiber, Stream } from "effect";
import type { ResolvedTheme } from "@the-editor/theme";
import { CodeView, type CodeViewDiffItem, type CodeViewItem } from "@pierre/diffs/react";
import { parsePatchFiles } from "@pierre/diffs";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "../workbench/panelRenderers";

export const DIFF_VIEW_TITLE = "Changes";

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
  const [appearance, setAppearance] = useState<ResolvedTheme["appearance"] | null>(null);

  useEffect(() => {
    let cancelled = false;
    const initial = Effect.runSync(runtime.themes.active.pipe(Effect.option));
    if (initial._tag === "Some") {
      setAppearance(initial.value.appearance);
    }
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

function DiffViewPanel({ runtime }: { readonly runtime: AppRuntime }) {
  const [items, setItems] = useState<CodeViewItem<undefined>[]>([]);
  const [loading, setLoading] = useState(true);
  const [noRepo, setNoRepo] = useState(false);
  const appearance = useActiveThemeAppearance(runtime);
  const refreshGeneration = useRef(0);

  const options = useMemo(
    () => ({
      diffStyle: "unified" as const,
      // Pierre's default theme pair (pierre-dark / pierre-light) is already
      // registered by @pierre/diffs. We just pin the mode to the editor's
      // resolved appearance so the diff panel never disagrees with the chrome.
      themeType: appearance ?? ("system" as const),
      disableVirtualizationBuffers: false,
    }),
    [appearance],
  );

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

  if (loading && items.length === 0) return emptyState("Loading changes…");
  if (noRepo) return emptyState("No version control repository");
  if (items.length === 0) return emptyState("No changes");

  return jsx("div", {
    className: "diff-view-body",
    tabIndex: -1,
    children: jsx(CodeView, { items, options }),
  });
}

export function createDiffViewRenderer(runtime: AppRuntime): PanelRenderer {
  return {
    title: () => DIFF_VIEW_TITLE,
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
          const body = element.querySelector<HTMLElement>(".diff-view-body");
          body?.focus();
          return document.activeElement === body;
        },
      };
    },
  };
}
