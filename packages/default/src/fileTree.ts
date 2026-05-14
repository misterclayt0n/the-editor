import { mkdir, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { dirname, join, relative, sep } from "node:path";
import { commandId, matchesCommandPaletteItem, toolTypeId, type CommandPaletteItem, type EditorModule, defineHostStore } from "@the-editor/editor";

export interface FileTreeToolState {
  rootPath: string;
  paths: string[];
  activePath?: string;
  gitStatus: FileTreeGitStatusEntry[];
  loading: boolean;
  treeRevision: number;
  action?: FileTreeUiAction;
}

export type FileTreeUiAction =
  | { id: number; kind: "openSearch" }
  | { id: number; kind: "createEntry"; entryKind: "file" | "folder" };

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
  nextActionId: number;
  action?: FileTreeUiAction;
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
          action: state.action,
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
      id: commandId("fileTree.search"),
      title: "File Tree: Search",
      category: "Workbench",
      run() {
        const state = fileTreeState(editor);
        state.action = { id: state.nextActionId++, kind: "openSearch" };
        ensureFileTreeLoaded(editor);
        editor.workbench.openTool(fileTreeToolTypeId, { placement: { kind: "dock", root: "leftSidebar" }, focus: false });
        editor.workbench.invalidate("tools");
      },
    });

    editor.commands.register<void, void>({
      id: commandId("fileTree.newFile"),
      title: "File Tree: New File",
      category: "Workbench",
      run() {
        const state = fileTreeState(editor);
        state.action = { id: state.nextActionId++, kind: "createEntry", entryKind: "file" };
        editor.workbench.openTool(fileTreeToolTypeId, { placement: { kind: "dock", root: "leftSidebar" }, focus: false });
        editor.workbench.invalidate("tools");
      },
    });

    editor.commands.register<void, void>({
      id: commandId("fileTree.newFolder"),
      title: "File Tree: New Folder",
      category: "Workbench",
      run() {
        const state = fileTreeState(editor);
        state.action = { id: state.nextActionId++, kind: "createEntry", entryKind: "folder" };
        editor.workbench.openTool(fileTreeToolTypeId, { placement: { kind: "dock", root: "leftSidebar" }, focus: false });
        editor.workbench.invalidate("tools");
      },
    });

    editor.commands.register<{ id: number }, void>({
      id: commandId("fileTree.consumeAction"),
      title: "File Tree: Consume UI Action",
      category: "Workbench",
      hidden: true,
      run(_ctx, args) {
        const state = fileTreeState(editor);
        if (state.action?.id === args.id) state.action = undefined;
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
          refreshFileTree(editor);
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
        if (args.kind === "folder") await mkdir(absolute, { recursive: true });
        else {
          await mkdir(dirname(absolute), { recursive: true });
          await writeFile(absolute, "", { flag: "wx" });
        }
        refreshFileTree(editor);
      },
    });

    editor.commands.register<{ sourcePath: string; destinationPath: string }, void>({
      id: commandId("fileTree.renamePath"),
      title: "File Tree: Rename Path",
      category: "Workbench",
      hidden: true,
      async run(_ctx, args) {
        const state = fileTreeState(editor);
        await mkdir(dirname(join(state.rootPath, args.destinationPath)), { recursive: true });
        await rename(join(state.rootPath, args.sourcePath), join(state.rootPath, args.destinationPath));
        refreshFileTree(editor);
      },
    });

    editor.commands.register<{ path: string; recursive?: boolean }, void>({
      id: commandId("fileTree.deletePath"),
      title: "File Tree: Delete Path",
      category: "Workbench",
      hidden: true,
      async run(_ctx, args) {
        await rm(join(fileTreeState(editor).rootPath, args.path), { force: true, recursive: Boolean(args.recursive) });
        refreshFileTree(editor);
      },
    });

    editor.palette.registerProvider({
      id: "default.fileTree",
      title: "File Tree",
      priority: 10,
      provide(_ctx, query) {
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
        return items.filter((item) => matchesCommandPaletteItem(query, item));
      },
    });

    ensureFileTreeLoaded(editor);
    editor.workbench.openTool(fileTreeToolTypeId, { placement: { kind: "dock", root: "leftSidebar" }, focus: false });
  };
}

function fileTreeState(editor: Parameters<EditorModule>[0]): FileTreeState {
  const cwd = editor.cwd.current();
  const state = editor.stores.host(defineHostStore<FileTreeState>({ id: fileTreeStateSlot, init: () => ({ rootPath: cwd, paths: [], gitStatus: [], loading: false, treeRevision: 0, nextActionId: 1 }) }));
  if (state.rootPath !== cwd) {
    state.rootPath = cwd;
    state.paths = [];
    state.gitStatus = [];
    state.treeRevision += 1;
    state.action = undefined;
    state.loadPromise = undefined;
  }
  return state;
}

function ensureFileTreeLoaded(editor: Parameters<EditorModule>[0]): void {
  const state = fileTreeState(editor);
  if (state.paths.length || state.loading) return;
  refreshFileTree(editor);
}

function refreshFileTree(editor: Parameters<EditorModule>[0]): void {
  const state = fileTreeState(editor);
  state.loading = true;
  state.loadPromise = Promise.all([collectWorkspacePaths(state.rootPath), collectGitStatus(state.rootPath)])
    .then(([paths, gitStatus]) => {
      state.paths = paths;
      state.gitStatus = gitStatus;
      state.treeRevision += 1;
      state.loading = false;
      editor.workbench.invalidate("tools");
    })
    .catch((error) => {
      state.loading = false;
      editor.notifications.error("fileTree", `failed to load file tree: ${error instanceof Error ? error.message : String(error)}`);
      editor.workbench.invalidate("tools", "notifications");
    });
  editor.workbench.invalidate("tools");
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

async function collectWorkspacePaths(rootPath: string): Promise<string[]> {
  const paths: string[] = [];
  await visit(rootPath);
  return paths.sort((a, b) => a.localeCompare(b));

  async function visit(dir: string): Promise<void> {
    const entries = await readdir(dir, { withFileTypes: true });
    entries.sort((a, b) => Number(b.isDirectory()) - Number(a.isDirectory()) || a.name.localeCompare(b.name));
    for (const entry of entries) {
      if (entry.name.startsWith(".") && entry.name !== ".env" && entry.name !== ".gitignore") continue;
      if (entry.isDirectory() && ignoredDirectoryNames.has(entry.name)) continue;
      const absolute = join(dir, entry.name);
      const rel = normalizeTreePath(relative(rootPath, absolute));
      if (entry.isDirectory()) {
        paths.push(`${rel}/`);
        await visit(absolute);
      } else if (entry.isFile()) {
        paths.push(rel);
      }
    }
  }
}

function normalizeTreePath(path: string): string {
  return sep === "/" ? path : path.split(sep).join("/");
}

function activeSurfacePath(editor: Parameters<EditorModule>[0]): string | undefined {
  const pane = editor.workbench.activePane();
  return pane?.kind === "surface" ? pane.path : undefined;
}

export { fileTreeToolTypeId };
