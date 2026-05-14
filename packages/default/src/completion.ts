import type { SurfaceId } from "@the-editor/contracts/runtime";
import {
  commandId,
  completionMenuRoleId,
  roleImplementationId,
  type CompletionContext,
  type CompletionDisplayOptions,
  type CompletionItem,
  type CompletionResult,
  type CompletionSource,
  type EditorHost,
  type EditorModule,
  type InteractiveSessionController,
  defineHostStore,
  perfTraceLog, } from "@the-editor/editor";
import { activeSurfacePane, getDefaultState, refreshActiveSurface } from "./state.js";
import { currentLspStatuses, lspLanguageIdForPath } from "./lsp.js";
import { perfLog, perfSpan } from "./perfDiagnostics.js";
import { updateSurfaceSnapshot } from "./surfaceSnapshot.js";
import { isCompletionQueryCharacter, readCompletionContext, type RuntimeCompletionContext } from "./completionQuery.js";
import { highlightCompletionText, rankCompletionEntries } from "./completionRanker.js";
import { createLspCompletionSource, materializeLazyLspCompletionItem } from "./lspCompletionAdapter.js";

interface CompletionRoleState {
  visible: boolean;
  sessionId?: string;
  surfaceId?: number;
}

export interface CompletionSessionItemView {
  id: string;
  label: string;
  labelHighlights: number[];
  detail?: string;
  detailHighlights: number[];
  labelDetail?: string;
  labelDescription?: string;
  kind?: string;
  badge?: string;
  source?: string;
  deprecated?: boolean;
}

export interface CompletionSessionState {
  query: string;
  selectedIndex: number;
  items: CompletionSessionItemView[];
  loading: boolean;
  emptyText?: string;
  documentationMarkdown?: string;
  documentationLoading?: boolean;
  dynamicWidth?: boolean;
  showDocumentation?: boolean;
}

interface ActiveCompletionEntry {
  sourceId: string;
  sourceTitle?: string;
  sourcePriority: number;
  item: CompletionItem;
  view: CompletionSessionItemView;
  matchScore: number;
  providerIncomplete: boolean;
  providerPriority: number;
}

interface ActiveCompletionState {
  session: InteractiveSessionController<CompletionSessionState>;
  surfaceId: SurfaceId;
  path?: string;
  languageId?: string;
  query: string;
  replaceStart: number;
  replaceEnd: number;
  requestSequence: number;
  requestRevision: number;
  allItems: ActiveCompletionEntry[];
  items: ActiveCompletionEntry[];
  selectedIndex: number;
  loading: boolean;
  isIncomplete: boolean;
  providerContexts: Map<string, { isIncomplete: boolean; priority: number }>;
  display: CompletionDisplayOptions;
  resolvingItemId?: string;
}

interface CompletionControllerState {
  active?: ActiveCompletionState;
  timer?: ReturnType<typeof setTimeout>;
  requeryTimer?: ReturnType<typeof setTimeout>;
  automaticWord?: {
    surfaceId: SurfaceId;
    path?: string;
    query: string;
  };
}

const completionImplementationId = roleImplementationId("default.completionMenu");
const completionControllerSlot = "default.completion.controller";
const maxCompletionItems = 400;
const completionPreservingCommands = new Set([
  commandId("lsp_code_actions"),
  commandId("code_action"),
]);

const insertMutationCommands = new Set([
  commandId("edit.insertText"),
  commandId("delete_char_backward"),
  commandId("delete_char_forward"),
  commandId("delete_word_backward"),
  commandId("delete_word_forward"),
  commandId("kill_to_line_start"),
  commandId("kill_to_line_end"),
]);

function elapsed(start: number): number {
  return Math.round((performance.now() - start) * 10) / 10;
}

function completionDiag(phase: string, data: Record<string, unknown>): void {
  perfLog("completion", phase, data);
}

export function defaultCompletion(): EditorModule {
  return (editor) => {
    editor.roles.setFallback(completionMenuRoleId, completionImplementationId);

    editor.roles.register<CompletionRoleState>({
      role: completionMenuRoleId,
      implementationId: completionImplementationId,
      title: "Default Completion Menu",
      close() {
        hideCompletion(editor, { restoreFocus: false });
      },
      snapshot() {
        const active = completionController(editor).active;
        return {
          visible: Boolean(active) && editor.overlays.isOpen(completionMenuRoleId),
          sessionId: active?.session.id,
          surfaceId: active?.surfaceId,
        } satisfies CompletionRoleState;
      },
    });

    editor.commands.register<void, void>({
      id: commandId("completion.show"),
      title: "Show Completions",
      category: "Edit",
      keywords: ["completion", "completions", "autocomplete", "suggest"],
      async run() {
        await showOrRefreshCompletion(editor, { kind: "manual" });
      },
    });

    editor.commands.register<void, void>({
      id: commandId("completion.hide"),
      title: "Hide Completions",
      hidden: true,
      run() {
        hideCompletion(editor, { restoreFocus: false });
      },
    });

    editor.commands.register<{ delta?: number }, void>({
      id: commandId("completion.moveSelection"),
      title: "Move Completion Selection",
      hidden: true,
      async run(_ctx, args) {
        await moveCompletionSelection(editor, args?.delta ?? 1);
      },
    });

    editor.commands.register<{ index: number }, void>({
      id: commandId("completion.selectIndex"),
      title: "Select Completion Item",
      hidden: true,
      async run(_ctx, args) {
        await selectCompletionIndex(editor, args.index);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("completion.accept"),
      title: "Accept Completion",
      hidden: true,
      async run() {
        await acceptSelectedCompletion(editor);
      },
    });

    editor.keymaps.register({
      id: "default.completion.scope",
      scope: `role.${completionMenuRoleId}`,
      tree: {
        name: "Completion Menu",
        bindings: {
          ret: "completion.accept",
          tab: "completion.accept",
          up: { id: commandId("completion.moveSelection"), args: { delta: -1 } },
          down: { id: commandId("completion.moveSelection"), args: { delta: 1 } },
          "C-p": { id: commandId("completion.moveSelection"), args: { delta: -1 } },
          "C-n": { id: commandId("completion.moveSelection"), args: { delta: 1 } },
        },
      },
    });

    const sourceDisposables = [
      editor.completion.registerPresentation({
        id: "default.completion.badges",
        priority: 10,
        present(item) {
          return {
            detail: singleLineDetail(item.detail),
          };
        },
      }),
    ];

    const afterDispatch = editor.hooks.tap("commands.afterDispatch", {
      id: "default.completion.afterDispatch",
      handler: async ({ dispatch }: { dispatch: { id: string; args?: unknown } }) => {
        const span = perfSpan("completion", "afterDispatch", { command: dispatch.id }, 8);
        try {
        if (dispatch.id.startsWith("completion.") || completionPreservingCommands.has(dispatch.id as ReturnType<typeof commandId>)) return;
        const controller = completionController(editor);
        const active = controller.active;
        const isInsertMutation = insertMutationCommands.has(dispatch.id as ReturnType<typeof commandId>);
        const inInsertMode = getDefaultState(editor).mode === "insert" && editor.input.current().kind === "editor";

        if (active) {
          if (isInsertMutation && inInsertMode) {
            if (updateActiveCompletionFromDispatch(editor, active, dispatch)) return;
            const trigger = triggerForDispatch(dispatch);
            const queryLength = updateAutomaticWordFromDispatch(editor, dispatch, trigger) || active.query.length;
            if (shouldRefreshActiveCompletion(editor, active, trigger, queryLength)) {
              scheduleCompletion(editor, trigger);
              return;
            }
          }
          hideCompletion(editor, { restoreFocus: false });
          return;
        }

        if (isInsertMutation && inInsertMode) {
          const trigger = triggerForDispatch(dispatch);
          const queryLength = updateAutomaticWordFromDispatch(editor, dispatch, trigger);
          if (shouldScheduleAutomaticCompletionTrigger(editor, trigger, queryLength)) scheduleCompletion(editor, trigger);
          return;
        }

        controller.automaticWord = undefined;
        } finally {
          span.end({ active: Boolean(completionController(editor).active) });
        }
      },
    });

    return {
      dispose() {
        afterDispatch.dispose();
        for (const disposable of sourceDisposables.reverse()) disposable.dispose();
        hideCompletion(editor, { restoreFocus: false });
      },
    };
  };
}

function completionController(editor: EditorHost): CompletionControllerState {
  return editor.stores.host(defineHostStore({ id: completionControllerSlot, init: () => ({} satisfies CompletionControllerState) }));
}

function scheduleCompletion(editor: EditorHost, trigger: CompletionContext["trigger"], delayMs = trigger.kind === "triggerCharacter" ? 20 : 160): void {
  const controller = completionController(editor);
  if (controller.timer) clearTimeout(controller.timer);
  completionDiag("schedule", { trigger: trigger.kind, character: trigger.character, delayMs });
  controller.timer = setTimeout(() => {
    controller.timer = undefined;
    void showOrRefreshCompletion(editor, trigger);
  }, delayMs);
}

async function showOrRefreshCompletion(editor: EditorHost, trigger: CompletionContext["trigger"]): Promise<void> {
  const start = performance.now();
  perfTraceLog(editor, "completion", "show.start", { trigger: trigger.kind, character: trigger.character });
  completionDiag("show start", { trigger: trigger.kind, character: trigger.character });
  const controller = completionController(editor);
  if (controller.timer) {
    clearTimeout(controller.timer);
    controller.timer = undefined;
  }

  if (!controller.active && trigger.kind !== "manual" && !shouldScheduleAutomaticCompletionTrigger(editor, trigger, controller.automaticWord?.query.length ?? 0)) {
    completionDiag("show end", { ms: elapsed(start), reason: "no-automatic-provider" });
    return;
  }

  const contextStart = performance.now();
  const context = await readCompletionContext(editor, trigger, { allowRuntimeRead: trigger.kind === "manual", languageIdForPath: lspLanguageIdForPath });
  completionDiag("context", { ms: elapsed(contextStart), textLength: context?.text.length ?? 0, query: context?.query, trigger: trigger.kind });
  const active = controller.active;
  if (!context || !shouldOpenCompletion(context, trigger, Boolean(active))) {
    hideCompletion(editor, { restoreFocus: false });
    completionDiag("show end", { ms: elapsed(start), reason: "no-context-or-closed" });
    return;
  }

  const nextActive = active && active.surfaceId === context.surfaceId && active.path === context.path
    ? active
    : createCompletionSession(editor, context);
  if (active && active !== nextActive) {
    editor.sessions.dispose(active.session.id);
  }
  controller.active = nextActive;

  nextActive.surfaceId = context.surfaceId as SurfaceId;
  nextActive.path = context.path;
  nextActive.languageId = context.languageId;
  nextActive.query = context.query;
  nextActive.replaceStart = context.replaceStart;
  nextActive.replaceEnd = context.replaceEnd;
  const previousSelection = nextActive.items[nextActive.selectedIndex]?.item.id;
  if (nextActive.allItems.length) {
    nextActive.items = rankCompletionEntries(context.query, nextActive.allItems);
    nextActive.selectedIndex = clampIndex(nextActive.items, previousSelection, nextActive.selectedIndex);
  }
  const canFilterExistingItems = Boolean(active)
    && nextActive.allItems.length > 0
    && (trigger.kind === "input" || trigger.kind === "delete" || trigger.kind === "retrigger")
    && !nextActive.isIncomplete;

  if (canFilterExistingItems) {
    nextActive.loading = false;
    if (!nextActive.items.length && trigger.kind !== "manual") {
      hideCompletion(editor, { restoreFocus: false });
      completionDiag("show end", { ms: elapsed(start), reason: "filtered-empty" });
      return;
    }
    publishCompletionSession(nextActive, {
      loading: false,
      emptyText: nextActive.items.length ? undefined : "No completions",
    });
    completionDiag("show end", { ms: elapsed(start), reason: "filtered", items: nextActive.items.length });
    return;
  }

  nextActive.loading = nextActive.allItems.length === 0;
  nextActive.requestSequence += 1;
  nextActive.requestRevision = context.revision;
  const requestSequence = nextActive.requestSequence;
  const shouldPublishLoading = trigger.kind === "manual" || editor.overlays.isOpen(completionMenuRoleId) || nextActive.items.length > 0;

  if (shouldPublishLoading) {
    openCompletionOverlay(editor);
    publishCompletionSession(nextActive, {
      loading: nextActive.loading,
      emptyText: nextActive.items.length ? undefined : "Loading…",
    });
  }

  const requestContext: RuntimeCompletionContext = nextActive.isIncomplete && (trigger.kind === "input" || trigger.kind === "delete" || trigger.kind === "retrigger")
    ? { ...context, trigger: { kind: "incomplete" as const, character: trigger.character }, completionRequestSequence: requestSequence }
    : { ...context, completionRequestSequence: requestSequence };
  const collectStart = performance.now();
  const results = await collectCompletionItems(editor, requestContext, requestSequence);
  completionDiag("collect", { ms: elapsed(collectStart), items: results.items.length, sequence: requestSequence });
  if (completionController(editor).active !== nextActive || nextActive.requestSequence !== requestSequence) return;

  nextActive.display = results.display;
  nextActive.isIncomplete = results.isIncomplete;
  nextActive.providerContexts = results.providerContexts;
  nextActive.allItems = results.items;
  const rankStart = performance.now();
  nextActive.items = rankCompletionEntries(context.query, nextActive.allItems);
  completionDiag("rank", { ms: elapsed(rankStart), input: nextActive.allItems.length, output: nextActive.items.length, sequence: requestSequence });
  nextActive.loading = false;
  nextActive.selectedIndex = clampIndex(nextActive.items, previousSelection, nextActive.selectedIndex);

  if (!nextActive.items.length && trigger.kind !== "manual") {
    hideCompletion(editor, { restoreFocus: false });
    completionDiag("show end", { ms: elapsed(start), reason: "empty" });
    return;
  }

  openCompletionOverlay(editor);
  publishCompletionSession(nextActive, {
    emptyText: nextActive.items.length ? undefined : "No completions",
    loading: false,
  });
  completionDiag("show end", { ms: elapsed(start), reason: "published", items: nextActive.items.length });
  perfTraceLog(editor, "completion", "show.end", { trigger: trigger.kind, items: nextActive.items.length, ms: elapsed(start) });
}

function openCompletionOverlay(editor: EditorHost): void {
  if (editor.overlays.isOpen(completionMenuRoleId)) return;
  editor.overlays.open(completionMenuRoleId, {
    focus: false,
    attachedToEditor: true,
    scope: `role.${completionMenuRoleId}`,
  });
}

function createCompletionSession(editor: EditorHost, context: RuntimeCompletionContext): ActiveCompletionState {
  const session = editor.sessions.create<CompletionSessionState>({
    kind: "completionMenu",
    role: completionMenuRoleId,
    title: "Completions",
    state: {
      query: context.query,
      selectedIndex: 0,
      items: [],
      loading: true,
      emptyText: "Loading…",
      documentationMarkdown: undefined,
      documentationLoading: false,
      dynamicWidth: false,
      showDocumentation: true,
    },
  });

  return {
    session,
    surfaceId: context.surfaceId as SurfaceId,
    path: context.path,
    languageId: context.languageId,
    query: context.query,
    replaceStart: context.replaceStart,
    replaceEnd: context.replaceEnd,
    requestSequence: 0,
    requestRevision: context.revision,
    allItems: [],
    items: [],
    selectedIndex: 0,
    loading: true,
    isIncomplete: false,
    providerContexts: new Map(),
    display: {},
  };
}

function publishCompletionSession(active: ActiveCompletionState, overrides: Partial<CompletionSessionState> = {}): void {
  const start = performance.now();
  const visibleItems = active.items.slice(0, maxCompletionItems);
  if (active.selectedIndex >= visibleItems.length) active.selectedIndex = Math.max(0, visibleItems.length - 1);
  const selected = visibleItems[active.selectedIndex];
  active.session.setState({
    query: active.query,
    selectedIndex: active.selectedIndex,
    items: visibleItems.map((entry) => entry.view),
    loading: overrides.loading ?? active.loading,
    emptyText: overrides.emptyText,
    documentationMarkdown: overrides.documentationMarkdown ?? buildCompletionDocumentationMarkdown(selected?.item),
    documentationLoading: overrides.documentationLoading ?? active.resolvingItemId === selected?.item.id,
    dynamicWidth: overrides.dynamicWidth ?? active.display.dynamicWidth ?? false,
    showDocumentation: overrides.showDocumentation ?? active.display.showDocumentation ?? true,
  });
  completionDiag("publish", { ms: elapsed(start), totalItems: active.items.length, visibleItems: visibleItems.length });
}

async function moveCompletionSelection(editor: EditorHost, delta: number): Promise<void> {
  const active = completionController(editor).active;
  if (!active?.items.length) return;
  active.selectedIndex = mod(active.selectedIndex + delta, active.items.length);
  publishCompletionSession(active);
}

async function selectCompletionIndex(editor: EditorHost, index: number): Promise<void> {
  const active = completionController(editor).active;
  if (!active?.items.length) return;
  active.selectedIndex = clamp(index, 0, active.items.length - 1);
  publishCompletionSession(active);
}

async function resolveCompletionItem(
  editor: EditorHost,
  entry: ActiveCompletionEntry,
  context: RuntimeCompletionContext,
): Promise<CompletionItem | undefined> {
  const source = entry.sourceId === "default.lsp"
    ? createLspCompletionSource(editor, { activeRequestSequence: () => completionController(editor).active?.requestSequence, log: completionDiag })
    : editor.completion.source(entry.sourceId);
  if (!source?.resolve) {
    const materialized = materializeLazyLspCompletionItem(entry.item, context.text);
    return materialized ? { ...entry.item, ...materialized } : undefined;
  }
  const resolved = await source.resolve(entry.item, context);
  return resolved ? { ...entry.item, ...resolved } : undefined;
}

function updateResolvedCompletionEntry(active: ActiveCompletionState, itemId: string, item: CompletionItem): void {
  const index = active.items.findIndex((candidate) => candidate.item.id === itemId);
  if (index < 0) return;
  const entry = active.items[index]!;
  active.items[index] = {
    ...entry,
    item,
    view: toCompletionItemView(item, entry.sourceTitle, active.query),
  };
}

async function acceptSelectedCompletion(editor: EditorHost): Promise<void> {
  const active = completionController(editor).active;
  if (!active) return;
  const entry = active.items[active.selectedIndex];
  if (!entry) {
    hideCompletion(editor, { restoreFocus: false });
    return;
  }

  const context = await readCompletionContext(editor, { kind: "accept" }, { allowRuntimeRead: true, languageIdForPath: lspLanguageIdForPath });
  if (!context) {
    hideCompletion(editor, { restoreFocus: false });
    return;
  }

  let item = entry.item;
  try {
    const resolved = await resolveCompletionItem(editor, entry, context);
    if (resolved) {
      item = resolved;
      if (completionController(editor).active === active) updateResolvedCompletionEntry(active, entry.item.id, resolved);
    }
  } catch {
    // Fall back to the current item if resolve fails or is unsupported.
  }

  const source = editor.completion.source(entry.sourceId);
  if (source?.accept) await source.accept(item, context);
  else await applyCompletionItem(editor, item, context, active);
  await refreshActiveSurface(editor);
  hideCompletion(editor, { restoreFocus: false });
}

export function hideCompletion(editor: EditorHost, options: { restoreFocus?: boolean } = {}): void {
  const controller = completionController(editor);
  if (controller.timer) {
    clearTimeout(controller.timer);
    controller.timer = undefined;
  }
  if (controller.requeryTimer) {
    clearTimeout(controller.requeryTimer);
    controller.requeryTimer = undefined;
  }
  const active = controller.active;
  controller.active = undefined;
  controller.automaticWord = undefined;
  if (active) editor.sessions.dispose(active.session.id);
  editor.overlays.close(completionMenuRoleId, { restoreFocus: options.restoreFocus ?? false });
  editor.workbench.invalidate("roles");
}

function shouldOpenCompletion(context: RuntimeCompletionContext, trigger: CompletionContext["trigger"], alreadyVisible: boolean): boolean {
  if (trigger.kind === "manual") return true;
  if (context.selection.length !== 1) return false;
  if (context.query.length > 0) return true;
  if (trigger.kind === "triggerCharacter") return true;
  return alreadyVisible && context.query.length > 0;
}

function updateAutomaticWordFromDispatch(editor: EditorHost, dispatch: { id: string; args?: unknown }, trigger: CompletionContext["trigger"]): number {
  const controller = completionController(editor);
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId) {
    controller.automaticWord = undefined;
    return 0;
  }

  const current = controller.automaticWord;
  const sameSurface = current?.surfaceId === pane.surfaceId && current.path === pane.path;
  if (dispatch.id === commandId("edit.insertText")) {
    const text = trigger.character ?? "";
    if (text.length !== 1 || !isCompletionQueryCharacter(text)) {
      controller.automaticWord = undefined;
      return 0;
    }
    controller.automaticWord = {
      surfaceId: pane.surfaceId as SurfaceId,
      path: pane.path,
      query: sameSurface ? `${current.query}${text}` : text,
    };
    return controller.automaticWord.query.length;
  }

  if (dispatch.id === commandId("delete_char_backward") && sameSurface && current.query.length > 0) {
    const query = current.query.slice(0, -1);
    controller.automaticWord = query ? { ...current, query } : undefined;
    return query.length;
  }

  controller.automaticWord = undefined;
  return 0;
}

function shouldRefreshActiveCompletion(editor: EditorHost, active: ActiveCompletionState, trigger: CompletionContext["trigger"], queryLength: number): boolean {
  if (trigger.kind === "triggerCharacter") return shouldScheduleAutomaticCompletionTrigger(editor, trigger, queryLength);
  if (trigger.kind === "input" && isCompletionQueryCharacter(trigger.character ?? "") && active.loading) return true;
  return false;
}

function scheduleIncompleteRequery(editor: EditorHost, trigger: CompletionContext["trigger"]): void {
  const controller = completionController(editor);
  if (controller.requeryTimer) clearTimeout(controller.requeryTimer);
  controller.requeryTimer = setTimeout(() => {
    controller.requeryTimer = undefined;
    if (!completionController(editor).active?.isIncomplete) return;
    void showOrRefreshCompletion(editor, trigger.kind === "delete" ? { kind: "retrigger" } : trigger);
  }, 350);
}

function shouldScheduleAutomaticCompletionTrigger(editor: EditorHost, trigger: CompletionContext["trigger"], queryLength: number): boolean {
  if (trigger.kind === "manual") return true;
  const pane = activeSurfacePane(editor);
  if (!pane?.path) return false;
  const providers = cachedLspCompletionProvidersForPath(editor, pane.path);
  if (!providers.length) return false;
  if (trigger.kind === "triggerCharacter") {
    const character = trigger.character ?? "";
    return character.length > 0 && providers.some((status) => (status.completionTriggerCharacters ?? []).includes(character));
  }
  if (trigger.kind === "input") return queryLength >= 2 && isCompletionQueryCharacter(trigger.character ?? "");
  return false;
}

function cachedLspCompletionProvidersForPath(editor: EditorHost, path: string) {
  const serverIds = editor.lsp.serverIdsForPath(path);
  if (!serverIds.length) return [];
  return currentLspStatuses(editor).filter((status) => serverIds.includes(status.id)
    && (status.state === "running" || status.state === "initialized")
    && status.completionProvider !== false);
}

function updateActiveCompletionFromDispatch(editor: EditorHost, active: ActiveCompletionState, dispatch: { id: string; args?: unknown }): boolean {
  if (!active.allItems.length) return false;
  let changed = false;
  if (dispatch.id === commandId("edit.insertText")) {
    const text = typeof (dispatch.args as { text?: unknown } | undefined)?.text === "string"
      ? (dispatch.args as { text: string }).text
      : "";
    if (text.length !== 1 || !isCompletionQueryCharacter(text)) return false;
    active.query += text;
    active.replaceEnd += utf8ByteLength(text);
    changed = true;
  } else if (dispatch.id === commandId("delete_char_backward")) {
    if (!active.query) {
      hideCompletion(editor, { restoreFocus: false });
      return true;
    }
    const removed = active.query.slice(-1);
    active.query = active.query.slice(0, -1);
    active.replaceEnd = Math.max(active.replaceStart, active.replaceEnd - utf8ByteLength(removed));
    changed = true;
    if (!active.query) {
      hideCompletion(editor, { restoreFocus: false });
      return true;
    }
  } else {
    return false;
  }

  if (!changed) return false;
  const previousSelection = active.items[active.selectedIndex]?.item.id;
  active.items = rankCompletionEntries(active.query, active.allItems);
  if (!active.items.length) {
    hideCompletion(editor, { restoreFocus: false });
    return true;
  }
  active.selectedIndex = clampIndex(active.items, previousSelection, active.selectedIndex);
  active.loading = false;
  publishCompletionSession(active, { loading: false, emptyText: undefined });
  if (active.isIncomplete) scheduleIncompleteRequery(editor, dispatch.id === commandId("delete_char_backward") ? { kind: "delete" } : triggerForDispatch(dispatch));
  completionDiag("fast filter", { query: active.query, items: active.items.length, incomplete: active.isIncomplete });
  return true;
}

function triggerForDispatch(dispatch: { id: string; args?: unknown }): CompletionContext["trigger"] {
  if (dispatch.id === commandId("edit.insertText")) {
    const text = typeof (dispatch.args as { text?: unknown } | undefined)?.text === "string"
      ? (dispatch.args as { text: string }).text
      : "";
    if (text.length === 1 && !isCompletionQueryCharacter(text)) {
      return { kind: "triggerCharacter", character: text };
    }
    return {
      kind: text.trim() ? "input" : "retrigger",
      character: text.length === 1 ? text : undefined,
    };
  }
  return { kind: "delete" };
}

async function collectCompletionItems(editor: EditorHost, context: RuntimeCompletionContext, requestSequence: number): Promise<{
  items: ActiveCompletionEntry[];
  display: CompletionDisplayOptions;
  isIncomplete: boolean;
  providerContexts: Map<string, { isIncomplete: boolean; priority: number }>;
}> {
  const items: ActiveCompletionEntry[] = [];
  const display: CompletionDisplayOptions = {
    dynamicWidth: false,
    showDocumentation: true,
  };
  let isIncomplete = false;
  const providerContexts = new Map<string, { isIncomplete: boolean; priority: number }>();

  const source = { ...createLspCompletionSource(editor, { activeRequestSequence: () => completionController(editor).active?.requestSequence, log: completionDiag }), moduleId: "default" as never, order: 0 };
  const sources = [source];
  const responses = await Promise.all(sources.map(async (source, providerIndex) => {
    const start = performance.now();
    try {
      if (!(source.shouldTrigger?.(context) ?? true)) return undefined;
      const response = await source.provide(context);
      completionDiag("provider", { sourceId: source.id, ms: elapsed(start), items: response?.items.length ?? 0, sequence: requestSequence });
      return response ? { source, response, providerIndex } : undefined;
    } catch (error) {
      completionDiag("provider error", { sourceId: source.id, ms: elapsed(start), error: error instanceof Error ? error.message : String(error), sequence: requestSequence });
      return undefined;
    }
  }));

  for (const result of responses) {
    if (!result) continue;
    if (completionController(editor).active?.requestSequence !== requestSequence) {
      completionDiag("collect stale before normalize", { sequence: requestSequence });
      break;
    }
    const { source, response, providerIndex } = result;
    const providerIncomplete = Boolean(response.isIncomplete);
    display.dynamicWidth = display.dynamicWidth || Boolean(response.display?.dynamicWidth);
    display.showDocumentation = display.showDocumentation || Boolean(response.display?.showDocumentation);
    isIncomplete = isIncomplete || providerIncomplete;
    providerContexts.set(source.id, { isIncomplete: providerIncomplete, priority: source.priority ?? 0 });
    const normalizeStart = performance.now();
    for (const item of response.items) {
      const normalized = editor.completion.present({ ...item, sourceId: source.id }, source, {
        editor,
        path: context.path,
        languageId: context.languageId,
        query: context.query,
      });
      items.push({
        sourceId: source.id,
        sourceTitle: source.title,
        sourcePriority: source.priority ?? 0,
        providerPriority: providerIndex,
        providerIncomplete,
        item: normalized,
        view: toCompletionItemView(normalized, source.title, context.query),
        matchScore: 0,
      });
    }
    completionDiag("normalize", { sourceId: source.id, ms: elapsed(normalizeStart), items: response.items.length, sequence: requestSequence });
  }

  return { items, display, isIncomplete, providerContexts };
}

function toCompletionItemView(item: CompletionItem, sourceTitle: string | undefined, query: string): CompletionSessionItemView {
  const detail = completionMenuDetail(item);
  return {
    id: item.id,
    label: item.label,
    labelHighlights: highlightCompletionText(query, item.label),
    detail,
    detailHighlights: highlightCompletionText(query, detail ?? item.labelDescription ?? ""),
    labelDetail: item.labelDetail,
    labelDescription: item.labelDescription,
    kind: item.kind,
    badge: item.badge,
    source: completionMenuSource(item, sourceTitle),
    deprecated: item.deprecated,
  };
}

async function applyCompletionItem(
  editor: EditorHost,
  item: CompletionItem,
  context: RuntimeCompletionContext,
  active: ActiveCompletionState,
): Promise<void> {
  const mainReplace = item.replace ?? { start: active.replaceStart, end: active.replaceEnd };
  const insertion = item.insertTextFormat === "snippet"
    ? expandSnippetInsertion(item.insertText ?? item.label)
    : expandPlainInsertion(item.insertText ?? item.label, item);
  const changes = [
    ...(item.additionalTextEdits ?? []),
    { start: mainReplace.start, end: mainReplace.end, text: insertion.text },
  ].sort((left, right) => left.start - right.start || left.end - right.end);

  let deltaBeforeMain = 0;
  for (const change of changes) {
    if (change.start >= mainReplace.start) break;
    deltaBeforeMain += utf8ByteLength(change.text) - (change.end - change.start);
  }
  const insertedStart = mainReplace.start + deltaBeforeMain;
  const finalSelection = insertion.selection
    ? {
        anchor: insertedStart + utf8ByteLength(insertion.text.slice(0, insertion.selection.start)),
        head: insertedStart + utf8ByteLength(insertion.text.slice(0, insertion.selection.end)),
      }
    : (() => {
        const cursor = insertedStart + utf8ByteLength(insertion.text.slice(0, insertion.finalCursorOffset));
        return { anchor: cursor, head: cursor };
      })();
  const cursorId = context.activeCursorId ?? context.selection[0]?.cursorId ?? 1;

  const state = await editor.runtime.surfaces.applyEdit({
    surfaceId: context.surfaceId as SurfaceId,
    edit: {
      changes,
      selection: [{ cursorId, ...finalSelection }],
    },
  });
  updateSurfaceSnapshot(editor, context.surfaceId as SurfaceId, state);
}

function buildCompletionDocumentationMarkdown(item: CompletionItem | undefined): string | undefined {
  if (!item) return undefined;
  const documentation = item.documentation?.trim();
  if (!documentation) return undefined;
  if (isInlineCompletionDocumentation(documentation)) return undefined;
  const detail = completionDocumentationDetail(item);
  if (detail) return `\`\`\`\n${detail}\n\`\`\`\n\n${documentation}`;
  return documentation;
}

function singleLineDetail(detail: string | undefined): string | undefined {
  const value = detail?.replace(/\s+/g, " ").trim();
  return value ? value : undefined;
}

function completionMenuDetail(item: CompletionItem): string | undefined {
  const detail = singleLineDetail(item.detail);
  if (!detail || detail === item.label) return undefined;
  if (looksLikeNoiseDetail(detail) && (item.label.includes("(") || Boolean(item.documentation && isInlineCompletionDocumentation(item.documentation)))) {
    return undefined;
  }
  return detail;
}

function completionDocumentationDetail(item: CompletionItem): string | undefined {
  const detail = singleLineDetail(item.detail);
  if (!detail || detail === item.label) return undefined;
  if (looksLikeNoiseDetail(detail) && item.label.includes("(")) return undefined;
  return detail;
}

function completionMenuSource(item: CompletionItem, sourceTitle: string | undefined): string | undefined {
  const inlineDocumentation = inlineCompletionDocumentation(item.documentation);
  if (inlineDocumentation) return inlineDocumentation;
  if (!sourceTitle) return undefined;
  if (sourceTitle.toLowerCase() === "lsp") return undefined;
  return sourceTitle;
}

function inlineCompletionDocumentation(documentation: string | undefined): string | undefined {
  const value = documentation?.trim();
  if (!value || !isInlineCompletionDocumentation(value)) return undefined;
  return value;
}

function isInlineCompletionDocumentation(documentation: string): boolean {
  if (!documentation || /\r?\n/.test(documentation)) return false;
  if (documentation.length > 96) return false;
  if (/```|^#{1,6}\s|\[[^\]]+\]\([^\)]+\)/m.test(documentation)) return false;
  return true;
}

function looksLikeNoiseDetail(detail: string): boolean {
  if (!detail || detail.length > 40) return false;
  if (/[(){};=]/.test(detail)) return false;
  return /^[A-Za-z_][A-Za-z0-9_\s:*&<>,.?\[\]-]*$/.test(detail);
}

function utf8ByteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

function expandSnippetInsertion(text: string): {
  text: string;
  selection?: { start: number; end: number };
  finalCursorOffset: number;
} {
  let output = "";
  let index = 0;
  let selection: { start: number; end: number } | undefined;
  let finalCursorOffset: number | undefined;

  while (index < text.length) {
    const char = text[index];
    if (char === "\\" && index + 1 < text.length) {
      output += text[index + 1] ?? "";
      index += 2;
      continue;
    }
    if (char !== "$") {
      output += char;
      index += 1;
      continue;
    }

    const next = text[index + 1] ?? "";
    if (/\d/.test(next)) {
      let end = index + 1;
      while (end < text.length && /\d/.test(text[end] ?? "")) end += 1;
      const tabstop = Number.parseInt(text.slice(index + 1, end), 10);
      const position = output.length;
      if (tabstop === 0) finalCursorOffset = position;
      else if (!selection) selection = { start: position, end: position };
      index = end;
      continue;
    }

    if (next === "{") {
      const close = findSnippetPlaceholderEnd(text, index + 2);
      if (close < 0) {
        output += "$";
        index += 1;
        continue;
      }
      const body = text.slice(index + 2, close);
      const placeholder = parseSnippetPlaceholder(body);
      if (!placeholder) {
        output += `$\{${body}\}`;
        index = close + 1;
        continue;
      }
      const start = output.length;
      output += placeholder.text;
      const end = output.length;
      if (placeholder.tabstop === 0) finalCursorOffset = end;
      else if (!selection) selection = { start, end };
      index = close + 1;
      continue;
    }

    output += "$";
    index += 1;
  }

  return {
    text: output,
    selection,
    finalCursorOffset: finalCursorOffset ?? selection?.end ?? output.length,
  };
}

function findSnippetPlaceholderEnd(text: string, start: number): number {
  for (let index = start; index < text.length; index += 1) {
    if (text[index] === "}" && text[index - 1] !== "\\") return index;
  }
  return -1;
}

function parseSnippetPlaceholder(body: string): { tabstop: number; text: string } | undefined {
  const choice = body.match(/^(\d+)\|([\s\S]*)\|$/);
  if (choice) {
    return {
      tabstop: Number.parseInt(choice[1] ?? "0", 10),
      text: (choice[2] ?? "").split(",")[0] ?? "",
    };
  }

  const placeholder = body.match(/^(\d+):(.*)$/s);
  if (placeholder) {
    return {
      tabstop: Number.parseInt(placeholder[1] ?? "0", 10),
      text: placeholder[2] ?? "",
    };
  }

  if (/^\d+$/.test(body)) {
    return {
      tabstop: Number.parseInt(body, 10),
      text: "",
    };
  }

  return undefined;
}

function expandPlainInsertion(text: string, item: CompletionItem): {
  text: string;
  selection?: { start: number; end: number };
  finalCursorOffset: number;
} {
  const selection = callableArgumentSelection(text, item);
  return {
    text,
    selection,
    finalCursorOffset: selection?.end ?? text.length,
  };
}

function callableArgumentSelection(text: string, item: CompletionItem): { start: number; end: number } | undefined {
  if (!isCallableCompletionKind(item.kind)) return undefined;
  const open = text.indexOf("(");
  if (open < 0) return undefined;
  const close = text.indexOf(")", open + 1);
  if (close <= open + 1) return undefined;
  return { start: open + 1, end: close };
}

function isCallableCompletionKind(kind: string | undefined): boolean {
  return kind === "function" || kind === "method" || kind === "constructor";
}

function clampIndex(items: readonly ActiveCompletionEntry[], previousItemId: string | undefined, fallback: number): number {
  if (!items.length) return 0;
  if (previousItemId) {
    const index = items.findIndex((entry) => entry.item.id === previousItemId);
    if (index >= 0) return index;
  }
  return clamp(fallback, 0, items.length - 1);
}

function compareNullable(left: string | undefined, right: string | undefined): number {
  if (left && right) return left.localeCompare(right);
  if (left) return -1;
  if (right) return 1;
  return 0;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function mod(value: number, base: number): number {
  return ((value % base) + base) % base;
}

