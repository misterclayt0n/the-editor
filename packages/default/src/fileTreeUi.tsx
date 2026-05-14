import { useEffect, useRef, type ReactNode } from "react";
import type { FileTreeContextMenuItem, FileTreeContextMenuOpenContext, FileTreeDropResult, FileTreeRenameEvent } from "@pierre/trees";
import { FileTree, useFileTree } from "@pierre/trees/react";
import type { FileTreeUiContribution, UiCommandDispatcher } from "@the-editor/editor/ui";
import type { FileTreeToolState } from "./fileTree.js";
import "./fileTree.css";

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
  const latestSelectedRef = useRef<string | undefined>(activePath);
  const handledActionIdRef = useRef<number | undefined>(undefined);
  const draftEntriesRef = useRef(new Map<string, "file" | "folder">());
  const { model } = useFileTree({
    flattenEmptyDirectories: true,
    initialExpansion: "closed",
    initialSelectedPaths: activePath ? [activePath] : [],
    dragAndDrop: {
      canDrop(event) {
        return event.draggedPaths.every((draggedPath) => {
          const target = event.target.directoryPath ?? "";
          const normalizedDragged = draggedPath.replace(/\/$/, "");
          return target !== normalizedDragged && !target.startsWith(`${normalizedDragged}/`);
        });
      },
      onDropComplete(event) {
        for (const draggedPath of event.draggedPaths) {
          const destinationPath = destinationPathForDrop(draggedPath, event);
          if (!destinationPath || destinationPath === draggedPath) continue;
          void commands.dispatch({ id: "fileTree.renamePath", args: { sourcePath: draggedPath, destinationPath } });
        }
      },
      onDropError(error) {
        console.error("file tree drop failed", error);
      },
    },
    ...mergedFileTreeOptions(contributions),
    fileTreeSearchMode: "hide-non-matches",
    gitStatus: state?.gitStatus ?? [],
    paths: state?.paths ?? [],
    renaming: {
      onRename(event: FileTreeRenameEvent) {
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
      latestSelectedRef.current = path;
      void openTreePath(commands, path);
    },
  });

  const paths = state?.paths ?? [];
  const treeRevision = state?.treeRevision ?? 0;

  useEffect(() => {
    const expandedPaths = paths.filter((path) => path.endsWith("/") && model.getItem(path)?.isDirectory() && model.getItem(path)?.isExpanded());
    model.resetPaths(paths, { initialExpandedPaths: expandedPaths });
  }, [model, treeRevision]);

  useEffect(() => {
    model.setGitStatus(state?.gitStatus ?? []);
  }, [model, state?.gitStatus]);

  useEffect(() => model.onMutation("remove", (event) => {
    if (consumeDraftEntry(draftEntriesRef.current, event.path)) return;
    void commands.dispatch({
      id: "fileTree.deletePath",
      args: { path: event.path.replace(/\/$/, ""), recursive: event.recursive || event.path.endsWith("/") },
    });
  }), [commands, model]);

  useEffect(() => {
    latestSelectedRef.current = activePath;
    if (!activePath) return;
    model.getItem(activePath)?.select();
  }, [activePath, treeRevision, model]);

  useEffect(() => {
    const action = state?.action;
    if (!action || handledActionIdRef.current === action.id) return;
    handledActionIdRef.current = action.id;
    if (action.kind === "openSearch") model.openSearch();
    if (action.kind === "createEntry") createDraftEntry(model, draftEntriesRef.current, "", action.entryKind);
    void commands.dispatch({ id: "fileTree.consumeAction", args: { id: action.id } });
  }, [commands, model, state?.action]);

  return (
    <div
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
          <FileTreeContextMenu item={item} context={context} model={model} commands={commands} drafts={draftEntriesRef.current} />
        )}
        style={{ height: "100%" }}
      />
    </div>
  );
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
  commands,
  context,
  item,
  model,
  drafts,
}: {
  commands: UiCommandDispatcher;
  context: FileTreeContextMenuOpenContext;
  item: FileTreeContextMenuItem;
  model: ReturnType<typeof useFileTree>["model"];
  drafts: Map<string, "file" | "folder">;
}): ReactNode {
  const directoryPath = item.kind === "directory" ? item.path.replace(/\/$/, "") : parentPath(item.path);
  const close = (restoreFocus = true) => context.close({ restoreFocus });
  return (
    <div className="defaultFileTreeContextMenu" data-file-tree-context-menu-root="true">
      {item.kind === "file" ? <button onClick={() => { close(); void openTreePath(commands, item.path); }}>Open</button> : null}
      <button onClick={() => { close(false); model.startRenaming(item.path); }}>Rename</button>
      <button onClick={() => { close(false); createDraftEntry(model, drafts, directoryPath, "file"); }}>New File</button>
      <button onClick={() => { close(false); createDraftEntry(model, drafts, directoryPath, "folder"); }}>New Folder</button>
      <button className="danger" onClick={() => { close(false); model.remove(item.path, item.kind === "directory" ? { recursive: true } : undefined); }}>Delete</button>
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

function createDraftEntry(model: ReturnType<typeof useFileTree>["model"], drafts: Map<string, "file" | "folder">, directoryPath: string, kind: "file" | "folder"): void {
  const basePath = directoryPath ? `${directoryPath}/` : "";
  const template = kind === "folder" ? "untitled/" : "untitled";
  const path = uniquePath(model, `${basePath}${template}`);
  drafts.set(path, kind);
  drafts.set(path.replace(/\/$/, ""), kind);
  model.add(path);
  if (!model.startRenaming(path, { removeIfCanceled: true })) consumeDraftEntry(drafts, path);
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

function destinationPathForDrop(draggedPath: string, event: FileTreeDropResult): string | undefined {
  const targetDirectory = event.target.directoryPath ?? "";
  const name = basename(draggedPath.replace(/\/$/, ""));
  if (!name) return undefined;
  return `${targetDirectory ? `${targetDirectory.replace(/\/$/, "")}/` : ""}${name}${draggedPath.endsWith("/") ? "/" : ""}`;
}

function parentPath(path: string): string {
  const normalized = path.replace(/\/$/, "");
  const index = normalized.lastIndexOf("/");
  return index < 0 ? "" : normalized.slice(0, index);
}

function basename(path: string): string {
  return path.split("/").filter(Boolean).at(-1) ?? path;
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
