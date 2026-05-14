import { commandId, type EditorHost, type EditorModule, type PickerItem, type PickerPreview } from "@the-editor/editor";
import type { JsonValue, SurfaceChange, SurfaceId, SurfaceSelectionRange, SurfaceState } from "@the-editor/contracts/runtime";
import { ensureLspForPath, fileUri } from "./lsp.js";
import { activeSurfacePane, getDefaultState, refreshActiveSurface } from "./state.js";
import { updateSurfaceSnapshot } from "./surfaceSnapshot.js";

const documentSymbolsProviderId = "default.lsp.documentSymbols";
const workspaceSymbolsProviderId = "default.lsp.workspaceSymbols";
const codeActionsProviderId = "default.lsp.codeActions";

type LspPosition = { line: number; character: number };
type LspRange = { start: LspPosition; end: LspPosition };
type LspLocation = { uri: string; range: LspRange };
type LspSymbol = { name: string; detail?: string; kind: number; containerName?: string; location?: LspLocation };
type CodeActionEntry = { id: string; serverHandle: number; title: string; kind?: string; isPreferred?: boolean; raw: JsonValue };


export function defaultLspSymbols(): EditorModule {
  return (editor) => {
    editor.commands.register({
      id: commandId("lsp_document_symbols"),
      title: "Document Symbols",
      category: "LSP",
      async run() { await editor.commands.dispatch({ id: commandId("picker.open"), args: { providerId: documentSymbolsProviderId } }); },
    });
    editor.commands.register({
      id: commandId("document_symbols"),
      title: "Document Symbols",
      category: "LSP",
      async run() { await editor.commands.dispatch({ id: commandId("lsp_document_symbols") }); },
    });
    editor.commands.register({
      id: commandId("lsp_workspace_symbols"),
      title: "Workspace Symbols",
      category: "LSP",
      async run() { await editor.commands.dispatch({ id: commandId("picker.open"), args: { providerId: workspaceSymbolsProviderId } }); },
    });
    editor.commands.register({
      id: commandId("workspace_symbols"),
      title: "Workspace Symbols",
      category: "LSP",
      async run() { await editor.commands.dispatch({ id: commandId("lsp_workspace_symbols") }); },
    });
    editor.commands.register({
      id: commandId("rename_symbol"),
      title: "Rename Symbol",
      category: "LSP",
      async run() { await editor.commands.dispatch({ id: commandId("input_prompt.open"), args: { kind: "renameSymbol" } }); },
    });
    editor.commands.register({
      id: commandId("select_references_to_symbol_under_cursor"),
      title: "Select References To Symbol Under Cursor",
      category: "LSP",
      async run() { await selectReferencesToSymbolUnderCursor(editor); },
    });
    editor.commands.register({
      id: commandId("lsp_code_actions"),
      title: "Code Actions",
      category: "LSP",
      async run() { await showCodeActions(editor); },
    });
    editor.commands.register({
      id: commandId("code_action"),
      title: "Code Actions",
      category: "LSP",
      async run() { await editor.commands.dispatch({ id: commandId("lsp_code_actions") }); },
    });

    editor.pickers.registerProvider({
      id: documentSymbolsProviderId,
      title: "Document Symbols",
      placeholder: "Filter document symbols",
      async createSession() {
        const path = getDefaultState(editor).activePath;
        const symbols = path ? await requestDocumentSymbols(editor, path) : [];
        return symbolPickerSession(editor, symbols, "Document symbols");
      },
    });

    editor.pickers.registerProvider({
      id: workspaceSymbolsProviderId,
      title: "Workspace Symbols",
      placeholder: "Search workspace symbols",
      async createSession() {
        let symbols: LspSymbol[] = [];
        let latest = "";
        return {
          async search(query) {
            latest = query;
            symbols = await requestWorkspaceSymbols(editor, query);
            return symbolPickerPage(filterSymbols(symbols, query), "Workspace symbols");
          },
          preview(item) { return symbolPreview(editor, item); },
          async accept(item) { await acceptSymbolItem(editor, item); },
          dispose() { void latest; },
        };
      },
    });

    editor.pickers.registerProvider({
      id: codeActionsProviderId,
      title: "Code Actions",
      placeholder: "Filter code actions",
      async createSession() {
        const items = await requestCodeActions(editor);
        return {
          search(query: string) {
            const normalized = query.trim().toLowerCase();
            const filtered = normalized
              ? items.filter((action) => [action.title, action.kind].filter(Boolean).join("\n").toLowerCase().includes(normalized))
              : items;
            return codeActionPickerPage(filtered);
          },
          async accept(item: PickerItem) {
            const action = item.payload as CodeActionEntry | undefined;
            if (action) await applyCodeAction(editor, action);
          },
        };
      },
    });
  };
}

function symbolPickerSession(editor: EditorHost, symbols: LspSymbol[], title: string) {
  return {
    search(query: string) { return symbolPickerPage(filterSymbols(symbols, query), title); },
    preview(item: PickerItem) { return symbolPreview(editor, item); },
    async accept(item: PickerItem) { await acceptSymbolItem(editor, item); },
  };
}

function symbolPickerPage(symbols: LspSymbol[], title: string) {
  return {
    items: symbols.map((symbol, index) => ({
      id: `symbol:${index}:${symbol.location?.uri ?? ""}:${symbol.name}`,
      title: symbol.name,
      subtitle: symbol.containerName,
      detail: [symbolKindName(symbol.kind), symbol.detail].filter(Boolean).join(" · "),
      keywords: [symbol.name, symbol.detail, symbol.containerName, symbolKindName(symbol.kind)].filter((v): v is string => Boolean(v)),
      payload: symbol,
    } satisfies PickerItem)),
    totalCount: symbols.length,
    status: { primary: title, secondary: String(symbols.length) },
    emptyText: "No symbols.",
  };
}

function filterSymbols(symbols: LspSymbol[], query: string): LspSymbol[] {
  const q = query.trim().toLowerCase();
  if (!q) return symbols;
  return symbols.filter((symbol) => [symbol.name, symbol.detail, symbol.containerName, symbolKindName(symbol.kind)].filter(Boolean).join("\n").toLowerCase().includes(q));
}

async function symbolPreview(_editor: EditorHost, item: PickerItem): Promise<PickerPreview | null> {
  const symbol = item.payload as LspSymbol | undefined;
  const location = symbol?.location;
  if (!location) return null;
  const path = pathForFileUri(location.uri);
  if (!path) return null;
  const line = location.range.start.line + 1;
  return {
    kind: "surface",
    title: symbol?.name,
    detail: `${path}:${line}:${location.range.start.character + 1}`,
    path,
    viewport: {
      firstRow: Math.max(0, line - 9),
      rowCount: 80,
      firstCol: 0,
      wrapWidth: 4096,
    },
    activeLine: line,
  } as PickerPreview;
}

async function acceptSymbolItem(editor: EditorHost, item: PickerItem): Promise<void> {
  const symbol = item.payload as LspSymbol | undefined;
  const location = symbol?.location;
  const path = location ? pathForFileUri(location.uri) : undefined;
  if (!path || !location) return;
  await editor.commands.dispatch({ id: commandId("save_selection") });
  await editor.commands.dispatch({ id: commandId("file.open"), args: { path, line: location.range.start.line + 1, column: location.range.start.character + 1 } });
}

async function requestDocumentSymbols(editor: EditorHost, path: string): Promise<LspSymbol[]> {
  await ensureLspForPath(editor, path);
  const uri = fileUri(path);
  const running = await runningServersForPath(editor, path);
  const out: LspSymbol[] = [];
  for (const server of running) {
    try { out.push(...parseDocumentSymbols(uri, await editor.lsp.request({ serverId: server.handle, method: "textDocument/documentSymbol", timeoutMs: 5000, params: { textDocument: { uri } } }))); } catch {}
  }
  return out;
}

async function requestWorkspaceSymbols(editor: EditorHost, query: string): Promise<LspSymbol[]> {
  const path = getDefaultState(editor).activePath;
  if (path) await ensureLspForPath(editor, path);
  const statuses = await editor.lsp.status();
  const running = statuses.filter((status) => status.state === "running" || status.state === "initialized");
  const out: LspSymbol[] = [];
  for (const server of running) {
    try { out.push(...parseWorkspaceSymbols(await editor.lsp.request({ serverId: server.handle, method: "workspace/symbol", timeoutMs: 5000, params: { query } }))); } catch {}
  }
  return out;
}

async function selectReferencesToSymbolUnderCursor(editor: EditorHost): Promise<void> {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId || !pane.path) return;
  await ensureLspForPath(editor, pane.path);
  const state = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  updateSurfaceSnapshot(editor, pane.surfaceId, state);
  const cursor = state.selection.find((range) => range.cursorId === state.activeCursorId) ?? state.selection[0];
  if (!cursor) return;
  const uri = fileUri(pane.path);
  const position = positionAtByteOffset(state.text, cursorByteOffsetForState(state, cursor));
  const running = await runningServersForPath(editor, pane.path);
  for (const server of running) {
    try {
      const response = await editor.lsp.request({ serverId: server.handle, method: "textDocument/documentHighlight", timeoutMs: 3000, params: { textDocument: { uri }, position } });
      const selection = parseDocumentHighlights(response, state.text);
      if (!selection.length) continue;
      await editor.commands.dispatch({ id: commandId("save_selection") });
      updateSurfaceSnapshot(editor, pane.surfaceId, await editor.runtime.surfaces.applyEdit({ surfaceId: pane.surfaceId, edit: { selection } }));
      updateSurfaceSnapshot(editor, pane.surfaceId, await editor.runtime.surfaces.setActiveCursor({ surfaceId: pane.surfaceId, cursorId: selection[0]?.cursorId ?? null }));
      await refreshActiveSurface(editor);
      editor.workbench.invalidate("frame");
      return;
    } catch {
      // Try the next server.
    }
  }
}

async function showCodeActions(editor: EditorHost): Promise<void> {
  await editor.commands.dispatch({ id: commandId("picker.open"), args: { providerId: codeActionsProviderId } });
}

async function requestCodeActions(editor: EditorHost): Promise<CodeActionEntry[]> {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId || !pane.path) {
    return [];
  }
  await ensureLspForPath(editor, pane.path);
  const surface = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  updateSurfaceSnapshot(editor, pane.surfaceId, surface);
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  if (!cursor) {
    return [];
  }
  const range = currentLineRange(surface, cursor);
  const uri = fileUri(pane.path);
  const diagnostics: JsonValue[] = editor.diagnostics.forPath(pane.path).filter((diagnostic) => rangesOverlap(diagnostic.range, range)).map((diagnostic) => ({
    range: {
      start: { line: diagnostic.range.start.line, character: diagnostic.range.start.character },
      end: { line: diagnostic.range.end.line, character: diagnostic.range.end.character },
    },
    severity: lspSeverity(diagnostic.severity),
    message: diagnostic.message,
    source: diagnostic.source ?? null,
    code: diagnostic.code ?? null,
    data: null,
  }));
  const running = await runningServersForPath(editor, pane.path);
  const items: CodeActionEntry[] = [];
  for (const server of running) {
    try {
      const params = { textDocument: { uri }, range, context: { diagnostics, only: null, triggerKind: 1 } };
      const response = await editor.lsp.request({ serverId: server.handle, method: "textDocument/codeAction", timeoutMs: 5000, params });
      const parsed = parseCodeActions(response, server.handle as unknown as number);
      items.push(...parsed);
    } catch {
    }
  }
  items.sort(compareCodeActions);
  return items;
}

function codeActionPickerPage(actions: CodeActionEntry[]) {
  return {
    items: actions.map((action) => ({
      id: action.id,
      title: action.title,
      subtitle: action.kind,
      detail: action.isPreferred ? "preferred" : undefined,
      keywords: [action.title, action.kind].filter((value): value is string => Boolean(value)),
      payload: action,
    } satisfies PickerItem)),
    totalCount: actions.length,
    status: { primary: "Code actions", secondary: String(actions.length) },
    emptyText: "No code actions.",
  };
}

async function applyCodeAction(editor: EditorHost, entry: CodeActionEntry): Promise<void> {
  let raw = entry.raw;
  if (isObject(raw) && (!raw.edit || !raw.command)) {
    try {
      raw = await editor.lsp.request({ serverId: entry.serverHandle as never, method: "codeAction/resolve", timeoutMs: 3000, params: raw });
    } catch {
    }
  }
  const object = asRecord(raw);
  if (!object) {
    return;
  }
  if (object.edit) {
    await applyWorkspaceEdit(editor, object.edit);
  }
  const command = asRecord(object.command) ?? (typeof object.command === "string" ? object : undefined);
  if (command && typeof command.command === "string") {
    await editor.lsp.notify({ serverId: entry.serverHandle as never, method: "workspace/executeCommand", params: { command: command.command, arguments: Array.isArray(command.arguments) ? command.arguments : [] } }).catch((error) => {
    });
  }
  await refreshActiveSurface(editor);
}

async function applyWorkspaceEdit(editor: EditorHost, edit: JsonValue): Promise<void> {
  const object = asRecord(edit);
  const documentChanges = Array.isArray(object?.documentChanges) ? object.documentChanges : undefined;
  if (documentChanges) {
    for (const change of documentChanges) {
      const changeObject = asRecord(change);
      const textDocument = asRecord(changeObject?.textDocument);
      const uri = typeof textDocument?.uri === "string" ? textDocument.uri : undefined;
      const edits = Array.isArray(changeObject?.edits) ? changeObject.edits : undefined;
      const path = uri ? pathForFileUri(uri) : undefined;
      if (!path || !edits) {
        continue;
      }
      await applyTextEditsToPath(editor, path, edits);
    }
    return;
  }

  const changes = asRecord(object?.changes);
  if (!changes) {
    return;
  }
  for (const [uri, edits] of Object.entries(changes)) {
    const path = pathForFileUri(uri);
    if (!path || !Array.isArray(edits)) continue;
    await applyTextEditsToPath(editor, path, edits);
  }
}

async function applyTextEditsToPath(editor: EditorHost, path: string, edits: JsonValue[]): Promise<void> {
  await editor.commands.dispatch({ id: commandId("file.open"), args: { path } });
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId) return;
  const state = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  updateSurfaceSnapshot(editor, pane.surfaceId, state);
  const changes = edits.map((edit) => lspTextEditToSurfaceChange(edit, state.text)).filter((change): change is SurfaceChange => Boolean(change)).sort((a, b) => b.start - a.start || b.end - a.end);
  if (changes.length) updateSurfaceSnapshot(editor, pane.surfaceId, await editor.runtime.surfaces.applyEdit({ surfaceId: pane.surfaceId, edit: { changes } }));
}

function lspTextEditToSurfaceChange(value: JsonValue, text: string): SurfaceChange | undefined {
  const object = asRecord(value);
  const range = parseRange(object?.range);
  const newText = typeof object?.newText === "string" ? object.newText : undefined;
  if (!range || newText === undefined) return undefined;
  return { start: utf8ByteOffsetAtStringIndex(text, utf16PositionToStringIndex(text, range.start)), end: utf8ByteOffsetAtStringIndex(text, utf16PositionToStringIndex(text, range.end)), text: newText };
}

async function runningServersForPath(editor: EditorHost, path: string) {
  const serverIds = editor.lsp.serverIdsForPath(path);
  const statuses = await editor.lsp.status();
  return statuses.filter((status) => serverIds.includes(status.id) && (status.state === "running" || status.state === "initialized"));
}

function parseDocumentSymbols(uri: string, value: JsonValue): LspSymbol[] {
  if (!Array.isArray(value)) return [];
  const out: LspSymbol[] = [];
  for (const entry of value) {
    if (isDocumentSymbol(entry)) flattenDocumentSymbol(uri, entry, undefined, out);
    else {
      const symbol = parseSymbolInformation(entry);
      if (symbol) out.push(symbol);
    }
  }
  return out;
}

function parseWorkspaceSymbols(value: JsonValue): LspSymbol[] {
  return Array.isArray(value) ? value.map(parseWorkspaceSymbol).filter((symbol): symbol is LspSymbol => Boolean(symbol)) : [];
}

function flattenDocumentSymbol(uri: string, value: Record<string, any>, containerName: string | undefined, out: LspSymbol[]): void {
  const range = parseRange(value.selectionRange) ?? parseRange(value.range);
  out.push({ name: value.name, detail: typeof value.detail === "string" ? value.detail : undefined, kind: typeof value.kind === "number" ? value.kind : 0, containerName, location: range ? { uri, range } : undefined });
  if (Array.isArray(value.children)) for (const child of value.children) if (isDocumentSymbol(child)) flattenDocumentSymbol(uri, child, value.name, out);
}

function parseSymbolInformation(value: JsonValue): LspSymbol | undefined {
  const object = asRecord(value);
  const location = parseLocation(object?.location);
  return object && typeof object.name === "string" ? { name: object.name, kind: typeof object.kind === "number" ? object.kind : 0, containerName: typeof object.containerName === "string" ? object.containerName : undefined, location } : undefined;
}

function parseWorkspaceSymbol(value: JsonValue): LspSymbol | undefined {
  const object = asRecord(value);
  if (!object || typeof object.name !== "string") return undefined;
  const locationObject = asRecord(object.location);
  const location = parseLocation(object.location) ?? (typeof locationObject?.uri === "string" ? { uri: locationObject.uri, range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } } } : undefined);
  return { name: object.name, kind: typeof object.kind === "number" ? object.kind : 0, containerName: typeof object.containerName === "string" ? object.containerName : undefined, location };
}

function parseCodeActions(value: JsonValue, serverHandle: number): CodeActionEntry[] {
  if (!Array.isArray(value)) return [];
  const out: CodeActionEntry[] = [];
  value.forEach((entry, index) => {
    const object = asRecord(entry);
    if (!object || typeof object.title !== "string") return;
    if (asRecord(object.disabled)) return;
    out.push({
      id: `codeAction:${serverHandle}:${index}:${object.title}`,
      serverHandle,
      title: object.title,
      kind: typeof object.kind === "string" ? object.kind : undefined,
      isPreferred: Boolean(object.isPreferred),
      raw: entry,
    });
  });
  return out;
}

function compareCodeActions(a: CodeActionEntry, b: CodeActionEntry): number {
  return codeActionCategory(a).localeCompare(codeActionCategory(b)) || Number(b.isPreferred) - Number(a.isPreferred) || a.title.localeCompare(b.title);
}
function codeActionCategory(a: CodeActionEntry): string { return a.kind?.split(".")[0] ?? "zz"; }

function isDocumentSymbol(value: JsonValue): value is Record<string, any> { const object = asRecord(value); return Boolean(object && typeof object.name === "string" && (object.selectionRange || object.range)); }
function parseLocation(value: JsonValue | undefined): LspLocation | undefined { const object = asRecord(value); const range = parseRange(object?.range); return object && typeof object.uri === "string" && range ? { uri: object.uri, range } : undefined; }
function parseRange(value: JsonValue | undefined): LspRange | undefined { const object = asRecord(value); const start = parsePosition(object?.start); const end = parsePosition(object?.end); return start && end ? { start, end } : undefined; }
function parsePosition(value: JsonValue | undefined): LspPosition | undefined { const object = asRecord(value); return object && typeof object.line === "number" && typeof object.character === "number" ? { line: object.line, character: object.character } : undefined; }
function asRecord(value: JsonValue | undefined): Record<string, any> | undefined { return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, any> : undefined; }
function isObject(value: JsonValue): value is Record<string, JsonValue> { return Boolean(value && typeof value === "object" && !Array.isArray(value)); }
function pathForFileUri(uri: string): string | undefined { if (!uri.startsWith("file://")) return undefined; try { return decodeURIComponent(new URL(uri).pathname); } catch { return undefined; } }

function parseDocumentHighlights(value: JsonValue, text: string): SurfaceSelectionRange[] {
  if (!Array.isArray(value)) return [];
  return value.map((entry, index) => {
    const range = parseRange(asRecord(entry)?.range);
    if (!range) return undefined;
    const anchor = utf8ByteOffsetAtStringIndex(text, utf16PositionToStringIndex(text, range.start));
    const head = utf8ByteOffsetAtStringIndex(text, utf16PositionToStringIndex(text, range.end));
    return { cursorId: index + 1, anchor, head } satisfies SurfaceSelectionRange;
  }).filter((range): range is SurfaceSelectionRange => Boolean(range));
}

function cursorByteOffsetForState(state: SurfaceState, cursor: SurfaceSelectionRange): number {
  if (state.cursorSemantics === "InsertionPoint") return cursor.head;
  if (cursor.head <= cursor.anchor) return cursor.head;
  return utf8ByteOffsetAtStringIndex(state.text, previousGraphemeIndex(state.text, stringIndexAtUtf8ByteOffset(state.text, cursor.head)));
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

function currentLineRange(state: SurfaceState, cursor: { head: number }): LspRange {
  const index = stringIndexAtUtf8ByteOffset(state.text, cursor.head);
  const lineStart = state.text.lastIndexOf("\n", Math.max(0, index - 1)) + 1;
  const nextNewline = state.text.indexOf("\n", index);
  const lineEnd = nextNewline < 0 ? state.text.length : nextNewline;
  return {
    start: positionAtByteOffset(state.text, utf8ByteOffsetAtStringIndex(state.text, lineStart)),
    end: positionAtByteOffset(state.text, utf8ByteOffsetAtStringIndex(state.text, lineEnd)),
  };
}
function positionAtByteOffset(text: string, byteOffset: number): LspPosition { const index = stringIndexAtUtf8ByteOffset(text, byteOffset); let line = 0; let lineStart = 0; for (let i = 0; i < index; i++) if (text.charCodeAt(i) === 10) { line++; lineStart = i + 1; } return { line, character: index - lineStart }; }
function rangesOverlap(a: { start: LspPosition; end: LspPosition }, b: LspRange): boolean { return comparePosition(a.start, b.end) <= 0 && comparePosition(b.start, a.end) <= 0; }
function comparePosition(a: LspPosition, b: LspPosition): number { return a.line - b.line || a.character - b.character; }
function lspSeverity(value: string | undefined): number | null { return value === "error" ? 1 : value === "warning" ? 2 : value === "info" ? 3 : value === "hint" ? 4 : null; }
function utf16PositionToStringIndex(text: string, position: LspPosition): number { let line = 0; let lineStart = 0; for (let i = 0; i < text.length && line < position.line; i++) if (text.charCodeAt(i) === 10) { line++; lineStart = i + 1; } return Math.min(lineStart + position.character, text.indexOf("\n", lineStart) < 0 ? text.length : text.indexOf("\n", lineStart)); }
function stringIndexAtUtf8ByteOffset(text: string, byteOffset: number): number { if (byteOffset <= 0) return 0; let bytes = 0; for (let i = 0; i < text.length; i++) { const code = text.charCodeAt(i); const next = code >= 0xd800 && code <= 0xdbff && i + 1 < text.length ? text.slice(i, i + 2) : text[i] ?? ""; const length = new TextEncoder().encode(next).length; if (bytes + length > byteOffset) return i; bytes += length; if (next.length === 2) i++; if (bytes >= byteOffset) return i + 1; } return text.length; }
function utf8ByteOffsetAtStringIndex(text: string, index: number): number { return index <= 0 ? 0 : new TextEncoder().encode(text.slice(0, index)).length; }
function symbolKindName(kind: number): string { return ["", "file", "module", "namespace", "package", "class", "method", "property", "field", "constructor", "enum", "interface", "function", "variable", "constant", "string", "number", "boolean", "array", "object", "key", "null", "enumMember", "struct", "event", "operator", "typeParameter"][kind] ?? "symbol"; }
