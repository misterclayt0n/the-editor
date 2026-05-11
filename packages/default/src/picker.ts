import { existsSync, statSync } from "node:fs";
import { resolve } from "node:path";
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
let pickerTraceSequence = 0;

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
        pickerTrace(editor, "provider.createSession.begin", { providerId: FILE_PICKER_PROVIDER_ID, rootPath });
        const { indexId } = await editor.runtime.search.ensureIndex({ rootPath });
        pickerTrace(editor, "provider.createSession.ready", { providerId: FILE_PICKER_PROVIDER_ID, rootPath, indexId });
        let latestQuery = "";

        return {
          async search(query) {
            latestQuery = query;
            pickerTrace(editor, "provider.search.begin", { indexId, query, currentPath: getDefaultState(editor).activePath });
            const result = await editor.runtime.search.queryFiles({
              indexId,
              query,
              limit: FILE_PICKER_LIMIT,
              currentPath: getDefaultState(editor).activePath,
            });
            pickerTrace(editor, "provider.search.result", {
              indexId,
              query,
              matched: result.totalMatched,
              itemCount: result.items.length,
              scanning: result.scanning,
              rootPath: result.rootPath,
            });
            return filePickerPage(result);
          },
          async preview(item) {
            const path = filePickerPath(item);
            pickerTrace(editor, "provider.preview.begin", { indexId, itemId: item.id, path });
            if (!path) return null;
            const preview = await editor.runtime.search.readTextPreview({
              path,
              maxBytes: 12_000,
              maxLines: 96,
            });
            pickerTrace(editor, "provider.preview.result", {
              indexId,
              path,
              binary: preview.binary,
              truncated: preview.truncated,
              startLine: preview.startLine,
              endLine: preview.endLine,
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
            pickerTrace(editor, "provider.accept.begin", { indexId, itemId: item.id, path, latestQuery });
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
            pickerTrace(editor, "provider.accept.done", { indexId, path, latestQuery });
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
  const session = editor.sessions.snapshot(active.sessionId);
  if (provider && provider.moduleId === active.providerModuleId && session) return active;
  pickerTrace(editor, "activePicker.invalid", {
    activeSessionId: active.sessionId,
    providerId: active.providerId,
    providerModuleId: active.providerModuleId,
    providerPresent: Boolean(provider),
    providerModuleMatches: provider?.moduleId === active.providerModuleId,
    sessionPresent: Boolean(session),
  });
  disposePicker(editor, false, "activePicker.invalid");
  return undefined;
}

async function dispatchPickerSession(editor: Parameters<EditorModule>[0], method: string, args?: unknown): Promise<void> {
  const active = activePicker(editor);
  pickerTrace(editor, "session.dispatch", {
    method,
    args,
    activeSessionId: active?.sessionId,
  });
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
  pickerTrace(editor, "open.begin", { providerId, options });
  disposePicker(editor, false, "openPicker.begin");
  const provider = editor.pickers.entry(providerId);
  if (!provider) throw new Error(`unknown picker provider ${providerId}`);
  pickerTrace(editor, "open.provider", {
    providerId,
    providerModuleId: provider.moduleId,
    providerTitle: provider.title,
  });
  const providerSession = await provider.createSession({ editor }, options.sessionArgs);
  pickerTrace(editor, "open.providerSession.created", { providerId });
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
        pickerTrace(editor, "session.setQuery", {
          sessionId: session.id,
          previousQuery: session.snapshot().state.query,
          query,
        });
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
        pickerTrace(editor, "session.moveSelection", {
          sessionId: session.id,
          delta,
          selectedIndexBefore: session.snapshot().state.selectedIndex,
          itemCount: session.snapshot().state.items.length,
        });
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
        pickerTrace(editor, "session.selectIndex", {
          sessionId: session.id,
          index,
          itemCount: session.snapshot().state.items.length,
        });
        session.updateState((state) => {
          if (!state.items.length) return;
          state.selectedIndex = clampIndex(index, state.items.length);
          state.preview = null;
        });
        schedulePickerPreview(editor, session, handle);
      },
      accept: async ({ session }) => {
        const snapshot = session.snapshot().state;
        const item = snapshot.items[snapshot.selectedIndex];
        pickerTrace(editor, "session.accept", {
          sessionId: session.id,
          selectedIndex: snapshot.selectedIndex,
          itemId: item?.id,
          itemDetail: item?.detail,
        });
        if (!item) return;
        closePicker(editor, true, "session.accept");
        await providerSession.accept(item);
      },
      close: () => {
        pickerTrace(editor, "session.close", { sessionId: session.id });
        closePicker(editor, true, "session.close");
      },
    },
    dispose: () => {
      pickerTrace(editor, "session.dispose", {
        sessionId: handle.sessionId,
        requestVersion: handle.requestVersion,
        hadRefreshTimer: Boolean(handle.refreshTimer),
        hadPreviewTimer: Boolean(handle.previewTimer),
      });
      if (handle.refreshTimer) clearTimeout(handle.refreshTimer);
      if (handle.previewTimer) clearTimeout(handle.previewTimer);
      providerSession.dispose?.();
    },
  });

  handle.sessionId = session.id;
  pickerState(editor).active = handle;
  pickerTrace(editor, "open.activated", {
    sessionId: handle.sessionId,
    providerId,
    providerModuleId: handle.providerModuleId,
    query: options.query ?? "",
    cwd: editor.cwd.current(),
    focusBefore: editor.focus.current(),
  });
  editor.focus.setRole(pickerRoleId);
  editor.workbench.invalidate("roles", "focus");
  await refreshPicker(editor, session, handle);
}

function closePicker(editor: Parameters<EditorModule>[0], restoreFocus: boolean, reason = "closePicker"): void {
  disposePicker(editor, restoreFocus, reason);
}

function disposePicker(editor: Parameters<EditorModule>[0], restoreFocus: boolean, reason = "disposePicker"): void {
  const active = pickerState(editor).active;
  pickerTrace(editor, "dispose", {
    reason,
    restoreFocus,
    activeSessionId: active?.sessionId,
    requestVersion: active?.requestVersion,
    focusBefore: editor.focus.current(),
  });
  pickerState(editor).active = undefined;
  if (active?.refreshTimer) clearTimeout(active.refreshTimer);
  if (active?.previewTimer) clearTimeout(active.previewTimer);
  if (active) editor.sessions.dispose(active.sessionId);
  if (restoreFocus) editor.focus.restore();
  editor.workbench.invalidate("roles", "focus");
  pickerTrace(editor, "dispose.done", {
    reason,
    restoreFocus,
    focusAfter: editor.focus.current(),
  });
}

async function refreshPicker(
  editor: Parameters<EditorModule>[0],
  session: InteractiveSessionController<PickerSessionState>,
  handle: ActivePickerHandle,
): Promise<void> {
  const snapshotBefore = session.snapshot().state;
  const requestVersion = ++handle.requestVersion;
  pickerTrace(editor, "refresh.begin", {
    sessionId: session.id,
    requestVersion,
    query: snapshotBefore.query,
    selectedIndex: snapshotBefore.selectedIndex,
    itemCount: snapshotBefore.items.length,
  });
  const page = await handle.providerSession.search(snapshotBefore.query, { offset: 0, limit: FILE_PICKER_LIMIT });
  pickerTrace(editor, "refresh.result", {
    sessionId: session.id,
    requestVersion,
    query: snapshotBefore.query,
    resultCount: page.items.length,
    refreshAfterMs: page.refreshAfterMs,
    status: page.status,
    emptyText: page.emptyText,
  });
  if (!isCurrentPicker(editor, handle, requestVersion)) return;
  session.updateState((state) => {
    state.items = page.items;
    state.status = page.status;
    state.emptyText = page.emptyText;
    state.selectedIndex = clampIndex(state.selectedIndex, state.items.length);
    state.preview = null;
  });
  const snapshotAfter = session.snapshot().state;
  pickerTrace(editor, "refresh.applied", {
    sessionId: session.id,
    requestVersion,
    selectedIndex: snapshotAfter.selectedIndex,
    itemCount: snapshotAfter.items.length,
    firstItem: snapshotAfter.items[0]?.detail ?? snapshotAfter.items[0]?.title,
  });
  if (handle.refreshTimer) clearTimeout(handle.refreshTimer);
  if (handle.previewTimer) clearTimeout(handle.previewTimer);
  handle.refreshTimer = page.refreshAfterMs && page.refreshAfterMs > 0
    ? setTimeout(() => {
        pickerTrace(editor, "refresh.timer.fire", {
          sessionId: session.id,
          requestVersion: handle.requestVersion,
        });
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
  const snapshot = session.snapshot().state;
  pickerTrace(editor, "preview.schedule", {
    sessionId: session.id,
    requestVersion,
    selectedIndex: snapshot.selectedIndex,
    itemCount: snapshot.items.length,
    item: snapshot.items[snapshot.selectedIndex]?.detail ?? snapshot.items[snapshot.selectedIndex]?.title,
  });
  if (handle.previewTimer) clearTimeout(handle.previewTimer);
  handle.previewTimer = setTimeout(() => {
    handle.previewTimer = undefined;
    pickerTrace(editor, "preview.timer.fire", {
      sessionId: session.id,
      requestVersion,
    });
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
  pickerTrace(editor, "preview.begin", {
    sessionId: session.id,
    requestVersion,
    selectedIndex: state.selectedIndex,
    itemCount: state.items.length,
    item: item?.detail ?? item?.title,
    hasPreviewProvider: Boolean(handle.providerSession.preview),
  });
  if (!item || !handle.providerSession.preview) {
    session.updateState((next) => {
      next.preview = null;
    });
    pickerTrace(editor, "preview.skipped", {
      sessionId: session.id,
      requestVersion,
      reason: !item ? "no-item" : "no-preview-provider",
    });
    return;
  }

  try {
    const preview = await handle.providerSession.preview(item);
    if (!isCurrentPicker(editor, handle, requestVersion)) return;
    session.updateState((next) => {
      next.preview = preview;
    });
    pickerTrace(editor, "preview.applied", {
      sessionId: session.id,
      requestVersion,
      previewKind: preview?.kind,
      previewDetail: preview?.detail,
      binary: preview?.kind === "text" ? preview.binary : undefined,
    });
  } catch (error) {
    if (!isCurrentPicker(editor, handle, requestVersion)) return;
    const detail = error instanceof Error ? error.message : String(error);
    session.updateState((next) => {
      next.preview = {
        kind: "empty",
        title: item.title,
        detail,
      };
    });
    pickerTrace(editor, "preview.error", {
      sessionId: session.id,
      requestVersion,
      item: item.detail ?? item.title,
      detail,
    });
  }
}

function isCurrentPicker(
  editor: Parameters<EditorModule>[0],
  handle: ActivePickerHandle,
  requestVersion: number,
): boolean {
  const active = pickerState(editor).active;
  const ok = active === handle && handle.requestVersion === requestVersion;
  if (!ok) {
    pickerTrace(editor, "request.stale", {
      requestVersion,
      handleRequestVersion: handle.requestVersion,
      activeSessionId: active?.sessionId,
      handleSessionId: handle.sessionId,
      sameHandle: active === handle,
    });
  }
  return ok;
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
  const cwd = resolve(editor.cwd.current());
  return existingDirectory(cwd) ?? cwd;
}

function existingDirectory(path: string): string | undefined {
  try {
    if (!existsSync(path)) return undefined;
    return statSync(path).isDirectory() ? path : undefined;
  } catch {
    return undefined;
  }
}

function pickerTrace(editor: Parameters<EditorModule>[0], event: string, details: Record<string, unknown> = {}): void {
  const active = pickerState(editor).active;
  const activeSession = active ? editor.sessions.snapshot<PickerSessionState>(active.sessionId)?.state : undefined;
  const payload = {
    seq: ++pickerTraceSequence,
    event,
    cwd: editor.cwd.current(),
    activeSessionId: active?.sessionId,
    activeProviderId: active?.providerId,
    activeRequestVersion: active?.requestVersion,
    focus: editor.focus.current(),
    query: activeSession?.query,
    selectedIndex: activeSession?.selectedIndex,
    itemCount: activeSession?.items.length,
    visible: Boolean(active),
    ...details,
  };
  console.log(`[picker-trace] ${JSON.stringify(payload)}`);
}
