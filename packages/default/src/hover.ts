import { commandId, type EditorHost, type EditorModule, type LspServerStatus, defineHostStore } from "@the-editor/editor";
import type { CursorSemantics, JsonValue, SurfaceId, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import { diagnosticMarkdownForPathPosition } from "./diagnostics.js";
import { ensureLspForPath, fileUri } from "./lsp.js";
import { activeSurfacePane } from "./state.js";

export interface HoverRoleState {
  visible: boolean;
  markdown?: string;
  source?: string;
  surfaceId?: number;
}

interface HoverRequestContext {
  sequence: number;
  surfaceId: SurfaceId;
  path: string;
  cursorHead: number;
}

const hoverRoleId = "hover";
const hoverImplementationId = "default.hover";
const hoverStateSlot = "default.hover.state";
const hoverRequestSequenceSlot = "default.hover.requestSequence";

export function defaultHover(): EditorModule {
  return (editor) => {
    editor.roles.setFallback(hoverRoleId, hoverImplementationId);
    const dismissOnCommand = editor.hooks.tap("commands.beforeDispatch", {
      id: "default.hover.dismissOnCommand",
      handler: async (dispatch: { id: string }) => {
        if (dispatch.id === commandId("hover.show") || dispatch.id === commandId("hover.hide")) return;
        dismissHover(editor, { restoreFocus: false });
      },
    });

    editor.roles.register<HoverRoleState>({
      role: hoverRoleId,
      implementationId: hoverImplementationId,
      close() {
        dismissHover(editor, { restoreFocus: true });
      },
      snapshot() {
        return editor.stores.host(defineHostStore<HoverRoleState>({ id: hoverStateSlot, init: () => ({ visible: false }) }));
      },
    });

    editor.commands.register({
      id: commandId("hover.show"),
      title: "Show Hover",
      async run() {
        const request = await beginHoverRequest(editor);
        if (!request) {
          dismissHover(editor, { restoreFocus: false });
          return;
        }

        const hover = await resolveHover(editor, request);
        if (!(await isCurrentHoverRequest(editor, request))) return;

        if (!hover?.markdown?.trim()) {
          dismissHover(editor, { restoreFocus: false });
          return;
        }

        editor.stores.set(defineHostStore({ id: hoverStateSlot, init: () => undefined }), { visible: true, ...hover } satisfies HoverRoleState);
        editor.overlays.open(hoverRoleId, { focus: false, attachedToEditor: true });
        editor.workbench.invalidate("roles");
      },
    });

    editor.commands.register({
      id: commandId("hover.hide"),
      title: "Hide Hover",
      run() {
        dismissHover(editor, { restoreFocus: true });
      },
    });

    return {
      dispose() {
        dismissOnCommand.dispose();
      },
    };
  };
}

export function dismissHover(editor: EditorHost, options: { restoreFocus?: boolean } = {}): void {
  bumpHoverRequestSequence(editor);
  const state = editor.stores.host(defineHostStore<HoverRoleState>({ id: hoverStateSlot, init: () => ({ visible: false }) }));
  const visible = state.visible || editor.overlays.isOpen(hoverRoleId);
  if (!visible) return;
  editor.stores.set(defineHostStore({ id: hoverStateSlot, init: () => undefined }), { visible: false } satisfies HoverRoleState);
  editor.overlays.close(hoverRoleId, { restoreFocus: options.restoreFocus ?? false });
  editor.workbench.invalidate("roles");
}

export function isHoverOpen(editor: EditorHost): boolean {
  return editor.overlays.isOpen(hoverRoleId);
}

async function beginHoverRequest(editor: EditorHost): Promise<HoverRequestContext | undefined> {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId || !pane.path) return undefined;
  const sequence = bumpHoverRequestSequence(editor);
  const surface = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  if (!cursor) return undefined;
  return {
    sequence,
    surfaceId: pane.surfaceId,
    path: pane.path,
    cursorHead: cursor.head,
  };
}

async function isCurrentHoverRequest(editor: EditorHost, request: HoverRequestContext): Promise<boolean> {
  if (currentHoverRequestSequence(editor) !== request.sequence) return false;
  const pane = activeSurfacePane(editor);
  if (!pane || pane.surfaceId !== request.surfaceId || pane.path !== request.path) return false;
  const surface = await editor.runtime.surfaces.readState({ surfaceId: request.surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  return cursor?.head === request.cursorHead;
}

async function resolveHover(editor: EditorHost, request: HoverRequestContext): Promise<{ markdown: string; source?: string; surfaceId?: number } | undefined> {
  await ensureLspForPath(editor, request.path);
  const surface = await editor.runtime.surfaces.readState({ surfaceId: request.surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  if (!cursor || cursor.head !== request.cursorHead) return undefined;
  const position = positionAtByteOffset(surface.text, hoverByteOffset(surface.text, cursor, surface.cursorSemantics));
  const uri = fileUri(request.path);
  const serverIds = editor.lsp.serverIdsForPath(request.path);
  if (!serverIds.length) return undefined;

  const statuses = await editor.lsp.status();
  const running = statuses.filter((server) => serverIds.includes(server.id) && isRunning(server));
  if (!running.length) return undefined;

  const diagnosticMarkdown = diagnosticMarkdownForPathPosition(editor, {
    path: request.path,
    line: position.line,
    character: position.character,
  });

  for (const server of running) {
    try {
      const response = await editor.lsp.request({
        serverId: server.handle,
        method: "textDocument/hover",
        timeoutMs: 2500,
        params: { textDocument: { uri }, position },
      });
      const hoverMarkdown = normalizeHoverMarkdown(hoverResponseToMarkdown(response), {
        source: server.id,
        path: request.path,
      });
      const markdown = [hoverMarkdown.trim(), diagnosticMarkdown?.trim()].filter(Boolean).join("\n\n---\n\n");
      if (markdown.trim()) return { markdown, source: server.id, surfaceId: request.surfaceId };
    } catch {
      // Try the next running server. Hover is opportunistic; status/log APIs expose failures.
    }
  }

  if (diagnosticMarkdown?.trim()) {
    return {
      markdown: diagnosticMarkdown,
      source: "diagnostics",
      surfaceId: request.surfaceId,
    };
  }
  return undefined;
}

function currentHoverRequestSequence(editor: EditorHost): number {
  return editor.stores.host(defineHostStore<number>({ id: hoverRequestSequenceSlot, init: () => 0 }));
}

function bumpHoverRequestSequence(editor: EditorHost): number {
  const next = currentHoverRequestSequence(editor) + 1;
  editor.stores.set(defineHostStore({ id: hoverRequestSequenceSlot, init: () => undefined }), next);
  return next;
}

function isRunning(server: LspServerStatus): boolean {
  return server.state === "running" || server.state === "initialized";
}

function hoverByteOffset(
  text: string,
  cursor: SurfaceSelectionRange,
  semantics: CursorSemantics,
): number {
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

function positionAtByteOffset(text: string, byteOffset: number): { line: number; character: number } {
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

function hoverResponseToMarkdown(value: JsonValue | undefined): string {
  if (!value || typeof value !== "object" || Array.isArray(value)) return "";
  return contentsToMarkdown((value as { contents?: JsonValue }).contents);
}

function contentsToMarkdown(contents: JsonValue | undefined): string {
  if (!contents) return "";
  if (typeof contents === "string") return contents;
  if (Array.isArray(contents)) return contents.map(contentsToMarkdown).filter(Boolean).join("\n\n---\n\n");
  if (typeof contents !== "object") return "";
  const object = contents as { kind?: JsonValue; value?: JsonValue; language?: JsonValue };
  if (typeof object.value === "string") {
    if (typeof object.language === "string" && object.language) return `\`\`\`${object.language}\n${object.value}\n\`\`\``;
    return object.value;
  }
  return "";
}

function normalizeHoverMarkdown(markdown: string, options: { source?: string; path?: string }): string {
  if (!markdown.trim()) return "";
  if (options.source === "clangd") return normalizeClangdHoverMarkdown(markdown, options.path);
  return markdown;
}

function normalizeClangdHoverMarkdown(markdown: string, path?: string): string {
  const trimmed = markdown.trim();
  if (!trimmed) return "";
  if (/```|^#{1,6}\s|\[[^\]]+\]\([^\)]+\)/m.test(trimmed)) return trimmed;

  const lines = trimmed.split(/\r?\n/);
  const output: string[] = [];
  let signature: string | undefined;

  const first = lines[0]?.trim();
  const headerMatch = first?.match(/^(function|method|macro|class|struct|enum|typedef)\s+([^\s]+)(?:\s+provided by\s+(.+))?$/i);
  if (headerMatch) {
    output.push(`### ${headerMatch[1]} \`${headerMatch[2]}\``);
    if (headerMatch[3]) output.push(`provided by \`${headerMatch[3]}\``);
    lines.shift();
  }

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) {
      if (output[output.length - 1] !== "") output.push("");
      continue;
    }

    const arrowMatch = line.match(/^→\s+(.+)$/);
    if (arrowMatch) {
      if (output.length && output[output.length - 1] !== "---") output.push("---", "");
      output.push(`→ \`${arrowMatch[1]}\``);
      continue;
    }

    if (/^[\w:\s\*&<>,.~\[\]-]+\(.*\)\s*;?$/.test(line) && !line.startsWith("- ")) {
      signature = line;
      continue;
    }

    const bulletMatch = line.match(/^-\s+(.+)$/);
    if (bulletMatch) {
      output.push(`- \`${bulletMatch[1]}\``);
      continue;
    }

    if (/^(Parameters|Returns?|Result|Template parameters):$/i.test(line)) {
      output.push(line);
      continue;
    }

    output.push(line);
  }

  while (output[output.length - 1] === "") output.pop();

  if (signature) {
    if (output.length) output.push("", "---", "");
    const language = codeFenceLanguageForPath(path);
    output.push(`\`\`\`${language}\n${signature}\n\`\`\``);
  }

  return output.join("\n").replace(/\n{3,}/g, "\n\n");
}

function codeFenceLanguageForPath(path: string | undefined): string {
  const lower = path?.toLowerCase() ?? "";
  if (lower.endsWith(".c") || lower.endsWith(".h")) return "c";
  if (lower.endsWith(".cc") || lower.endsWith(".cpp") || lower.endsWith(".cxx") || lower.endsWith(".hpp") || lower.endsWith(".hh") || lower.endsWith(".hxx")) return "cpp";
  if (lower.endsWith(".m")) return "objective-c";
  if (lower.endsWith(".mm")) return "objective-cpp";
  return "text";
}
