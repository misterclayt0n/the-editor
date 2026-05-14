import type { CursorSemantics, JsonValue, SurfaceId, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule, type LspServerStatus, defineHostStore } from "@the-editor/editor";
import { ensureLspForPath, fileUri } from "./lsp.js";
import { activeSurfacePane, getDefaultState } from "./state.js";

export interface SignatureHelpItemView {
  label: string;
  documentation?: string;
  activeParameter?: number;
  activeParameterRange?: { start: number; end: number };
}

export interface SignatureHelpRoleState {
  visible: boolean;
  surfaceId?: number;
  items?: SignatureHelpItemView[];
  activeSignature?: number;
}

interface SignatureHelpRequestContext {
  sequence: number;
  surfaceId: SurfaceId;
  path: string;
  cursorHead: number;
}

interface SignatureHelpControllerState {
  timer?: ReturnType<typeof setTimeout>;
  beforeDispatchMode?: string;
}

interface ParsedSignatureHelp {
  items: SignatureHelpItemView[];
  activeSignature: number;
}

type SignatureHelpTrigger =
  | { kind: "invoked" }
  | { kind: "triggerCharacter"; character: string; retrigger: boolean }
  | { kind: "contentChange" };

const signatureHelpRoleId = "signatureHelp";
const signatureHelpImplementationId = "default.signatureHelp";
const signatureHelpStateSlot = "default.signatureHelp.state";
const signatureHelpRequestSequenceSlot = "default.signatureHelp.requestSequence";
const signatureHelpControllerSlot = "default.signatureHelp.controller";
const insertDeleteCommands = new Set([
  commandId("delete_char_backward"),
  commandId("delete_char_forward"),
  commandId("delete_word_backward"),
  commandId("delete_word_forward"),
  commandId("kill_to_line_start"),
  commandId("kill_to_line_end"),
]);
const enterInsertCommands = new Set([
  commandId("insert_mode"),
  commandId("mode.insert"),
  commandId("append_mode"),
  commandId("insert_at_line_start"),
  commandId("insert_at_line_end"),
  commandId("open_below"),
  commandId("open_above"),
]);
const signatureTriggerCharacters = new Set(["(", ","]);
const signatureRetriggerCharacters = new Set([")", ",", "(", "<", ">" , "]", "["
]);

export function defaultSignatureHelp(): EditorModule {
  return (editor) => {
    editor.roles.setFallback(signatureHelpRoleId, signatureHelpImplementationId);

    const beforeDispatch = editor.hooks.tap("commands.beforeDispatch", {
      id: "default.signatureHelp.beforeDispatch",
      handler: async () => {
        signatureHelpController(editor).beforeDispatchMode = getDefaultState(editor).mode;
      },
    });

    const afterDispatch = editor.hooks.tap("commands.afterDispatch", {
      id: "default.signatureHelp.afterDispatch",
      handler: async ({ dispatch }: { dispatch: { id: string; args?: unknown } }) => {
        if (dispatch.id.startsWith("signatureHelp.")) return;

        const previousMode = signatureHelpController(editor).beforeDispatchMode;
        const currentMode = getDefaultState(editor).mode;
        const editorFocused = editor.input.current().kind === "editor";
        const active = isSignatureHelpOpen(editor);

        if (currentMode !== "insert" || !editorFocused) {
          hideSignatureHelp(editor, { restoreFocus: false });
          return;
        }

        if (dispatch.id === commandId("completion.accept")) {
          scheduleSignatureHelp(editor, { kind: "invoked" }, 20);
          return;
        }

        if (dispatch.id.startsWith("completion.")) return;

        if ((previousMode && previousMode !== "insert") || enterInsertCommands.has(dispatch.id as ReturnType<typeof commandId>)) {
          scheduleSignatureHelp(editor, { kind: "invoked" }, 20);
          return;
        }

        if (dispatch.id === commandId("edit.insertText")) {
          const text = typeof (dispatch.args as { text?: unknown } | undefined)?.text === "string"
            ? (dispatch.args as { text: string }).text
            : "";
          const char = text.length === 1 ? text : undefined;
          if (char && signatureTriggerCharacters.has(char)) {
            scheduleSignatureHelp(editor, { kind: "triggerCharacter", character: char, retrigger: active }, 20);
            return;
          }
          if (active) {
            if (char && signatureRetriggerCharacters.has(char)) {
              scheduleSignatureHelp(editor, { kind: "triggerCharacter", character: char, retrigger: true }, 80);
            } else {
              scheduleSignatureHelp(editor, { kind: "contentChange" }, 80);
            }
            return;
          }
          if (char && !isSymbolWordChar(char)) hideSignatureHelp(editor, { restoreFocus: false });
          return;
        }

        if (insertDeleteCommands.has(dispatch.id as ReturnType<typeof commandId>)) {
          if (active) scheduleSignatureHelp(editor, { kind: "contentChange" }, 80);
          return;
        }

        if (active) {
          scheduleSignatureHelp(editor, { kind: "contentChange" }, 20);
        }
      },
    });

    editor.roles.register<SignatureHelpRoleState>({
      role: signatureHelpRoleId,
      implementationId: signatureHelpImplementationId,
      close() {
        hideSignatureHelp(editor, { restoreFocus: true });
      },
      snapshot() {
        return editor.stores.host(defineHostStore<SignatureHelpRoleState>({ id: signatureHelpStateSlot, init: () => ({ visible: false }) }));
      },
    });

    editor.commands.register({
      id: commandId("signatureHelp.show"),
      title: "Show Signature Help",
      async run() {
        await showOrRefreshSignatureHelp(editor, { kind: "invoked" });
      },
    });

    editor.commands.register({
      id: commandId("signatureHelp.hide"),
      title: "Hide Signature Help",
      hidden: true,
      run() {
        hideSignatureHelp(editor, { restoreFocus: false });
      },
    });

    return {
      dispose() {
        beforeDispatch.dispose();
        afterDispatch.dispose();
        hideSignatureHelp(editor, { restoreFocus: false });
      },
    };
  };
}

export function hideSignatureHelp(editor: EditorHost, options: { restoreFocus?: boolean } = {}): void {
  const controller = signatureHelpController(editor);
  if (controller.timer) {
    clearTimeout(controller.timer);
    controller.timer = undefined;
  }
  bumpSignatureHelpRequestSequence(editor);
  const state = editor.stores.host(defineHostStore<SignatureHelpRoleState>({ id: signatureHelpStateSlot, init: () => ({ visible: false }) }));
  const visible = state.visible || editor.overlays.isOpen(signatureHelpRoleId);
  if (!visible) return;
  editor.stores.set(defineHostStore({ id: signatureHelpStateSlot, init: () => undefined }), { visible: false } satisfies SignatureHelpRoleState);
  editor.overlays.close(signatureHelpRoleId, { restoreFocus: options.restoreFocus ?? false });
  editor.workbench.invalidate("roles");
}

export function isSignatureHelpOpen(editor: EditorHost): boolean {
  return editor.overlays.isOpen(signatureHelpRoleId);
}

function signatureHelpController(editor: EditorHost): SignatureHelpControllerState {
  return editor.stores.host(defineHostStore({ id: signatureHelpControllerSlot, init: () => ({} satisfies SignatureHelpControllerState) }));
}

function scheduleSignatureHelp(editor: EditorHost, trigger: SignatureHelpTrigger, delayMs: number): void {
  const controller = signatureHelpController(editor);
  if (controller.timer) clearTimeout(controller.timer);
  controller.timer = setTimeout(() => {
    controller.timer = undefined;
    void showOrRefreshSignatureHelp(editor, trigger);
  }, delayMs);
}

async function showOrRefreshSignatureHelp(editor: EditorHost, trigger: SignatureHelpTrigger): Promise<void> {
  const controller = signatureHelpController(editor);
  if (controller.timer) {
    clearTimeout(controller.timer);
    controller.timer = undefined;
  }

  const request = await beginSignatureHelpRequest(editor);
  if (!request) {
    hideSignatureHelp(editor, { restoreFocus: false });
    return;
  }

  const signatureHelp = await resolveSignatureHelp(editor, request, trigger);
  if (!(await isCurrentSignatureHelpRequest(editor, request))) return;

  if (!signatureHelp?.items.length) {
    hideSignatureHelp(editor, { restoreFocus: false });
    return;
  }

  editor.stores.set(defineHostStore({ id: signatureHelpStateSlot, init: () => undefined }), { visible: true, ...signatureHelp } satisfies SignatureHelpRoleState);
  editor.overlays.open(signatureHelpRoleId, { focus: false, attachedToEditor: true });
  editor.workbench.invalidate("roles");
}

async function beginSignatureHelpRequest(editor: EditorHost): Promise<SignatureHelpRequestContext | undefined> {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId || !pane.path) return undefined;
  const sequence = bumpSignatureHelpRequestSequence(editor);
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

async function isCurrentSignatureHelpRequest(editor: EditorHost, request: SignatureHelpRequestContext): Promise<boolean> {
  if (currentSignatureHelpRequestSequence(editor) !== request.sequence) return false;
  const pane = activeSurfacePane(editor);
  if (!pane || pane.surfaceId !== request.surfaceId || pane.path !== request.path) return false;
  const surface = await editor.runtime.surfaces.readState({ surfaceId: request.surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  return cursor?.head === request.cursorHead;
}

async function resolveSignatureHelp(
  editor: EditorHost,
  request: SignatureHelpRequestContext,
  trigger: SignatureHelpTrigger,
): Promise<ParsedSignatureHelp & { surfaceId: SurfaceId } | undefined> {
  await ensureLspForPath(editor, request.path);
  const surface = await editor.runtime.surfaces.readState({ surfaceId: request.surfaceId });
  const cursor = surface.selection.find((range) => range.cursorId === surface.activeCursorId) ?? surface.selection[0];
  if (!cursor || cursor.head !== request.cursorHead) return undefined;
  const position = positionAtByteOffset(surface.text, signatureHelpByteOffset(surface.text, cursor, surface.cursorSemantics));
  const uri = fileUri(request.path);
  const serverIds = editor.lsp.serverIdsForPath(request.path);
  if (!serverIds.length) return undefined;

  const statuses = await editor.lsp.status();
  const running = statuses.filter((server) => serverIds.includes(server.id) && isRunning(server));
  if (!running.length) return undefined;

  for (const server of running) {
    try {
      const response = await editor.lsp.request({
        serverId: server.handle,
        method: "textDocument/signatureHelp",
        timeoutMs: 2500,
        params: buildSignatureHelpParams(uri, position, trigger, currentActiveSignatureHelp(editor)),
      });
      const parsed = parseSignatureHelpResponse(response);
      if (parsed && parsed.items.length) return { surfaceId: request.surfaceId, ...parsed };
    } catch {
      // Try the next running server. Signature help is opportunistic.
    }
  }
  return undefined;
}

function currentActiveSignatureHelp(editor: EditorHost): SignatureHelpRoleState | undefined {
  const state = editor.stores.get(defineHostStore<SignatureHelpRoleState | undefined>({ id: signatureHelpStateSlot, init: () => undefined as SignatureHelpRoleState | undefined }));
  return state?.visible ? state : undefined;
}

function buildSignatureHelpParams(
  uri: string,
  position: { line: number; character: number },
  trigger: SignatureHelpTrigger,
  active: SignatureHelpRoleState | undefined,
): JsonValue {
  const context: Record<string, JsonValue> = trigger.kind === "invoked"
    ? { triggerKind: 1, isRetrigger: false }
    : trigger.kind === "triggerCharacter"
      ? { triggerKind: 2, isRetrigger: trigger.retrigger, triggerCharacter: trigger.character }
      : { triggerKind: 3, isRetrigger: true };

  if (active?.items?.length) {
    const activeSignature = Math.max(0, Math.min(active.activeSignature ?? 0, active.items.length - 1));
    context.activeSignatureHelp = {
      signatures: active.items.map((item) => {
        const signature: Record<string, JsonValue> = { label: item.label };
        if (item.documentation) signature.documentation = item.documentation;
        if (item.activeParameter != null) signature.activeParameter = item.activeParameter;
        return signature;
      }),
      activeSignature,
      activeParameter: active.items[activeSignature]?.activeParameter ?? 0,
    };
  }

  return {
    textDocument: { uri },
    position,
    context,
  };
}

function currentSignatureHelpRequestSequence(editor: EditorHost): number {
  return editor.stores.host(defineHostStore<number>({ id: signatureHelpRequestSequenceSlot, init: () => 0 }));
}

function bumpSignatureHelpRequestSequence(editor: EditorHost): number {
  const next = currentSignatureHelpRequestSequence(editor) + 1;
  editor.stores.set(defineHostStore({ id: signatureHelpRequestSequenceSlot, init: () => undefined }), next);
  return next;
}

function isRunning(server: LspServerStatus): boolean {
  return server.state === "running" || server.state === "initialized";
}

function signatureHelpByteOffset(text: string, cursor: SurfaceSelectionRange, semantics: CursorSemantics): number {
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
      line += 1;
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
    if (next.length === 2) i += 1;
    if (bytes >= byteOffset) return i + 1;
  }
  return text.length;
}

function parseSignatureHelpResponse(value: JsonValue | undefined): ParsedSignatureHelp | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const result = value as {
    signatures?: JsonValue;
    activeSignature?: JsonValue;
    activeParameter?: JsonValue;
  };
  const rawSignatures = Array.isArray(result.signatures) ? result.signatures : [];
  if (!rawSignatures.length) return undefined;

  const activeSignature = clampNumber(result.activeSignature, 0, Math.max(0, rawSignatures.length - 1));
  const items = rawSignatures
    .map((signature) => toSignatureHelpItem(signature, asNumber(result.activeParameter)))
    .filter((item): item is SignatureHelpItemView => Boolean(item));
  if (!items.length) return undefined;

  return {
    items,
    activeSignature: clampNumber(activeSignature, 0, Math.max(0, items.length - 1)),
  };
}

function toSignatureHelpItem(value: JsonValue, fallbackActiveParameter: number | undefined): SignatureHelpItemView | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const signature = value as {
    label?: JsonValue;
    documentation?: JsonValue;
    activeParameter?: JsonValue;
    parameters?: JsonValue;
  };
  if (typeof signature.label !== "string" || !signature.label.trim()) return undefined;
  const activeParameter = asNumber(signature.activeParameter) ?? fallbackActiveParameter;
  const parameters = Array.isArray(signature.parameters) ? signature.parameters : [];
  const parameter = activeParameter != null && activeParameter >= 0 && activeParameter < parameters.length
    ? parameters[activeParameter]
    : undefined;
  return {
    label: signature.label,
    documentation: documentationToMarkdown(signature.documentation),
    activeParameter,
    activeParameterRange: signatureParameterRange(signature.label, parameter),
  };
}

function signatureParameterRange(
  signatureLabel: string,
  parameter: JsonValue | undefined,
): { start: number; end: number } | undefined {
  if (!parameter || typeof parameter !== "object" || Array.isArray(parameter)) return undefined;
  const label = (parameter as { label?: JsonValue }).label;
  if (Array.isArray(label) && label.length >= 2) {
    const start = asNumber(label[0]);
    const end = asNumber(label[1]);
    if (start != null && end != null && start >= 0 && end > start && end <= signatureLabel.length) {
      return { start, end };
    }
    return undefined;
  }
  if (typeof label !== "string" || !label) return undefined;
  const start = signatureLabel.indexOf(label);
  if (start < 0) return undefined;
  return { start, end: start + label.length };
}

function documentationToMarkdown(value: JsonValue | undefined): string | undefined {
  if (!value) return undefined;
  if (typeof value === "string") return value.trim() || undefined;
  if (Array.isArray(value)) return value.map(documentationToMarkdown).filter(Boolean).join("\n\n---\n\n") || undefined;
  if (typeof value !== "object") return undefined;
  const object = value as { kind?: JsonValue; value?: JsonValue; language?: JsonValue };
  if (typeof object.value === "string") {
    if (typeof object.language === "string" && object.language) return `\`\`\`${object.language}\n${object.value}\n\`\`\``;
    return object.value.trim() || undefined;
  }
  return undefined;
}

function asNumber(value: JsonValue | undefined): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function clampNumber(value: JsonValue | undefined, min: number, max: number): number {
  const number = asNumber(value) ?? min;
  return Math.min(max, Math.max(min, Math.floor(number)));
}

function isSymbolWordChar(char: string): boolean {
  return /[A-Za-z0-9_$]/.test(char);
}
