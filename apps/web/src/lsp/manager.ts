/**
 * The LSP manager: documents in, language intelligence out.
 *
 * One client per (server definition, workspace root); roots found by
 * marker walk-up through the FileSystemPort. Document sync is INCREMENTAL
 * when the server supports it (per-transaction change conversion through
 * DocumentSyncState — tiny messages, no debounce) and full-text on a
 * debounce otherwise. Diagnostics and inlay hints enter the editor as TWO
 * separate EditorVisuals contributions per document (underline decorations
 * / virtual text), so each rides the commit path's RangeSet mapping and a
 * diagnostics publish can never re-apply stale hint coordinates (or vice
 * versa). Every push carries the diffed offset spans, and every response
 * is version-gated — see visualRanges.ts for the coordinate discipline.
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
import type { Processes, ProcessHandle } from "@the-editor/process";
import { chooseServerRoot } from "./roots";
import { changedSpans, mapOffsetThrough, mapRangedValues, sortRanges } from "./visualRanges";
import type { LanguageServerDefinition, LanguageServers } from "@the-editor/intel";
import type {
  CompletionContext,
  CompletionItem,
  CompletionResult,
  HoverContext,
  HoverResult,
  SignatureContext,
  SignatureResult,
} from "@the-editor/intel";

const FULL_SYNC_DEBOUNCE_MS = 150;
const INLAY_HINT_DEBOUNCE_MS = 400;
/** Whole-document hint requests are capped to keep payloads sane. */
const INLAY_HINT_MAX_CHARS = 200_000;
/** Don't respawn a server that just failed; wait this long. */
const SPAWN_RETRY_MS = 30_000;
/** Shut a server down this long after its last document detaches. */
const IDLE_CLIENT_GRACE_MS = 15_000;

export type DiagnosticCounts = { readonly errors: number; readonly warnings: number };

const severityStyle: Record<number, { className: string; color: string }> = {
  1: { className: "lsp-diagnostic-error", color: "var(--theme-diagnostic-error)" },
  2: { className: "lsp-diagnostic-warning", color: "var(--theme-diagnostic-warning)" },
  3: { className: "lsp-diagnostic-info", color: "var(--theme-diagnostic-info)" },
  4: { className: "lsp-diagnostic-hint", color: "var(--theme-diagnostic-hint)" },
};

type ClientHandle = {
  readonly key: string;
  readonly definition: LanguageServerDefinition;
  readonly client: LspClient;
  readonly proc: ProcessHandle;
  /** The spawn's scope; closing it kills the process (belt to `proc.kill`). */
  readonly scope: Scope.CloseableScope;
  ready: Promise<boolean>;
  alive: boolean;
};

/** Kill a client's process and release its scope. Idempotent. */
const closeClient = (handle: ClientHandle): void => {
  handle.proc.kill();
  void Effect.runPromise(Scope.close(handle.scope, Exit.void));
};

type DiagnosticEntry = {
  from: number;
  to: number;
  message: string;
  severity: number;
  source: string | undefined;
};

/**
 * One document's connection to ONE server. A document can have several
 * (helix's language-servers list): each keeps its own sync chain and its
 * own diagnostics; features route by priority order + helix only-/except-
 * features.
 */
type ServerBinding = {
  readonly handle: ClientHandle;
  readonly languageId: string;
  readonly only?: readonly string[];
  readonly except?: readonly string[];
  readonly sync: DocumentSyncState | null; // null = full-text sync
  /** Last version the server has (chain-gated incremental sync). */
  syncedVersion: number;
  fullSyncTimer: number | null;
  diagnosticRanges: RangedValue<DecorationSpec>[];
  diagnosticList: DiagnosticEntry[];
  counts: DiagnosticCounts;
};

/** Helix feature gating: only ⇒ allow-list, except ⇒ deny-list. */
const featureAllowed = (binding: ServerBinding, feature: string): boolean => {
  if (binding.only !== undefined && !binding.only.includes(feature)) return false;
  if (binding.except !== undefined && binding.except.includes(feature)) return false;
  return true;
};

type Attachment = {
  readonly docId: string;
  readonly uri: string;
  readonly path: string;
  readonly document: DocumentScope;
  /** Priority order — the first eligible binding serves each feature. */
  readonly bindings: ServerBinding[];
  readonly visualsScope: Scope.CloseableScope;
  /** Separate handles: a diagnostics push must never re-send hint
   *  coordinates (and vice versa) — the service maps each set through
   *  edits on its own. */
  readonly diagnosticsVisuals: VisualsHandle;
  readonly hintsVisuals: VisualsHandle;
  readonly busFiber: Fiber.RuntimeFiber<unknown, unknown>;
  hintTimer: number | null;
  hintCancel: { cancel(): void } | null;
  /** Last pushed hint ranges, kept in CURRENT document coordinates
   *  (mapped through every edit) — the diff base for localized pushes. */
  hintRanges: RangedValue<VirtualTextSpec>[];
  /** Last pushed merged diagnostics, current coordinates (diff base). */
  pushedDiagnostics: RangedValue<DecorationSpec>[];
  /** False after a dropped notification batch: offsets can no longer be
   *  trusted for diffing, so the next push invalidates the whole window. */
  hintsDiffBaseValid: boolean;
  diagnosticsDiffBaseValid: boolean;
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

// Value equality for push diffs (changedSpans): compare exactly what the
// renderer paints from each spec.
const decorationSpecEquals = (a: DecorationSpec, b: DecorationSpec): boolean =>
  a.layer === b.layer &&
  a.className === b.className &&
  a.priority === b.priority &&
  a.style?.["borderBottomColor"] === b.style?.["borderBottomColor"];

const virtualTextSpecEquals = (a: VirtualTextSpec, b: VirtualTextSpec): boolean =>
  a.text === b.text && a.className === b.className && a.affinity === b.affinity;

const completionKindNames: Record<number, string> = {
  1: "text",
  2: "method",
  3: "function",
  4: "constructor",
  5: "field",
  6: "variable",
  7: "class",
  8: "interface",
  9: "module",
  10: "property",
  11: "unit",
  12: "value",
  13: "enum",
  14: "keyword",
  15: "snippet",
  16: "color",
  17: "file",
  18: "reference",
  19: "folder",
  20: "enum-member",
  21: "constant",
  22: "struct",
  23: "event",
  24: "operator",
  25: "type-parameter",
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
  private enabled = true;
  private hintsEnabled = true;
  /** Pending idle-server sweep (debounced after a detach). */
  private reapTimer: number | null = null;
  /** Host hook: diagnostics for a document changed (status surfaces). */
  onDiagnostics?: (docId: string) => void;

  constructor(
    private readonly processes: Processes.Service,
    private readonly fileSystem: FileSystemPort.Service | undefined,
    private readonly servers: LanguageServers.Service,
    private readonly workspaceRoot: () => string | null = () => null,
  ) {}

  /** Attach a file-backed document; no matching server is a quiet no-op. */
  async attach(document: DocumentScope, path: string): Promise<void> {
    const docId = String(document.id);
    if (this.disposed || !this.enabled || this.attachments.has(docId)) return;
    const definitions = Effect.runSync(this.servers.list);
    const extension = path.split(".").pop() ?? "";
    // EVERY matching server attaches (helix's language-servers list);
    // per-language priority decides feature routing order.
    type Candidate = {
      definition: LanguageServerDefinition;
      languageId: string;
      priority: number;
      only?: readonly string[];
      except?: readonly string[];
    };
    const candidates: Candidate[] = [];
    for (const definition of definitions) {
      const language = definition.languages.find((l) => l.fileTypes.includes(extension));
      if (language === undefined) continue;
      if (candidates.some((c) => c.definition.id === definition.id)) continue;
      candidates.push({
        definition,
        languageId: language.languageId,
        priority: language.priority ?? 0,
        ...(language.only !== undefined ? { only: language.only } : {}),
        ...(language.except !== undefined ? { except: language.except } : {}),
      });
    }
    if (candidates.length === 0) return;
    candidates.sort((a, b) => a.priority - b.priority);

    const handles = await Promise.all(
      candidates.map(async (candidate) => {
        const markerRoot = await this.findRoot(path, candidate.definition.rootMarkers ?? []);
        const root = chooseServerRoot(path, this.workspaceRoot(), markerRoot);
        const handle = await this.ensureClient(candidate.definition, root);
        if (handle === null || !(await handle.ready)) return null;
        return { candidate, handle };
      }),
    );
    const ready = handles.filter((entry) => entry !== null);
    if (ready.length === 0 || this.disposed) return;
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

    const bindings: ServerBinding[] = ready.map(({ candidate, handle }) => {
      handle.client.openDocument(uri, candidate.languageId, snapshot.version, initialText);
      const incremental = handle.client.syncKind() === lsp.TextDocumentSyncKind.Incremental;
      return {
        handle,
        languageId: candidate.languageId,
        ...(candidate.only !== undefined ? { only: candidate.only } : {}),
        ...(candidate.except !== undefined ? { except: candidate.except } : {}),
        sync: incremental ? new DocumentSyncState(initialText) : null,
        syncedVersion: snapshot.version,
        fullSyncTimer: null,
        diagnosticRanges: [],
        diagnosticList: [],
        counts: { errors: 0, warnings: 0 },
      };
    });

    const visualsScope = Effect.runSync(Scope.make());
    const diagnosticsVisuals = Effect.runSync(
      editorVisuals.contribute("lsp", {}).pipe(Scope.extend(visualsScope)),
    );
    const hintsVisuals = Effect.runSync(
      editorVisuals.contribute("lsp-hints", {}).pipe(Scope.extend(visualsScope)),
    );

    const attachment: Attachment = {
      docId,
      uri,
      path,
      document,
      bindings,
      visualsScope,
      diagnosticsVisuals,
      hintsVisuals,
      busFiber: Effect.runFork(
        Stream.runForEach(bus.notifications, (notification) =>
          Effect.sync(() => {
            for (const payload of payloadsOf(notification, DocumentInvalidated)) {
              this.onDocumentChanged(attachment, text, payload.version, payload.changes);
            }
          }),
        ),
      ),
      hintTimer: null,
      hintCancel: null,
      hintRanges: [],
      pushedDiagnostics: [],
      hintsDiffBaseValid: true,
      diagnosticsDiffBaseValid: true,
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
    for (const binding of attachment.bindings) {
      if (binding.fullSyncTimer !== null) window.clearTimeout(binding.fullSyncTimer);
      if (binding.handle.alive) binding.handle.client.closeDocument(attachment.uri);
    }
    if (attachment.hintTimer !== null) window.clearTimeout(attachment.hintTimer);
    attachment.hintCancel?.cancel();
    Effect.runFork(Fiber.interrupt(attachment.busFiber));
    Effect.runFork(Scope.close(attachment.visualsScope, Exit.void));
    // A server with no open documents is dead weight: shut it down after a
    // grace window (so a scroll-away-and-back, or a tab reopen, reuses it).
    if (this.reapTimer === null) {
      this.reapTimer = window.setTimeout(() => {
        this.reapTimer = null;
        this.reapIdleClients();
      }, IDLE_CLIENT_GRACE_MS);
    }
  }

  /** Shut down every client no remaining attachment is using. */
  private reapIdleClients(): void {
    if (this.disposed) return;
    const inUse = new Set<ClientHandle>();
    for (const attachment of this.attachments.values()) {
      for (const binding of attachment.bindings) inUse.add(binding.handle);
    }
    for (const [key, handle] of [...this.clients]) {
      if (inUse.has(handle)) continue;
      this.clients.delete(key);
      if (handle.alive) void handle.client.shutdown().then(() => closeClient(handle));
    }
  }

  counts(docId: string): DiagnosticCounts | null {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined) return null;
    return attachment.bindings.reduce(
      (sum, binding) => ({
        errors: sum.errors + binding.counts.errors,
        warnings: sum.warnings + binding.counts.warnings,
      }),
      { errors: 0, warnings: 0 },
    );
  }

  /** Host save hook: didSave + watched-files fan-out to interested servers. */
  documentSaved(docId: string): void {
    const attachment = this.attachments.get(docId);
    for (const binding of attachment?.bindings ?? []) {
      if (binding.handle.alive) binding.handle.client.saveDocument(attachment!.uri);
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
        attachment.hintCancel?.cancel();
        attachment.hintCancel = null;
        this.pushHints(attachment, []);
      }
    }
  }

  // -------------------------------------------------------------- features

  /** Bindings able to serve `feature`, in priority order. */
  private eligibleBindings(
    attachment: Attachment,
    method: string,
    feature: string,
  ): ServerBinding[] {
    return attachment.bindings.filter(
      (binding) =>
        binding.handle.alive &&
        binding.handle.client.supports(method) &&
        featureAllowed(binding, feature),
    );
  }

  /** The first binding able to serve `feature` (helix routing). */
  private bindingFor(
    attachment: Attachment,
    method: string,
    feature: string,
  ): ServerBinding | null {
    return this.eligibleBindings(attachment, method, feature)[0] ?? null;
  }

  /** LSP completion as a generic intel result — merged across servers. */
  async completionAt(context: CompletionContext): Promise<CompletionResult | null> {
    const attachment = this.attachments.get(context.docId);
    if (attachment === undefined) return null;
    const bindings = this.eligibleBindings(attachment, lsp.CompletionRequest.method, "completion");
    if (bindings.length === 0) return null;
    this.flushPendingSync(attachment);
    const results = await Promise.all(
      bindings.map((binding) =>
        this.completionFrom(attachment, binding, context).catch(() => null),
      ),
    );
    const merged = results.filter((result) => result !== null);
    if (merged.length === 0) return null;
    return {
      items: merged.flatMap((result) => result.items),
      isIncomplete: merged.some((result) => result.isIncomplete === true),
    };
  }

  private async completionFrom(
    attachment: Attachment,
    binding: ServerBinding,
    context: CompletionContext,
  ): Promise<CompletionResult | null> {
    const buffer = this.bufferOf(attachment);
    const triggerKind =
      context.trigger.kind === "triggerCharacter"
        ? lsp.CompletionTriggerKind.TriggerCharacter
        : context.trigger.kind === "incomplete"
          ? lsp.CompletionTriggerKind.TriggerForIncompleteCompletions
          : lsp.CompletionTriggerKind.Invoked;
    const result = await binding.handle.client.completion(
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
        const editRange = "range" in item.textEdit ? item.textEdit.range : item.textEdit.replace;
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
        if (!binding.handle.alive) return null;
        const resolved = await binding.handle.client.resolveCompletionItem(item);
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
        if (command === undefined || !binding.handle.alive) return;
        await binding.handle.client.executeCommand(command.command, command.arguments);
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
    const hits = attachment.bindings
      .flatMap((binding) => binding.diagnosticList)
      .filter((d) => context.offset >= d.from && context.offset <= d.to);
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
    if (attachment === undefined) return null;
    const binding = this.bindingFor(attachment, lsp.HoverRequest.method, "hover");
    if (binding === null) return null;
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const hover = await binding.handle.client.hover(
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
    if (attachment === undefined) return [];
    const chars = this.eligibleBindings(
      attachment,
      lsp.CompletionRequest.method,
      "completion",
    ).flatMap((binding) => binding.handle.client.completionTriggerCharacters());
    return [...new Set(chars)];
  }

  signatureTriggerCharacters(docId: string): readonly string[] {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined) return [];
    const chars = this.eligibleBindings(
      attachment,
      lsp.SignatureHelpRequest.method,
      "signature-help",
    ).flatMap((binding) => binding.handle.client.signatureTriggerCharacters());
    return [...new Set(chars)];
  }

  signatureRetriggerCharacters(docId: string): readonly string[] {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined) return [];
    const chars = this.eligibleBindings(
      attachment,
      lsp.SignatureHelpRequest.method,
      "signature-help",
    ).flatMap((binding) => binding.handle.client.signatureRetriggerCharacters());
    return [...new Set(chars)];
  }

  /** LSP signature help as a generic intel result. */
  async signatureHelpAt(context: SignatureContext): Promise<SignatureResult | null> {
    const attachment = this.attachments.get(context.docId);
    if (attachment === undefined) return null;
    const binding = this.bindingFor(attachment, lsp.SignatureHelpRequest.method, "signature-help");
    if (binding === null) return null;
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const triggerKind =
      context.trigger.kind === "triggerCharacter"
        ? lsp.SignatureHelpTriggerKind.TriggerCharacter
        : context.trigger.kind === "contentChange"
          ? lsp.SignatureHelpTriggerKind.ContentChange
          : lsp.SignatureHelpTriggerKind.Invoked;
    const help = await binding.handle.client.signatureHelp(
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
    if (attachment === undefined) return [];
    const binding = this.bindingFor(attachment, lsp.DefinitionRequest.method, "goto-definition");
    if (binding === null) return [];
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    return binding.handle.client.definition(attachment.uri, offsetToPosition(buffer, offset));
  }

  async format(docId: string): Promise<boolean> {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined) return false;
    const binding = this.bindingFor(attachment, lsp.DocumentFormattingRequest.method, "format");
    if (binding === null) return false;
    this.flushPendingSync(attachment);
    const edits = await binding.handle.client.formatting(attachment.uri, {
      tabSize: 2,
      insertSpaces: true,
    });
    if (edits.length === 0) return false;
    this.applyEditsToOpenDocument(attachment, edits);
    return true;
  }

  async rename(docId: string, offset: number, newName: string): Promise<boolean> {
    const attachment = this.attachments.get(docId);
    if (attachment === undefined) return false;
    const binding = this.bindingFor(attachment, lsp.RenameRequest.method, "rename-symbol");
    if (binding === null) return false;
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const edit = await binding.handle.client.rename(
      attachment.uri,
      offsetToPosition(buffer, offset),
      newName,
    );
    if (edit === null) return false;
    const result = await this.applyWorkspaceEdit({ edit });
    return result.applied;
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  /**
   * Stop every server WITHOUT permanently disposing the manager: detach all
   * documents (clears squiggles/hints), shut down + kill all clients, and
   * forget spawn-failure backoffs so a later start() gets a clean retry.
   * attach() is a no-op until start() flips the flag back; the host re-attaches
   * its open documents then.
   */
  stop(): void {
    if (this.disposed || !this.enabled) return;
    this.enabled = false;
    for (const docId of [...this.attachments.keys()]) this.detach(docId);
    for (const handle of this.clients.values()) {
      if (handle.alive) {
        void handle.client.shutdown().then(() => closeClient(handle));
      }
    }
    this.clients.clear();
    this.failedSpawns.clear();
    if (this.reapTimer !== null) window.clearTimeout(this.reapTimer);
    this.reapTimer = null;
  }

  /** Re-enable attaching; the host calls attach() for each open document. */
  start(): void {
    if (this.disposed) return;
    this.enabled = true;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const docId of [...this.attachments.keys()]) this.detach(docId);
    for (const handle of this.clients.values()) {
      if (handle.alive) {
        void handle.client.shutdown().then(() => closeClient(handle));
      }
    }
    this.clients.clear();
    if (this.reapTimer !== null) window.clearTimeout(this.reapTimer);
    this.reapTimer = null;
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
    if (this.disposed) return;
    // The manager's cached coordinate copies (hover diagnostics, the
    // cross-server merge inputs, the push-diff bases) slide through every
    // edit exactly like the contributed sets do inside EditorVisuals — a
    // later push must never re-apply pre-edit offsets. A batch without
    // change ranges leaves them in place but poisons the diff bases: the
    // next push invalidates fully instead of trusting suspect offsets.
    if (changes !== undefined) {
      attachment.hintRanges = mapRangedValues(attachment.hintRanges, changes);
      attachment.pushedDiagnostics = mapRangedValues(attachment.pushedDiagnostics, changes);
      for (const binding of attachment.bindings) {
        binding.diagnosticRanges = mapRangedValues(binding.diagnosticRanges, changes);
        binding.diagnosticList = binding.diagnosticList.map((diagnostic) => {
          const from = mapOffsetThrough(changes, diagnostic.from);
          return { ...diagnostic, from, to: Math.max(from, mapOffsetThrough(changes, diagnostic.to)) };
        });
      }
    } else {
      attachment.hintsDiffBaseValid = false;
      attachment.diagnosticsDiffBaseValid = false;
    }
    // Incremental sync demands an INTACT chain and a FRESH notification:
    // the bus is sliding (slow consumers drop batches), and the inserted
    // text is sliced from the live buffer — valid only while the buffer is
    // still at the notification's version. Anything else resyncs fully.
    // Each binding keeps its OWN chain (servers differ in sync kind).
    const snapshot = changes !== undefined ? Effect.runSync(text.snapshot) : null;
    for (const binding of attachment.bindings) {
      if (!binding.handle.alive) continue;
      // A position-sensitive request may already have force-synced this binding
      // forward (flushPendingSync); a stale/earlier notification then has nothing
      // to add and must not schedule a redundant resync.
      if (snapshot !== null && version <= binding.syncedVersion) continue;
      if (
        binding.sync !== null &&
        changes !== undefined &&
        snapshot !== null &&
        snapshot.version === version &&
        version === binding.syncedVersion + 1
      ) {
        const edits = changes.map((change) => ({
          start: change.fromA,
          end: change.toA,
          text: snapshot.buffer.sliceString(change.fromB, change.toB),
        }));
        binding.handle.client.changeDocumentIncremental(
          attachment.uri,
          version,
          binding.sync.applyEdits(edits),
        );
        binding.syncedVersion = version;
      } else {
        // Full sync (or unknown changes): debounce whole-text sends.
        if (binding.fullSyncTimer !== null) window.clearTimeout(binding.fullSyncTimer);
        binding.fullSyncTimer = window.setTimeout(() => {
          binding.fullSyncTimer = null;
          this.sendFullSync(attachment, binding);
        }, FULL_SYNC_DEBOUNCE_MS);
      }
    }
    this.scheduleHints(attachment, INLAY_HINT_DEBOUNCE_MS);
  }

  private sendFullSync(attachment: Attachment, binding: ServerBinding): void {
    if (this.disposed || !binding.handle.alive) return;
    const text = Context.get(
      attachment.document.context as Context.Context<TextDocument>,
      TextDocument,
    );
    const current = Effect.runSync(text.snapshot);
    const fullText = current.buffer.sliceString(0, current.buffer.length);
    binding.sync?.reset(fullText);
    binding.handle.client.changeDocumentFull(attachment.uri, current.version, fullText);
    binding.syncedVersion = current.version;
  }

  /**
   * Position-sensitive requests must not race the document sync. Two ways the
   * server's mirror can lag the buffer:
   *   1. a typing burst tripped the chain gate into the debounced full sync;
   *   2. the edit that TRIGGERED this request (e.g. the "." that opens member
   *      completion) hasn't reached the manager's document-bus subscriber yet —
   *      bus delivery order between the controller (which fires the request) and
   *      the manager (which syncs) is not guaranteed, so the completion can be
   *      sent before the "." is synced and tsserver answers with GLOBALS, not
   *      members.
   * Either way: if the server isn't at the buffer's current version, push the
   * full text now (resets the incremental chain) before the request goes out.
   */
  private flushPendingSync(attachment: Attachment): void {
    const text = Context.get(
      attachment.document.context as Context.Context<TextDocument>,
      TextDocument,
    );
    const currentVersion = Effect.runSync(text.snapshot).version;
    for (const binding of attachment.bindings) {
      if (!binding.handle.alive) continue;
      const pending = binding.fullSyncTimer !== null;
      if (pending) {
        window.clearTimeout(binding.fullSyncTimer ?? undefined);
        binding.fullSyncTimer = null;
      }
      if (pending || binding.syncedVersion < currentVersion) {
        this.sendFullSync(attachment, binding);
      }
    }
  }

  // ----------------------------------------------------------- diagnostics

  private handleDiagnostics(handle: ClientHandle, params: lsp.PublishDiagnosticsParams): void {
    const attachment = this.byUri.get(params.uri);
    if (attachment === undefined) return;
    const binding = attachment.bindings.find((b) => b.handle === handle);
    if (binding === undefined || !featureAllowed(binding, "diagnostics")) return;
    // Version gate: a publish computed for an OLDER synced version carries
    // positions in old coordinates — converting them against today's buffer
    // draws underlines mid-word. Discard; the server re-lints after every
    // didChange, so a fresh publish for the current version follows. (The
    // existing, correctly-mapped diagnostics stay up meanwhile.)
    if (params.version !== undefined && params.version < this.documentVersion(attachment)) {
      return;
    }
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
    binding.diagnosticRanges = ranges;
    binding.diagnosticList = params.diagnostics.map((diagnostic) => {
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
    this.pushDiagnostics(attachment);
    binding.counts = {
      errors: params.diagnostics.filter((d) => (d.severity ?? 1) === 1).length,
      warnings: params.diagnostics.filter((d) => d.severity === 2).length,
    };
    this.onDiagnostics?.(attachment.docId);
  }

  // ----------------------------------------------------------- inlay hints

  private scheduleHints(attachment: Attachment, delayMs: number): void {
    if (!this.hintsEnabled || this.disposed) return;
    if (this.bindingFor(attachment, lsp.InlayHintRequest.method, "inlay-hints") === null) return;
    if (attachment.hintTimer !== null) window.clearTimeout(attachment.hintTimer);
    attachment.hintTimer = window.setTimeout(() => {
      attachment.hintTimer = null;
      void this.requestHints(attachment);
    }, delayMs);
  }

  private async requestHints(attachment: Attachment): Promise<void> {
    if (this.disposed || !this.hintsEnabled) return;
    const binding = this.bindingFor(attachment, lsp.InlayHintRequest.method, "inlay-hints");
    if (binding === null) return;
    // The response's positions are only meaningful against the exact
    // version the server answered for. Sync the server fully to the current
    // buffer first, remember that version, and DISCARD the response if the
    // buffer moved on while the request was in flight — the edit that moved
    // it already scheduled a fresh request (Zed's InlayHintCache enforces
    // the same rule via buffer_version.changed_since). Applying a stale
    // response splices hints mid-word and every column right of them
    // shifts: the "hints teleport while typing" bug.
    this.flushPendingSync(attachment);
    const buffer = this.bufferOf(attachment);
    const requestVersion = this.documentVersion(attachment);
    if (buffer.length > INLAY_HINT_MAX_CHARS) return;
    attachment.hintCancel?.cancel();
    const source = binding.handle.client.cancellationSource();
    attachment.hintCancel = source;
    let hints: lsp.InlayHint[];
    try {
      hints = await binding.handle.client.inlayHints(
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
    if (attachment.hintCancel !== source || this.disposed || !this.hintsEnabled) return;
    if (this.documentVersion(attachment) !== requestVersion) return; // stale: a re-request is scheduled
    const ranges: RangedValue<VirtualTextSpec>[] = hints.map((hint) => {
      const at = positionToOffset(buffer, hint.position);
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
    this.pushHints(attachment, sortRanges(ranges));
  }

  /** The document's version now — the gate every async response checks. */
  private documentVersion(attachment: Attachment): number {
    const text = Context.get(
      attachment.document.context as Context.Context<TextDocument>,
      TextDocument,
    );
    return Effect.runSync(text.snapshot).version;
  }

  /**
   * Push the merged cross-server diagnostics as their own contribution,
   * invalidating only the spans that actually changed — an identical
   * republish (servers re-lint constantly) repaints nothing.
   */
  private pushDiagnostics(attachment: Attachment): void {
    const merged = attachment.bindings
      .flatMap((binding) => binding.diagnosticRanges)
      .sort((a, b) => a.from - b.from || a.to - b.to);
    const spans = attachment.diagnosticsDiffBaseValid
      ? changedSpans(attachment.pushedDiagnostics, merged, decorationSpecEquals)
      : undefined;
    attachment.pushedDiagnostics = merged;
    attachment.diagnosticsDiffBaseValid = true;
    if (spans !== undefined && spans.length === 0) return; // identical: no-op
    Effect.runSync(
      attachment.diagnosticsVisuals.update({ decorations: RangeSet.of(merged) }, spans),
    );
  }

  /** Push a new hint set, invalidating only rows whose hints changed. */
  private pushHints(attachment: Attachment, next: RangedValue<VirtualTextSpec>[]): void {
    const spans = attachment.hintsDiffBaseValid
      ? changedSpans(attachment.hintRanges, next, virtualTextSpecEquals)
      : undefined;
    attachment.hintRanges = next;
    attachment.hintsDiffBaseValid = true;
    if (spans !== undefined && spans.length === 0) return; // identical: no-op
    Effect.runSync(attachment.hintsVisuals.update({ virtualText: RangeSet.of(next) }, spans));
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
    const transaction = Effect.runSync(Transaction.fromReplacements(buffer.length, replacements));
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

    // Per-client scope: the process is bound to it, so closeClient() (shutdown,
    // detach-reap, stop, dispose) kills it deterministically. If the manager's
    // owning scope is torn down without a clean shutdown, the process still dies
    // — ownership is lifetime.
    const scope = Effect.runSync(Scope.make());
    const proc = await Effect.runPromise(
      this.processes
        .spawn({
          command: definition.command,
          args: definition.args ?? [],
          cwd: root,
          label: `lsp:${definition.id}`,
        })
        .pipe(Scope.extend(scope), Effect.option),
    );
    if (proc._tag === "None") {
      void Effect.runPromise(Scope.close(scope, Exit.void));
      this.failedSpawns.set(key, Date.now());
      return null;
    }
    const procHandle = proc.value;

    const chunkListeners = new Set<(chunk: Uint8Array) => void>();
    const closeListeners = new Set<() => void>();
    procHandle.onData((chunk, stream) => {
      if (stream !== "stdout") return; // stderr is server log noise
      for (const listener of chunkListeners) listener(chunk);
    });
    procHandle.onExit(() => {
      for (const listener of closeListeners) listener();
    });
    const streams: ByteStreams = {
      send: (chunk) => procHandle.write(chunk),
      onChunk: (listener) => void chunkListeners.add(listener),
      onClose: (listener) => void closeListeners.add(listener),
    };

    const handle: ClientHandle = {
      key,
      definition,
      proc: procHandle,
      scope,
      alive: true,
      ready: Promise.resolve(false),
      client: new LspClient(createByteConnection(streams), {
        rootUri: pathToUri(root),
        ...(definition.initializationOptions !== undefined
          ? { initializationOptions: definition.initializationOptions }
          : {}),
        settings: () => definition.settings,
        onDiagnostics: (params) => this.handleDiagnostics(handle, params),
        onApplyEdit: (params) => this.applyWorkspaceEdit(params),
        onInlayHintRefresh: () => {
          for (const attachment of this.attachments.values()) {
            if (attachment.bindings.some((b) => b.handle === handle)) {
              this.scheduleHints(attachment, 0);
            }
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
      // Server died: clear ITS visuals (other bindings keep theirs).
      for (const attachment of this.attachments.values()) {
        const binding = attachment.bindings.find((b) => b.handle === handle);
        if (binding === undefined) continue;
        binding.diagnosticRanges = [];
        binding.diagnosticList = [];
        binding.counts = { errors: 0, warnings: 0 };
        this.pushDiagnostics(attachment);
        if (!attachment.bindings.some((b) => b.handle.alive)) {
          this.pushHints(attachment, []);
        }
        this.onDiagnostics?.(attachment.docId);
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
