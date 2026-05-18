import { mkdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { basename, dirname, join, relative, sep } from "node:path";
import type { WorkspaceRootId } from "@the-editor/contracts/runtime";
import { commandId, matchesCommandPaletteItem, toolTypeId, type CommandPaletteItem, type EditorModule, defineHostStore } from "@the-editor/editor";
import { syncWorkbenchActiveSurface, workbenchSnapshotState } from "./workbenchSnapshot.js";

export interface FileTreeToolState {
  rootPath: string;
  paths: string[];
  activePath?: string;
  gitStatus: FileTreeGitStatusEntry[];
  loading: boolean;
  treeRevision: number;
}

export interface FileTreeGitStatusEntry {
  path: string;
  status: "added" | "deleted" | "ignored" | "modified" | "renamed" | "untracked";
}

const fileTreeToolTypeId = toolTypeId("default.fileTree");
const fileTreeStateSlot = "default.fileTree.state";
const ignoredDirectoryNames = new Set([".git", ".jj", "node_modules", "target", "dist", ".next"]);

interface FileTreeState {
  rootPath: string;
  paths: string[];
  gitStatus: FileTreeGitStatusEntry[];
  loading: boolean;
  treeRevision: number;
  workspaceRootId?: WorkspaceRootId;
  pollTimer?: ReturnType<typeof setInterval>;
  loadPromise?: Promise<void>;
}

export function defaultFileTree(): EditorModule {
  return (editor) => {
    editor.tools.registerType({
      id: fileTreeToolTypeId,
      title: "Files",
      root: "leftSidebar",
      instancePolicy: "singletonPerWorkspace",
      closeable: true,
      persistenceKey: "default.fileTree",
      snapshot() {
        const state = fileTreeState(editor);
        ensureFileTreeLoaded(editor);
        return {
          rootPath: state.rootPath,
          paths: state.paths,
          activePath: activeSurfacePath(editor),
          gitStatus: state.gitStatus,
          loading: state.loading,
          treeRevision: state.treeRevision,
        } satisfies FileTreeToolState;
      },
    });

    editor.commands.register<void, void>({
      id: commandId("fileTree.open"),
      title: "File Tree: Open",
      category: "Workbench",
      run() {
        ensureFileTreeLoaded(editor);
        editor.workbench.openTool(fileTreeToolTypeId, { placement: { kind: "dock", root: "leftSidebar" } });
      },
    });

    editor.commands.register<void, void>({
      id: commandId("fileTree.toggle"),
      title: "File Tree: Toggle",
      category: "Workbench",
      run() {
        const existing = editor.tools.findByType(fileTreeToolTypeId);
        if (existing) {
          editor.tools.close(existing.id);
          editor.workbench.invalidate("tools", "focus");
          return;
        }
        ensureFileTreeLoaded(editor);
        editor.workbench.openTool(fileTreeToolTypeId, { placement: { kind: "dock", root: "leftSidebar" }, focus: false });
      },
    });

    editor.commands.register<void, void>({
      id: commandId("fileTree.refresh"),
      title: "File Tree: Refresh",
      category: "Workbench",
      run() {
        refreshFileTree(editor);
      },
    });

    editor.commands.register<{ path: string }, void>({
      id: commandId("fileTree.openPath"),
      title: "File Tree: Open Path",
      category: "Workbench",
      hidden: true,
      async run(_ctx, args) {
        const absolute = join(fileTreeState(editor).rootPath, args.path.replace(/\/$/, ""));
        try {
          const entry = await stat(absolute);
          if (!entry.isFile()) return;
        } catch {
          return;
        }
        await editor.commands.dispatch({ id: commandId("file.open"), args: { path: absolute } });
      },
    });

    editor.commands.register<{ path: string; kind: "file" | "folder" }, void>({
      id: commandId("fileTree.createPath"),
      title: "File Tree: Create Path",
      category: "Workbench",
      hidden: true,
      async run(_ctx, args) {
        const state = fileTreeState(editor);
        const absolute = join(state.rootPath, args.path);
        try {
          if (args.kind === "folder") await mkdir(absolute, { recursive: true });
          else {
            await mkdir(dirname(absolute), { recursive: true });
            await writeFile(absolute, "", { flag: "wx" });
          }
        } catch (error) {
          editor.notifications.error("fileTree", `failed to create ${args.path}: ${error instanceof Error ? error.message : String(error)}`);
          refreshFileTree(editor);
        }
      },
    });

    editor.commands.register<{ sourcePath: string; destinationPath: string }, void>({
      id: commandId("fileTree.renamePath"),
      title: "File Tree: Rename Path",
      category: "Workbench",
      hidden: true,
      async run(_ctx, args) {
        const state = fileTreeState(editor);
        try {
          const sourceAbsolute = join(state.rootPath, args.sourcePath);
          const destinationAbsolute = join(state.rootPath, args.destinationPath);
          await mkdir(dirname(destinationAbsolute), { recursive: true });
          await rename(sourceAbsolute, destinationAbsolute);
          await retargetOpenFilePanes(editor, sourceAbsolute, destinationAbsolute);
        } catch (error) {
          editor.notifications.error("fileTree", `failed to rename ${args.sourcePath}: ${error instanceof Error ? error.message : String(error)}`);
          refreshFileTree(editor);
        }
      },
    });

    editor.commands.register<{ path: string; recursive?: boolean }, void>({
      id: commandId("fileTree.deletePath"),
      title: "File Tree: Delete Path",
      category: "Workbench",
      hidden: true,
      async run(_ctx, args) {
        const absolute = join(fileTreeState(editor).rootPath, args.path);
        try {
          await rm(absolute, { force: true, recursive: Boolean(args.recursive) });
        } catch (error) {
          editor.notifications.error("fileTree", `failed to delete ${args.path}: ${error instanceof Error ? error.message : String(error)}`);
          refreshFileTree(editor);
        }
      },
    });

    editor.palette.registerProvider({
      id: "default.fileTree",
      title: "File Tree",
      priority: 10,
      visibility: "developer",
      provide(ctx) {
        const items: CommandPaletteItem[] = [
          {
            id: "fileTree.open",
            title: "File Tree: Open",
            detail: "Open project files in the left sidebar",
            group: "Tools",
            keywords: ["files", "tree", "explorer", "sidebar"],
            command: { id: commandId("fileTree.open") },
          },
          {
            id: "fileTree.refresh",
            title: "File Tree: Refresh",
            detail: fileTreeState(editor).rootPath,
            group: "Tools",
            keywords: ["files", "tree", "refresh"],
            command: { id: commandId("fileTree.refresh") },
          },
          {
            id: "fileTree.toggle",
            title: "File Tree: Toggle",
            detail: "Show or hide the left sidebar file tree",
            group: "Tools",
            keywords: ["files", "tree", "toggle", "sidebar"],
            command: { id: commandId("fileTree.toggle") },
          },
        ];
        return items.filter((item) => matchesCommandPaletteItem(ctx.query, item));
      },
    });

    ensureFileTreeLoaded(editor);
    startFileTreeWatcher(editor);
    editor.workbench.openTool(fileTreeToolTypeId, { placement: { kind: "dock", root: "leftSidebar" }, focus: false });

    return {
      dispose() {
        const state = fileTreeState(editor);
        if (state.pollTimer) clearInterval(state.pollTimer);
        state.pollTimer = undefined;
      },
    };
  };
}

function fileTreeState(editor: Parameters<EditorModule>[0]): FileTreeState {
  const cwd = editor.cwd.current();
  const state = editor.stores.host(defineHostStore<FileTreeState>({ id: fileTreeStateSlot, init: () => ({ rootPath: cwd, paths: [], gitStatus: [], loading: false, treeRevision: 0 }) }));
  if (state.rootPath !== cwd) {
    state.rootPath = cwd;
    state.paths = [];
    state.gitStatus = [];
    state.treeRevision += 1;
    state.loadPromise = undefined;
  }
  return state;
}

function ensureFileTreeLoaded(editor: Parameters<EditorModule>[0]): void {
  const state = fileTreeState(editor);
  if (state.paths.length || state.loading) return;
  refreshFileTree(editor);
}

async function retargetOpenFilePanes(editor: Parameters<EditorModule>[0], sourceAbsolute: string, destinationAbsolute: string): Promise<void> {
  const state = workbenchSnapshotState(editor);
  const layout = editor.workbench.snapshotEditorLayout();
  const sourcePrefix = `${sourceAbsolute}${sep}`;
  const isAffected = (path: string | undefined): path is string => path === sourceAbsolute || path?.startsWith(sourcePrefix) === true;
  let changed = false;

  for (const pane of layout.panes) {
    if (pane.kind !== "surface" || !isAffected(pane.path)) continue;
    const nextPath = pane.path === sourceAbsolute
      ? destinationAbsolute
      : join(destinationAbsolute, relative(sourceAbsolute, pane.path));
    const title = basename(nextPath);

    editor.workbench.updateItem(pane.itemId, { title, path: nextPath });
    const paneState = state.surfacePanes.get(pane.id);
    if (paneState) state.surfacePanes.set(pane.id, { ...paneState, path: nextPath, title });
    await editor.runtime.surfaces.loadFile({ surfaceId: pane.surfaceId, path: nextPath });
    changed = true;
  }

  if (!changed) return;
  syncWorkbenchActiveSurface(editor, state);
  editor.workbench.invalidate("frame", "layout", "tabs");
}

function refreshFileTree(editor: Parameters<EditorModule>[0]): void {
  const state = fileTreeState(editor);
  state.loading = true;
  state.loadPromise = (async () => {
    const root = state.workspaceRootId
      ? await editor.runtime.workspace.snapshot({ rootId: state.workspaceRootId })
      : await editor.runtime.workspace.addRoot({ path: state.rootPath });
    state.workspaceRootId = root.rootId;
    const gitStatus = await collectGitStatus(state.rootPath);
    state.paths = workspacePaths(root.entries);
    state.gitStatus = gitStatus;
    state.treeRevision = root.revision;
    state.loading = false;
    editor.workbench.invalidate("tools");
  })()
    .catch((error) => {
      state.loading = false;
      editor.notifications.error("fileTree", `failed to load file tree: ${error instanceof Error ? error.message : String(error)}`);
      editor.workbench.invalidate("tools", "notifications");
    });
  editor.workbench.invalidate("tools");
}

function startFileTreeWatcher(editor: Parameters<EditorModule>[0]): void {
  const state = fileTreeState(editor);
  if (state.pollTimer) return;
  state.pollTimer = setInterval(() => {
    void refreshWatchedFileTree(editor);
  }, 750);
}

async function refreshWatchedFileTree(editor: Parameters<EditorModule>[0]): Promise<void> {
  const state = fileTreeState(editor);
  if (!state.workspaceRootId || state.loading) return;
  try {
    const changed = await editor.runtime.workspace.refreshRoot({ rootId: state.workspaceRootId });
    if (!changed.changes.length && changed.revision === state.treeRevision) return;
    const snapshot = await editor.runtime.workspace.snapshot({ rootId: state.workspaceRootId });
    state.paths = workspacePaths(snapshot.entries);
    state.gitStatus = await collectGitStatus(state.rootPath);
    state.treeRevision = snapshot.revision;
    editor.workbench.invalidate("frame", "tabs", "tools");
  } catch (error) {
    editor.notifications.error("fileTree", `failed to refresh file tree: ${error instanceof Error ? error.message : String(error)}`);
    editor.workbench.invalidate("notifications");
  }
}

const execFileAsync = promisify(execFile);

async function collectGitStatus(rootPath: string): Promise<FileTreeGitStatusEntry[]> {
  try {
    const { stdout } = await execFileAsync("git", ["-C", rootPath, "status", "--porcelain=v1", "-z"], {
      maxBuffer: 8 * 1024 * 1024,
    });
    return parseGitStatus(stdout.toString());
  } catch {
    return [];
  }
}

function parseGitStatus(output: string): FileTreeGitStatusEntry[] {
  const entries: FileTreeGitStatusEntry[] = [];
  const records = output.split("\0").filter(Boolean);
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index] ?? "";
    const xy = record.slice(0, 2);
    const path = normalizeTreePath(record.slice(3));
    if (!path) continue;
    if (xy.includes("R")) {
      entries.push({ path, status: "renamed" });
      index += 1;
      continue;
    }
    entries.push({ path, status: gitStatusKind(xy) });
  }
  return entries;
}

function gitStatusKind(xy: string): FileTreeGitStatusEntry["status"] {
  if (xy === "!!") return "ignored";
  if (xy === "??") return "untracked";
  if (xy.includes("A")) return "added";
  if (xy.includes("D")) return "deleted";
  if (xy.includes("R")) return "renamed";
  return "modified";
}

function workspacePaths(entries: Array<{ path: string; kind: string }>): string[] {
  return entries
    .filter((entry) => entry.kind === "file" || entry.kind === "directory")
    .map((entry) => entry.kind === "directory" ? `${entry.path}/` : entry.path)
    .sort((a, b) => a.localeCompare(b));
}

function normalizeTreePath(path: string): string {
  return sep === "/" ? path : path.split(sep).join("/");
}

function activeSurfacePath(editor: Parameters<EditorModule>[0]): string | undefined {
  const pane = editor.workbench.activePane();
  return pane?.kind === "surface" ? pane.path : undefined;
}

export { fileTreeToolTypeId };
