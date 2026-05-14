import type { CursorSemantics, JsonValue, RenderFrame, SurfaceId, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import {
  commandId,
  compareDiagnosticsByLocation,
  compareDiagnosticsBySeverityAndLocation,
  defineHostStore,
  resolveThemeStyle,
  resolveThemeStyleLoose,
  type BufferDecoration,
  type DiagnosticCollection,
  type DiagnosticInput,
  type DiagnosticPosition,
  type DiagnosticRange,
  type DiagnosticRecord,
  type EditorHost,
  type EditorModule,
  type GutterLine,
  type InlineTextDecoration,
  type PickerItem,
  type PickerPreview,
  type RangeDecoration,
  type ResolvedThemeDefinition,
} from "@the-editor/editor";
import {
  buildDiagnosticMarkdown,
  currentDiagnosticsPickerProviderId,
  diagnosticPrimaryLine,
  diagnosticSeverityLetter,
  diagnosticSeverityLabel,
  formatDiagnosticSummary,
  workspaceDiagnosticsPickerProviderId,
} from "./diagnosticsShared.js";
import { fileUri } from "./lsp.js";
import { activeSurfacePane, getDefaultState, refreshActiveSurface } from "./state.js";

interface DiagnosticsControllerState {
  lspCollections: Map<string, DiagnosticCollection>;
  refreshTimer?: ReturnType<typeof setTimeout>;
  pendingPaths: Set<string>;
  pendingActiveRefresh: boolean;
}

interface DiagnosticPresentationState {
  inlineEnabled: boolean;
  underlinesEnabled: boolean;
  inlinePaddingCols: number;
  inlineMinColumn: number;
}

const diagnosticsControllerSlot = "default.diagnostics.controller";
const diagnosticPresentationSlot = "default.diagnostics.presentation";
const DIAGNOSTIC_TAB_WIDTH = 4;
const severityRank: Record<DiagnosticRecord["severity"], number> = {
  error: 0,
  warning: 1,
  info: 2,
  hint: 3,
};

export function defaultDiagnostics(): EditorModule {
  return (editor) => {
    const unsubscribe = editor.diagnostics.subscribe((event) => {
      scheduleDiagnosticsRefresh(editor, event.paths);
    });

    editor.commands.register<void, void>({
      id: commandId("diagnostics.toggleInline"),
      title: "Toggle Inline Diagnostics",
      category: "Diagnostics",
      keywords: ["diagnostics", "inline", "eol", "end of line", "annotations"],
      async run() {
        const state = diagnosticsPresentationState(editor);
        state.inlineEnabled = !state.inlineEnabled;
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame");
      },
    });

    editor.commands.register<void, void>({
      id: commandId("diagnostics.toggleUnderlines"),
      title: "Toggle Diagnostic Underlines",
      category: "Diagnostics",
      keywords: ["diagnostics", "underline", "undercurl", "squiggle"],
      async run() {
        const state = diagnosticsPresentationState(editor);
        state.underlinesEnabled = !state.underlinesEnabled;
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame");
      },
    });

    editor.commands.register<void, void>({
      id: commandId("diagnostics.next"),
      title: "Go to Next Diagnostic",
      category: "Diagnostics",
      keywords: ["diagnostics", "problems", "next", "error", "warning"],
      async run() {
        await navigateDiagnostic(editor, 1);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("diagnostics.previous"),
      title: "Go to Previous Diagnostic",
      category: "Diagnostics",
      keywords: ["diagnostics", "problems", "previous", "error", "warning"],
      async run() {
        await navigateDiagnostic(editor, -1);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("diagnostics_picker"),
      title: "Current File Diagnostics",
      category: "Diagnostics",
      keywords: ["diagnostics", "problems", "current", "file"],
      async run() {
        await editor.commands.dispatch({ id: commandId("picker.open"), args: { providerId: currentDiagnosticsPickerProviderId } });
      },
    });

    editor.commands.register<void, void>({
      id: commandId("workspace_diagnostics_picker"),
      title: "Workspace Diagnostics",
      category: "Diagnostics",
      keywords: ["diagnostics", "problems", "workspace"],
      async run() {
        await editor.commands.dispatch({ id: commandId("picker.open"), args: { providerId: workspaceDiagnosticsPickerProviderId } });
      },
    });

    editor.pickers.registerProvider({
      id: currentDiagnosticsPickerProviderId,
      title: "Current File Diagnostics",
      placeholder: "Filter diagnostics",
      async createSession() {
        return {
          search(query) {
            const path = getDefaultState(editor).activePath;
            const diagnostics = path ? editor.diagnostics.forPath(path) : [];
            return diagnosticPickerPage(filterDiagnostics(diagnostics, query), path ? "Current file diagnostics" : "No active file");
          },
          async preview(item) {
            return diagnosticPickerPreview(editor, item);
          },
          async accept(item) {
            await openDiagnosticItem(editor, item.payload as DiagnosticPickerPayload | undefined);
          },
        };
      },
    });

    editor.pickers.registerProvider({
      id: workspaceDiagnosticsPickerProviderId,
      title: "Workspace Diagnostics",
      placeholder: "Filter diagnostics",
      async createSession() {
        return {
          search(query) {
            return diagnosticPickerPage(filterDiagnostics(editor.diagnostics.all(), query), "Workspace diagnostics");
          },
          async preview(item) {
            return diagnosticPickerPreview(editor, item);
          },
          async accept(item) {
            await openDiagnosticItem(editor, item.payload as DiagnosticPickerPayload | undefined);
          },
        };
      },
    });

    editor.decorations.registerProvider({
      id: "default.diagnostics.decorations",
      title: "Diagnostics",
      priority: 90,
      provide(ctx) {
        if (!ctx.activePath || !ctx.frame) return [];
        const diagnostics = ctx.editor.diagnostics.forPath(ctx.activePath);
        if (!diagnostics.length) return [];
        return buildDiagnosticDecorations(
          ctx.frame,
          diagnostics,
          ctx.editor.themes.snapshot().active,
          diagnosticsPresentationState(ctx.editor),
          ctx.text,
        );
      },
    });

    editor.gutters.registerProvider({
      id: "default.diagnostics.gutter",
      title: "Diagnostics",
      lane: "diagnostics",
      laneSpec: {
        id: "diagnostics",
        order: 24,
        widthPx: 7,
        paddingLeftPx: 2,
        paddingRightPx: 2,
      },
      provide(ctx) {
        if (!ctx.activePath || !ctx.frame) return [];
        const diagnostics = ctx.editor.diagnostics.forPath(ctx.activePath);
        if (!diagnostics.length) return [];
        return buildDiagnosticGutterLines(ctx.frame, diagnostics, ctx.editor.themes.snapshot().active);
      },
    });

    let disposed = false;
    void (async () => {
      while (!disposed) {
        try {
          await syncLspDiagnostics(editor);
        } catch {
          // Diagnostics are opportunistic; status/log APIs expose transport issues.
        }
        await delay(220);
      }
    })();

    return {
      dispose() {
        disposed = true;
        unsubscribe();
        const controller = diagnosticsController(editor);
        if (controller.refreshTimer) clearTimeout(controller.refreshTimer);
        for (const collection of controller.lspCollections.values()) collection.dispose();
        controller.lspCollections.clear();
      },
    };
  };
}

function diagnosticsPresentationState(editor: EditorHost): DiagnosticPresentationState {
  return editor.stores.host(defineHostStore<DiagnosticPresentationState>({
    id: diagnosticPresentationSlot,
    init: () => ({
      inlineEnabled: true,
      underlinesEnabled: true,
      inlinePaddingCols: 4,
      inlineMinColumn: 0,
    }),
  }));
}

export function currentPathDiagnosticSummary(editor: EditorHost): string | undefined {
  const path = getDefaultState(editor).activePath;
  if (!path) return undefined;
  const summary = editor.diagnostics.summaryForPath(path);
  return formatDiagnosticSummary(summary) || undefined;
}

export function diagnosticMarkdownForPathPosition(
  editor: EditorHost,
  params: { path: string; line: number; character: number },
): string | undefined {
  const exact = editor.diagnostics.atPosition(params);
  const diagnostics = exact.length ? exact : editor.diagnostics.forLine(params);
  if (!diagnostics.length) return undefined;
  return buildDiagnosticMarkdown(diagnostics);
}

async function syncLspDiagnostics(editor: EditorHost): Promise<void> {
  const rawEntries = await editor.lsp.diagnostics();
  const statuses = await editor.lsp.status();
  const activePane = activeSurfacePane(editor);
  const activeUri = activePane?.path ? fileUri(activePane.path) : undefined;
  const activeRevision = activePane?.surfaceId
    ? (await editor.runtime.surfaces.readState({ surfaceId: activePane.surfaceId })).revision
    : undefined;
  const handleToId = new Map(statuses.map((status) => [status.handle, status.id] as const));
  const nextByServer = new Map<string, Map<string, DiagnosticInput[]>>();

  for (const entry of rawEntries) {
    const serverId = handleToId.get(entry.server);
    if (!serverId) continue;
    if (activeUri && entry.uri === activeUri && entry.version != null && activeRevision != null && entry.version !== activeRevision) continue;
    const byUri = nextByServer.get(serverId) ?? new Map<string, DiagnosticInput[]>();
    const diagnostics = [] as DiagnosticInput[];
    for (const rawDiagnostic of entry.diagnostics) {
      const parsed = parseLspDiagnostic(rawDiagnostic);
      if (parsed) diagnostics.push(parsed);
    }
    byUri.set(entry.uri, diagnostics);
    nextByServer.set(serverId, byUri);
  }

  const controller = diagnosticsController(editor);
  const knownServerIds = new Set<string>([
    ...controller.lspCollections.keys(),
    ...nextByServer.keys(),
    ...statuses.map((status) => status.id),
  ]);

  for (const serverId of knownServerIds) {
    let collection = controller.lspCollections.get(serverId);
    if (!collection) {
      collection = editor.diagnostics.createCollection({ id: `lsp:${serverId}`, title: serverId });
      controller.lspCollections.set(serverId, collection);
    }
    const next = nextByServer.get(serverId);
    collection.replace(next ? [...next.entries()].map(([uri, diagnostics]) => ({ uri, diagnostics })) : []);
  }
}

function diagnosticsController(editor: EditorHost): DiagnosticsControllerState {
  return editor.stores.host(defineHostStore<DiagnosticsControllerState>({
    id: diagnosticsControllerSlot,
    init: () => ({
      lspCollections: new Map<string, DiagnosticCollection>(),
      pendingPaths: new Set<string>(),
      pendingActiveRefresh: false,
    } satisfies DiagnosticsControllerState),
  }));
}

function scheduleDiagnosticsRefresh(editor: EditorHost, paths: readonly string[]): void {
  const controller = diagnosticsController(editor);
  const activePath = getDefaultState(editor).activePath;
  for (const path of paths) controller.pendingPaths.add(path);
  if (!paths.length || (activePath && paths.includes(activePath))) controller.pendingActiveRefresh = true;
  if (controller.refreshTimer) return;
  controller.refreshTimer = setTimeout(() => {
    controller.refreshTimer = undefined;
    const shouldRefreshSurface = controller.pendingActiveRefresh;
    controller.pendingActiveRefresh = false;
    controller.pendingPaths.clear();
    void (async () => {
      if (shouldRefreshSurface && getDefaultState(editor).activeSurfaceId) {
        await refreshActiveSurface(editor);
      }
      editor.workbench.invalidate("roles", "tools", "decorations", "gutters");
    })();
  }, 24);
}

function parseLspDiagnostic(value: JsonValue): DiagnosticInput | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const diagnostic = value as {
    range?: JsonValue;
    severity?: JsonValue;
    message?: JsonValue;
    source?: JsonValue;
    code?: JsonValue;
    codeDescription?: JsonValue;
    tags?: JsonValue;
    relatedInformation?: JsonValue;
    data?: JsonValue;
  };
  const range = parseLspRange(diagnostic.range);
  const message = typeof diagnostic.message === "string" ? diagnostic.message : "";
  if (!range || !message.trim()) return undefined;
  return {
    severity: lspSeverity(diagnostic.severity),
    range,
    message,
    source: typeof diagnostic.source === "string" ? diagnostic.source : undefined,
    code: typeof diagnostic.code === "string" || typeof diagnostic.code === "number" ? diagnostic.code : undefined,
    codeHref: parseLspCodeHref(diagnostic.codeDescription),
    tags: parseLspTags(diagnostic.tags),
    relatedInformation: parseLspRelatedInformation(diagnostic.relatedInformation),
    data: diagnostic.data,
  };
}

function parseLspRange(value: JsonValue | undefined): DiagnosticRange | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const range = value as { start?: JsonValue; end?: JsonValue };
  const start = parseLspPosition(range.start);
  const end = parseLspPosition(range.end);
  return start && end ? { start, end } : undefined;
}

function parseLspPosition(value: JsonValue | undefined): DiagnosticPosition | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const position = value as { line?: JsonValue; character?: JsonValue };
  return typeof position.line === "number" && typeof position.character === "number"
    ? { line: position.line, character: position.character }
    : undefined;
}

function parseLspCodeHref(value: JsonValue | undefined): string | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  return typeof (value as { href?: JsonValue }).href === "string" ? (value as { href: string }).href : undefined;
}

function parseLspTags(value: JsonValue | undefined): DiagnosticRecord["tags"] {
  if (!Array.isArray(value)) return [];
  const tags: DiagnosticRecord["tags"] = [];
  for (const tag of value) {
    if (tag === 1) tags.push("unnecessary");
    else if (tag === 2) tags.push("deprecated");
  }
  return [...new Set(tags)];
}

function parseLspRelatedInformation(value: JsonValue | undefined): DiagnosticRecord["relatedInformation"] {
  if (!Array.isArray(value)) return [];
  const related: DiagnosticRecord["relatedInformation"] = [];
  for (const entry of value) {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) continue;
    const info = entry as { location?: JsonValue; message?: JsonValue };
    if (typeof info.message !== "string") continue;
    const location = info.location;
    if (!location || typeof location !== "object" || Array.isArray(location)) continue;
    const uri = typeof (location as { uri?: JsonValue }).uri === "string" ? (location as { uri: string }).uri : undefined;
    const range = parseLspRange((location as { range?: JsonValue }).range);
    if (!uri || !range) continue;
    related.push({ uri, path: pathForFileUri(uri), range, message: info.message });
  }
  return related;
}

function lspSeverity(value: JsonValue | undefined): DiagnosticRecord["severity"] {
  switch (value) {
    case 1:
      return "error";
    case 2:
      return "warning";
    case 3:
      return "info";
    case 4:
      return "hint";
    default:
      return "warning";
  }
}

function filterDiagnostics(diagnostics: readonly DiagnosticRecord[], query: string): DiagnosticRecord[] {
  const trimmed = query.trim().toLowerCase();
  const sorted = [...diagnostics].sort(compareDiagnosticsBySeverityAndLocation);
  if (!trimmed) return sorted;
  return sorted.filter((diagnostic) => {
    const haystack = [
      diagnostic.message,
      diagnostic.source,
      diagnostic.path,
      diagnostic.code == null ? undefined : String(diagnostic.code),
      diagnosticSeverityLabel(diagnostic.severity),
    ].filter(Boolean).join("\n").toLowerCase();
    return haystack.includes(trimmed);
  });
}

function diagnosticPickerPage(diagnostics: readonly DiagnosticRecord[], title: string) {
  return {
    items: diagnostics.map((diagnostic) => ({
      id: `diagnostic:${diagnostic.id}`,
      title: diagnosticPrimaryLine(diagnostic),
      subtitle: diagnostic.path ? `${diagnostic.path}:${diagnostic.range.start.line + 1}:${diagnostic.range.start.character + 1}` : undefined,
      detail: `${diagnosticSeverityLetter(diagnostic.severity)}${diagnostic.source ? ` · ${diagnostic.source}` : ""}${diagnostic.code == null ? "" : ` · ${diagnostic.code}`}`,
      keywords: [diagnostic.message, diagnostic.source, diagnostic.path].filter((value): value is string => Boolean(value)),
      payload: {
        id: diagnostic.id,
        path: diagnostic.path,
        line: diagnostic.range.start.line + 1,
        column: diagnostic.range.start.character + 1,
        severity: diagnostic.severity,
        message: diagnostic.message,
      } satisfies DiagnosticPickerPayload,
    } satisfies PickerItem)),
    totalCount: diagnostics.length,
    status: {
      primary: title,
      secondary: diagnostics.length ? formatDiagnosticSummary(summarizeDiagnostics(diagnostics)) || `${diagnostics.length}` : "0",
    },
    emptyText: diagnostics.length ? "No matches." : "No diagnostics.",
  };
}

async function diagnosticPickerPreview(editor: EditorHost, item: PickerItem): Promise<PickerPreview> {
  const payload = item.payload as DiagnosticPickerPayload | undefined;
  if (!payload?.path) return { kind: "empty", title: item.title };
  const preview = await editor.runtime.search.readTextPreview({
    path: payload.path,
    maxBytes: 16_384,
    maxLines: 80,
    aroundLine: payload.line,
    contextLines: 10,
  });
  return {
    kind: "text",
    title: diagnosticPreviewTitle(payload, item.title),
    detail: `${payload.path}:${payload.line}:${payload.column}`,
    text: preview.text,
    startLine: preview.startLine,
    endLine: preview.endLine,
    truncated: preview.truncated,
    binary: preview.binary,
    rows: preview.rows,
    syntax: preview.syntax,
    activeLine: payload.line,
    annotation: payload.message ? { line: payload.line, severity: payload.severity, message: payload.message } : undefined,
  };
}

function diagnosticPreviewTitle(payload: DiagnosticPickerPayload, fallback: string): string {
  const prefix = payload.severity ? diagnosticSeverityLabel(payload.severity) : "Diagnostic";
  return `${prefix}: ${firstLine(payload.message ?? fallback)}`;
}

function firstLine(value: string): string {
  return value.split(/\r?\n/, 1)[0]?.trim() ?? value;
}

async function openDiagnosticItem(editor: EditorHost, payload: DiagnosticPickerPayload | undefined): Promise<void> {
  if (!payload?.path) return;
  await editor.commands.dispatch({
    id: commandId("file.open"),
    args: {
      path: payload.path,
      line: payload.line,
      column: payload.column,
    },
  });
}

async function navigateDiagnostic(editor: EditorHost, direction: 1 | -1): Promise<void> {
  const diagnostics = editor.diagnostics.all().filter((diagnostic) => diagnostic.path).sort(compareDiagnosticsByLocation);
  if (!diagnostics.length) {
    editor.notifications.info("diagnostics", "No diagnostics.");
    return;
  }

  const state = getDefaultState(editor);
  const activePath = state.activePath;
  const activeSurfaceId = state.activeSurfaceId;
  const currentPosition = activeSurfaceId ? await currentSurfaceDiagnosticPosition(editor, activeSurfaceId) : undefined;
  const hasCursorContext = Boolean(activePath && currentPosition);

  let target: DiagnosticRecord | undefined;
  if (hasCursorContext && activePath && currentPosition) {
    const ordered = direction === 1 ? diagnostics : [...diagnostics].reverse();
    target = ordered.find((diagnostic) => compareDiagnosticToCursor(diagnostic, activePath, currentPosition, direction) > 0);
    if (!target) target = ordered[0];
  } else {
    target = direction === 1 ? diagnostics[0] : diagnostics.at(-1);
  }

  if (!target?.path) return;
  await editor.commands.dispatch({
    id: commandId("file.open"),
    args: {
      path: target.path,
      line: target.range.start.line + 1,
      column: target.range.start.character + 1,
    },
  });
}

function compareDiagnosticToCursor(
  diagnostic: DiagnosticRecord,
  path: string,
  position: DiagnosticPosition,
  direction: 1 | -1,
): number {
  const pathCompare = (diagnostic.path ?? "").localeCompare(path);
  if (pathCompare !== 0) return direction === 1 ? pathCompare : -pathCompare;
  const lineCompare = diagnostic.range.start.line - position.line;
  if (lineCompare !== 0) return direction === 1 ? lineCompare : -lineCompare;
  const charCompare = diagnostic.range.start.character - position.character;
  return direction === 1 ? charCompare : -charCompare;
}

async function currentSurfaceDiagnosticPosition(editor: EditorHost, surfaceId: SurfaceId): Promise<DiagnosticPosition | undefined> {
  const surface = await editor.runtime.surfaces.readState({ surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  if (!cursor) return undefined;
  const byteOffset = cursorByteOffset(surface.text, cursor, surface.cursorSemantics);
  return positionAtByteOffset(surface.text, byteOffset);
}

function cursorByteOffset(text: string, cursor: SurfaceSelectionRange, semantics: CursorSemantics): number {
  if (semantics === "InsertionPoint") return cursor.head;
  if (cursor.head <= cursor.anchor) return cursor.head;
  return utf8ByteOffsetAtStringIndex(text, previousGraphemeIndex(text, stringIndexAtUtf8ByteOffset(text, cursor.head)));
}

function previousGraphemeIndex(text: string, index: number): number {
  if (index <= 0) return 0;
  if (typeof Intl !== "undefined" && "Segmenter" in Intl) {
    const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
    let previous = 0;
    for (const part of segmenter.segment(text)) {
      if (part.index >= index) break;
      previous = part.index;
    }
    return previous;
  }
  return index - 1;
}

function utf8ByteOffsetAtStringIndex(text: string, index: number): number {
  if (index <= 0) return 0;
  return new TextEncoder().encode(text.slice(0, index)).length;
}

function positionAtByteOffset(text: string, byteOffset: number): DiagnosticPosition {
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
  for (let i = 0; i < text.length; i += 1) {
    const code = text.charCodeAt(i);
    const next = code >= 0xd800 && code <= 0xdbff && i + 1 < text.length ? text.slice(i, i + 2) : text[i] ?? "";
    const length = new TextEncoder().encode(next).length;
    if (bytes + length > byteOffset) return i;
    bytes += length;
    if (next.length === 2) i += 1;
    if (bytes >= byteOffset) return i + 1;
  }
  return text.length;
}

function buildDiagnosticDecorations(
  frame: RenderFrame,
  diagnostics: readonly DiagnosticRecord[],
  theme: ResolvedThemeDefinition | undefined,
  presentation: DiagnosticPresentationState,
  text?: string,
): BufferDecoration[] {
  const wrappedLines = wrappedFrameLines(frame);
  const sourceLines = documentLines(text);
  const decorations: BufferDecoration[] = [];

  if (presentation.underlinesEnabled) {
    for (const diagnostic of diagnostics) {
      const style = diagnosticVisualStyle(diagnostic, theme);
      const segments = diagnosticWrappedSegments(diagnostic, wrappedLines, sourceLines);
      for (const segment of segments) {
        decorations.push({
          kind: "range",
          id: `diagnostic:${diagnostic.id}:${segment.surfaceRow}:${segment.startCol}:${segment.endCol}`,
          surfaceRow: segment.surfaceRow,
          startCol: segment.startCol,
          endCol: segment.endCol,
          underline: style.underline,
          strike: style.strike,
          opacity: style.opacity,
        });
      }
    }
  }

  if (presentation.inlineEnabled) {
    decorations.push(...buildInlineDiagnosticDecorations(wrappedLines, diagnostics, theme, presentation));
  }

  return decorations;
}

function buildInlineDiagnosticDecorations(
  wrappedLines: Map<number, WrappedFrameLine>,
  diagnostics: readonly DiagnosticRecord[],
  theme: ResolvedThemeDefinition | undefined,
  presentation: DiagnosticPresentationState,
): InlineTextDecoration[] {
  const byStartLine = new Map<number, DiagnosticRecord[]>();
  for (const diagnostic of diagnostics) {
    const existing = byStartLine.get(diagnostic.range.start.line);
    if (existing) existing.push(diagnostic);
    else byStartLine.set(diagnostic.range.start.line, [diagnostic]);
  }

  const resolvedTheme = theme ?? emptyTheme();
  const annotationColor = resolveThemeStyleLoose(resolvedTheme, "annotation", ["comment", "ui.text"])?.fg ?? "#7f848e";
  const decorations: InlineTextDecoration[] = [];

  for (const [docLine, lineDiagnostics] of byStartLine.entries()) {
    const wrapped = wrappedLines.get(docLine);
    const lastSegment = wrapped?.segments.at(-1);
    if (!lastSegment) continue;
    lineDiagnostics.sort(compareDiagnosticsBySeverityAndLocation);
    const primary = lineDiagnostics[0]!;
    const visual = diagnosticVisualStyle(primary, theme);
    const message = inlineDiagnosticMessage(primary, lineDiagnostics.length);
    if (!message) continue;
    const lineWidth = Math.max(0, lastSegment.endCol - lastSegment.startCol);
    const startCol = Math.max(lineWidth + presentation.inlinePaddingCols, presentation.inlineMinColumn);
    decorations.push({
      kind: "inlineText",
      id: `diagnostic:inline:bullet:${primary.id}:${lastSegment.surfaceRow}`,
      surfaceRow: lastSegment.surfaceRow,
      col: startCol,
      text: "• ",
      foreground: visual.gutterColor,
      opacity: 0.95,
    });
    decorations.push({
      kind: "inlineText",
      id: `diagnostic:inline:text:${primary.id}:${lastSegment.surfaceRow}`,
      surfaceRow: lastSegment.surfaceRow,
      col: startCol + 2,
      text: message,
      foreground: annotationColor,
      opacity: 0.82,
    });
  }

  return decorations;
}

function inlineDiagnosticMessage(primary: DiagnosticRecord, count: number): string {
  const line = diagnosticPrimaryLine(primary).replace(/\s+/g, " ").trim();
  if (!line) return count > 1 ? `(+${count - 1} more)` : "";
  const suffix = count > 1 ? ` (+${count - 1} more)` : "";
  const maxLength = 120;
  const text = `${line}${suffix}`;
  return text.length > maxLength ? `${text.slice(0, maxLength - 1).trimEnd()}…` : text;
}

function buildDiagnosticGutterLines(
  frame: RenderFrame,
  diagnostics: readonly DiagnosticRecord[],
  theme: ResolvedThemeDefinition | undefined,
): GutterLine[] {
  const wrappedLines = wrappedFrameLines(frame);
  const byLine = new Map<number, DiagnosticRecord[]>();
  for (const diagnostic of diagnostics) {
    for (let line = diagnostic.range.start.line; line <= diagnostic.range.end.line; line += 1) {
      const existing = byLine.get(line);
      if (existing) existing.push(diagnostic);
      else byLine.set(line, [diagnostic]);
    }
  }

  const lines: GutterLine[] = [];
  for (const [docLine, lineDiagnostics] of byLine.entries()) {
    const wrapped = wrappedLines.get(docLine);
    const startRow = wrapped?.segments[0]?.surfaceRow;
    if (startRow == null) continue;
    lineDiagnostics.sort(compareDiagnosticsBySeverityAndLocation);
    const primary = lineDiagnostics[0]!;
    const visual = diagnosticVisualStyle(primary, theme);
    lines.push({
      id: `diagnostic:gutter:${docLine}`,
      surfaceRow: startRow,
      kind: "marker",
      color: visual.gutterColor,
      priority: severityRank[primary.severity],
      title: lineDiagnostics.length > 1
        ? `${diagnosticPrimaryLine(primary)} (+${lineDiagnostics.length - 1} more)`
        : diagnosticPrimaryLine(primary),
    });
  }
  return lines;
}

function diagnosticVisualStyle(diagnostic: DiagnosticRecord, theme: ResolvedThemeDefinition | undefined) {
  const severityKey = `diagnostic.${diagnostic.severity}` as const;
  const resolvedTheme = theme ?? emptyTheme();
  const severityStyle = resolveThemeStyle(resolvedTheme, severityKey, ["diagnostic", "ui.text"]);
  const fallbackColor = fallbackSeverityColor(diagnostic.severity);
  const baseColor = severityStyle?.underline?.color ?? severityStyle?.fg ?? fallbackColor;
  const baseStyle = severityStyle?.underline?.style ?? "curl";

  if (diagnostic.tags.includes("deprecated")) {
    const deprecatedStyle = resolveThemeStyleLoose(resolvedTheme, "diagnostic.deprecated", severityKey);
    return {
      underline: { color: baseColor, style: normalizeStrokeStyle(baseStyle) },
      strike: { color: deprecatedStyle?.fg ?? baseColor, style: "solid" as const },
      gutterColor: baseColor,
      opacity: diagnostic.tags.includes("unnecessary") ? 0.68 : 1,
    };
  }

  if (diagnostic.tags.includes("unnecessary")) {
    const unnecessaryStyle = resolveThemeStyleLoose(resolvedTheme, "diagnostic.unnecessary", severityKey);
    return {
      underline: { color: unnecessaryStyle?.underline?.color ?? unnecessaryStyle?.fg ?? baseColor, style: "dotted" as const },
      strike: undefined,
      gutterColor: unnecessaryStyle?.fg ?? baseColor,
      opacity: 0.68,
    };
  }

  return {
    underline: { color: baseColor, style: normalizeStrokeStyle(baseStyle) },
    strike: undefined,
    gutterColor: baseColor,
    opacity: 1,
  };
}

function normalizeStrokeStyle(style: string | undefined): "solid" | "dotted" | "dashed" | "double" | "curl" {
  switch (style) {
    case "dotted":
    case "dashed":
    case "double":
    case "curl":
    case "solid":
      return style;
    default:
      return "curl";
  }
}

function fallbackSeverityColor(severity: DiagnosticRecord["severity"]): string {
  switch (severity) {
    case "error":
      return "#ff7b72";
    case "warning":
      return "#d29922";
    case "info":
      return "#6cb6ff";
    case "hint":
      return "#7ad1c2";
  }
}

interface WrappedFrameLine {
  text: string;
  totalCols: number;
  segments: Array<{ surfaceRow: number; startCol: number; endCol: number; text: string }>;
}

function wrappedFrameLines(frame: RenderFrame): Map<number, WrappedFrameLine> {
  const lines = new Map<number, WrappedFrameLine>();
  for (const row of frame.rows) {
    if (row.docLine == null) continue;
    const current = lines.get(row.docLine) ?? { text: "", totalCols: 0, segments: [] };
    const width = Math.max(0, measureDiagnosticTextSurfaceColumns(row.text, current.totalCols));
    current.segments.push({
      surfaceRow: row.surfaceRow,
      startCol: current.totalCols,
      endCol: current.totalCols + width,
      text: row.text,
    });
    current.totalCols += width;
    current.text += row.text;
    lines.set(row.docLine, current);
  }
  return lines;
}

function diagnosticWrappedSegments(
  diagnostic: DiagnosticRecord,
  wrappedLines: Map<number, WrappedFrameLine>,
  sourceLines: string[],
): Array<{ surfaceRow: number; startCol: number; endCol: number }> {
  const segments: Array<{ surfaceRow: number; startCol: number; endCol: number }> = [];
  for (let line = diagnostic.range.start.line; line <= diagnostic.range.end.line; line += 1) {
    const wrapped = wrappedLines.get(line);
    if (!wrapped) continue;
    const lineText = sourceLines[line] ?? wrapped.text;
    const startCell = line === diagnostic.range.start.line
      ? cellColumnAtUtf16(lineText, diagnostic.range.start.character)
      : 0;
    const rawEndCell = line === diagnostic.range.end.line
      ? cellColumnAtUtf16(lineText, diagnostic.range.end.character)
      : wrapped.totalCols;
    const endCell = Math.max(startCell + 1, rawEndCell || (wrapped.totalCols ? rawEndCell : 1));

    for (const segment of wrapped.segments) {
      const overlapStart = Math.max(startCell, segment.startCol);
      const overlapEnd = Math.min(endCell, Math.max(segment.startCol + 1, segment.endCol || segment.startCol + 1));
      if (overlapEnd <= overlapStart) continue;
      segments.push({
        surfaceRow: segment.surfaceRow,
        startCol: overlapStart - segment.startCol,
        endCol: overlapEnd - segment.startCol,
      });
    }
  }
  return segments;
}

function cellColumnAtUtf16(text: string, utf16Character: number, startCol = 0): number {
  if (utf16Character <= 0 || !text) return 0;
  let codeUnits = 0;
  let col = startCol;
  const segmenter = typeof Intl !== "undefined" && "Segmenter" in Intl
    ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
    : undefined;
  const graphemes = segmenter ? Array.from(segmenter.segment(text), (segment) => segment.segment) : Array.from(text);
  for (const grapheme of graphemes) {
    if (codeUnits >= utf16Character) break;
    codeUnits += grapheme.length;
    col += diagnosticGraphemeColumns(grapheme, col);
  }
  return col - startCol;
}

function summarizeDiagnostics(diagnostics: readonly DiagnosticRecord[]) {
  let errorCount = 0;
  let warningCount = 0;
  let infoCount = 0;
  let hintCount = 0;
  const files = new Set<string>();
  for (const diagnostic of diagnostics) {
    if (diagnostic.path) files.add(diagnostic.path);
    switch (diagnostic.severity) {
      case "error":
        errorCount += 1;
        break;
      case "warning":
        warningCount += 1;
        break;
      case "info":
        infoCount += 1;
        break;
      case "hint":
        hintCount += 1;
        break;
    }
  }
  return {
    errorCount,
    warningCount,
    infoCount,
    hintCount,
    totalCount: diagnostics.length,
    fileCount: files.size,
  };
}

function pathForFileUri(uri: string): string | undefined {
  if (!uri.startsWith("file://")) return undefined;
  try {
    return decodeURIComponent(new URL(uri).pathname);
  } catch {
    return undefined;
  }
}

interface DiagnosticPickerPayload {
  id: string;
  path?: string;
  line: number;
  column: number;
  severity?: DiagnosticRecord["severity"];
  message?: string;
}

function emptyTheme(): ResolvedThemeDefinition {
  return {
    id: "" as ResolvedThemeDefinition["id"],
    palette: {},
    styles: {},
  };
}

function measureDiagnosticTextSurfaceColumns(text: string, startCol = 0): number {
  if (!text) return 0;
  let col = startCol;
  const segmenter = typeof Intl !== "undefined" && "Segmenter" in Intl
    ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
    : undefined;
  const graphemes = segmenter ? Array.from(segmenter.segment(text), (segment) => segment.segment) : Array.from(text);
  for (const grapheme of graphemes) {
    col += diagnosticGraphemeColumns(grapheme, col);
  }
  return col - startCol;
}

function diagnosticGraphemeColumns(grapheme: string, col: number): number {
  if (grapheme !== "\t") return 1;
  return DIAGNOSTIC_TAB_WIDTH - (col % DIAGNOSTIC_TAB_WIDTH || 0);
}

function documentLines(text: string | undefined): string[] {
  return text ? text.split(/\r?\n/) : [];
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
