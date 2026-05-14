import { defineHostStore, type EditorHost } from "@the-editor/editor";
import type { SurfaceId, SurfaceSelectionRange, SurfaceState } from "@the-editor/contracts/runtime";
import { activeSurfacePane } from "./state.js";

interface SurfaceSnapshotStore {
  surfaces: Map<SurfaceId, SurfaceState>;
}

const surfaceSnapshotStore = defineHostStore<SurfaceSnapshotStore>({
  id: "default.surfaceSnapshots",
  init: () => ({ surfaces: new Map() }),
});

export interface CompletionSnapshotContext {
  surfaceId: SurfaceId;
  path?: string;
  languageId?: string;
  trigger: { kind: "manual" | "input" | "delete" | "triggerCharacter" | "retrigger" | "incomplete" | "accept"; character?: string };
  query: string;
  cursorOffset: number;
  replaceStart: number;
  replaceEnd: number;
  linePrefix: string;
  lineSuffix: string;
  text: string;
  revision: number;
  selection: SurfaceSelectionRange[];
  activeCursorId: number | null;
  cursorHead: number;
}

export function updateSurfaceSnapshot(editor: EditorHost, surfaceId: SurfaceId, state: SurfaceState): void {
  editor.stores.host(surfaceSnapshotStore).surfaces.set(surfaceId, state);
}

export function surfaceSnapshot(editor: EditorHost, surfaceId: SurfaceId): SurfaceState | undefined {
  return editor.stores.host(surfaceSnapshotStore).surfaces.get(surfaceId);
}

export function activeSurfaceSnapshot(editor: EditorHost): { surfaceId: SurfaceId; path?: string; state: SurfaceState } | undefined {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId) return undefined;
  const state = surfaceSnapshot(editor, pane.surfaceId);
  if (!state) return undefined;
  return { surfaceId: pane.surfaceId, path: pane.path, state };
}

export function completionContextFromSnapshot(
  editor: EditorHost,
  trigger: CompletionSnapshotContext["trigger"],
  languageIdForPath: (editor: EditorHost, path: string) => string,
): CompletionSnapshotContext | undefined {
  const active = activeSurfaceSnapshot(editor);
  if (!active) return undefined;
  const { surfaceId, path, state } = active;
  const cursor = state.selection.find((range) => range.cursorId === state.activeCursorId) ?? state.selection[0];
  if (!cursor) return undefined;
  const cursorHead = cursor.head;
  const index = stringIndexAtUtf8ByteOffset(state.text, cursorHead);
  const [lineStart, lineEnd] = lineBoundsAtStringIndex(state.text, index);
  const queryStartIndex = scanCompletionQueryStart(state.text, index);
  return {
    surfaceId,
    path,
    languageId: path ? languageIdForPath(editor, path) : undefined,
    trigger,
    query: state.text.slice(queryStartIndex, index),
    cursorOffset: cursorHead,
    replaceStart: utf8ByteLength(state.text.slice(0, queryStartIndex)),
    replaceEnd: cursorHead,
    linePrefix: state.text.slice(lineStart, index),
    lineSuffix: state.text.slice(index, lineEnd),
    text: state.text,
    revision: state.revision,
    selection: state.selection,
    activeCursorId: state.activeCursorId,
    cursorHead,
  };
}

export function scanCompletionQueryStart(text: string, end: number): number {
  let index = end;
  while (index > 0 && isCompletionQueryCharacter(text[index - 1] ?? "")) index -= 1;
  return index;
}

export function isCompletionQueryCharacter(char: string): boolean {
  return /[A-Za-z0-9_$]/.test(char);
}

export function stringIndexAtUtf8ByteOffset(text: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0;
  let bytes = 0;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    const char = code >= 0xd800 && code <= 0xdbff && index + 1 < text.length
      ? text.slice(index, index + 2)
      : text[index] ?? "";
    const length = utf8ByteLength(char);
    if (bytes + length > byteOffset) return index;
    bytes += length;
    if (char.length === 2) index += 1;
    if (bytes === byteOffset) return index + 1;
  }
  return text.length;
}

export function utf8ByteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

function lineBoundsAtStringIndex(text: string, index: number): [number, number] {
  const start = text.lastIndexOf("\n", Math.max(0, index - 1)) + 1;
  const end = text.indexOf("\n", index);
  return [start, end >= 0 ? end : text.length];
}
