import { existsSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { FileSearchResult } from "@the-editor/contracts/runtime";
import {
  commandId,
  pickerRoleId,
  roleImplementationId,
  type EditorModule,
  type PickerItem,
  type PickerPreview,
  type PickerSession,
  type PickerStatusLine,
} from "@the-editor/editor";
import { getDefaultState } from "./state.js";

interface PickerRoleState {
  visible: boolean;
  providerId?: string;
  title?: string;
  placeholder?: string;
  query: string;
  selectedIndex: number;
  items: PickerItem[];
  status?: PickerStatusLine;
  emptyText?: string;
  preview?: PickerPreview | null;
}

interface ActivePickerState {
  visible: true;
  providerId: string;
  providerModuleId: string;
  title: string;
  placeholder: string;
  query: string;
  selectedIndex: number;
  items: PickerItem[];
  status?: PickerStatusLine;
  emptyText?: string;
  preview?: PickerPreview | null;
  requestVersion: number;
  refreshTimer?: ReturnType<typeof setTimeout>;
  previewTimer?: ReturnType<typeof setTimeout>;
  session: PickerSession;
}

interface StoredPickerState {
  active?: ActivePickerState;
}

const pickerImplementationId = roleImplementationId("default.picker");
const pickerStateSlot = "default.picker.state";
const FILE_PICKER_PROVIDER_ID = "default.files";
const FILE_PICKER_LIMIT = 200;

export function defaultPicker(): EditorModule {
  return (editor) => {
    editor.roles.setFallback(pickerRoleId, pickerImplementationId);

    editor.roles.register<PickerRoleState>({
      role: pickerRoleId,
      implementationId: pickerImplementationId,
      title: "Default Picker",
      async close() {
        closePicker(editor, true);
      },
      async focus() {
        editor.focus.setRole(pickerRoleId);
      },
      snapshot() {
        const active = activePicker(editor);
        return {
          visible: Boolean(active),
          providerId: active?.providerId,
          title: active?.title,
          placeholder: active?.placeholder,
          query: active?.query ?? "",
          selectedIndex: active?.selectedIndex ?? 0,
          items: active?.items ?? [],
          status: active?.status,
          emptyText: active?.emptyText,
          preview: active?.preview,
        } satisfies PickerRoleState;
      },
    });

    editor.commands.register<{
      providerId: string;
      query?: string;
      title?: string;
      placeholder?: string;
      sessionArgs?: unknown;
    }, void>({
      id: commandId("picker.open"),
      title: "Open Picker",
      hidden: true,
      async run(_ctx, args) {
        await openPicker(editor, args.providerId, {
          query: args.query,
          title: args.title,
          placeholder: args.placeholder,
          sessionArgs: args.sessionArgs,
        });
      },
    });

    editor.commands.register<void, void>({
      id: commandId("picker.close"),
      title: "Close Picker",
      hidden: true,
      run() {
        closePicker(editor, true);
      },
    });

    editor.commands.register<{ query: string }, void>({
      id: commandId("picker.setQuery"),
      title: "Set Picker Query",
      hidden: true,
      async run(_ctx, args) {
        const active = activePicker(editor);
        if (!active) return;
        active.query = args.query;
        active.selectedIndex = 0;
        await refreshPicker(editor, active);
      },
    });

    editor.commands.register<{ delta: number }, void>({
      id: commandId("picker.moveSelection"),
      title: "Move Picker Selection",
      hidden: true,
      async run(_ctx, args) {
        const active = activePicker(editor);
        if (!active?.items.length) return;
        active.selectedIndex = clampIndex(active.selectedIndex + args.delta, active.items.length);
        active.preview = null;
        editor.workbench.invalidate("roles");
        schedulePickerPreview(editor, active);
      },
    });

    editor.commands.register<{ index: number }, void>({
      id: commandId("picker.selectIndex"),
      title: "Select Picker Item",
      hidden: true,
      async run(_ctx, args) {
        const active = activePicker(editor);
        if (!active?.items.length) return;
        active.selectedIndex = clampIndex(args.index, active.items.length);
        active.preview = null;
        editor.workbench.invalidate("roles");
        schedulePickerPreview(editor, active);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("picker.accept"),
      title: "Accept Picker Selection",
      hidden: true,
      async run() {
        const active = activePicker(editor);
        const item = active?.items[active.selectedIndex];
        if (!active || !item) return;
        pickerState(editor).active = undefined;
        await active.session.accept(item);
        if (active.refreshTimer) clearTimeout(active.refreshTimer);
        if (active.previewTimer) clearTimeout(active.previewTimer);
        active.session.dispose?.();
      },
    });

    editor.commands.register<void, void>({
      id: commandId("file_picker"),
      title: "Open File Picker",
      category: "Files",
      keywords: ["file", "picker", "open", "fff"],
      async run() {
        await openPicker(editor, FILE_PICKER_PROVIDER_ID);
      },
    });

    editor.keymaps.register({
      id: "default.picker.scope",
      scope: "role.picker",
      tree: {
        name: "Picker",
        bindings: {
          esc: "picker.close",
          ret: "picker.accept",
          up: { id: commandId("picker.moveSelection"), args: { delta: -1 } },
          down: { id: commandId("picker.moveSelection"), args: { delta: 1 } },
          "C-p": { id: commandId("picker.moveSelection"), args: { delta: -1 } },
          "C-n": { id: commandId("picker.moveSelection"), args: { delta: 1 } },
        },
      },
    });

    editor.pickers.registerProvider({
      id: FILE_PICKER_PROVIDER_ID,
      title: "Files",
      placeholder: "Find files",
      async createSession() {
        const rootPath = resolveFilePickerRoot(editor);
        const { indexId } = await editor.runtime.search.ensureIndex({ rootPath });
        let latestQuery = "";

        return {
          async search(query) {
            latestQuery = query;
            const result = await editor.runtime.search.queryFiles({
              indexId,
              query,
              limit: FILE_PICKER_LIMIT,
              currentPath: getDefaultState(editor).activePath,
            });
            return filePickerPage(result);
          },
          async preview(item) {
            const path = filePickerPath(item);
            if (!path) return null;
            const preview = await editor.runtime.search.readTextPreview({
              path,
              maxBytes: 12_000,
              maxLines: 96,
            });
            return {
              kind: "text",
              title: item.title,
              detail: preview.path,
              text: preview.binary ? "" : preview.text,
              startLine: preview.startLine,
              endLine: preview.endLine,
              truncated: preview.truncated,
              binary: preview.binary,
              rows: preview.rows?.map((row) => ({
                text: row.text,
                lineNumber: row.lineNumber,
              })),
              syntax: preview.syntax,
            } satisfies PickerPreview;
          },
          async accept(item) {
            const path = filePickerPath(item);
            if (!path) return;
            await editor.runtime.search.recordSelection({
              indexId,
              query: latestQuery,
              path,
            });
            await editor.commands.dispatch({
              id: commandId("file.open"),
              args: { path },
            });
          },
        } satisfies PickerSession;
      },
    });
  };
}

function pickerState(editor: Parameters<EditorModule>[0]): StoredPickerState {
  return editor.stateSlot(pickerStateSlot, () => ({} satisfies StoredPickerState));
}

function activePicker(editor: Parameters<EditorModule>[0]): ActivePickerState | undefined {
  const active = pickerState(editor).active;
  if (!active) return undefined;
  const provider = editor.pickers.entry(active.providerId);
  if (provider && provider.moduleId === active.providerModuleId) return active;
  disposePicker(editor, false);
  return undefined;
}

async function openPicker(
  editor: Parameters<EditorModule>[0],
  providerId: string,
  options: {
    query?: string;
    title?: string;
    placeholder?: string;
    sessionArgs?: unknown;
  } = {},
): Promise<void> {
  disposePicker(editor, false);
  const provider = editor.pickers.entry(providerId);
  if (!provider) throw new Error(`unknown picker provider ${providerId}`);
  const session = await provider.createSession({ editor }, options.sessionArgs);
  const active: ActivePickerState = {
    visible: true,
    providerId,
    providerModuleId: provider.moduleId,
    title: options.title ?? provider.title ?? providerId,
    placeholder: options.placeholder ?? provider.placeholder ?? "Type to search",
    query: options.query ?? "",
    selectedIndex: 0,
    items: [],
    status: undefined,
    emptyText: undefined,
    preview: null,
    requestVersion: 0,
    session,
  };
  pickerState(editor).active = active;
  editor.focus.setRole(pickerRoleId);
  await refreshPicker(editor, active);
}

function closePicker(editor: Parameters<EditorModule>[0], restoreFocus: boolean): void {
  disposePicker(editor, restoreFocus);
}

function disposePicker(editor: Parameters<EditorModule>[0], restoreFocus: boolean): void {
  const active = pickerState(editor).active;
  pickerState(editor).active = undefined;
  if (active?.refreshTimer) clearTimeout(active.refreshTimer);
  if (active?.previewTimer) clearTimeout(active.previewTimer);
  active?.session.dispose?.();
  if (restoreFocus) editor.focus.restore();
}

async function refreshPicker(editor: Parameters<EditorModule>[0], active: ActivePickerState): Promise<void> {
  const requestVersion = ++active.requestVersion;
  const page = await active.session.search(active.query, { offset: 0, limit: FILE_PICKER_LIMIT });
  if (!isCurrentPicker(editor, active, requestVersion)) return;
  active.items = page.items;
  active.status = page.status;
  active.emptyText = page.emptyText;
  active.selectedIndex = clampIndex(active.selectedIndex, active.items.length);
  active.preview = null;
  if (active.refreshTimer) clearTimeout(active.refreshTimer);
  if (active.previewTimer) clearTimeout(active.previewTimer);
  active.refreshTimer = page.refreshAfterMs && page.refreshAfterMs > 0
    ? setTimeout(() => {
        void refreshPicker(editor, active);
      }, page.refreshAfterMs)
    : undefined;
  editor.workbench.invalidate("roles");
  schedulePickerPreview(editor, active, requestVersion);
}

function schedulePickerPreview(
  editor: Parameters<EditorModule>[0],
  active: ActivePickerState,
  requestVersion = active.requestVersion,
): void {
  if (active.previewTimer) clearTimeout(active.previewTimer);
  active.previewTimer = setTimeout(() => {
    active.previewTimer = undefined;
    void refreshPickerPreview(editor, active, requestVersion);
  }, 75);
}

async function refreshPickerPreview(
  editor: Parameters<EditorModule>[0],
  active: ActivePickerState,
  requestVersion = active.requestVersion,
): Promise<void> {
  const item = active.items[active.selectedIndex];
  if (!item || !active.session.preview) {
    active.preview = null;
    editor.workbench.invalidate("roles");
    return;
  }
  const preview = await active.session.preview(item);
  if (!isCurrentPicker(editor, active, requestVersion)) return;
  active.preview = preview;
  editor.workbench.invalidate("roles");
}

function isCurrentPicker(
  editor: Parameters<EditorModule>[0],
  active: ActivePickerState,
  requestVersion: number,
): boolean {
  return pickerState(editor).active === active && active.requestVersion === requestVersion;
}

function clampIndex(index: number, length: number): number {
  if (!length) return 0;
  return Math.max(0, Math.min(length - 1, index));
}

function filePickerPage(result: FileSearchResult) {
  return {
    items: result.items.map((item) => ({
      id: item.path,
      title: item.fileName,
      subtitle: item.parentPath,
      detail: item.relativePath,
      keywords: [item.relativePath, item.fileName, item.parentPath],
      payload: { path: item.path },
    } satisfies PickerItem)),
    totalCount: result.totalMatched,
    status: {
      primary: result.rootPath,
      secondary: `${result.totalMatched} matches${result.scanning ? " · indexing…" : ""}`,
    },
    emptyText: result.scanning ? "Indexing files…" : "No matching files",
    refreshAfterMs: result.scanning ? 150 : undefined,
  };
}

function filePickerPath(item: PickerItem): string | undefined {
  const payload = item.payload;
  if (!payload || typeof payload !== "object") return undefined;
  const path = (payload as { path?: unknown }).path;
  return typeof path === "string" ? path : undefined;
}

function resolveFilePickerRoot(editor: Parameters<EditorModule>[0]): string {
  const activePath = getDefaultState(editor).activePath;
  if (!activePath) {
    const cwd = resolve(process.cwd());
    const start = existingDirectory(cwd) ?? cwd;
    return findProjectRoot(start) ?? start;
  }
  const absolute = resolve(activePath);
  const start = existingDirectory(absolute) ?? dirname(absolute);
  return findProjectRoot(start) ?? start;
}

function existingDirectory(path: string): string | undefined {
  try {
    if (!existsSync(path)) return undefined;
    return statSync(path).isDirectory() ? path : dirname(path);
  } catch {
    return undefined;
  }
}

function findProjectRoot(start: string): string | undefined {
  let current = start;
  let found: string | undefined;
  while (true) {
    if (hasProjectMarker(current)) found = current;
    const parent = dirname(current);
    if (parent === current) return found;
    current = parent;
  }
}

function hasProjectMarker(dir: string): boolean {
  return [".git", ".the-editor", "package.json", "Cargo.toml", "pnpm-workspace.yaml", "bun.lock", "bun.lockb"]
    .some((name) => existsSync(join(dir, name)));
}
