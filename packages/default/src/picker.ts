import { existsSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { FileSearchResult } from "@the-editor/contracts/runtime";
import {
  commandId,
  pickerRoleId,
  roleImplementationId,
  type EditorModule,
  type InteractiveSessionController,
  type InteractiveSessionId,
  type PickerItem,
  type PickerPreview,
  type PickerSession,
  type PickerStatusLine,
} from "@the-editor/editor";
import { getDefaultState } from "./state.js";

interface PickerRoleState {
  visible: boolean;
  sessionId?: string;
}

interface PickerSessionState {
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

interface ActivePickerHandle {
  sessionId: InteractiveSessionId;
  providerId: string;
  providerModuleId: string;
  requestVersion: number;
  refreshTimer?: ReturnType<typeof setTimeout>;
  previewTimer?: ReturnType<typeof setTimeout>;
  providerSession: PickerSession;
}

interface StoredPickerState {
  active?: ActivePickerHandle;
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
        const session = active ? editor.sessions.snapshot<PickerSessionState>(active.sessionId) : undefined;
        return {
          visible: Boolean(active),
          sessionId: active?.sessionId,
          providerId: session?.state.providerId,
          title: session?.state.title,
          placeholder: session?.state.placeholder,
          query: session?.state.query ?? "",
          selectedIndex: session?.state.selectedIndex ?? 0,
          items: session?.state.items ?? [],
          status: session?.state.status,
          emptyText: session?.state.emptyText,
          preview: session?.state.preview,
        } as PickerRoleState & PickerSessionState;
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
        await dispatchPickerSession(editor, "setQuery", args);
      },
    });

    editor.commands.register<{ delta: number }, void>({
      id: commandId("picker.moveSelection"),
      title: "Move Picker Selection",
      hidden: true,
      async run(_ctx, args) {
        await dispatchPickerSession(editor, "moveSelection", args);
      },
    });

    editor.commands.register<{ index: number }, void>({
      id: commandId("picker.selectIndex"),
      title: "Select Picker Item",
      hidden: true,
      async run(_ctx, args) {
        await dispatchPickerSession(editor, "selectIndex", args);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("picker.accept"),
      title: "Accept Picker Selection",
      hidden: true,
      async run() {
        await dispatchPickerSession(editor, "accept");
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

function activePicker(editor: Parameters<EditorModule>[0]): ActivePickerHandle | undefined {
  const active = pickerState(editor).active;
  if (!active) return undefined;
  const provider = editor.pickers.entry(active.providerId);
  if (provider && provider.moduleId === active.providerModuleId && editor.sessions.snapshot(active.sessionId)) return active;
  disposePicker(editor, false);
  return undefined;
}

async function dispatchPickerSession(editor: Parameters<EditorModule>[0], method: string, args?: unknown): Promise<void> {
  const active = activePicker(editor);
  if (!active) return;
  await editor.sessions.dispatch(active.sessionId, method, args);
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
  const providerSession = await provider.createSession({ editor }, options.sessionArgs);
  const handle: ActivePickerHandle = {
    sessionId: "" as InteractiveSessionId,
    providerId,
    providerModuleId: provider.moduleId,
    requestVersion: 0,
    providerSession,
  };

  const session = editor.sessions.create<PickerSessionState>({
    kind: "picker",
    role: pickerRoleId,
    title: options.title ?? provider.title ?? providerId,
    moduleId: provider.moduleId,
    state: {
      providerId,
      title: options.title ?? provider.title ?? providerId,
      placeholder: options.placeholder ?? provider.placeholder ?? "Type to search",
      query: options.query ?? "",
      selectedIndex: 0,
      items: [],
      status: undefined,
      emptyText: undefined,
      preview: null,
    },
    methods: {
      setQuery: async ({ session }, args) => {
        const query = typeof (args as { query?: unknown } | undefined)?.query === "string"
          ? (args as { query: string }).query
          : "";
        session.updateState((state) => {
          state.query = query;
          state.selectedIndex = 0;
        });
        await refreshPicker(editor, session, handle);
      },
      moveSelection: ({ session }, args) => {
        const delta = typeof (args as { delta?: unknown } | undefined)?.delta === "number"
          ? (args as { delta: number }).delta
          : 0;
        session.updateState((state) => {
          if (!state.items.length) return;
          state.selectedIndex = clampIndex(state.selectedIndex + delta, state.items.length);
          state.preview = null;
        });
        schedulePickerPreview(editor, session, handle);
      },
      selectIndex: ({ session }, args) => {
        const index = typeof (args as { index?: unknown } | undefined)?.index === "number"
          ? (args as { index: number }).index
          : 0;
        session.updateState((state) => {
          if (!state.items.length) return;
          state.selectedIndex = clampIndex(index, state.items.length);
          state.preview = null;
        });
        schedulePickerPreview(editor, session, handle);
      },
      accept: async ({ session }) => {
        const item = session.snapshot().state.items[session.snapshot().state.selectedIndex];
        if (!item) return;
        closePicker(editor, true);
        await providerSession.accept(item);
      },
      close: () => {
        closePicker(editor, true);
      },
    },
    dispose: () => {
      if (handle.refreshTimer) clearTimeout(handle.refreshTimer);
      if (handle.previewTimer) clearTimeout(handle.previewTimer);
      providerSession.dispose?.();
    },
  });

  handle.sessionId = session.id;
  pickerState(editor).active = handle;
  editor.focus.setRole(pickerRoleId);
  editor.workbench.invalidate("roles", "focus");
  await refreshPicker(editor, session, handle);
}

function closePicker(editor: Parameters<EditorModule>[0], restoreFocus: boolean): void {
  disposePicker(editor, restoreFocus);
}

function disposePicker(editor: Parameters<EditorModule>[0], restoreFocus: boolean): void {
  const active = pickerState(editor).active;
  pickerState(editor).active = undefined;
  if (active?.refreshTimer) clearTimeout(active.refreshTimer);
  if (active?.previewTimer) clearTimeout(active.previewTimer);
  if (active) editor.sessions.dispose(active.sessionId);
  if (restoreFocus) editor.focus.restore();
  editor.workbench.invalidate("roles", "focus");
}

async function refreshPicker(
  editor: Parameters<EditorModule>[0],
  session: InteractiveSessionController<PickerSessionState>,
  handle: ActivePickerHandle,
): Promise<void> {
  const requestVersion = ++handle.requestVersion;
  const page = await handle.providerSession.search(session.snapshot().state.query, { offset: 0, limit: FILE_PICKER_LIMIT });
  if (!isCurrentPicker(editor, handle, requestVersion)) return;
  session.updateState((state) => {
    state.items = page.items;
    state.status = page.status;
    state.emptyText = page.emptyText;
    state.selectedIndex = clampIndex(state.selectedIndex, state.items.length);
    state.preview = null;
  });
  if (handle.refreshTimer) clearTimeout(handle.refreshTimer);
  if (handle.previewTimer) clearTimeout(handle.previewTimer);
  handle.refreshTimer = page.refreshAfterMs && page.refreshAfterMs > 0
    ? setTimeout(() => {
        void refreshPicker(editor, session, handle);
      }, page.refreshAfterMs)
    : undefined;
  schedulePickerPreview(editor, session, handle, requestVersion);
}

function schedulePickerPreview(
  editor: Parameters<EditorModule>[0],
  session: InteractiveSessionController<PickerSessionState>,
  handle: ActivePickerHandle,
  requestVersion = handle.requestVersion,
): void {
  if (handle.previewTimer) clearTimeout(handle.previewTimer);
  handle.previewTimer = setTimeout(() => {
    handle.previewTimer = undefined;
    void refreshPickerPreview(editor, session, handle, requestVersion);
  }, 75);
}

async function refreshPickerPreview(
  editor: Parameters<EditorModule>[0],
  session: InteractiveSessionController<PickerSessionState>,
  handle: ActivePickerHandle,
  requestVersion = handle.requestVersion,
): Promise<void> {
  const state = session.snapshot().state;
  const item = state.items[state.selectedIndex];
  if (!item || !handle.providerSession.preview) {
    session.updateState((next) => {
      next.preview = null;
    });
    return;
  }
  const preview = await handle.providerSession.preview(item);
  if (!isCurrentPicker(editor, handle, requestVersion)) return;
  session.updateState((next) => {
    next.preview = preview;
  });
}

function isCurrentPicker(
  editor: Parameters<EditorModule>[0],
  handle: ActivePickerHandle,
  requestVersion: number,
): boolean {
  const active = pickerState(editor).active;
  return active === handle && handle.requestVersion === requestVersion;
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
