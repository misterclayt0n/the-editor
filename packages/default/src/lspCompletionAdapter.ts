import type { JsonValue } from "@the-editor/contracts/runtime";
import type { CompletionContext, CompletionItem, CompletionSource, EditorHost } from "@the-editor/editor";
import { currentLspStatuses, ensureLspForPath, fileUri } from "./lsp.js";
import type { RuntimeCompletionContext } from "./completionQuery.js";

interface LineOffsetCache {
  text: string;
  starts: number[];
}

export function createLspCompletionSource(
  editor: EditorHost,
  options: {
    activeRequestSequence: () => number | undefined;
    log: (phase: string, data: Record<string, unknown>) => void;
  },
): CompletionSource {
  const { activeRequestSequence, log } = options;
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
      const serverIds = editor.lsp.serverIdsForPath(ctx.path);
      if (!serverIds.length) return null;
      if (ctx.trigger.kind === "manual") await ensureLspForPath(editor, ctx.path);
      const statuses = ctx.trigger.kind === "manual" ? await editor.lsp.status() : [...currentLspStatuses(editor)];
      const running = statuses.filter((status) => serverIds.includes(status.id)
        && (status.state === "running" || status.state === "initialized")
        && status.completionProvider !== false);
      if (!running.length) return null;
      if (ctx.trigger.kind === "triggerCharacter" && !running.some((status) => (status.completionTriggerCharacters ?? []).includes(ctx.trigger.character ?? ""))) {
        return null;
      }

      const position = positionAtByteOffset(ctx.text, ctx.cursorOffset);
      const uri = fileUri(ctx.path);
      const responses = await Promise.all(running.map(async (server) => {
        const startedAt = performance.now();
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
          const requestMs = elapsed(startedAt);
          const requestSequence = (ctx as RuntimeCompletionContext).completionRequestSequence;
          if (activeRequestSequence() !== requestSequence) {
            log("lsp stale before normalize", { serverId: server.id, requestMs, sequence: requestSequence });
            return { items: [], isIncomplete: false };
          }
          const normalizeStartedAt = performance.now();
          const normalized = normalizeLspCompletionResponse(response, ctx.text, server.id, server.handle);
          log("lsp response", { serverId: server.id, requestMs, normalizeMs: elapsed(normalizeStartedAt), items: normalized.items.length, isIncomplete: normalized.isIncomplete });
          return normalized;
        } catch (error) {
          log("lsp error", { serverId: server.id, ms: elapsed(startedAt), error: error instanceof Error ? error.message : String(error) });
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

export function materializeLazyLspCompletionItem(item: CompletionItem, text: string): Partial<CompletionItem> | undefined {
  const payload = item.payload as { raw?: JsonValue; serverHandle?: number } | undefined;
  if (!payload?.raw || !payload.serverHandle) return undefined;
  return normalizeResolvedLspCompletionItem(payload.raw, text, item, payload.serverHandle);
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

function utf8ByteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

function elapsed(start: number): number {
  return Math.round((performance.now() - start) * 10) / 10;
}

function isObject(value: unknown): value is Record<string, any> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): Record<string, any> | undefined {
  return isObject(value) ? value : undefined;
}
