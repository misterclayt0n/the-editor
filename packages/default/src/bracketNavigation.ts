import type { DiagnosticRecord, EditorHost, EditorModule } from "@the-editor/editor";
import { commandId } from "@the-editor/editor";
import type { JsonValue, SurfaceId, SurfaceSelectionRange, SurfaceState } from "@the-editor/contracts/runtime";
import { ensureLspForPath, fileUri } from "./lsp.js";
import { activeSurfacePane, refreshActiveSurface } from "./state.js";

type Direction = "prev" | "next";
type Boundary = "first" | "last";
const objectKinds = ["function", "class", "parameter", "comment", "entry", "test", "xml_element"] as const;
type ObjectKind = typeof objectKinds[number];

export function defaultBracketNavigation(): EditorModule {
  return (editor) => {
    register(editor, "goto_prev_diag", () => gotoDiagnostic(editor, "prev"));
    register(editor, "goto_next_diag", () => gotoDiagnostic(editor, "next"));
    register(editor, "goto_first_diag", () => gotoDiagnosticBoundary(editor, "first"));
    register(editor, "goto_last_diag", () => gotoDiagnosticBoundary(editor, "last"));
    register(editor, "goto_prev_change", () => undefined);
    register(editor, "goto_next_change", () => undefined);
    register(editor, "goto_first_change", () => undefined);
    register(editor, "goto_last_change", () => undefined);
    for (const kind of objectKinds) {
      register(editor, `goto_prev_${kind}`, () => gotoObject(editor, kind, "prev"));
      register(editor, `goto_next_${kind}`, () => gotoObject(editor, kind, "next"));
    }
    register(editor, "goto_prev_paragraph", () => gotoParagraph(editor, "prev"));
    register(editor, "goto_next_paragraph", () => gotoParagraph(editor, "next"));
    register(editor, "add_newline_above", () => addNewline(editor, "above"));
    register(editor, "add_newline_below", () => addNewline(editor, "below"));
  };
}

function register(editor: EditorHost, id: string, run: () => Promise<void> | void): void {
  editor.commands.register({ id: commandId(id), title: id, category: "Navigation", async run() { await run(); } });
}

async function gotoDiagnostic(editor: EditorHost, direction: Direction): Promise<void> {
  const pane = activeSurfacePane(editor);
  if (!pane?.path || !pane.surfaceId) return;
  const state = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const cursor = activeCursorByteOffset(state);
  const ranges = diagnosticRanges(editor, pane.path, state.text);
  const target = direction === "next"
    ? ranges.find((r) => rangeStart(r) > cursor)
    : [...ranges].reverse().find((r) => rangeStart(r) < cursor);
  if (target) await selectWithJump(editor, pane.surfaceId, orientRange(target, direction));
}

async function gotoDiagnosticBoundary(editor: EditorHost, boundary: Boundary): Promise<void> {
  const pane = activeSurfacePane(editor);
  if (!pane?.path || !pane.surfaceId) return;
  const state = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const ranges = diagnosticRanges(editor, pane.path, state.text);
  const target = boundary === "first" ? ranges[0] : ranges.at(-1);
  if (target) await selectWithJump(editor, pane.surfaceId, target);
}

function diagnosticRanges(editor: EditorHost, path: string, text: string): SurfaceSelectionRange[] {
  return editor.diagnostics.all()
    .filter((d: DiagnosticRecord) => d.path === path)
    .sort((a, b) => a.range.start.line - b.range.start.line || a.range.start.character - b.range.start.character)
    .map((d, i) => ({ cursorId: i + 1, anchor: offsetAtPosition(text, d.range.start), head: offsetAtPosition(text, d.range.end) }));
}

async function gotoObject(editor: EditorHost, kind: ObjectKind, direction: Direction): Promise<void> {
  const pane = activeSurfacePane(editor);
  if (!pane?.path || !pane.surfaceId) return;
  const state = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const ranges = kind === "comment" ? commentRanges(state.text) : await symbolRanges(editor, pane.path, state.text, kind);
  const cursor = activeCursorByteOffset(state);
  const target = direction === "next"
    ? ranges.find((r) => rangeStart(r) > cursor)
    : [...ranges].reverse().find((r) => rangeStart(r) < cursor);
  if (target) await selectWithJump(editor, pane.surfaceId, orientRange(target, direction));
}

async function symbolRanges(editor: EditorHost, path: string, text: string, kind: ObjectKind): Promise<SurfaceSelectionRange[]> {
  if (kind === "parameter" || kind === "entry" || kind === "test" || kind === "xml_element") return [];
  await ensureLspForPath(editor, path);
  const server = (await runningServersForPath(editor, path))[0];
  if (!server) return fallbackSymbolRanges(text, kind);
  try {
    const result = await editor.lsp.request({ serverId: server.handle, method: "textDocument/documentSymbol", timeoutMs: 3000, params: { textDocument: { uri: fileUri(path) } } });
    const ranges = flattenSymbols(result).filter((s) => symbolMatchesKind(s, kind)).map((s, i) => expandObjectRange(text, { cursorId: i + 1, anchor: offsetAtPosition(text, s.range.start), head: offsetAtPosition(text, s.range.end) }, kind));
    return ranges.length ? ranges.sort((a, b) => a.anchor - b.anchor) : fallbackSymbolRanges(text, kind);
  } catch { return fallbackSymbolRanges(text, kind); }
}

function flattenSymbols(value: JsonValue): Array<{ kind?: number; range: { start: Position; end: Position }; name?: string }> {
  if (!Array.isArray(value)) return [];
  const out: Array<{ kind?: number; range: { start: Position; end: Position }; name?: string }> = [];
  const visit = (item: JsonValue) => {
    const o = asRecord(item); const range = parseRange(o?.range ?? (o?.location && asRecord(o.location)?.range));
    if (range) out.push({ kind: typeof o?.kind === "number" ? o.kind : undefined, range, name: typeof o?.name === "string" ? o.name : undefined });
    if (Array.isArray(o?.children)) for (const child of o.children) visit(child);
  };
  for (const item of value) visit(item);
  return out;
}

function symbolMatchesKind(symbol: { kind?: number; name?: string }, kind: ObjectKind): boolean {
  if (kind === "function") return symbol.kind === 12 || symbol.kind === 6 || symbol.kind === 5;
  if (kind === "class") return symbol.kind === 5 || symbol.kind === 23;
  return false;
}

function fallbackSymbolRanges(text: string, kind: ObjectKind): SurfaceSelectionRange[] {
  if (kind !== "function" && kind !== "class") return [];
  const re = kind === "function" ? /^\s*(?:func|function|def|fn)\b.*$/gm : /^\s*(?:class|struct|interface|type)\b.*$/gm;
  return [...text.matchAll(re)].map((m, i) => expandObjectRange(text, { cursorId: i + 1, anchor: utf8ByteOffsetAtStringIndex(text, m.index ?? 0), head: utf8ByteOffsetAtStringIndex(text, (m.index ?? 0) + m[0].length) }, kind));
}

function commentRanges(text: string): SurfaceSelectionRange[] {
  return [...text.matchAll(/\/\/.*|#.*|--.*|\/\*[\s\S]*?\*\//g)].map((m, i) => ({ cursorId: i + 1, anchor: utf8ByteOffsetAtStringIndex(text, m.index ?? 0), head: utf8ByteOffsetAtStringIndex(text, (m.index ?? 0) + m[0].length) }));
}

async function gotoParagraph(editor: EditorHost, direction: Direction): Promise<void> {
  const pane = activeSurfacePane(editor); if (!pane?.surfaceId) return;
  const state = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const cursor = activeCursorByteOffset(state); const index = stringIndexAtUtf8ByteOffset(state.text, cursor);
  const paras = paragraphRanges(state.text);
  const target = direction === "next"
    ? paras.find((r) => rangeStart(r) > cursor)
    : [...paras].reverse().find((r) => rangeStart(r) < cursor);
  if (target) await selectWithJump(editor, pane.surfaceId, orientRange(target, direction));
}

function paragraphRanges(text: string): SurfaceSelectionRange[] {
  const ranges: SurfaceSelectionRange[] = []; let start = 0; let id = 1;
  for (const match of text.matchAll(/\n\s*\n/g)) { const end = match.index ?? 0; if (end > start) ranges.push({ cursorId: id++, anchor: utf8ByteOffsetAtStringIndex(text, start), head: utf8ByteOffsetAtStringIndex(text, end) }); start = end + match[0].length; }
  if (start < text.length) ranges.push({ cursorId: id, anchor: utf8ByteOffsetAtStringIndex(text, start), head: utf8ByteOffsetAtStringIndex(text, text.length) });
  return ranges;
}

function rangeStart(range: SurfaceSelectionRange): number { return Math.min(range.anchor, range.head); }
function rangeEnd(range: SurfaceSelectionRange): number { return Math.max(range.anchor, range.head); }
function orientRange(range: SurfaceSelectionRange, direction: Direction): SurfaceSelectionRange {
  const start = rangeStart(range); const end = rangeEnd(range);
  return direction === "prev" ? { ...range, anchor: end, head: start } : { ...range, anchor: start, head: end };
}

function expandObjectRange(text: string, range: SurfaceSelectionRange, kind: ObjectKind): SurfaceSelectionRange {
  if (kind !== "function" && kind !== "class") return range;
  const startIndex = lineStartIndex(text, stringIndexAtUtf8ByteOffset(text, rangeStart(range)));
  const open = text.indexOf("{", stringIndexAtUtf8ByteOffset(text, rangeStart(range)));
  if (open < 0) return { ...range, anchor: utf8ByteOffsetAtStringIndex(text, startIndex) };
  const close = matchingBraceIndex(text, open);
  if (close < 0) return { ...range, anchor: utf8ByteOffsetAtStringIndex(text, startIndex) };
  return { ...range, anchor: utf8ByteOffsetAtStringIndex(text, startIndex), head: utf8ByteOffsetAtStringIndex(text, close + 1) };
}

function lineStartIndex(text: string, index: number): number { return text.lastIndexOf("\n", Math.max(0, index - 1)) + 1; }
function matchingBraceIndex(text: string, open: number): number {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    const ch = text[i];
    if (ch === "{") depth++;
    else if (ch === "}") { depth--; if (depth === 0) return i; }
  }
  return -1;
}

async function addNewline(editor: EditorHost, where: "above" | "below"): Promise<void> {
  const pane = activeSurfacePane(editor); if (!pane?.surfaceId) return;
  const state = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const cursor = activeCursorByteOffset(state); const index = stringIndexAtUtf8ByteOffset(state.text, cursor);
  const lineStart = state.text.lastIndexOf("\n", Math.max(0, index - 1)) + 1;
  const lineEndIndex = state.text.indexOf("\n", index); const lineEnd = lineEndIndex < 0 ? state.text.length : lineEndIndex;
  const insertIndex = where === "above" ? lineStart : lineEnd;
  const insertByte = utf8ByteOffsetAtStringIndex(state.text, insertIndex);
  const text = where === "above" ? "\n" : "\n";
  await editor.runtime.surfaces.applyEdit({ surfaceId: pane.surfaceId, edit: { changes: [{ start: insertByte, end: insertByte, text }] } });
  await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId: pane.surfaceId });
  await refreshActiveSurface(editor); editor.workbench.invalidate("frame");
}

async function selectWithJump(editor: EditorHost, surfaceId: SurfaceId, range: SurfaceSelectionRange): Promise<void> {
  await editor.commands.dispatch({ id: commandId("save_selection") });
  await editor.runtime.surfaces.applyEdit({ surfaceId, edit: { selection: [range] } });
  await editor.runtime.surfaces.setActiveCursor({ surfaceId, cursorId: range.cursorId });
  await refreshActiveSurface(editor); editor.workbench.invalidate("frame");
}

async function runningServersForPath(editor: EditorHost, path: string) { const ids = editor.lsp.serverIdsForPath(path); return (await editor.lsp.status()).filter((s) => ids.includes(s.id) && (s.state === "running" || s.state === "initialized")); }
type Position = { line: number; character: number };
function activeCursorByteOffset(state: SurfaceState): number { const c = state.selection.find((r) => r.cursorId === state.activeCursorId) ?? state.selection[0]; return c ? c.head : 0; }
function parseRange(value: unknown): { start: Position; end: Position } | undefined { const o = asRecord(value); const s = parsePos(o?.start); const e = parsePos(o?.end); return s && e ? { start: s, end: e } : undefined; }
function parsePos(value: unknown): Position | undefined { const o = asRecord(value); return typeof o?.line === "number" && typeof o?.character === "number" ? { line: o.line, character: o.character } : undefined; }
function asRecord(value: unknown): Record<string, any> | undefined { return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, any> : undefined; }
function offsetAtPosition(text: string, pos: Position): number { return utf8ByteOffsetAtStringIndex(text, utf16PositionToStringIndex(text, pos)); }
function utf16PositionToStringIndex(text: string, pos: Position): number { let line = 0, start = 0; for (let i = 0; i < text.length && line < pos.line; i++) if (text.charCodeAt(i) === 10) { line++; start = i + 1; } const end = text.indexOf("\n", start); return Math.min(start + pos.character, end < 0 ? text.length : end); }
function stringIndexAtUtf8ByteOffset(text: string, byteOffset: number): number { if (byteOffset <= 0) return 0; let bytes = 0; for (let i = 0; i < text.length; i++) { const code = text.charCodeAt(i); const next = code >= 0xd800 && code <= 0xdbff && i + 1 < text.length ? text.slice(i, i + 2) : text[i] ?? ""; const len = new TextEncoder().encode(next).length; if (bytes + len > byteOffset) return i; bytes += len; if (next.length === 2) i++; if (bytes >= byteOffset) return i + 1; } return text.length; }
function utf8ByteOffsetAtStringIndex(text: string, index: number): number { return index <= 0 ? 0 : new TextEncoder().encode(text.slice(0, index)).length; }
