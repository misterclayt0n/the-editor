import { commandId, type EditorHost, type EditorModule, type LspServerStatus } from "@the-editor/editor";
import type { CursorSemantics, JsonValue, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import { ensureLspForPath, fileUri } from "./lsp.js";
import { activeSurfacePane } from "./state.js";

interface LspPosition {
  line: number;
  character: number;
}

interface LspLocation {
  uri: string;
  range: {
    start: LspPosition;
    end: LspPosition;
  };
}

type GotoKind = "definition" | "declaration" | "typeDefinition" | "implementation" | "references";

const gotoDefinitions: Record<GotoKind, { commandIds: string[]; title: string; method: string }> = {
  definition: {
    commandIds: ["goto_definition", "lsp_goto_definition"],
    title: "Go to Definition",
    method: "textDocument/definition",
  },
  declaration: {
    commandIds: ["goto_declaration", "lsp_goto_declaration"],
    title: "Go to Declaration",
    method: "textDocument/declaration",
  },
  typeDefinition: {
    commandIds: ["goto_type_definition", "lsp_goto_type_definition"],
    title: "Go to Type Definition",
    method: "textDocument/typeDefinition",
  },
  implementation: {
    commandIds: ["goto_implementation", "lsp_goto_implementation"],
    title: "Go to Implementation",
    method: "textDocument/implementation",
  },
  references: {
    commandIds: ["goto_reference", "lsp_references"],
    title: "Go to Reference",
    method: "textDocument/references",
  },
};

export function defaultLspNavigation(): EditorModule {
  return (editor) => {
    for (const kind of Object.keys(gotoDefinitions) as GotoKind[]) {
      const definition = gotoDefinitions[kind];
      for (const id of definition.commandIds) {
        editor.commands.register<void, void>({
          id: commandId(id),
          title: definition.title,
          category: "LSP",
          keywords: ["lsp", "goto", kind],
          async run() {
            await gotoLspLocation(editor, kind);
          },
        });
      }
    }
  };
}

async function gotoLspLocation(editor: EditorHost, kind: GotoKind): Promise<void> {
  const definition = gotoDefinitions[kind];
  const request = await currentTextDocumentPosition(editor);
  if (!request) return;

  await ensureLspForPath(editor, request.path);

  const serverIds = editor.lsp.serverIdsForPath(request.path);
  if (!serverIds.length) return;

  const statuses = await editor.lsp.status();
  const running = statuses.filter((server) => serverIds.includes(server.id) && isRunning(server));
  if (!running.length) return;

  for (const server of running) {
    try {
      const response = await editor.lsp.request({
        serverId: server.handle,
        method: definition.method,
        timeoutMs: 5000,
        params: lspGotoParams(request, kind),
      });
      const locations = parseLocationsResponse(response);
      if (!locations.length) continue;
      await jumpToLocation(editor, locations);
      return;
    } catch {
      // Try the next running server. Status/log APIs expose transport details.
    }
  }

}

async function currentTextDocumentPosition(editor: EditorHost): Promise<{ path: string; uri: string; position: LspPosition } | undefined> {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId || !pane.path) return undefined;
  const surface = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  if (!cursor) return undefined;
  return {
    path: pane.path,
    uri: fileUri(pane.path),
    position: positionAtByteOffset(surface.text, lspCursorByteOffset(surface.text, cursor, surface.cursorSemantics)),
  };
}

function lspGotoParams(request: { uri: string; position: LspPosition }, kind: GotoKind): JsonValue {
  const params = {
    textDocument: { uri: request.uri },
    position: { line: request.position.line, character: request.position.character },
  };
  if (kind === "references") return { ...params, context: { includeDeclaration: false } };
  return params;
}

async function jumpToLocation(editor: EditorHost, locations: readonly LspLocation[]): Promise<void> {
  const location = locations[0];
  if (!location) return;
  const path = pathForFileUri(location.uri);
  if (!path) return;

  await editor.commands.dispatch({ id: commandId("save_selection") });
  await editor.commands.dispatch({
    id: commandId("file.open"),
    args: {
      path,
      line: location.range.start.line + 1,
      column: location.range.start.character + 1,
    },
  });

}

function parseLocationsResponse(value: JsonValue | undefined): LspLocation[] {
  if (value == null) return [];
  const single = parseLocation(value);
  if (single) return [single];
  if (!Array.isArray(value)) return [];
  return value.map((entry) => parseLocation(entry) ?? parseLocationLink(entry)).filter((entry): entry is LspLocation => Boolean(entry));
}

function parseLocation(value: JsonValue | undefined): LspLocation | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const object = value as { uri?: JsonValue; range?: JsonValue };
  const range = parseRange(object.range);
  return typeof object.uri === "string" && range ? { uri: object.uri, range } : undefined;
}

function parseLocationLink(value: JsonValue | undefined): LspLocation | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const object = value as { targetUri?: JsonValue; targetSelectionRange?: JsonValue; targetRange?: JsonValue };
  const range = parseRange(object.targetSelectionRange) ?? parseRange(object.targetRange);
  return typeof object.targetUri === "string" && range ? { uri: object.targetUri, range } : undefined;
}

function parseRange(value: JsonValue | undefined): LspLocation["range"] | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const object = value as { start?: JsonValue; end?: JsonValue };
  const start = parsePosition(object.start);
  const end = parsePosition(object.end);
  return start && end ? { start, end } : undefined;
}

function parsePosition(value: JsonValue | undefined): LspPosition | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const object = value as { line?: JsonValue; character?: JsonValue };
  return typeof object.line === "number" && typeof object.character === "number"
    ? { line: object.line, character: object.character }
    : undefined;
}

function pathForFileUri(uri: string): string | undefined {
  if (!uri.startsWith("file://")) return undefined;
  try {
    return decodeURIComponent(new URL(uri).pathname);
  } catch {
    return undefined;
  }
}

function isRunning(server: LspServerStatus): boolean {
  return server.state === "running" || server.state === "initialized";
}

function lspCursorByteOffset(text: string, cursor: SurfaceSelectionRange, semantics: CursorSemantics): number {
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

function positionAtByteOffset(text: string, byteOffset: number): LspPosition {
  const index = stringIndexAtUtf8ByteOffset(text, byteOffset);
  let line = 0;
  let lineStart = 0;
  for (let i = 0; i < index; i++) {
    if (text.charCodeAt(i) === 10) {
      line++;
      lineStart = i + 1;
    }
  }
  return { line, character: index - lineStart };
}

function stringIndexAtUtf8ByteOffset(text: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0;
  let bytes = 0;
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    const next = code >= 0xd800 && code <= 0xdbff && i + 1 < text.length ? text.slice(i, i + 2) : text[i] ?? "";
    const length = new TextEncoder().encode(next).length;
    if (bytes + length > byteOffset) return i;
    bytes += length;
    if (next.length === 2) i++;
    if (bytes >= byteOffset) return i + 1;
  }
  return text.length;
}

function utf8ByteOffsetAtStringIndex(text: string, index: number): number {
  if (index <= 0) return 0;
  return new TextEncoder().encode(text.slice(0, index)).length;
}
