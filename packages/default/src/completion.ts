import type { JsonValue, SurfaceId, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
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
  defineHostStore, } from "@the-editor/editor";
import { activeSurfacePane, getDefaultState, refreshActiveSurface } from "./state.js";
import { ensureLspForPath, fileUri, lspLanguageIdForPath } from "./lsp.js";

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
}

interface RuntimeCompletionContext extends CompletionContext {
  surfaceId: SurfaceId;
  activeCursorId: number | null;
  cursorHead: number;
  selection: SurfaceSelectionRange[];
  revision: number;
  completionRequestSequence?: number;
}

interface LineOffsetCache {
  text: string;
  starts: number[];
}

const completionImplementationId = roleImplementationId("default.completionMenu");
const completionControllerSlot = "default.completion.controller";
const codeActionStateSlot = "default.lsp.codeActions.state";
const codeActionCompletionSourceId = "default.lsp.codeActions";
const maxCompletionItems = 400;
const completionDiagnosticsEnabled = true;
const completionPreservingCommands = new Set([
  commandId("lsp_code_actions"),
  commandId("code_action"),
]);

function elapsed(start: number): number {
  return Math.round((performance.now() - start) * 10) / 10;
}

function completionDiag(phase: string, data: Record<string, unknown>): void {
  if (!completionDiagnosticsEnabled) return;
  console.info("[completion]", phase, data);
}

const insertMutationCommands = new Set([
  commandId("edit.insertText"),
  commandId("delete_char_backward"),
  commandId("delete_char_forward"),
  commandId("delete_word_backward"),
  commandId("delete_word_forward"),
  commandId("kill_to_line_start"),
  commandId("kill_to_line_end"),
]);


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
          visible: Boolean(active),
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
      editor.completion.registerSource(bufferWordCompletionSource()),
      editor.completion.registerSource(lspCompletionSource(editor)),
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
        if (dispatch.id.startsWith("completion.") || completionPreservingCommands.has(dispatch.id as ReturnType<typeof commandId>)) return;
        const active = completionController(editor).active;
        const inInsertMode = getDefaultState(editor).mode === "insert" && editor.input.current().kind === "editor";
        if (insertMutationCommands.has(dispatch.id as ReturnType<typeof commandId>) || active) {
          completionDiag("after dispatch", { id: dispatch.id, active: Boolean(active), inInsertMode });
        }

        if (active) {
          if (insertMutationCommands.has(dispatch.id as ReturnType<typeof commandId>) && inInsertMode) {
            if (updateActiveCompletionFromDispatch(editor, active, dispatch)) return;
            const trigger = triggerForDispatch(dispatch);
            if (!shouldScheduleCompletionTrigger(editor, trigger)) {
              hideCompletion(editor, { restoreFocus: false });
              return;
            }
            scheduleCompletion(editor, trigger);
            return;
          }
          hideCompletion(editor, { restoreFocus: false });
          return;
        }

        if (insertMutationCommands.has(dispatch.id as ReturnType<typeof commandId>) && inInsertMode) {
          const trigger = triggerForDispatch(dispatch);
          if (shouldScheduleCompletionTrigger(editor, trigger)) scheduleCompletion(editor, trigger);
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

function scheduleCompletion(editor: EditorHost, trigger: CompletionContext["trigger"], delayMs = 40): void {
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
  completionDiag("show start", { trigger: trigger.kind, character: trigger.character });
  const controller = completionController(editor);
  if (controller.timer) {
    clearTimeout(controller.timer);
    controller.timer = undefined;
  }

  const contextStart = performance.now();
  const context = await readCompletionContext(editor, trigger);
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
    const previewItems = rankCompletionItems(context.query, nextActive.allItems);
    if (previewItems.length) {
      nextActive.items = previewItems;
      nextActive.selectedIndex = clampIndex(nextActive.items, previousSelection, nextActive.selectedIndex);
    }
  }
  const hasLspProvider = nextActive.providerContexts.has("default.lsp");
  const canFilterExistingItems = Boolean(active)
    && nextActive.allItems.length > 0
    && (trigger.kind === "input" || trigger.kind === "delete" || trigger.kind === "retrigger")
    && !nextActive.isIncomplete
    && !hasLspProvider;

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
    void resolveSelectedCompletion(editor, nextActive);
    completionDiag("show end", { ms: elapsed(start), reason: "filtered", items: nextActive.items.length });
    return;
  }

  nextActive.loading = nextActive.allItems.length === 0;
  nextActive.requestSequence += 1;
  nextActive.requestRevision = context.revision;
  const requestSequence = nextActive.requestSequence;
  const requestRevision = context.revision;

  if (!editor.overlays.isOpen(completionMenuRoleId)) {
    editor.overlays.open(completionMenuRoleId, {
      focus: false,
      attachedToEditor: true,
      scope: `role.${completionMenuRoleId}`,
    });
  }
  publishCompletionSession(nextActive, {
    loading: nextActive.loading,
    emptyText: nextActive.items.length ? undefined : "Loading…",
  });

  const requestContext: RuntimeCompletionContext = nextActive.isIncomplete && (trigger.kind === "input" || trigger.kind === "delete" || trigger.kind === "retrigger")
    ? { ...context, trigger: { kind: "incomplete" as const, character: trigger.character }, completionRequestSequence: requestSequence }
    : { ...context, completionRequestSequence: requestSequence };
  const collectStart = performance.now();
  const results = await collectCompletionItems(editor, requestContext, requestSequence);
  completionDiag("collect", { ms: elapsed(collectStart), items: results.items.length, sequence: requestSequence });
  if (completionController(editor).active !== nextActive || nextActive.requestSequence !== requestSequence) return;
  const latestContext = await readCompletionContext(editor, { kind: "retrigger" });
  if (!latestContext || latestContext.surfaceId !== nextActive.surfaceId || latestContext.revision !== requestRevision) return;

  nextActive.display = results.display;
  nextActive.isIncomplete = results.isIncomplete;
  nextActive.providerContexts = results.providerContexts;
  nextActive.allItems = results.items;
  const rankStart = performance.now();
  nextActive.items = rankCompletionItems(context.query, nextActive.allItems);
  completionDiag("rank", { ms: elapsed(rankStart), input: nextActive.allItems.length, output: nextActive.items.length, sequence: requestSequence });
  nextActive.loading = false;
  nextActive.selectedIndex = clampIndex(nextActive.items, previousSelection, nextActive.selectedIndex);

  if (!nextActive.items.length && trigger.kind !== "manual") {
    hideCompletion(editor, { restoreFocus: false });
    completionDiag("show end", { ms: elapsed(start), reason: "empty" });
    return;
  }

  publishCompletionSession(nextActive, {
    emptyText: nextActive.items.length ? undefined : "No completions",
    loading: false,
  });
  void resolveSelectedCompletion(editor, nextActive);
  completionDiag("show end", { ms: elapsed(start), reason: "published", items: nextActive.items.length });
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
    loading: active.loading,
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
  void resolveSelectedCompletion(editor, active);
}

async function selectCompletionIndex(editor: EditorHost, index: number): Promise<void> {
  const active = completionController(editor).active;
  if (!active?.items.length) return;
  active.selectedIndex = clamp(index, 0, active.items.length - 1);
  publishCompletionSession(active);
  void resolveSelectedCompletion(editor, active);
}

async function resolveSelectedCompletion(editor: EditorHost, active: ActiveCompletionState): Promise<void> {
  const start = performance.now();
  const entry = active.items[active.selectedIndex];
  if (!entry) return;
  if (active.resolvingItemId === entry.item.id) return;

  const source = editor.completion.source(entry.sourceId);
  if (!source?.resolve) {
    publishCompletionSession(active, { documentationLoading: false });
    return;
  }

  const context = await readCompletionContext(editor, { kind: "retrigger" });
  if (!context) return;

  const requestSequence = active.requestSequence;
  active.resolvingItemId = entry.item.id;
  publishCompletionSession(active, { documentationLoading: !entry.item.documentation?.trim() });

  try {
    const resolved = await resolveCompletionItem(editor, entry, context);
    completionDiag("resolve selected", { ms: elapsed(start), sourceId: entry.sourceId, label: entry.item.label });
    if (!resolved) return;
    if (completionController(editor).active !== active || active.requestSequence !== requestSequence) return;
    updateResolvedCompletionEntry(active, entry.item.id, resolved);
  } catch {
    return;
  } finally {
    if (completionController(editor).active === active) {
      active.resolvingItemId = undefined;
      publishCompletionSession(active, { documentationLoading: false });
    }
  }
}

async function resolveCompletionItem(
  editor: EditorHost,
  entry: ActiveCompletionEntry,
  context: RuntimeCompletionContext,
): Promise<CompletionItem | undefined> {
  const source = editor.completion.source(entry.sourceId);
  if (!source?.resolve) {
    const materialized = materializeLazyLspCompletionItem(entry.item, context.text);
    if (!materialized) return undefined;
    if (!source) return { ...entry.item, ...materialized };
    return editor.completion.present({ ...entry.item, ...materialized }, source, {
      editor,
      path: context.path,
      languageId: context.languageId,
      query: context.query,
    });
  }
  const resolved = await source.resolve(entry.item, context);
  if (!resolved) return undefined;
  return editor.completion.present({ ...entry.item, ...resolved }, source, {
    editor,
    path: context.path,
    languageId: context.languageId,
    query: context.query,
  });
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

  const context = await readCompletionContext(editor, { kind: "accept" });
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
  const active = controller.active;
  controller.active = undefined;
  if (active) editor.sessions.dispose(active.session.id);
  editor.overlays.close(completionMenuRoleId, { restoreFocus: options.restoreFocus ?? false });
  editor.workbench.invalidate("roles");
}

function shouldOpenCompletion(context: RuntimeCompletionContext, trigger: CompletionContext["trigger"], alreadyVisible: boolean): boolean {
  if (codeActionCompletionActive(context.editor) && trigger.kind === "manual") return true;
  if (trigger.kind === "manual") return true;
  if (context.selection.length !== 1) return false;
  if (context.query.length > 0) return true;
  if (trigger.kind === "triggerCharacter") return true;
  return alreadyVisible && context.query.length > 0;
}

function codeActionCompletionActive(editor: EditorHost): boolean {
  return Boolean(editor.stores.get(defineHostStore<{ active?: unknown } | undefined>({ id: codeActionStateSlot, init: () => undefined }))?.active);
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

  const sources = codeActionCompletionActive(editor)
    ? editor.completion.sources().filter((source) => source.id === codeActionCompletionSourceId)
    : editor.completion.sources();
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

function rankCompletionItems(query: string, items: ActiveCompletionEntry[]): ActiveCompletionEntry[] {
  const normalizedQuery = normalizeCompletionText(query);
  const ranked = items
    .map((entry) => {
      const score = scoreCompletionItem(normalizedQuery, entry.item);
      const labelHighlights = highlightCompletionText(query, entry.item.label);
      const detailHighlights = highlightCompletionText(query, entry.item.detail ?? entry.item.labelDescription ?? "");
      return {
        ...entry,
        matchScore: score,
        view: {
          ...entry.view,
          labelHighlights,
          detailHighlights,
        },
      } satisfies ActiveCompletionEntry;
    })
    .filter((entry) => !normalizedQuery || entry.matchScore > 0)
    .sort((left, right) => {
      const lspOrdering = compareLspCompletionEntries(left, right);
      if (lspOrdering !== undefined) return lspOrdering;
      return right.matchScore - left.matchScore
        || right.sourcePriority - left.sourcePriority
        || compareNullable(left.item.sortText, right.item.sortText)
        || left.item.label.localeCompare(right.item.label);
    });

  return ranked;
}

function compareLspCompletionEntries(left: ActiveCompletionEntry, right: ActiveCompletionEntry): number | undefined {
  const leftIsLsp = left.sourceId === "default.lsp";
  const rightIsLsp = right.sourceId === "default.lsp";
  if (!leftIsLsp && !rightIsLsp) return undefined;
  if (leftIsLsp !== rightIsLsp) return leftIsLsp ? -1 : 1;
  return Number(Boolean(right.item.payload && (right.item.payload as { preselect?: boolean }).preselect))
    - Number(Boolean(left.item.payload && (left.item.payload as { preselect?: boolean }).preselect))
    || compareNullable(left.item.sortText, right.item.sortText)
    || right.matchScore - left.matchScore
    || left.item.label.localeCompare(right.item.label);
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

function scoreCompletionItem(query: string, item: CompletionItem): number {
  if (!query) return 1;
  const haystacks = [item.filterText, item.label, item.detail, item.labelDescription]
    .filter((value): value is string => Boolean(value?.trim()))
    .map(normalizeCompletionText);
  let best = 0;
  for (const haystack of haystacks) {
    best = Math.max(best, directCompletionScore(query, haystack));
    best = Math.max(best, fuzzyCompletionScore(query, haystack));
  }
  return best;
}

function directCompletionScore(query: string, haystack: string): number {
  if (!query || !haystack) return 0;
  if (haystack === query) return 1200 - haystack.length;
  if (haystack.startsWith(query)) return 950 - haystack.length;
  const index = haystack.indexOf(query);
  if (index >= 0) return 750 - haystack.length - Math.min(index, 32);
  return 0;
}

function fuzzyCompletionScore(query: string, haystack: string): number {
  if (!query || !haystack) return 0;
  let queryIndex = 0;
  let score = 0;
  let firstMatch = -1;
  let consecutive = 0;
  for (let index = 0; index < haystack.length && queryIndex < query.length; index += 1) {
    if (haystack[index] !== query[queryIndex]) {
      consecutive = 0;
      continue;
    }
    if (firstMatch < 0) firstMatch = index;
    score += 10;
    if (index === 0 || !/[a-z0-9]/.test(haystack[index - 1] ?? "")) score += 16;
    if (consecutive > 0) score += 12 + consecutive * 3;
    consecutive += 1;
    queryIndex += 1;
  }
  if (queryIndex !== query.length) return 0;
  score -= Math.max(0, haystack.length - query.length);
  if (firstMatch > 0) score -= Math.min(firstMatch, 40);
  return Math.max(score, 1);
}

function highlightCompletionText(query: string, value: string): number[] {
  const normalizedQuery = normalizeCompletionText(query);
  if (!normalizedQuery || !value) return [];
  const normalizedValue = normalizeCompletionText(value);
  const directIndex = normalizedValue.indexOf(normalizedQuery);
  if (directIndex >= 0) {
    return Array.from({ length: normalizedQuery.length }, (_, offset) => directIndex + offset);
  }

  const positions: number[] = [];
  let queryIndex = 0;
  for (let index = 0; index < normalizedValue.length && queryIndex < normalizedQuery.length; index += 1) {
    if (normalizedValue[index] !== normalizedQuery[queryIndex]) continue;
    positions.push(index);
    queryIndex += 1;
  }
  return queryIndex === normalizedQuery.length ? positions : [];
}

function normalizeCompletionText(value: string): string {
  return value.trim().toLowerCase();
}

async function readCompletionContext(editor: EditorHost, trigger: CompletionContext["trigger"]): Promise<RuntimeCompletionContext | undefined> {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId) return undefined;
  const surface = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  if (!cursor) return undefined;

  const cursorHead = cursor.head;
  const index = stringIndexAtUtf8ByteOffset(surface.text, cursorHead);
  const [lineStart, lineEnd] = lineBoundsAtStringIndex(surface.text, index);
  const queryStartIndex = scanQueryStart(surface.text, index);
  const linePrefix = surface.text.slice(lineStart, index);
  const lineSuffix = surface.text.slice(index, lineEnd);
  const replaceStart = utf8ByteLength(surface.text.slice(0, queryStartIndex));
  return {
    editor,
    surfaceId: pane.surfaceId as SurfaceId,
    path: pane.path,
    languageId: pane.path ? lspLanguageIdForPath(editor, pane.path) : undefined,
    trigger,
    query: surface.text.slice(queryStartIndex, index),
    cursorOffset: cursorHead,
    revision: surface.revision,
    replaceStart,
    replaceEnd: cursorHead,
    linePrefix,
    lineSuffix,
    text: surface.text,
    activeCursorId: surface.activeCursorId,
    cursorHead,
    selection: surface.selection,
  };
}

function scanQueryStart(text: string, end: number): number {
  let index = end;
  while (index > 0 && isCompletionQueryCharacter(text[index - 1] ?? "")) index -= 1;
  return index;
}

function isCompletionQueryCharacter(char: string): boolean {
  return /[A-Za-z0-9_$]/.test(char);
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

  await editor.runtime.surfaces.applyEdit({
    surfaceId: context.surfaceId as SurfaceId,
    edit: {
      changes,
      selection: [{ cursorId, ...finalSelection }],
    },
  });
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

function bufferWordCompletionSource(): CompletionSource {
  return {
    id: "default.bufferWords",
    title: "words",
    priority: 10,
    shouldTrigger(ctx) {
      return ctx.trigger.kind === "manual" || ctx.query.length >= 2;
    },
    async provide(ctx): Promise<CompletionResult | null> {
      const query = ctx.query.trim();
      if (!query && ctx.trigger.kind !== "manual") return null;
      const words = completeBufferWords(ctx.text, ctx.cursorOffset, query, 100);
      return {
        items: words.map(({ label }) => ({
          id: `word:${label}`,
          label,
          filterText: label,
          insertText: label,
          replace: { start: ctx.replaceStart, end: ctx.replaceEnd },
          kind: "text",
          labelDescription: "buffer word",
        })),
      };
    },
  };
}

function completeBufferWords(text: string, cursorOffset: number, query: string, limit: number): Array<{ label: string }> {
  const start = performance.now();
  const maxChars = 160_000;
  const cursorIndex = stringIndexAtUtf8ByteOffset(text, cursorOffset);
  const windowStart = Math.max(0, cursorIndex - Math.floor(maxChars / 2));
  const windowEnd = Math.min(text.length, windowStart + maxChars);
  const queryLower = query.toLowerCase();
  const seen = new Set<string>();
  const words: Array<{ label: string }> = [];
  const matcher = /[A-Za-z0-9_$]{2,}/g;
  matcher.lastIndex = windowStart;

  while (words.length < limit) {
    const match = matcher.exec(text);
    if (!match || match.index >= windowEnd) break;
    const label = match[0] ?? "";
    const startIndex = match.index;
    const endIndex = startIndex + label.length;
    if (startIndex <= cursorIndex && cursorIndex <= endIndex) continue;
    if (queryLower && !label.toLowerCase().includes(queryLower)) continue;
    if (seen.has(label)) continue;
    seen.add(label);
    words.push({ label });
  }

  completionDiag("buffer words", { ms: elapsed(start), textLength: text.length, scanned: windowEnd - windowStart, items: words.length });
  return words;
}

function lspCompletionSource(editor: EditorHost): CompletionSource {
  return {
    id: "default.lsp",
    title: "lsp",
    priority: 100,
    shouldTrigger(ctx) {
      return ctx.trigger.kind === "manual"
        || ctx.trigger.kind === "incomplete"
        || ctx.trigger.kind === "triggerCharacter"
        || ((ctx.trigger.kind === "input" || ctx.trigger.kind === "retrigger") && ctx.query.length >= 2);
    },
    async provide(ctx) {
      if (!ctx.path) return null;
      await ensureLspForPath(editor, ctx.path);
      const serverIds = editor.lsp.serverIdsForPath(ctx.path);
      if (!serverIds.length) return null;
      const statuses = await editor.lsp.status();
      const running = statuses.filter((status) => serverIds.includes(status.id) && (status.state === "running" || status.state === "initialized"));
      if (!running.length) return null;
      if (ctx.trigger.kind === "triggerCharacter" && !running.some((status) => (status.completionTriggerCharacters ?? []).includes(ctx.trigger.character ?? ""))) {
        return null;
      }

      const position = positionAtByteOffset(ctx.text, ctx.cursorOffset);
      const uri = fileUri(ctx.path);
      const responses = await Promise.all(running.map(async (server) => {
        const start = performance.now();
        try {
          const response = await editor.lsp.request({
            serverId: server.handle,
            method: "textDocument/completion",
            timeoutMs: ctx.trigger.kind === "manual" ? 2000 : ctx.trigger.kind === "input" || ctx.trigger.kind === "retrigger" || ctx.trigger.kind === "incomplete" ? 1000 : 750,
            params: {
              textDocument: { uri },
              position,
              context: lspCompletionRequestContext(ctx.trigger),
            },
          });
          const requestMs = elapsed(start);
          const requestSequence = (ctx as RuntimeCompletionContext).completionRequestSequence;
          if (completionController(editor).active?.requestSequence !== requestSequence) {
            completionDiag("lsp stale before normalize", { serverId: server.id, requestMs, sequence: requestSequence });
            return { items: [], isIncomplete: false };
          }
          const normalizeStart = performance.now();
          const normalized = normalizeLspCompletionResponse(response, ctx.text, server.id, server.handle);
          completionDiag("lsp response", { serverId: server.id, requestMs, normalizeMs: elapsed(normalizeStart), items: normalized.items.length, isIncomplete: normalized.isIncomplete });
          return normalized;
        } catch (error) {
          completionDiag("lsp error", { serverId: server.id, ms: elapsed(start), error: error instanceof Error ? error.message : String(error) });
          return undefined;
        }
      }));

      const items: CompletionItem[] = [];
      let isIncomplete = false;
      for (const response of responses) {
        if (!response) continue;
        items.push(...response.items);
        isIncomplete = isIncomplete || response.isIncomplete;
      }
      return { items, isIncomplete, display: { dynamicWidth: true, showDocumentation: true } };
    },
    async resolve(item, ctx) {
      const payload = item.payload as { serverHandle?: number; raw?: JsonValue } | undefined;
      if (!ctx.path || !payload?.serverHandle || !payload.raw) return;
      try {
        const resolved = await editor.lsp.request({
          serverId: payload.serverHandle as never,
          method: "completionItem/resolve",
          timeoutMs: 1000,
          params: payload.raw,
        });
        return normalizeResolvedLspCompletionItem(resolved, ctx.text, item, payload.serverHandle);
      } catch {
        return undefined;
      }
    },
  };
}

function lspCompletionRequestContext(trigger: CompletionContext["trigger"]): JsonValue {
  switch (trigger.kind) {
    case "triggerCharacter":
      return { triggerKind: 2, triggerCharacter: trigger.character ?? null };
    case "incomplete":
      return { triggerKind: 3 };
    default:
      return { triggerKind: 1 };
  }
}

function normalizeLspCompletionResponse(
  value: JsonValue,
  text: string,
  serverId: string,
  serverHandle: number,
): { items: CompletionItem[]; isIncomplete: boolean } {
  const lineOffsets = createLineOffsetCache(text);
  if (Array.isArray(value)) {
    return {
      items: value
        .map((entry, index) => normalizeLspCompletionItem(entry, text, serverId, serverHandle, index, undefined, lineOffsets, false))
        .filter((entry): entry is CompletionItem => Boolean(entry)),
      isIncomplete: false,
    };
  }
  if (!isObject(value)) return { items: [], isIncomplete: false };
  const rawItems = Array.isArray(value.items) ? value.items : [];
  const defaults = lspCompletionItemDefaults(asRecord(value.itemDefaults));
  return {
    items: rawItems
      .map((entry, index) => normalizeLspCompletionItem(entry, text, serverId, serverHandle, index, defaults, lineOffsets, false))
      .filter((entry): entry is CompletionItem => Boolean(entry)),
    isIncomplete: Boolean(value.isIncomplete),
  };
}

interface LspCompletionItemDefaults {
  editRange?: JsonValue;
  data?: JsonValue;
  commitCharacters?: string[];
}

function lspCompletionItemDefaults(value: Record<string, any> | undefined): LspCompletionItemDefaults | undefined {
  if (!value) return undefined;
  return {
    editRange: value.editRange,
    data: value.data,
    commitCharacters: Array.isArray(value.commitCharacters)
      ? value.commitCharacters.filter((entry): entry is string => typeof entry === "string")
      : undefined,
  };
}

function normalizeResolvedLspCompletionItem(value: JsonValue, text: string, original: CompletionItem, serverHandle: number): Partial<CompletionItem> | undefined {
  const resolved = normalizeLspCompletionItem(value, text, String((original.payload as { serverId?: string } | undefined)?.serverId ?? "lsp"), serverHandle, 0, undefined, createLineOffsetCache(text), true);
  if (!resolved) return undefined;
  return {
    detail: resolved.detail,
    documentation: resolved.documentation,
    replace: resolved.replace,
    insertText: resolved.insertText,
    insertTextFormat: resolved.insertTextFormat,
    additionalTextEdits: resolved.additionalTextEdits,
    payload: resolved.payload,
  };
}

function normalizeLspCompletionItem(
  value: JsonValue,
  text: string,
  serverId: string,
  serverHandle: number,
  index: number,
  defaults?: LspCompletionItemDefaults,
  lineOffsets = createLineOffsetCache(text),
  includeExpensiveFields = true,
): CompletionItem | undefined {
  const object = asRecord(value);
  if (!object || typeof object.label !== "string") return undefined;
  const textEdit = includeExpensiveFields ? lspCompletionEdit(object, defaults, text, lineOffsets) : undefined;
  const rawAdditionalTextEdits = Array.isArray(object.additionalTextEdits) ? object.additionalTextEdits : undefined;
  const additionalTextEdits = includeExpensiveFields ? rawAdditionalTextEdits
    ?.map((edit: JsonValue) => lspTextEditToCompletionEdit(edit, text, lineOffsets))
    .filter((edit): edit is { start: number; end: number; text: string } => Boolean(edit)) : undefined;
  const labelDetails = asRecord(object.labelDetails);
  const rawForResolve = defaults?.data !== undefined && object.data === undefined
    ? { ...object, data: defaults.data }
    : object;
  return {
    id: `${serverId}:${index}:${object.label}`,
    label: object.label,
    filterText: typeof object.filterText === "string" ? object.filterText : object.label,
    sortText: typeof object.sortText === "string" ? object.sortText : object.label,
    detail: typeof object.detail === "string" ? object.detail : undefined,
    documentation: includeExpensiveFields ? lspDocumentationToMarkdown(object.documentation) : undefined,
    kind: completionKindName(object.kind),
    labelDetail: typeof labelDetails?.detail === "string" ? labelDetails.detail : undefined,
    labelDescription: typeof labelDetails?.description === "string" ? labelDetails.description : undefined,
    replace: textEdit ? { start: textEdit.start, end: textEdit.end } : undefined,
    insertText: textEdit?.text
      ?? (typeof object.insertText === "string" ? object.insertText : object.label),
    insertTextFormat: object.insertTextFormat === 2 ? "snippet" : "plain",
    additionalTextEdits,
    commitCharacters: Array.isArray(object.commitCharacters)
      ? object.commitCharacters.filter((entry): entry is string => typeof entry === "string")
      : defaults?.commitCharacters,
    deprecated: Boolean(object.deprecated) || (Array.isArray(object.tags) && object.tags.includes(1)),
    payload: {
      serverId,
      serverHandle,
      raw: rawForResolve,
      preselect: Boolean(object.preselect),
    },
  };
}

function materializeLazyLspCompletionItem(item: CompletionItem, text: string): Partial<CompletionItem> | undefined {
  const payload = item.payload as { raw?: JsonValue; serverId?: string; serverHandle?: number } | undefined;
  if (!payload?.raw || !payload.serverHandle) return undefined;
  return normalizeResolvedLspCompletionItem(payload.raw, text, item, payload.serverHandle);
}

function lspCompletionEdit(
  object: Record<string, any>,
  defaults: LspCompletionItemDefaults | undefined,
  text: string,
  lineOffsets = createLineOffsetCache(text),
): { start: number; end: number; text: string } | undefined {
  const explicit = lspTextEditToCompletionEdit(object.textEdit, text, lineOffsets);
  if (explicit) return explicit;
  const newText = typeof object.textEditText === "string"
    ? object.textEditText
    : typeof object.insertText === "string"
      ? object.insertText
      : typeof object.label === "string"
        ? object.label
        : undefined;
  if (!defaults?.editRange || typeof newText !== "string") return undefined;
  return lspDefaultEditRangeToCompletionEdit(defaults.editRange, newText, text, lineOffsets);
}

function lspDefaultEditRangeToCompletionEdit(
  value: JsonValue,
  newText: string,
  text: string,
  lineOffsets = createLineOffsetCache(text),
): { start: number; end: number; text: string } | undefined {
  const object = asRecord(value);
  if (!object) return undefined;
  const replace = asRecord(object.replace);
  if (replace) {
    const start = lspPositionToByteOffset(text, replace.start, lineOffsets);
    const end = lspPositionToByteOffset(text, replace.end, lineOffsets);
    if (start === undefined || end === undefined) return undefined;
    return { start, end, text: newText };
  }
  const start = lspPositionToByteOffset(text, object.start, lineOffsets);
  const end = lspPositionToByteOffset(text, object.end, lineOffsets);
  if (start === undefined || end === undefined) return undefined;
  return { start, end, text: newText };
}

function lspTextEditToCompletionEdit(value: JsonValue, text: string, lineOffsets = createLineOffsetCache(text)): { start: number; end: number; text: string } | undefined {
  const object = asRecord(value);
  if (!object) return undefined;
  const range = asRecord(object.range);
  if (typeof object.newText === "string" && range) {
    const start = lspPositionToByteOffset(text, range.start, lineOffsets);
    const end = lspPositionToByteOffset(text, range.end, lineOffsets);
    if (start === undefined || end === undefined) return undefined;
    return { start, end, text: object.newText };
  }
  const replace = asRecord(object.replace);
  if (typeof object.newText === "string" && replace) {
    const start = lspPositionToByteOffset(text, replace.start, lineOffsets);
    const end = lspPositionToByteOffset(text, replace.end, lineOffsets);
    if (start === undefined || end === undefined) return undefined;
    return { start, end, text: object.newText };
  }
  return undefined;
}

function createLineOffsetCache(text: string): LineOffsetCache {
  const starts = [0];
  for (let index = 0; index < text.length; index += 1) {
    if (text.charCodeAt(index) === 10) starts.push(index + 1);
  }
  return { text, starts };
}

function lspPositionToByteOffset(text: string, value: unknown, lineOffsets = createLineOffsetCache(text)): number | undefined {
  const object = asRecord(value);
  if (!object || typeof object.line !== "number" || typeof object.character !== "number") return undefined;
  const lineStart = lineOffsets.starts[object.line];
  if (lineStart === undefined) return undefined;
  let index = lineStart;
  let character = object.character;
  while (character > 0 && index < text.length) {
    if (text.charCodeAt(index) === 10) break;
    index += 1;
    character -= 1;
  }
  return utf8ByteLength(text.slice(0, index));
}

function lspDocumentationToMarkdown(value: JsonValue | undefined): string | undefined {
  if (typeof value === "string") return value.trim() || undefined;
  const object = asRecord(value);
  if (!object) return undefined;
  if (typeof object.value === "string") return object.value.trim() || undefined;
  return undefined;
}

function completionKindName(value: JsonValue | undefined): string | undefined {
  const number = typeof value === "number" ? value : undefined;
  switch (number) {
    case 1: return "text";
    case 2: return "method";
    case 3: return "function";
    case 4: return "constructor";
    case 5: return "field";
    case 6: return "variable";
    case 7: return "class";
    case 8: return "interface";
    case 9: return "module";
    case 10: return "property";
    case 11: return "unit";
    case 12: return "value";
    case 13: return "enum";
    case 14: return "keyword";
    case 15: return "snippet";
    case 16: return "color";
    case 17: return "file";
    case 18: return "reference";
    case 19: return "folder";
    case 20: return "enumMember";
    case 21: return "constant";
    case 22: return "struct";
    case 23: return "event";
    case 24: return "operator";
    case 25: return "typeParameter";
    default: return undefined;
  }
}

function updateActiveCompletionFromDispatch(editor: EditorHost, active: ActiveCompletionState, dispatch: { id: string; args?: unknown }): boolean {
  if (active.isIncomplete || active.providerContexts.has("default.lsp")) return false;
  if (dispatch.id === commandId("edit.insertText")) {
    const text = typeof (dispatch.args as { text?: unknown } | undefined)?.text === "string"
      ? (dispatch.args as { text: string }).text
      : "";
    if (text.length !== 1 || !isCompletionQueryCharacter(text)) return false;
    active.query += text;
    active.replaceEnd += utf8ByteLength(text);
  } else if (dispatch.id === commandId("delete_char_backward")) {
    if (!active.query) return false;
    const removed = active.query.slice(-1);
    active.query = active.query.slice(0, -1);
    active.replaceEnd = Math.max(active.replaceStart, active.replaceEnd - utf8ByteLength(removed));
    if (!active.query) {
      hideCompletion(editor, { restoreFocus: false });
      return true;
    }
  } else {
    return false;
  }

  const previousSelection = active.items[active.selectedIndex]?.item.id;
  active.items = rankCompletionItems(active.query, active.allItems);
  if (!active.items.length) {
    hideCompletion(editor, { restoreFocus: false });
    return true;
  }
  active.selectedIndex = clampIndex(active.items, previousSelection, active.selectedIndex);
  publishCompletionSession(active, { loading: false, emptyText: undefined });
  void resolveSelectedCompletion(editor, active);
  completionDiag("fast filter", { query: active.query, items: active.items.length });
  return true;
}

function shouldScheduleCompletionTrigger(editor: EditorHost, trigger: CompletionContext["trigger"]): boolean {
  if (trigger.kind !== "triggerCharacter") return true;
  const pane = activeSurfacePane(editor);
  if (!pane?.path) return false;
  return editor.lsp.serverIdsForPath(pane.path).length > 0;
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

function positionAtByteOffset(text: string, byteOffset: number): { line: number; character: number } {
  const index = stringIndexAtUtf8ByteOffset(text, byteOffset);
  let line = 0;
  let lineStart = 0;
  for (let i = 0; i < index; i += 1) {
    if (text.charCodeAt(i) === 10) {
      line += 1;
      lineStart = i + 1;
    }
  }
  return { line, character: index - lineStart };
}

function stringIndexAtUtf8ByteOffset(text: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0;
  let bytes = 0;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    const next = code >= 0xd800 && code <= 0xdbff && index + 1 < text.length ? text.slice(index, index + 2) : text[index] ?? "";
    const length = new TextEncoder().encode(next).length;
    if (bytes + length > byteOffset) return index;
    bytes += length;
    if (next.length === 2) index += 1;
    if (bytes >= byteOffset) return index + 1;
  }
  return text.length;
}

function lineBoundsAtStringIndex(text: string, index: number): [number, number] {
  let start = index;
  while (start > 0 && text.charCodeAt(start - 1) !== 10) start -= 1;
  let end = index;
  while (end < text.length && text.charCodeAt(end) !== 10) end += 1;
  return [start, end];
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

function isObject(value: unknown): value is Record<string, any> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): Record<string, any> | undefined {
  return isObject(value) ? value : undefined;
}
