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
  const openedSearchRevisionRef = useRef(state?.searchRevision ?? 0);
  const createFileRevisionRef = useRef(state?.createFileRevision ?? 0);
  const createFolderRevisionRef = useRef(state?.createFolderRevision ?? 0);
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
  const pathsSignature = `${paths.length}:${paths[0] ?? ""}:${paths.at(-1) ?? ""}`;

  useEffect(() => {
    const expandedPaths = paths.filter((path) => path.endsWith("/") && model.getItem(path)?.isDirectory() && model.getItem(path)?.isExpanded());
    model.resetPaths(paths, { initialExpandedPaths: expandedPaths });
  }, [model, pathsSignature]);

  useEffect(() => {
    model.setGitStatus(state?.gitStatus ?? []);
  }, [model, state?.gitStatus]);

  useEffect(() => model.onMutation("add", (event) => {
    const path = event.path.replace(/\/$/, "");
    const kind = event.path.endsWith("/") ? "folder" : "file";
    void commands.dispatch({ id: "fileTree.createPath", args: { path, kind } });
  }), [commands, model]);

  useEffect(() => model.onMutation("remove", (event) => {
    void commands.dispatch({
      id: "fileTree.deletePath",
      args: { path: event.path.replace(/\/$/, ""), recursive: event.recursive || event.path.endsWith("/") },
    });
  }), [commands, model]);

  useEffect(() => {
    latestSelectedRef.current = activePath;
    if (!activePath) return;
    model.getItem(activePath)?.select();
  }, [activePath, model]);

  useEffect(() => {
    const revision = state?.searchRevision ?? 0;
    if (revision <= openedSearchRevisionRef.current) return;
    openedSearchRevisionRef.current = revision;
    model.openSearch();
  }, [model, state?.searchRevision]);

  useEffect(() => {
    const revision = state?.createFileRevision ?? 0;
    if (revision <= createFileRevisionRef.current) return;
    createFileRevisionRef.current = revision;
    addEntry(model, "", "file");
  }, [model, state?.createFileRevision]);

  useEffect(() => {
    const revision = state?.createFolderRevision ?? 0;
    if (revision <= createFolderRevisionRef.current) return;
    createFolderRevisionRef.current = revision;
    addEntry(model, "", "folder");
  }, [model, state?.createFolderRevision]);

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
          <FileTreeContextMenu item={item} context={context} model={model} commands={commands} />
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
}: {
  commands: UiCommandDispatcher;
  context: FileTreeContextMenuOpenContext;
  item: FileTreeContextMenuItem;
  model: ReturnType<typeof useFileTree>["model"];
}): ReactNode {
  const directoryPath = item.kind === "directory" ? item.path.replace(/\/$/, "") : parentPath(item.path);
  const close = (restoreFocus = true) => context.close({ restoreFocus });
  return (
    <div className="defaultFileTreeContextMenu" data-file-tree-context-menu-root="true">
      {item.kind === "file" ? <button onClick={() => { close(); void openTreePath(commands, item.path); }}>Open</button> : null}
      <button onClick={() => { close(false); model.startRenaming(item.path); }}>Rename</button>
      <button onClick={() => { close(false); addEntry(model, directoryPath, "file"); }}>New File</button>
      <button onClick={() => { close(false); addEntry(model, directoryPath, "folder"); }}>New Folder</button>
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

function addEntry(model: ReturnType<typeof useFileTree>["model"], directoryPath: string, kind: "file" | "folder"): void {
  const basePath = directoryPath ? `${directoryPath}/` : "";
  const template = kind === "folder" ? "untitled/" : "untitled";
  const path = uniquePath(model, `${basePath}${template}`);
  model.add(path);
  model.startRenaming(path, { removeIfCanceled: true });
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
