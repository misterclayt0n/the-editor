/**
 * The LSP client: lifecycle, capability accounting, document sync, and the
 * feature requests the editor surfaces. Protocol types and the connection
 * machinery come from vscode-languageserver-protocol / vscode-jsonrpc (the
 * same foundation Theia and Monaco's client build on); this class is the
 * part that must stay ours — it is the seam the editor's documents,
 * registries, and visuals integrate against.
 *
 * Capability accounting: servers declare capabilities statically (the
 * initialize result) or DYNAMICALLY (client/registerCapability, which many
 * servers use exclusively). `supports(method)` consults both, so features
 * never probe a server that can't answer.
 */

import * as lsp from "vscode-languageserver-protocol/browser";
import {
  CancellationTokenSource,
  type CancellationToken,
  type ProtocolConnection,
} from "vscode-languageserver-protocol/browser";

export type LspClientOptions = {
  readonly rootUri: string | null;
  readonly clientName?: string;
  readonly initializationOptions?: unknown;
  /** Workspace settings; answers workspace/configuration by section path. */
  readonly settings?: () => Record<string, unknown> | undefined;
  readonly onDiagnostics?: (params: lsp.PublishDiagnosticsParams) => void;
  readonly onApplyEdit?: (
    params: lsp.ApplyWorkspaceEditParams,
  ) => Promise<lsp.ApplyWorkspaceEditResult> | lsp.ApplyWorkspaceEditResult;
  readonly onInlayHintRefresh?: () => void;
  readonly onLog?: (message: string) => void;
  readonly onShowMessage?: (type: number, message: string) => void;
};

const staticCapabilityKeys: Record<string, string> = {
  [lsp.DefinitionRequest.method]: "definitionProvider",
  [lsp.DocumentFormattingRequest.method]: "documentFormattingProvider",
  [lsp.RenameRequest.method]: "renameProvider",
  [lsp.InlayHintRequest.method]: "inlayHintProvider",
  [lsp.HoverRequest.method]: "hoverProvider",
  [lsp.CompletionRequest.method]: "completionProvider",
  [lsp.SignatureHelpRequest.method]: "signatureHelpProvider",
  [lsp.ReferencesRequest.method]: "referencesProvider",
  [lsp.DocumentSymbolRequest.method]: "documentSymbolProvider",
};

type Registration = { readonly method: string; readonly options: unknown };

export class LspClient {
  private initialized = false;
  private serverCapabilities: lsp.ServerCapabilities = {};
  private readonly registrations = new Map<string, Registration>();
  private readonly openVersions = new Map<string, number>();

  constructor(
    readonly connection: ProtocolConnection,
    private readonly options: LspClientOptions,
  ) {
    connection.onNotification(lsp.PublishDiagnosticsNotification.type, (params) => {
      this.options.onDiagnostics?.(params);
    });
    connection.onNotification(lsp.LogMessageNotification.type, (params) => {
      this.options.onLog?.(params.message);
    });
    connection.onNotification(lsp.ShowMessageNotification.type, (params) => {
      this.options.onShowMessage?.(params.type, params.message);
    });
    connection.onRequest(lsp.ShowMessageRequest.type, () => null);
    connection.onRequest(lsp.RegistrationRequest.type, (params) => {
      for (const registration of params.registrations) {
        this.registrations.set(registration.id, {
          method: registration.method,
          options: registration.registerOptions,
        });
      }
    });
    connection.onRequest(lsp.UnregistrationRequest.type, (params) => {
      for (const unregistration of params.unregisterations) {
        this.registrations.delete(unregistration.id);
      }
    });
    connection.onRequest(lsp.ConfigurationRequest.type, (params) => {
      const settings = this.options.settings?.() ?? {};
      return params.items.map((item) => {
        if (!item.section) return settings;
        let value: unknown = settings;
        for (const key of item.section.split(".")) {
          value = (value as Record<string, unknown> | undefined)?.[key];
        }
        return value ?? null;
      });
    });
    connection.onRequest(lsp.ApplyWorkspaceEditRequest.type, async (params) => {
      if (this.options.onApplyEdit === undefined) {
        return { applied: false, failureReason: "client does not apply edits" };
      }
      return this.options.onApplyEdit(params);
    });
    connection.onRequest(lsp.InlayHintRefreshRequest.type, () => {
      this.options.onInlayHintRefresh?.();
    });
    connection.onRequest(lsp.WorkDoneProgressCreateRequest.type, () => {});
    connection.onRequest(lsp.WorkspaceFoldersRequest.type, () =>
      this.options.rootUri === null
        ? []
        : [{ uri: this.options.rootUri, name: "workspace" }],
    );
  }

  async initialize(): Promise<lsp.InitializeResult> {
    const params: lsp.InitializeParams = {
      processId: null,
      clientInfo: { name: this.options.clientName ?? "the-editor" },
      rootUri: this.options.rootUri,
      workspaceFolders:
        this.options.rootUri === null
          ? null
          : [{ uri: this.options.rootUri, name: "workspace" }],
      ...(this.options.initializationOptions !== undefined
        ? { initializationOptions: this.options.initializationOptions }
        : {}),
      capabilities: {
        general: { positionEncodings: ["utf-16"] },
        textDocument: {
          synchronization: { dynamicRegistration: true, didSave: true },
          publishDiagnostics: {
            relatedInformation: true,
            versionSupport: true,
            tagSupport: { valueSet: [1, 2] },
          },
          definition: { dynamicRegistration: true, linkSupport: true },
          completion: {
            dynamicRegistration: true,
            contextSupport: true,
            completionItem: {
              snippetSupport: true,
              documentationFormat: [lsp.MarkupKind.Markdown, lsp.MarkupKind.PlainText],
              deprecatedSupport: true,
              insertReplaceSupport: true,
              // Lazily resolved on accept: auto-import edits and docs.
              resolveSupport: {
                properties: ["documentation", "detail", "additionalTextEdits"],
              },
            },
          },
          hover: {
            dynamicRegistration: true,
            contentFormat: [lsp.MarkupKind.Markdown, lsp.MarkupKind.PlainText],
          },
          signatureHelp: {
            dynamicRegistration: true,
            contextSupport: true,
            signatureInformation: {
              documentationFormat: [lsp.MarkupKind.Markdown, lsp.MarkupKind.PlainText],
              parameterInformation: { labelOffsetSupport: true },
              activeParameterSupport: true,
            },
          },
          formatting: { dynamicRegistration: true },
          rename: { dynamicRegistration: true, prepareSupport: false },
          inlayHint: { dynamicRegistration: true },
        },
        workspace: {
          applyEdit: true,
          workspaceEdit: { documentChanges: true, failureHandling: lsp.FailureHandlingKind.Abort },
          configuration: true,
          workspaceFolders: true,
          didChangeConfiguration: { dynamicRegistration: true },
          didChangeWatchedFiles: { dynamicRegistration: true, relativePatternSupport: false },
          inlayHint: { refreshSupport: true },
        },
        window: { workDoneProgress: true },
      },
    };
    const result = await this.connection.sendRequest(lsp.InitializeRequest.type, params);
    this.serverCapabilities = result.capabilities;
    await this.connection.sendNotification(lsp.InitializedNotification.type, {});
    const settings = this.options.settings?.();
    if (settings !== undefined) {
      await this.connection.sendNotification(lsp.DidChangeConfigurationNotification.type, {
        settings,
      });
    }
    this.initialized = true;
    return result;
  }

  get isInitialized(): boolean {
    return this.initialized;
  }

  get capabilities(): lsp.ServerCapabilities {
    return this.serverCapabilities;
  }

  // ------------------------------------------------- capability accounting

  /** Static (initialize) OR dynamic (registerCapability) support. */
  supports(method: string): boolean {
    for (const registration of this.registrations.values()) {
      if (registration.method === method) return true;
    }
    const key = staticCapabilityKeys[method];
    if (key === undefined) return false;
    const value = (this.serverCapabilities as Record<string, unknown>)[key];
    return value !== undefined && value !== false && value !== null;
  }

  /** How didChange must be shaped for this server. */
  syncKind(): lsp.TextDocumentSyncKind {
    const sync = this.serverCapabilities.textDocumentSync;
    if (sync === undefined || sync === null) return lsp.TextDocumentSyncKind.None;
    if (typeof sync === "number") return sync;
    return sync.change ?? lsp.TextDocumentSyncKind.None;
  }

  /** Glob patterns from dynamic didChangeWatchedFiles registrations. */
  watchedFilePatterns(): string[] {
    const patterns: string[] = [];
    for (const registration of this.registrations.values()) {
      if (registration.method !== lsp.DidChangeWatchedFilesNotification.method) continue;
      const options = registration.options as lsp.DidChangeWatchedFilesRegistrationOptions;
      for (const watcher of options?.watchers ?? []) {
        if (typeof watcher.globPattern === "string") patterns.push(watcher.globPattern);
      }
    }
    return patterns;
  }

  // ------------------------------------------------------- document sync

  openDocument(uri: string, languageId: string, version: number, text: string): void {
    this.openVersions.set(uri, version);
    void this.connection.sendNotification(lsp.DidOpenTextDocumentNotification.type, {
      textDocument: { uri, languageId, version, text },
    });
  }

  changeDocumentFull(uri: string, version: number, text: string): void {
    if (!this.openVersions.has(uri)) return;
    this.openVersions.set(uri, version);
    void this.connection.sendNotification(lsp.DidChangeTextDocumentNotification.type, {
      textDocument: { uri, version },
      contentChanges: [{ text }],
    });
  }

  changeDocumentIncremental(
    uri: string,
    version: number,
    contentChanges: lsp.TextDocumentContentChangeEvent[],
  ): void {
    if (!this.openVersions.has(uri) || contentChanges.length === 0) return;
    this.openVersions.set(uri, version);
    void this.connection.sendNotification(lsp.DidChangeTextDocumentNotification.type, {
      textDocument: { uri, version },
      contentChanges,
    });
  }

  saveDocument(uri: string): void {
    if (!this.openVersions.has(uri)) return;
    void this.connection.sendNotification(lsp.DidSaveTextDocumentNotification.type, {
      textDocument: { uri },
    });
  }

  closeDocument(uri: string): void {
    if (!this.openVersions.delete(uri)) return;
    void this.connection.sendNotification(lsp.DidCloseTextDocumentNotification.type, {
      textDocument: { uri },
    });
  }

  hasDocument(uri: string): boolean {
    return this.openVersions.has(uri);
  }

  notifyWatchedFiles(changes: lsp.FileEvent[]): void {
    if (changes.length === 0) return;
    void this.connection.sendNotification(lsp.DidChangeWatchedFilesNotification.type, {
      changes,
    });
  }

  /** Normalized: Location | Location[] | LocationLink[] → Location[]. */
  async definition(
    uri: string,
    position: lsp.Position,
    token?: CancellationToken,
  ): Promise<lsp.Location[]> {
    const result = await this.connection.sendRequest(
      lsp.DefinitionRequest.type,
      { textDocument: { uri }, position },
      token,
    );
    if (result === null || result === undefined) return [];
    const list = Array.isArray(result) ? result : [result];
    return list.map((entry) =>
      "targetUri" in entry
        ? { uri: entry.targetUri, range: entry.targetSelectionRange ?? entry.targetRange }
        : entry,
    );
  }

  async formatting(
    uri: string,
    formatOptions: lsp.FormattingOptions,
    token?: CancellationToken,
  ): Promise<lsp.TextEdit[]> {
    const result = await this.connection.sendRequest(
      lsp.DocumentFormattingRequest.type,
      { textDocument: { uri }, options: formatOptions },
      token,
    );
    return result ?? [];
  }

  async rename(
    uri: string,
    position: lsp.Position,
    newName: string,
    token?: CancellationToken,
  ): Promise<lsp.WorkspaceEdit | null> {
    const result = await this.connection.sendRequest(
      lsp.RenameRequest.type,
      { textDocument: { uri }, position, newName },
      token,
    );
    return result ?? null;
  }

  /** Normalized: CompletionItem[] | CompletionList | null → list shape. */
  async completion(
    uri: string,
    position: lsp.Position,
    context: lsp.CompletionContext,
    token?: CancellationToken,
  ): Promise<{ items: lsp.CompletionItem[]; isIncomplete: boolean }> {
    const result = await this.connection.sendRequest(
      lsp.CompletionRequest.type,
      { textDocument: { uri }, position, context },
      token,
    );
    if (result === null || result === undefined) return { items: [], isIncomplete: false };
    if (Array.isArray(result)) return { items: result, isIncomplete: false };
    return { items: result.items, isIncomplete: result.isIncomplete };
  }

  /**
   * workspace/executeCommand — completion-item commands run AFTER the
   * insert; some servers deliver auto-imports through these (the edits
   * come back as a workspace/applyEdit request).
   */
  async executeCommand(
    command: string,
    args?: unknown[],
    token?: CancellationToken,
  ): Promise<unknown> {
    if (this.serverCapabilities.executeCommandProvider === undefined) return null;
    return this.connection.sendRequest(
      lsp.ExecuteCommandRequest.type,
      { command, ...(args !== undefined ? { arguments: args as any[] } : {}) },
      token,
    );
  }

  /**
   * completionItem/resolve — fills lazy properties (documentation,
   * additionalTextEdits such as auto-imports). Identity when the server
   * doesn't resolve.
   */
  async resolveCompletionItem(
    item: lsp.CompletionItem,
    token?: CancellationToken,
  ): Promise<lsp.CompletionItem> {
    if (this.serverCapabilities.completionProvider?.resolveProvider !== true) return item;
    const result = await this.connection.sendRequest(
      lsp.CompletionResolveRequest.type,
      item,
      token,
    );
    return result ?? item;
  }

  async hover(
    uri: string,
    position: lsp.Position,
    token?: CancellationToken,
  ): Promise<lsp.Hover | null> {
    const result = await this.connection.sendRequest(
      lsp.HoverRequest.type,
      { textDocument: { uri }, position },
      token,
    );
    return result ?? null;
  }

  /** Server-declared completion trigger characters (static capability). */
  completionTriggerCharacters(): readonly string[] {
    const provider = this.serverCapabilities.completionProvider;
    return provider?.triggerCharacters ?? [];
  }

  signatureTriggerCharacters(): readonly string[] {
    return this.serverCapabilities.signatureHelpProvider?.triggerCharacters ?? [];
  }

  signatureRetriggerCharacters(): readonly string[] {
    return this.serverCapabilities.signatureHelpProvider?.retriggerCharacters ?? [];
  }

  async signatureHelp(
    uri: string,
    position: lsp.Position,
    context: lsp.SignatureHelpContext,
    token?: CancellationToken,
  ): Promise<lsp.SignatureHelp | null> {
    const result = await this.connection.sendRequest(
      lsp.SignatureHelpRequest.type,
      { textDocument: { uri }, position, context },
      token,
    );
    return result ?? null;
  }

  async inlayHints(
    uri: string,
    range: lsp.Range,
    token?: CancellationToken,
  ): Promise<lsp.InlayHint[]> {
    const result = await this.connection.sendRequest(
      lsp.InlayHintRequest.type,
      { textDocument: { uri }, range },
      token,
    );
    return result ?? [];
  }

  cancellationSource(): CancellationTokenSource {
    return new CancellationTokenSource();
  }

  async shutdown(timeoutMs = 1500): Promise<void> {
    try {
      await Promise.race([
        this.connection.sendRequest(lsp.ShutdownRequest.type),
        new Promise((resolve) => setTimeout(resolve, timeoutMs)),
      ]);
      await this.connection.sendNotification(lsp.ExitNotification.type);
    } catch {
      // The server is already gone; nothing to be polite to.
    }
    this.connection.dispose();
  }
}
