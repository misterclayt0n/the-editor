import { useEffect, useRef, type ReactNode } from "react";
import type { FileTreeContextMenuItem, FileTreeContextMenuOpenContext, FileTreeMutationEvent, FileTreeRenameEvent } from "@pierre/trees";
import { FileTree, useFileTree } from "@pierre/trees/react";
import type { FileTreeUiContribution, UiCommandDispatcher } from "@the-editor/editor/ui";
import type { FileTreeToolState } from "./fileTree.js";
import "./fileTree.css";

const fileTreeDomActionEvent = "the-editor:file-tree-action";
type FileTreeDomAction = { kind: "openSearch" | "newFile" | "newFolder" };


export function DefaultFileTree({
  commands,
  contributions = [],
  state,
}: {
  state: FileTreeToolState | undefined;
  commands: UiCommandDispatcher;
  contributions?: readonly FileTreeUiContribution[];
}) {
  const activePath = state?.activePath && state.rootPath && state.activePath.startsWith(state.rootPath)
    ? normalizePath(state.activePath.slice(state.rootPath.length).replace(/^\/+/, ""))
    : undefined;
  const rootRef = useRef<HTMLDivElement>(null);
  const latestSelectedRef = useRef<string | undefined>(activePath);
  const draftEntriesRef = useRef(new Map<string, "file" | "folder">());
  const lastResetRef = useRef<{ rootPath: string; treeRevision: number } | undefined>(undefined);
  const suppressedMovesRef = useRef(new Set<string>());
  const suppressedSelectionRef = useRef(new Set<string>());
  const { model } = useFileTree({
    flattenEmptyDirectories: true,
    initialExpansion: "closed",
    initialSelectedPaths: activePath ? [activePath] : [],
    composition: {
      contextMenu: {
        enabled: true,
        triggerMode: "both",
        buttonVisibility: "when-needed",
      },
    },
    dragAndDrop: {
      canDrop(event) {
        return event.draggedPaths.every((draggedPath) => {
          const target = event.target.directoryPath ?? "";
          const normalizedDragged = draggedPath.replace(/\/$/, "");
          return target !== normalizedDragged && !target.startsWith(`${normalizedDragged}/`);
        });
      },
      onDropError(error, event) {
        console.error("file tree drop failed", error);
      },
    },
    ...mergedFileTreeOptions(contributions),
    fileTreeSearchMode: "hide-non-matches",
    gitStatus: state?.gitStatus ?? [],
    paths: state?.paths ?? [],
    renaming: {
      onRename(event: FileTreeRenameEvent) {
        consumeSuppressedPath(suppressedSelectionRef.current, event.sourcePath);
        consumeSuppressedPath(suppressedSelectionRef.current, event.destinationPath);
        suppressMove(suppressedMovesRef.current, event.sourcePath, event.destinationPath);
        const draftKind = consumeDraftEntry(draftEntriesRef.current, event.sourcePath);
        if (draftKind) {
          void commands.dispatch({
            id: "fileTree.createPath",
            args: { path: event.destinationPath.replace(/\/$/, ""), kind: draftKind },
          });
          return;
        }
        void commands.dispatch({
          id: "fileTree.renamePath",
          args: { sourcePath: event.sourcePath, destinationPath: event.destinationPath },
        });
      },
      onError(error) {
        console.error("file tree rename failed", error);
      },
    },
    search: true,
    searchBlurBehavior: "retain",
    stickyFolders: true,
    unsafeCSS: fileTreeUnsafeCss,
    onSelectionChange(paths) {
      const path = paths.at(-1);
      if (!path || path === latestSelectedRef.current || model.getItem(path)?.isDirectory()) return;
      if (hasDraftEntry(draftEntriesRef.current, path) || consumeSuppressedPath(suppressedSelectionRef.current, path)) {
        latestSelectedRef.current = path;
        return;
      }
      latestSelectedRef.current = path;
      void openTreePath(commands, path);
    },
  });

  const paths = state?.paths ?? [];
  const treeRevision = state?.treeRevision ?? 0;

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const install = () => installPierreOverlayOverflowFix(root);
    install();
    const observer = new MutationObserver(install);
    observer.observe(root, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const rootPath = state?.rootPath ?? "";
    const previous = lastResetRef.current;
    if (previous?.rootPath === rootPath && previous.treeRevision === treeRevision) {
      return;
    }
    if (!rootPath || (paths.length === 0 && state?.loading)) {
      return;
    }
    lastResetRef.current = { rootPath, treeRevision };
    const expandedPaths = paths.filter((path) => path.endsWith("/") && model.getItem(path)?.isDirectory() && model.getItem(path)?.isExpanded());
    model.resetPaths(paths, { initialExpandedPaths: expandedPaths });
  }, [model, paths, state?.loading, state?.rootPath, treeRevision]);

  useEffect(() => {
    model.setGitStatus(state?.gitStatus ?? []);
  }, [model, state?.gitStatus]);

  useEffect(() => model.onMutation("*", (event) => {
    persistTreeMutation(commands, event, {
      drafts: draftEntriesRef.current,
      suppressedMoves: suppressedMovesRef.current,
    });
  }), [commands, model]);

  useEffect(() => {
    latestSelectedRef.current = activePath;
    if (!activePath) return;
    model.getItem(activePath)?.select();
  }, [activePath, treeRevision, model]);

  useEffect(() => {
    const handleAction = (event: Event) => {
      const action = (event as CustomEvent<FileTreeDomAction>).detail;
      if (action.kind === "openSearch") model.openSearch();
      else createDraftEntry(model, draftEntriesRef.current, suppressedSelectionRef.current, "", action.kind === "newFile" ? "file" : "folder");
    };
    window.addEventListener(fileTreeDomActionEvent, handleAction);
    return () => window.removeEventListener(fileTreeDomActionEvent, handleAction);
  }, [model]);

  return (
    <div
      ref={rootRef}
      className="defaultFileTree"
      onKeyDown={(event) => {
        if (event.key !== "Enter") return;
        const path = model.getFocusedPath() ?? model.getSelectedPaths().at(-1);
        if (!path || model.getItem(path)?.isDirectory()) return;
        event.preventDefault();
        event.stopPropagation();
        latestSelectedRef.current = path;
        void openTreePath(commands, path);
      }}
    >
      <FileTree
        model={model}
        renderContextMenu={(item, context) => renderContextMenu(contributions, item, context) ?? (
          <FileTreeContextMenu item={item} context={context} model={model} drafts={draftEntriesRef.current} suppressedSelection={suppressedSelectionRef.current} />
        )}
        style={{ height: "100%" }}
      />
    </div>
  );
}

function installPierreOverlayOverflowFix(root: HTMLElement): void {
  const tree = root.querySelector("file-tree-container") as HTMLElement & { shadowRoot?: ShadowRoot | null } | null;
  const shadowRoot = tree?.shadowRoot;
  if (!shadowRoot || shadowRoot.getElementById("the-editor-file-tree-overlay-overflow")) return;
  const style = document.createElement("style");
  style.id = "the-editor-file-tree-overlay-overflow";
  style.textContent = `
    :host([data-file-tree-virtualized='true']),
    [data-file-tree-virtualized-wrapper='true'],
    [data-file-tree-virtualized-root='true'] {
      overflow: visible !important;
    }

    [data-type='context-menu-anchor'] {
      z-index: 1000 !important;
    }
  `;
  shadowRoot.append(style);
}

function mergedFileTreeOptions(contributions: readonly FileTreeUiContribution[]): Record<string, unknown> {
  return Object.assign({}, ...contributions.map((contribution) => contribution.options ?? {}));
}

function renderContextMenu(
  contributions: readonly FileTreeUiContribution[],
  item: FileTreeContextMenuItem,
  context: FileTreeContextMenuOpenContext,
): ReactNode | null {
  for (const contribution of [...contributions].reverse()) {
    const rendered = contribution.renderContextMenu?.(item, context);
    if (rendered !== undefined && rendered !== null) return rendered as ReactNode;
  }
  return null;
}

function FileTreeContextMenu({
  context,
  item,
  model,
  drafts,
  suppressedSelection,
}: {
  context: FileTreeContextMenuOpenContext;
  item: FileTreeContextMenuItem;
  model: ReturnType<typeof useFileTree>["model"];
  drafts: Map<string, "file" | "folder">;
  suppressedSelection: Set<string>;
}): ReactNode {
  const directoryPath = item.kind === "directory" ? item.path.replace(/\/$/, "") : parentPath(item.path);
  const close = (restoreFocus = true) => context.close({ restoreFocus });
  return (
    <div className="defaultFileTreeContextMenu" data-file-tree-context-menu-root="true">
      <button type="button" onClick={() => { close(false); createDraftEntry(model, drafts, suppressedSelection, directoryPath, "file"); }}>New file</button>
      <button type="button" onClick={() => { close(false); createDraftEntry(model, drafts, suppressedSelection, directoryPath, "folder"); }}>New folder</button>
      <button type="button" onClick={() => { close(false); suppressPath(suppressedSelection, item.path); if (!model.startRenaming(item.path)) console.error("file tree rename unavailable", item.path); }}>Rename</button>
      <div className="defaultFileTreeContextMenuSeparator" role="separator" />
      <button type="button" className="danger" onClick={() => { close(); model.remove(item.path, item.kind === "directory" ? { recursive: true } : undefined); }}>Delete</button>
    </div>
  );
}

async function openTreePath(commands: UiCommandDispatcher, path: string): Promise<void> {
  await commands.dispatch({ id: "fileTree.openPath", args: { path } });
  blurDeepActiveElement();
  document.body.focus();
  await commands.dispatch({ id: "workbench.focusGroup", args: { groupId: "active" } });
}

function blurDeepActiveElement(root: Document | ShadowRoot = document): void {
  const active = root.activeElement;
  if (!active) return;
  if (active.shadowRoot) blurDeepActiveElement(active.shadowRoot);
  if (active instanceof HTMLElement) active.blur();
}

function createDraftEntry(model: ReturnType<typeof useFileTree>["model"], drafts: Map<string, "file" | "folder">, suppressedSelection: Set<string>, directoryPath: string, kind: "file" | "folder"): boolean {
  const basePath = directoryPath ? `${directoryPath}/` : "";
  const template = kind === "folder" ? "untitled/" : "untitled";
  const path = uniquePath(model, `${basePath}${template}`);
  drafts.set(path, kind);
  drafts.set(path.replace(/\/$/, ""), kind);
  suppressPath(suppressedSelection, path);
  model.add(path);
  const started = model.startRenaming(path, { removeIfCanceled: true });
  if (started) return true;
  consumeDraftEntry(drafts, path);
  console.error("file tree rename unavailable", path);
  return false;
}

function hasDraftEntry(drafts: Map<string, "file" | "folder">, path: string): boolean {
  const normalized = path.replace(/\/$/, "");
  return drafts.has(path) || drafts.has(normalized) || drafts.has(`${normalized}/`);
}

function consumeDraftEntry(drafts: Map<string, "file" | "folder">, path: string): "file" | "folder" | undefined {
  const normalized = path.replace(/\/$/, "");
  const kind = drafts.get(path) ?? drafts.get(normalized) ?? drafts.get(`${normalized}/`);
  drafts.delete(path);
  drafts.delete(normalized);
  drafts.delete(`${normalized}/`);
  return kind;
}

function uniquePath(model: ReturnType<typeof useFileTree>["model"], path: string): string {
  if (!model.getItem(path)) return path;
  const isFolder = path.endsWith("/");
  const trimmed = path.replace(/\/$/, "");
  for (let index = 2; index < 1000; index += 1) {
    const candidate = `${trimmed}-${index}${isFolder ? "/" : ""}`;
    if (!model.getItem(candidate)) return candidate;
  }
  return path;
}

function persistTreeMutation(
  commands: UiCommandDispatcher,
  event: FileTreeMutationEvent,
  state: {
    drafts: Map<string, "file" | "folder">;
    suppressedMoves: Set<string>;
  },
): void {
  if (event.operation === "batch") {
    for (const entry of event.events) persistTreeMutation(commands, entry, state);
    return;
  }
  if (event.operation === "reset") {
    return;
  }
  if (event.operation === "add") {
    return;
  }
  if (event.operation === "remove") {
    if (consumeDraftEntry(state.drafts, event.path)) {
      return;
    }
    void commands.dispatch({
      id: "fileTree.deletePath",
      args: { path: event.path.replace(/\/$/, ""), recursive: event.recursive || event.path.endsWith("/") },
    });
    return;
  }
  if (consumeSuppressedMove(state.suppressedMoves, event.from, event.to)) {
    return;
  }
  void commands.dispatch({
    id: "fileTree.renamePath",
    args: { sourcePath: event.from, destinationPath: event.to },
  });
}

function suppressPath(paths: Set<string>, path: string): void {
  paths.add(path);
  paths.add(path.replace(/\/$/, ""));
}

function consumeSuppressedPath(paths: Set<string>, path: string): boolean {
  const normalized = path.replace(/\/$/, "");
  const found = paths.has(path) || paths.has(normalized) || paths.has(`${normalized}/`);
  paths.delete(path);
  paths.delete(normalized);
  paths.delete(`${normalized}/`);
  return found;
}

function suppressMove(suppressedMoves: Set<string>, from: string, to: string): void {
  suppressedMoves.add(`${from}\u0000${to}`);
  suppressedMoves.add(`${from.replace(/\/$/, "")}\u0000${to.replace(/\/$/, "")}`);
}

function consumeSuppressedMove(suppressedMoves: Set<string>, from: string, to: string): boolean {
  const keys = [`${from}\u0000${to}`, `${from.replace(/\/$/, "")}\u0000${to.replace(/\/$/, "")}`];
  const found = keys.some((key) => suppressedMoves.has(key));
  for (const key of keys) suppressedMoves.delete(key);
  return found;
}

function parentPath(path: string): string {
  const normalized = path.replace(/\/$/, "");
  const index = normalized.lastIndexOf("/");
  return index < 0 ? "" : normalized.slice(0, index);
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, "/");
}

const fileTreeUnsafeCss = `
  :host {
    --trees-bg-override: var(--theme-ui-menu-bg, #101010);
    --trees-fg-override: var(--theme-ui-text-fg, #d8d8d8);
    --trees-selected-bg-override: var(--theme-ui-menu-selected-bg, #242424);
    --trees-selected-fg-override: var(--theme-ui-menu-selected-fg, #f5f5f5);
    --trees-border-color-override: transparent;
    color-scheme: dark;
    font-family: Inter, ui-sans-serif, system-ui, sans-serif;
    height: 100%;
  }

  [data-file-tree-search-container][data-open='false'] {
    display: none;
  }

  [data-file-tree-search-container] {
    margin-bottom: 6px;
  }

  button[data-type='item'] {
    border-radius: 5px;
    font-size: 13px;
    min-height: 28px;
  }
`;
