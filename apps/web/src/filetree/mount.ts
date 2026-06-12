/**
 * File tree panel content: @pierre/trees mounted into a workbench panel.
 * The tree is path-first (it renders whatever path list it is given), so
 * this module owns the LISTING — the VCS provider's file list when the
 * workspace is a repo (gitignore semantics for free), a capped fs walk
 * otherwise — plus watch-driven refresh, selection → openPath, and git
 * status badges from the VCS manager.
 */

import { FileTree, type GitStatusEntry } from "@pierre/trees";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "../workbench/panelRenderers";

const WALK_IGNORE = new Set([".git", ".jj", "node_modules", ".DS_Store"]);
const WALK_MAX_PATHS = 30_000;
const WALK_MAX_DEPTH = 32;
const REFRESH_DEBOUNCE_MS = 200;

/** Fallback listing for non-repo folders: bounded recursive walk. */
async function walk(runtime: AppRuntime, root: string): Promise<string[]> {
  const port = runtime.fileSystem();
  if (port?.readDir === undefined) return [];
  const { Effect } = await import("effect");
  const out: string[] = [];
  const visit = async (dir: string, prefix: string, depth: number): Promise<void> => {
    if (depth > WALK_MAX_DEPTH || out.length >= WALK_MAX_PATHS) return;
    let entries;
    try {
      entries = await Effect.runPromise(port.readDir!(dir));
    } catch {
      return;
    }
    for (const entry of entries) {
      if (WALK_IGNORE.has(entry.name)) continue;
      if (out.length >= WALK_MAX_PATHS) return;
      const rel = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
      if (entry.kind === "directory") {
        await visit(`${dir}/${entry.name}`, rel, depth + 1);
      } else {
        out.push(rel);
      }
    }
  };
  await visit(root, "", 0);
  return out;
}

async function listWorkspace(runtime: AppRuntime, root: string): Promise<string[]> {
  const fromVcs = await runtime.vcs?.listFiles(root);
  if (fromVcs !== null && fromVcs !== undefined) return [...fromVcs];
  return walk(runtime, root);
}

export const FILE_TREE_TITLE = "Files";

export function createFileTreeRenderer(runtime: AppRuntime): PanelRenderer {
  return {
    title: () => FILE_TREE_TITLE,
    mount: (element, _panel) => {
      let disposed = false;
      let tree: FileTree | null = null;
      let unwatch: (() => void) | null = null;
      let refreshTimer: number | null = null;
      const cleanups: Array<() => void> = [];

      element.classList.add("file-tree-host");

      const empty = () => {
        element.replaceChildren();
        const wrap = document.createElement("div");
        wrap.className = "file-tree-empty";
        const note = document.createElement("div");
        note.textContent = "No folder open";
        const button = document.createElement("button");
        button.type = "button";
        button.className = "file-tree-open-button";
        button.textContent = "Open Folder…";
        button.onclick = () => void runtime.openFolderViaDialog();
        wrap.append(note, button);
        element.append(wrap);
      };

      const applyGitStatus = () => {
        const entries = runtime.vcs?.treeStatusEntries() ?? [];
        tree?.setGitStatus(entries as GitStatusEntry[]);
      };

      const setUp = async (root: string) => {
        const paths = await listWorkspace(runtime, root);
        if (disposed) return;
        element.replaceChildren();
        tree?.cleanUp();
        tree = new FileTree({
          paths,
          search: true,
          flattenEmptyDirectories: true,
          initialExpansion: "closed",
          onSelectionChange: (selected) => {
            const path = selected[0];
            if (path === undefined) return;
            const item = tree?.getItem(path);
            if (item === null || item === undefined || item.isDirectory()) return;
            void runtime.openPath(`${root}/${path}`);
          },
        });
        tree.render({ containerWrapper: element });
        applyGitStatus();

        // Watch-driven refresh: any change under the root re-lists.
        const port = runtime.fileSystem();
        if (port?.watchDir !== undefined) {
          const { Effect } = await import("effect");
          try {
            const dispose = await Effect.runPromise(
              port.watchDir(root, (event) => {
                // VCS internals churn constantly (every `git status` touches
                // .git); the tree only cares about working files.
                if (event.path.includes("/.git/") || event.path.includes("/.jj/")) return;
                if (refreshTimer !== null) window.clearTimeout(refreshTimer);
                refreshTimer = window.setTimeout(() => {
                  refreshTimer = null;
                  void listWorkspace(runtime, root).then((next) => {
                    if (!disposed) tree?.resetPaths(next);
                  });
                }, REFRESH_DEBOUNCE_MS);
              }),
            );
            unwatch = dispose;
          } catch {
            // Watching is best-effort.
          }
        }
      };

      const boot = () => {
        unwatch?.();
        unwatch = null;
        const root = runtime.workspaceRoot;
        if (root === null) empty();
        else void setUp(root);
      };

      boot();
      cleanups.push(runtime.subscribeWorkspaceRoot(() => boot()));
      if (runtime.vcs !== undefined) {
        cleanups.push(runtime.vcs.subscribe(() => applyGitStatus()));
      }

      return () => {
        disposed = true;
        if (refreshTimer !== null) window.clearTimeout(refreshTimer);
        unwatch?.();
        for (const cleanup of cleanups) cleanup();
        tree?.cleanUp();
        element.replaceChildren();
      };
    },
  };
}
