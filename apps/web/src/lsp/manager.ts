/**
 * The LSP manager: documents in, language intelligence out.
 *
 * One client per (server definition, workspace root); roots found by
 * marker walk-up through the FileSystemPort. Document sync is INCREMENTAL
 * when the server supports it (per-transaction change conversion through
 * DocumentSyncState — tiny messages, no debounce) and full-text on a
 * debounce otherwise. Diagnostics and inlay hints enter the editor as one
 * EditorVisuals contribution per document (underline decorations + virtual
 * text), so they ride the commit path's RangeSet mapping between publishes.
 * Server-initiated workspace edits become ordinary transactions on open
 * documents and read-modify-write through the FileSystemPort for closed
 * ones — undo, selections, and every extension's ranges stay coherent.
 */

import { Context, Effect, Exit, Fiber, Option, Scope, Stream } from "effect";
import {
  commitDocumentTransaction,
  DocumentInvalidated,
  DocumentNotifications,
  EditorVisuals,
  FileSystemPort,
  payloadsOf,
  type DocumentScope,
  type VisualsHandle,
} from "@the-editor/editor";
import {
  RangeSet,
  TextDocument,
  Transaction,
  type TextBuffer,
  type RangedValue,
} from "@the-editor/engine";
import {
  createByteConnection,
  DocumentSyncState,
  globMatches,
  lsp,
  LspClient,
  offsetAtPosition,
  pathToUri,
  uriToPath,
  type ByteStreams,
} from "@the-editor/lsp";
import type { DecorationSpec, VirtualTextSpec } from "@the-editor/editor";
import type { ProcessPort } from "../platform/process";
import type { LanguageServerDefinition, LanguageServers } from "../product/lsp";
import type {
  CompletionContext,
  CompletionItem,
  CompletionResult,
  HoverContext,
  HoverResult,
  SignatureContext,
  SignatureResult,
} from "../intel/types";

const FULL_SYNC_DEBOUNCE_MS = 150;
const INLAY_HINT_DEBOUNCE_MS = 400;
/** Whole-document hint requests are capped to keep payloads sane. */
const INLAY_HINT_MAX_CHARS = 200_000;
/** Don't respawn a server that just failed; wait this long. */
const SPAWN_RETRY_MS = 30_000;

export type DiagnosticCounts = { readonly errors: number; readonly warnings: number };

const severityStyle: Record<number, { className: string; color: string }> = {
  1: { className: "lsp-diagnostic-error", color: "#f48771" },
  2: { className: "lsp-diagnostic-warning", color: "#cca700" },
  3: { className: "lsp-diagnostic-info", color: "#75beff" },
  4: { className: "lsp-diagnostic-hint", color: "#8a8a8a" },
};

type ClientHandle = {
  readonly key: string;
  readonly definition: LanguageServerDefinition;
  readonly client: LspClient;
  readonly procId: number;
  ready: Promise<boolean>;
  alive: boolean;
};

type Attachment = {
  readonly docId: string;
  readonly uri: string;
  readonly path: string;
  readonly document: DocumentScope;
  readonly handle: ClientHandle;
  readonly visualsScope: Scope.CloseableScope;
  readonly visuals: VisualsHandle;
  readonly busFiber: Fiber.RuntimeFiber<unknown, unknown>;
  readonly sync: DocumentSyncState | null; // null = full-text sync
  /** Last version the server has (chain-gated incremental sync). */
  syncedVersion: number;
  fullSyncTimer: number | null;
  hintTimer: number | null;
  hintCancel: { cancel(): void } | null;
  diagnosticsSet: RangeSet<DecorationSpec>;
  hintsSet: RangeSet<VirtualTextSpec>;
  counts: DiagnosticCounts;
  /** Offset-resolved diagnostics from the last publish (hover lookup). */
  diagnosticList: Array<{
    from: number;
    to: number;
    message: string;
    severity: number;
    source: string | undefined;
  }>;
};

const positionToOffset = (buffer: TextBuffer, position: lsp.Position): number => {
  if (position.line >= buffer.lineCount) return buffer.length;
  const range = buffer.lineRange(Math.max(0, position.line));
  return Math.min(range.from + Math.max(0, position.character), range.to);
};

const offsetToPosition = (buffer: TextBuffer, offset: number): lsp.Position => {
  const clamped = Math.max(0, Math.min(offset, buffer.length));
  const range = buffer.lineRangeAt(clamped);
  return { line: range.index, character: clamped - range.from };
};

const completionKindNames: Record<number, string> = {
  1: "text", 2: "method", 3: "function", 4: "constructor", 5: "field",
  6: "variable", 7: "class", 8: "interface", 9: "module", 10: "property",
  11: "unit", 12: "value", 13: "enum", 14: "keyword", 15: "snippet",
  16: "color", 17: "file", 18: "reference", 19: "folder", 20: "enum-member",
  21: "constant", 22: "struct", 23: "event", 24: "operator", 25: "type-parameter",
};

const editsFromTextEdits = (
  buffer: TextBuffer,
  edits: ReadonlyArray<lsp.TextEdit> | undefined,
): Array<{ from: number; to: number; text: string }> =>
  (edits ?? []).map((edit) => ({
    from: positionToOffset(buffer, edit.range.start),
    to: positionToOffset(buffer, edit.range.end),
    text: edit.newText,
  }));

const markupToString = (
  value: lsp.MarkupContent | lsp.MarkedString | lsp.MarkedString[] | undefined,
): string => {
  if (value === undefined) return "";
  if (Array.isArray(value)) return value.map((v) => markupToString(v)).join("\n\n");
  if (typeof value === "string") return value;
  if ("kind" in value) return value.value;
  return "```" + value.language + "\n" + value.value + "\n```";
};

const hintLabel = (hint: lsp.InlayHint): string =>
  typeof hint.label === "string" ? hint.label : hint.label.map((part) => part.value).join("");

export class LspManager {
  private readonly clients = new Map<string, ClientHandle>();
  private readonly attachments = new Map<string, Attachment>();
  private readonly byUri = new Map<string, Attachment>();
  private readonly failedSpawns = new Map<string, number>();
  private disposed = false;
  private hintsEnabled = true;
  /** Host hook: diagnostics for a document changed (status surfaces). */
  onDiagnostics?: (docId: string) => void;

  constructor(
    private readonly port: ProcessPort,
    private readonly fileSystem: FileSystemPort.Service | undefined,
    private readonly servers: LanguageServers.Service,
  ) {}

  /** Attach a file-backed document; no matching server is a quiet no-op. */
  async attach(document: DocumentScope, path: string): Promise<void> {
    const docId = String(document.id);
    if (this.disposed || this.attachments.has(docId)) return;
    const definitions = Effect.runSync(this.servers.list);
    const extension = path.split(".").pop() ?? "";
    let matched: { definition: LanguageServerDefinition; languageId: string } | null = null;
    for (const definition of definitions) {
      const language = definition.languages.find((l) => l.fileTypes.includes(extension));
      if (language !== undefined) {
        matched = { definition, languageId: language.languageId };
        break;
      }
    }
    if (matched === null) return;

    const root = await this.findRoot(path, matched.definition.rootMarkers ?? []);
    const handle = await this.ensureClient(matched.definition, root);
    if (handle === null || this.disposed) return;
    if (!(await handle.ready)) return;
    if (this.attachments.has(docId)) return; // raced a second attach

    const context = document.context as Context.Context<
      TextDocument | DocumentNotifications | EditorVisuals
    >;
    const text = Context.get(context, TextDocument);
    const bus = Context.get(context, DocumentNotifications);
    const editorVisuals = Context.get(context, EditorVisuals);

    const uri = pathToUri(path);
    const snapshot = Effect.runSync(text.snapshot);
    const initialText = snapshot.buffer.sliceString(0, snapshot.buffer.length);
    handle.client.openDocument(uri, matched.languageId, snapshot.version, initialText);

    const incremental = handle.client.syncKind() === lsp.TextDocumentSyncKind.Incremental;
    const visualsScope = Effect.runSync(Scope.make());
    const visuals = Effect.runSync(
      editorVisuals.contribute("lsp", {}).pipe(Scope.extend(visualsScope)),
    );

    const attachment: Attachment = {
      docId,
      uri,
      path,
      document,
      handle,
      visualsScope,
      visuals,
      sync: incremental ? new DocumentSyncState(initialText) : null,
      syncedVersion: snapshot.version,
      busFiber: Effect.runFork(
        Stream.runForEach(bus.notifications, (notification) =>
          Effect.sync(() => {
            for (const payload of payloadsOf(notification, DocumentInvalidated)) {
              this.onDocumentChanged(attachment, text, payload.version, payload.changes);
            }
          }),
        ),
      ),
      fullSyncTimer: null,
      hintTimer: null,
      hintCancel: null,
      diagnosticsSet: RangeSet.empty(),
      hintsSet: RangeSet.empty(),
      counts: { errors: 0, warnings: 0 },
      diagnosticList: [],
    };
    this.attachments.set(docId, attachment);
    this.byUri.set(uri, attachment);
    this.scheduleHints(attachment, 0);
  }

  detach(docId: string): void {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined) return;
    this.attachments.delete(docId);
    this.byUri.delete(attachment.uri);
    if (attachment.fullSyncTimer !== null) window.clearTimeout(attachment.fullSyncTimer);
    if (attachment.hintTimer !== null) window.clearTimeout(attachment.hintTimer);
    attachment.hintCancel?.cancel();
    Effect.runFork(Fiber.interrupt(attachment.busFiber));
    Effect.runFork(Scope.close(attachment.visualsScope, Exit.void));
    if (attachment.handle.alive) attachment.handle.client.closeDocument(attachment.uri);
  }

  counts(docId: string): DiagnosticCounts | null {
    return this.attachments.get(docId)?.counts ?? null;
  }

  /** Host save hook: didSave + watched-files fan-out to interested servers. */
  documentSaved(docId: string): void {
    const attachment = this.attachments.get(docId);
    if (attachment !== undefined && attachment.handle.alive) {
      attachment.handle.client.saveDocument(attachment.uri);
    }
    const path = attachment?.path;
    if (path === undefined) return;
    for (const handle of this.clients.values()) {
      if (!handle.alive || !handle.client.isInitialized) continue;
      const matches = handle.client
        .watchedFilePatterns()
        .some((pattern) => globMatches(pattern, path));
      if (matches) {
        handle.client.notifyWatchedFiles([
          { uri: pathToUri(path), type: lsp.FileChangeType.Changed },
        ]);
      }
    }
  }

  toggleInlayHints(): void {
    this.hintsEnabled = !this.hintsEnabled;
    for (const attachment of this.attachments.values()) {
      if (this.hintsEnabled) {
        this.scheduleHints(attachment, 0);
      } else {
        attachment.hintsSet = RangeSet.empty();
        this.pushVisuals(attachment);
      }
    }
  }

  // -------------------------------------------------------------- features

  /** LSP completion as a generic intel result. */
  async completionAt(context: CompletionContext): Promise<CompletionResult | null> {
    const attachment = this.attachments.get(context.docId);
    if (attachment === undefined || !attachment.handle.alive) return null;
    if (!attachment.handle.client.supports(lsp.CompletionRequest.method)) return null;
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const triggerKind =
      context.trigger.kind === "triggerCharacter"
        ? lsp.CompletionTriggerKind.TriggerCharacter
        : context.trigger.kind === "incomplete"
          ? lsp.CompletionTriggerKind.TriggerForIncompleteCompletions
          : lsp.CompletionTriggerKind.Invoked;
    const result = await attachment.handle.client.completion(
      attachment.uri,
      offsetToPosition(buffer, context.offset),
      {
        triggerKind,
        ...(context.trigger.kind === "triggerCharacter"
          ? { triggerCharacter: context.trigger.character }
          : {}),
      },
    );
    const items: CompletionItem[] = result.items.map((item) => {
      const label = typeof item.label === "string" ? item.label : String(item.label);
      let insertText = item.insertText ?? label;
      let range: { from: number; to: number } | undefined;
      if (item.textEdit !== undefined) {
        insertText = item.textEdit.newText;
        const editRange =
          "range" in item.textEdit ? item.textEdit.range : item.textEdit.replace;
        range = {
          from: positionToOffset(buffer, editRange.start),
          to: positionToOffset(buffer, editRange.end),
        };
      }
      const isSnippet = item.insertTextFormat === lsp.InsertTextFormat.Snippet;
      const additionalEdits = editsFromTextEdits(buffer, item.additionalTextEdits);
      // Auto-imports and docs are usually LAZY (completionItem/resolve);
      // resolution offsets convert against the buffer as it is THEN.
      let command = item.command;
      const resolve = async () => {
        if (!attachment.handle.alive) return null;
        const resolved = await attachment.handle.client.resolveCompletionItem(item);
        if (resolved.command !== undefined) command = resolved.command;
        const liveBuffer = this.bufferOf(attachment);
        return {
          additionalEdits: editsFromTextEdits(liveBuffer, resolved.additionalTextEdits),
          ...(resolved.documentation !== undefined
            ? { documentation: markupToString(resolved.documentation) }
            : {}),
          ...(resolved.detail !== undefined ? { detail: resolved.detail } : {}),
        };
      };
      // Spec: the item's command runs AFTER the edits are applied; servers
      // that route auto-imports through it answer with workspace/applyEdit.
      const postAccept = async () => {
        if (command === undefined || !attachment.handle.alive) return;
        await attachment.handle.client.executeCommand(command.command, command.arguments);
      };
      return {
        label,
        insertText,
        ...(isSnippet ? { isSnippet: true } : {}),
        ...(additionalEdits.length > 0 ? { additionalEdits } : {}),
        resolve,
        postAccept,
        ...(range !== undefined ? { range } : {}),
        ...(item.kind !== undefined ? { kind: completionKindNames[item.kind] ?? "text" } : {}),
        ...(item.detail !== undefined ? { detail: item.detail } : {}),
        ...(item.documentation !== undefined
          ? { documentation: markupToString(item.documentation) }
          : {}),
        ...(item.filterText !== undefined ? { filterText: item.filterText } : {}),
        ...(item.sortText !== undefined ? { sortText: item.sortText } : {}),
        ...(item.tags?.includes(lsp.CompletionItemTag.Deprecated) ? { deprecated: true } : {}),
      };
    });
    return { items, isIncomplete: result.isIncomplete };
  }

  /** Diagnostics under an offset, formatted as a hover card. */
  diagnosticsHoverAt(context: HoverContext): HoverResult | null {
    const attachment = this.attachments.get(context.docId);
    if (attachment === undefined) return null;
    const hits = attachment.diagnosticList.filter(
      (d) => context.offset >= d.from && context.offset <= d.to,
    );
    if (hits.length === 0) return null;
    const contents = hits
      .map((d) => `${d.message}${d.source !== undefined ? `  (${d.source})` : ""}`)
      .join("\n\n");
    const from = Math.min(...hits.map((d) => d.from));
    const to = Math.max(...hits.map((d) => d.to));
    return { contents, range: { from, to }, kind: "diagnostic" };
  }

  /** LSP hover as a generic intel result. */
  async hoverAt(context: HoverContext): Promise<HoverResult | null> {
    const attachment = this.attachments.get(context.docId);
    if (attachment === undefined || !attachment.handle.alive) return null;
    if (!attachment.handle.client.supports(lsp.HoverRequest.method)) return null;
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const hover = await attachment.handle.client.hover(
      attachment.uri,
      offsetToPosition(buffer, context.offset),
    );
    if (hover === null) return null;
    const contents = markupToString(hover.contents);
    if (contents.trim().length === 0) return null;
    return {
      kind: "info" as const,
      contents,
      ...(hover.range !== undefined
        ? {
            range: {
              from: positionToOffset(buffer, hover.range.start),
              to: positionToOffset(buffer, hover.range.end),
            },
          }
        : {}),
    };
  }

  completionTriggerCharacters(docId: string): readonly string[] {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined || !attachment.handle.alive) return [];
    return attachment.handle.client.completionTriggerCharacters();
  }

  signatureTriggerCharacters(docId: string): readonly string[] {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined || !attachment.handle.alive) return [];
    return attachment.handle.client.signatureTriggerCharacters();
  }

  signatureRetriggerCharacters(docId: string): readonly string[] {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined || !attachment.handle.alive) return [];
    return attachment.handle.client.signatureRetriggerCharacters();
  }

  /** LSP signature help as a generic intel result. */
  async signatureHelpAt(context: SignatureContext): Promise<SignatureResult | null> {
    const attachment = this.attachments.get(context.docId);
    if (attachment === undefined || !attachment.handle.alive) return null;
    if (!attachment.handle.client.supports(lsp.SignatureHelpRequest.method)) return null;
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const triggerKind =
      context.trigger.kind === "triggerCharacter"
        ? lsp.SignatureHelpTriggerKind.TriggerCharacter
        : context.trigger.kind === "contentChange"
          ? lsp.SignatureHelpTriggerKind.ContentChange
          : lsp.SignatureHelpTriggerKind.Invoked;
    const help = await attachment.handle.client.signatureHelp(
      attachment.uri,
      offsetToPosition(buffer, context.offset),
      {
        triggerKind,
        isRetrigger: context.isRetrigger,
        ...(context.trigger.kind === "triggerCharacter"
          ? { triggerCharacter: context.trigger.character }
          : {}),
        ...(context.isRetrigger && context.active !== undefined
          ? {
              activeSignatureHelp: {
                signatures: context.active.signatures.map((signature) => ({
                  label: signature.label,
                  ...(signature.parameters !== undefined
                    ? {
                        parameters: signature.parameters.map((parameter) => ({
                          label: parameter.label as string | [number, number],
                        })),
                      }
                    : {}),
                })),
                ...(context.active.activeSignature !== undefined
                  ? { activeSignature: context.active.activeSignature }
                  : {}),
                ...(context.active.activeParameter !== undefined
                  ? { activeParameter: context.active.activeParameter }
                  : {}),
              },
            }
          : {}),
      },
    );
    if (help === null || help.signatures.length === 0) return null;
    return {
      signatures: help.signatures.map((signature) => ({
        label: signature.label,
        ...(signature.documentation !== undefined
          ? { documentation: markupToString(signature.documentation) }
          : {}),
        ...(signature.parameters !== undefined
          ? {
              parameters: signature.parameters.map((parameter) => ({
                label: parameter.label as string | readonly [number, number],
                ...(parameter.documentation !== undefined
                  ? { documentation: markupToString(parameter.documentation) }
                  : {}),
              })),
            }
          : {}),
        ...(signature.activeParameter !== undefined && signature.activeParameter !== null
          ? { activeParameter: signature.activeParameter }
          : {}),
      })),
      ...(help.activeSignature !== undefined ? { activeSignature: help.activeSignature } : {}),
      ...(help.activeParameter !== undefined && help.activeParameter !== null
        ? { activeParameter: help.activeParameter }
        : {}),
    };
  }

  async definition(docId: string, offset: number): Promise<lsp.Location[]> {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined || !attachment.handle.alive) return [];
    if (!attachment.handle.client.supports(lsp.DefinitionRequest.method)) return [];
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    return attachment.handle.client.definition(attachment.uri, offsetToPosition(buffer, offset));
  }

  async format(docId: string): Promise<boolean> {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined || !attachment.handle.alive) return false;
    if (!attachment.handle.client.supports(lsp.DocumentFormattingRequest.method)) return false;
    this.flushPendingSync(attachment);
    const edits = await attachment.handle.client.formatting(attachment.uri, {
      tabSize: 2,
      insertSpaces: true,
    });
    if (edits.length === 0) return false;
    this.applyEditsToOpenDocument(attachment, edits);
    return true;
  }

  async rename(docId: string, offset: number, newName: string): Promise<boolean> {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined || !attachment.handle.alive) return false;
    if (!attachment.handle.client.supports(lsp.RenameRequest.method)) return false;
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const edit = await attachment.handle.client.rename(
      attachment.uri,
      offsetToPosition(buffer, offset),
      newName,
    );
    if (edit === null) return false;
    const result = await this.applyWorkspaceEdit({ edit });
    return result.applied;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const docId of [...this.attachments.keys()]) this.detach(docId);
    for (const handle of this.clients.values()) {
      if (handle.alive) {
        void handle.client.shutdown().then(() => this.port.kill(handle.procId));
      }
    }
    this.clients.clear();
  }

  // ------------------------------------------------------------- documents

  private bufferOf(attachment: Attachment): TextBuffer {
    const text = Context.get(
      attachment.document.context as Context.Context<TextDocument>,
      TextDocument,
    );
    return Effect.runSync(text.get);
  }

  private onDocumentChanged(
    attachment: Attachment,
    text: TextDocument.Service,
    version: number,
    changes: ReadonlyArray<{ fromA: number; toA: number; fromB: number; toB: number }> | undefined,
  ): void {
    if (this.disposed || !attachment.handle.alive) return;
    // Incremental sync demands an INTACT chain and a FRESH notification:
    // the bus is sliding (slow consumers drop batches), and the inserted
    // text is sliced from the live buffer — valid only while the buffer is
    // still at the notification's version. Anything else resyncs fully.
    const snapshot =
      attachment.sync !== null && changes !== undefined
        ? Effect.runSync(text.snapshot)
        : null;
    if (
      attachment.sync !== null &&
      changes !== undefined &&
      snapshot !== null &&
      snapshot.version === version &&
      version === attachment.syncedVersion + 1
    ) {
      const edits = changes.map((change) => ({
        start: change.fromA,
        end: change.toA,
        text: snapshot.buffer.sliceString(change.fromB, change.toB),
      }));
      attachment.handle.client.changeDocumentIncremental(
        attachment.uri,
        version,
        attachment.sync.applyEdits(edits),
      );
      attachment.syncedVersion = version;
    } else {
      // Full sync (or unknown changes): debounce whole-text sends.
      if (attachment.fullSyncTimer !== null) window.clearTimeout(attachment.fullSyncTimer);
      attachment.fullSyncTimer = window.setTimeout(() => {
        attachment.fullSyncTimer = null;
        this.sendFullSync(attachment);
      }, FULL_SYNC_DEBOUNCE_MS);
    }
    this.scheduleHints(attachment, INLAY_HINT_DEBOUNCE_MS);
  }

  private sendFullSync(attachment: Attachment): void {
    if (this.disposed || !attachment.handle.alive) return;
    const text = Context.get(
      attachment.document.context as Context.Context<TextDocument>,
      TextDocument,
    );
    const current = Effect.runSync(text.snapshot);
    const fullText = current.buffer.sliceString(0, current.buffer.length);
    attachment.sync?.reset(fullText);
    attachment.handle.client.changeDocumentFull(attachment.uri, current.version, fullText);
    attachment.syncedVersion = current.version;
  }

  /**
   * Position-sensitive requests must not race a PENDING resync: a typing
   * burst trips the chain gate into the debounced full sync, and a request
   * issued in that window reads positions against the server's STALE
   * mirror (signature help asked about "(" the server hadn't seen yet).
   */
  private flushPendingSync(attachment: Attachment): void {
    if (attachment.fullSyncTimer === null) return;
    window.clearTimeout(attachment.fullSyncTimer);
    attachment.fullSyncTimer = null;
    this.sendFullSync(attachment);
  }

  // ----------------------------------------------------------- diagnostics

  private handleDiagnostics(params: lsp.PublishDiagnosticsParams): void {
    const attachment = this.byUri.get(params.uri);
    if (attachment === undefined) return;
    const buffer = this.bufferOf(attachment);
    const ranges = params.diagnostics.map((diagnostic) => {
      const from = positionToOffset(buffer, diagnostic.range.start);
      let to = positionToOffset(buffer, diagnostic.range.end);
      if (to <= from) to = Math.min(buffer.length, from + 1); // visible width
      const style = severityStyle[diagnostic.severity ?? 1] ?? severityStyle[1]!;
      return {
        from,
        to,
        value: {
          layer: "underline" as const,
          className: style.className,
          style: { borderBottomColor: style.color },
          priority: 10 - (diagnostic.severity ?? 1),
        },
      };
    });
    attachment.diagnosticsSet = RangeSet.of(ranges);
    attachment.diagnosticList = params.diagnostics.map((diagnostic) => {
      const from = positionToOffset(buffer, diagnostic.range.start);
      let to = positionToOffset(buffer, diagnostic.range.end);
      if (to <= from) to = Math.min(buffer.length, from + 1);
      return {
        from,
        to,
        message:
          typeof diagnostic.message === "string" ? diagnostic.message : diagnostic.message.value,
        severity: diagnostic.severity ?? 1,
        source: diagnostic.source ?? undefined,
      };
    });
    this.pushVisuals(attachment);
    attachment.counts = {
      errors: params.diagnostics.filter((d) => (d.severity ?? 1) === 1).length,
      warnings: params.diagnostics.filter((d) => d.severity === 2).length,
    };
    this.onDiagnostics?.(attachment.docId);
  }

  // ----------------------------------------------------------- inlay hints

  private scheduleHints(attachment: Attachment, delayMs: number): void {
    if (!this.hintsEnabled || this.disposed) return;
    if (!attachment.handle.client.supports(lsp.InlayHintRequest.method)) return;
    if (attachment.hintTimer !== null) window.clearTimeout(attachment.hintTimer);
    attachment.hintTimer = window.setTimeout(() => {
      attachment.hintTimer = null;
      void this.requestHints(attachment);
    }, delayMs);
  }

  private async requestHints(attachment: Attachment): Promise<void> {
    if (this.disposed || !attachment.handle.alive || !this.hintsEnabled) return;
    const buffer = this.bufferOf(attachment);
    if (buffer.length > INLAY_HINT_MAX_CHARS) return;
    attachment.hintCancel?.cancel();
    const source = attachment.handle.client.cancellationSource();
    attachment.hintCancel = source;
    let hints: lsp.InlayHint[];
    try {
      hints = await attachment.handle.client.inlayHints(
        attachment.uri,
        {
          start: { line: 0, character: 0 },
          end: offsetToPosition(buffer, buffer.length),
        },
        source.token,
      );
    } catch {
      return; // cancelled or server error; a newer request follows
    }
    if (attachment.hintCancel !== source || this.disposed) return;
    const current = this.bufferOf(attachment);
    const ranges: RangedValue<VirtualTextSpec>[] = hints.map((hint) => {
      const at = positionToOffset(current, hint.position);
      return {
        from: at,
        to: at,
        value: {
          text: `${hint.paddingLeft ? " " : ""}${hintLabel(hint)}${hint.paddingRight ? " " : ""}`,
          affinity: "before" as const,
          className: "lsp-inlay-hint",
        },
      };
    });
    attachment.hintsSet = RangeSet.of(ranges);
    this.pushVisuals(attachment);
  }

  private pushVisuals(attachment: Attachment): void {
    Effect.runSync(
      attachment.visuals.update({
        decorations: attachment.diagnosticsSet,
        virtualText: attachment.hintsSet,
      }),
    );
  }

  // -------------------------------------------------------- workspace edits

  /** TextEdits applied to an open document become a single transaction. */
  private applyEditsToOpenDocument(attachment: Attachment, edits: lsp.TextEdit[]): void {
    const buffer = this.bufferOf(attachment);
    const replacements = edits
      .map((edit) => ({
        from: positionToOffset(buffer, edit.range.start),
        to: positionToOffset(buffer, edit.range.end),
        text: edit.newText,
      }))
      .sort((a, b) => a.from - b.from);
    const transaction = Effect.runSync(
      Transaction.fromReplacements(buffer.length, replacements),
    );
    Effect.runSync(
      commitDocumentTransaction(transaction).pipe(
        Effect.provide(attachment.document.context as Context.Context<any>),
        Effect.orDie,
      ),
    );
  }

  /** Server-initiated edits: open docs via transactions, closed via disk. */
  async applyWorkspaceEdit(
    params: lsp.ApplyWorkspaceEditParams,
  ): Promise<lsp.ApplyWorkspaceEditResult> {
    const perUri = new Map<string, lsp.TextEdit[]>();
    if (params.edit.documentChanges !== undefined) {
      for (const change of params.edit.documentChanges) {
        if (!("textDocument" in change)) {
          return { applied: false, failureReason: "resource operations are not supported yet" };
        }
        const list = perUri.get(change.textDocument.uri) ?? [];
        list.push(...change.edits.filter((e): e is lsp.TextEdit => "newText" in e));
        perUri.set(change.textDocument.uri, list);
      }
    } else if (params.edit.changes !== undefined) {
      for (const [uri, edits] of Object.entries(params.edit.changes)) {
        perUri.set(uri, [...edits]);
      }
    }

    for (const [uri, edits] of perUri) {
      const attachment = this.byUri.get(uri);
      if (attachment !== undefined) {
        this.applyEditsToOpenDocument(attachment, edits);
        continue;
      }
      // Closed file: read–modify–write through the platform.
      const path = uriToPath(uri);
      if (path === null || this.fileSystem === undefined) {
        return { applied: false, failureReason: `cannot apply edits to ${uri}` };
      }
      try {
        const original = await Effect.runPromise(this.fileSystem.readFile(path));
        const sorted = edits
          .map((edit) => ({
            from: offsetAtPosition(original, edit.range.start),
            to: offsetAtPosition(original, edit.range.end),
            text: edit.newText,
          }))
          .sort((a, b) => b.from - a.from);
        let next = original;
        for (const edit of sorted) {
          next = next.slice(0, edit.from) + edit.text + next.slice(edit.to);
        }
        await Effect.runPromise(this.fileSystem.writeFile(path, next));
      } catch {
        return { applied: false, failureReason: `failed to edit ${path}` };
      }
    }
    return { applied: true };
  }

  // --------------------------------------------------------------- clients

  private async ensureClient(
    definition: LanguageServerDefinition,
    root: string,
  ): Promise<ClientHandle | null> {
    const key = `${definition.id}:${root}`;
    const existing = this.clients.get(key);
    if (existing !== undefined && existing.alive) return existing;

    const failedAt = this.failedSpawns.get(key);
    if (failedAt !== undefined && Date.now() - failedAt < SPAWN_RETRY_MS) return null;

    const procId = await this.port.spawn({
      command: definition.command,
      args: definition.args ?? [],
      cwd: root,
    });
    if (procId === null) {
      this.failedSpawns.set(key, Date.now());
      return null;
    }

    const chunkListeners = new Set<(chunk: Uint8Array) => void>();
    const closeListeners = new Set<() => void>();
    this.port.onData(procId, (chunk, stream) => {
      if (stream !== "stdout") return; // stderr is server log noise
      for (const listener of chunkListeners) listener(chunk);
    });
    this.port.onExit(procId, () => {
      for (const listener of closeListeners) listener();
    });
    const streams: ByteStreams = {
      send: (chunk) => this.port.write(procId, chunk),
      onChunk: (listener) => void chunkListeners.add(listener),
      onClose: (listener) => void closeListeners.add(listener),
    };

    const handle: ClientHandle = {
      key,
      definition,
      procId,
      alive: true,
      ready: Promise.resolve(false),
      client: new LspClient(createByteConnection(streams), {
        rootUri: pathToUri(root),
        ...(definition.initializationOptions !== undefined
          ? { initializationOptions: definition.initializationOptions }
          : {}),
        settings: () => definition.settings,
        onDiagnostics: (params) => this.handleDiagnostics(params),
        onApplyEdit: (params) => this.applyWorkspaceEdit(params),
        onInlayHintRefresh: () => {
          for (const attachment of this.attachments.values()) {
            if (attachment.handle === handle) this.scheduleHints(attachment, 0);
          }
        },
      }),
    };
    handle.ready = handle.client
      .initialize()
      .then(() => true)
      .catch(() => {
        this.failedSpawns.set(key, Date.now());
        handle.alive = false;
        return false;
      });
    closeListeners.add(() => {
      handle.alive = false;
      this.clients.delete(key);
      // Server died: clear its visuals so stale squiggles don't linger.
      for (const attachment of this.attachments.values()) {
        if (attachment.handle === handle) {
          attachment.diagnosticsSet = RangeSet.empty();
          attachment.hintsSet = RangeSet.empty();
          this.pushVisuals(attachment);
          attachment.counts = { errors: 0, warnings: 0 };
          this.onDiagnostics?.(attachment.docId);
        }
      }
    });
    this.clients.set(key, handle);
    return handle;
  }

  /** Nearest ancestor containing a root marker; falls back to the file dir. */
  private async findRoot(path: string, markers: readonly string[]): Promise<string> {
    const parent = (p: string) => p.slice(0, Math.max(1, p.lastIndexOf("/")));
    const fileDir = parent(path);
    if (this.fileSystem === undefined || markers.length === 0) return fileDir;
    let dir = fileDir;
    for (let depth = 0; depth < 32 && dir.length > 1; depth++) {
      for (const marker of markers) {
        const stat = await Effect.runPromise(
          this.fileSystem
            .stat(`${dir}/${marker}`)
            .pipe(Effect.catchAll(() => Effect.succeed(Option.none()))),
        );
        if (Option.isSome(stat)) return dir;
      }
      dir = parent(dir);
    }
    return fileDir;
  }
}
