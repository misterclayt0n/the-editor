import type {
  CellGridMetrics,
  ClipboardKind,
  CursorSemantics,
  DocumentId,
  DocumentOpened,
  OkResult,
  RenderFrame,
  JsonValue,
  LspDiagnosticEntry,
  LspLogEntry,
  LspServerConfig,
  LspServerId,
  LspServerStatus,
  RuntimeApi,
  SurfaceCreated,
  SurfaceEdit,
  SurfaceId,
  SurfaceOperation,
  SurfaceState,
  Viewport,
} from "@the-editor/contracts/runtime";
import { NativeRuntimeSession } from "@the-editor/runtime-node";

export class RuntimeClientError extends Error {
  constructor(message: string, readonly cause?: unknown) {
    super(message);
    this.name = "RuntimeClientError";
  }
}

export interface RuntimeNativeClientOptions {
  session?: NativeRuntimeSession;
}

export class RuntimeNativeClient implements RuntimeApi {
  readonly server = {
    ping: async (): Promise<OkResult> => ({ ok: true }),
  };

  readonly syntax = {
    highlightLegend: async (): Promise<Record<number, string>> => {
      const read = (this.session as { readSyntaxHighlightLegendJson?: () => string }).readSyntaxHighlightLegendJson;
      if (typeof read !== "function") return {};
      return JSON.parse(read.call(this.session)) as Record<number, string>;
    },
  };

  readonly documents = {
    openFile: async (params: { path: string }): Promise<DocumentOpened> => ({
      documentId: this.session.openFile(params.path) as DocumentId,
    }),
    createScratch: async (params: { text?: string }): Promise<DocumentOpened> => ({
      documentId: this.session.createScratch(params.text ?? "") as DocumentId,
    }),
  };

  readonly surfaces = {
    create: async (params: { documentId: DocumentId; viewport?: Viewport }): Promise<SurfaceCreated> => ({
      surfaceId: this.session.createSurface(params.documentId, params.viewport ?? null) as SurfaceId,
    }),
    destroy: async (params: { surfaceId: SurfaceId }): Promise<OkResult> => {
      this.session.destroySurface(params.surfaceId);
      return { ok: true };
    },
    loadFile: async (params: { surfaceId: SurfaceId; path: string }): Promise<OkResult> => {
      this.session.loadSurfaceFile(params.surfaceId, params.path);
      return { ok: true };
    },
    lineCount: async (params: { surfaceId: SurfaceId }): Promise<{ lineCount: number }> => ({
      lineCount: this.session.readSurfaceLineCount(params.surfaceId),
    }),
    setViewport: async (params: { surfaceId: SurfaceId; viewport: Viewport }): Promise<OkResult> => {
      this.session.setSurfaceViewport(params.surfaceId, params.viewport);
      return { ok: true };
    },
    setMetrics: async (params: { surfaceId: SurfaceId; metrics: CellGridMetrics }): Promise<OkResult> => {
      this.session.setSurfaceMeasurements(params.surfaceId, params.metrics);
      return { ok: true };
    },
    setSoftWrap: async (params: { surfaceId: SurfaceId; softWrap: boolean }): Promise<OkResult> => {
      this.session.setSurfaceSoftWrap(params.surfaceId, params.softWrap);
      return { ok: true };
    },
    setCursorSemantics: async (params: { surfaceId: SurfaceId; semantics: CursorSemantics }): Promise<OkResult> => {
      this.session.setSurfaceCursorSemantics(params.surfaceId, params.semantics);
      return { ok: true };
    },
    applyOperation: async (params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<SurfaceState> => {
      const span = runtimePerfSpan("runtime", "applySurfaceOperation", { surfaceId: params.surfaceId, operation: params.operation.name }, 4);
      const state = surfaceStateFromJson(String(this.session.applySurfaceOperation(params.surfaceId, params.operation)));
      span.end({ textLength: state.text.length, revision: state.revision });
      return state;
    },
    readState: async (params: { surfaceId: SurfaceId }): Promise<SurfaceState> => {
      const span = runtimePerfSpan("runtime", "readSurfaceState", { surfaceId: params.surfaceId }, 4);
      const state = {
        text: this.session.readSurfaceText(params.surfaceId),
        revision: this.session.readSurfaceRevision(params.surfaceId),
        lineEnding: this.session.readSurfaceLineEnding(params.surfaceId),
        lineCount: this.session.readSurfaceLineCount(params.surfaceId),
        selection: this.session.readSurfaceSelection(params.surfaceId),
        activeCursorId: this.session.readSurfaceActiveCursor(params.surfaceId),
        cursorSemantics: this.session.readSurfaceCursorSemantics(params.surfaceId) as CursorSemantics,
        viewport: this.session.readSurfaceViewport(params.surfaceId),
      } satisfies SurfaceState;
      span.end({ textLength: state.text.length, revision: state.revision });
      return state;
    },
    setActiveCursor: async (params: { surfaceId: SurfaceId; cursorId: number | null }): Promise<SurfaceState> => surfaceStateFromJson(
      String(this.session.setSurfaceActiveCursor(params.surfaceId, params.cursorId)),
    ),
    applyEdit: async (params: { surfaceId: SurfaceId; edit: SurfaceEdit }): Promise<SurfaceState> => surfaceStateFromJson(
      String(this.session.applySurfaceEdit(params.surfaceId, params.edit)),
    ),
    commitUndoCheckpoint: async (params: { surfaceId: SurfaceId }): Promise<OkResult> => {
      this.session.commitSurfaceUndoCheckpoint(params.surfaceId);
      return { ok: true };
    },
    undo: async (params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState> => surfaceStateFromJson(
      String(this.session.undoSurface(params.surfaceId, params.count ?? 1)),
    ),
    redo: async (params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState> => surfaceStateFromJson(
      String(this.session.redoSurface(params.surfaceId, params.count ?? 1)),
    ),
    earlier: async (params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState> => surfaceStateFromJson(
      String(this.session.earlierSurface(params.surfaceId, params.count ?? 1)),
    ),
    later: async (params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState> => surfaceStateFromJson(
      String(this.session.laterSurface(params.surfaceId, params.count ?? 1)),
    ),
    renderFrame: async (params: {
      surfaceId: SurfaceId;
      viewport?: Viewport;
    }): Promise<RenderFrame> => {
      const span = runtimePerfSpan("runtime", "renderFrame", { surfaceId: params.surfaceId }, 4);
      const json = this.session.renderFrameJson(params.surfaceId, params.viewport ?? null);
      const frame = JSON.parse(json) as RenderFrame;
      span.end({ rows: frame.rows.length });
      return frame;
    },
  };

  readonly search = {
    ensureIndex: async (params: { rootPath: string }) => JSON.parse(this.session.ensureSearchIndexJson(params.rootPath)) as Awaited<ReturnType<RuntimeApi["search"]["ensureIndex"]>>,
    status: async (params: { indexId: Parameters<RuntimeApi["search"]["status"]>[0]["indexId"] }) => JSON.parse(this.session.readSearchIndexStatusJson(params.indexId)) as Awaited<ReturnType<RuntimeApi["search"]["status"]>>,
    refresh: async (params: { indexId: Parameters<RuntimeApi["search"]["refresh"]>[0]["indexId"] }) => {
      this.session.refreshSearchIndex(params.indexId);
      return { ok: true } as const;
    },
    queryFiles: async (params: Parameters<RuntimeApi["search"]["queryFiles"]>[0]) => JSON.parse(this.session.querySearchFilesJson(params.indexId, params.query, params.offset ?? null, params.limit ?? null, params.currentPath ?? null)) as Awaited<ReturnType<RuntimeApi["search"]["queryFiles"]>>,
    queryDirs: async (params: Parameters<RuntimeApi["search"]["queryDirs"]>[0]) => JSON.parse(this.session.querySearchDirsJson(params.indexId, params.query, params.offset ?? null, params.limit ?? null, params.currentPath ?? null)) as Awaited<ReturnType<RuntimeApi["search"]["queryDirs"]>>,
    queryMixed: async (params: Parameters<RuntimeApi["search"]["queryMixed"]>[0]) => JSON.parse(this.session.querySearchMixedJson(params.indexId, params.query, params.offset ?? null, params.limit ?? null, params.currentPath ?? null)) as Awaited<ReturnType<RuntimeApi["search"]["queryMixed"]>>,
    queryGrep: async (params: Parameters<RuntimeApi["search"]["queryGrep"]>[0]) => JSON.parse(this.session.querySearchGrepJson(
      params.indexId,
      params.query,
      params.offset ?? null,
      params.limit ?? null,
      params.currentPath ?? null,
      params.beforeContext ?? null,
      params.afterContext ?? null,
    )) as Awaited<ReturnType<RuntimeApi["search"]["queryGrep"]>>,
    recordSelection: async (params: Parameters<RuntimeApi["search"]["recordSelection"]>[0]) => {
      this.session.recordSearchSelection(params.indexId, params.query, params.path);
      return { ok: true } as const;
    },
    previewPathCacheKey: async (params: { path: string }): Promise<string> => this.session.previewPathCacheKey(params.path),
    readTextPreview: async (params: Parameters<RuntimeApi["search"]["readTextPreview"]>[0]) => JSON.parse(this.session.readTextPreviewJson(
      params.path,
      params.maxBytes ?? null,
      params.maxLines ?? null,
      params.aroundLine ?? null,
      params.contextLines ?? null,
    )) as Awaited<ReturnType<RuntimeApi["search"]["readTextPreview"]>>,
  };

  readonly registers = {
    read: async (params: { name: string }): Promise<{ values: string[] | null }> => ({
      values: this.session.readRegister(params.name) ?? null,
    }),
    write: async (params: { name: string; values: string[] }): Promise<OkResult> => {
      this.session.writeRegister(params.name, params.values);
      return { ok: true };
    },
  };

  readonly clipboard = {
    read: async (params: { kind: ClipboardKind }): Promise<{ value: string | null }> => ({
      value: this.session.readClipboard(params.kind) ?? null,
    }),
    write: async (params: { kind: ClipboardKind; value: string }): Promise<OkResult> => {
      this.session.writeClipboard(params.value, params.kind);
      return { ok: true };
    },
  };

  readonly lsp = {
    registerServer: async (params: LspServerConfig): Promise<OkResult> => {
      this.nativeLsp().lspRegisterServer(JSON.stringify(toNativeLspServerConfig(params)));
      return { ok: true };
    },
    configuredServers: async (): Promise<LspServerConfig[]> => JSON.parse(this.nativeLsp().lspConfiguredServersJson()) as LspServerConfig[],
    startServer: async (params: { id: string; rootUri?: string }): Promise<{ serverId: LspServerId }> => ({
      serverId: this.nativeLsp().lspStartServer(params.id, params.rootUri ?? null) as LspServerId,
    }),
    shutdownServer: async (params: { serverId: LspServerId }): Promise<OkResult> => {
      this.nativeLsp().lspShutdownServer(params.serverId);
      return { ok: true };
    },
    status: async (): Promise<LspServerStatus[]> => {
      const span = runtimePerfSpan("lsp", "status.native", {}, 8);
      const statuses = JSON.parse(String(await this.nativeLsp().lspStatusJsonAsync())) as LspServerStatus[];
      span.end({ count: statuses.length });
      return statuses;
    },
    diagnostics: async (): Promise<LspDiagnosticEntry[]> => JSON.parse(this.nativeLsp().lspDiagnosticsJson()) as LspDiagnosticEntry[],
    logs: async (params: { serverId: LspServerId }): Promise<LspLogEntry[]> => JSON.parse(this.nativeLsp().lspLogsJson(params.serverId)) as LspLogEntry[],
    request: async (params: { serverId: LspServerId; method: string; params?: JsonValue; timeoutMs?: number }): Promise<JsonValue> => {
      const span = runtimePerfSpan("lsp", "request.native", { serverId: params.serverId, method: params.method, timeoutMs: params.timeoutMs }, 8);
      const result = JSON.parse(String(await this.nativeLsp().lspRequestJsonAsync(params.serverId, params.method, JSON.stringify(params.params ?? null), params.timeoutMs ?? null))) as JsonValue;
      span.end();
      return result;
    },
    notify: async (params: { serverId: LspServerId; method: string; params?: JsonValue }): Promise<OkResult> => {
      this.nativeLsp().lspNotifyJson(params.serverId, params.method, JSON.stringify(params.params ?? null));
      return { ok: true };
    },
  };

  readonly resources = {
    save: async (params: { surfaceId: SurfaceId }): Promise<OkResult> => {
      this.session.saveSurfaceDocument(params.surfaceId);
      return { ok: true };
    },
  };

  private nativeLsp(): NativeRuntimeSession & {
    lspRegisterServer(configJson: string): void;
    lspConfiguredServersJson(): string;
    lspStartServer(id: string, rootUri: string | null): number;
    lspShutdownServer(serverId: number): void;
    lspStatusJsonAsync(): Promise<string>;
    lspDiagnosticsJson(): string;
    lspLogsJson(serverId: number): string;
    lspRequestJsonAsync(serverId: number, method: string, paramsJson: string, timeoutMs: number | null): Promise<string>;
    lspNotifyJson(serverId: number, method: string, paramsJson: string): void;
    previewPathCacheKey(path: string): string;
    readSurfaceRevision(surfaceId: number): number;
  } {
    return this.session as ReturnType<RuntimeNativeClient["nativeLsp"]>;
  }

  private readonly session: NativeRuntimeSession;

  constructor(options: RuntimeNativeClientOptions = {}) {
    this.session = options.session ?? new NativeRuntimeSession();
  }

  async request(method: string, params: unknown = {}): Promise<unknown> {
    switch (method) {
      case "server.ping":
        return this.server.ping();
      case "syntax.highlightLegend":
        return this.syntax.highlightLegend();
      case "documents.openFile":
        return this.documents.openFile(params as Parameters<RuntimeApi["documents"]["openFile"]>[0]);
      case "documents.createScratch":
        return this.documents.createScratch(params as Parameters<RuntimeApi["documents"]["createScratch"]>[0]);
      case "surfaces.create":
        return this.surfaces.create(params as Parameters<RuntimeApi["surfaces"]["create"]>[0]);
      case "surfaces.destroy":
        return this.surfaces.destroy(params as Parameters<RuntimeApi["surfaces"]["destroy"]>[0]);
      case "surfaces.loadFile":
        return this.surfaces.loadFile(params as Parameters<RuntimeApi["surfaces"]["loadFile"]>[0]);
      case "surfaces.lineCount":
        return this.surfaces.lineCount(params as Parameters<RuntimeApi["surfaces"]["lineCount"]>[0]);
      case "surfaces.setViewport":
        return this.surfaces.setViewport(params as Parameters<RuntimeApi["surfaces"]["setViewport"]>[0]);
      case "surfaces.setMetrics":
        return this.surfaces.setMetrics(params as Parameters<RuntimeApi["surfaces"]["setMetrics"]>[0]);
      case "surfaces.setSoftWrap":
        return this.surfaces.setSoftWrap(params as Parameters<RuntimeApi["surfaces"]["setSoftWrap"]>[0]);
      case "surfaces.setCursorSemantics":
        return this.surfaces.setCursorSemantics(params as Parameters<RuntimeApi["surfaces"]["setCursorSemantics"]>[0]);
      case "surfaces.applyOperation":
        return this.surfaces.applyOperation(params as Parameters<RuntimeApi["surfaces"]["applyOperation"]>[0]);
      case "surfaces.readState":
        return this.surfaces.readState(params as Parameters<RuntimeApi["surfaces"]["readState"]>[0]);
      case "surfaces.setActiveCursor":
        return this.surfaces.setActiveCursor(params as Parameters<RuntimeApi["surfaces"]["setActiveCursor"]>[0]);
      case "surfaces.applyEdit":
        return this.surfaces.applyEdit(params as Parameters<RuntimeApi["surfaces"]["applyEdit"]>[0]);
      case "surfaces.commitUndoCheckpoint":
        return this.surfaces.commitUndoCheckpoint(params as Parameters<RuntimeApi["surfaces"]["commitUndoCheckpoint"]>[0]);
      case "surfaces.undo":
        return this.surfaces.undo(params as Parameters<RuntimeApi["surfaces"]["undo"]>[0]);
      case "surfaces.redo":
        return this.surfaces.redo(params as Parameters<RuntimeApi["surfaces"]["redo"]>[0]);
      case "surfaces.earlier":
        return this.surfaces.earlier(params as Parameters<RuntimeApi["surfaces"]["earlier"]>[0]);
      case "surfaces.later":
        return this.surfaces.later(params as Parameters<RuntimeApi["surfaces"]["later"]>[0]);
      case "surfaces.renderFrame":
        return this.surfaces.renderFrame(params as Parameters<RuntimeApi["surfaces"]["renderFrame"]>[0]);
      case "search.ensureIndex":
        return this.search.ensureIndex(params as Parameters<RuntimeApi["search"]["ensureIndex"]>[0]);
      case "search.status":
        return this.search.status(params as Parameters<RuntimeApi["search"]["status"]>[0]);
      case "search.refresh":
        return this.search.refresh(params as Parameters<RuntimeApi["search"]["refresh"]>[0]);
      case "search.queryFiles":
        return this.search.queryFiles(params as Parameters<RuntimeApi["search"]["queryFiles"]>[0]);
      case "search.queryDirs":
        return this.search.queryDirs(params as Parameters<RuntimeApi["search"]["queryDirs"]>[0]);
      case "search.queryMixed":
        return this.search.queryMixed(params as Parameters<RuntimeApi["search"]["queryMixed"]>[0]);
      case "search.queryGrep":
        return this.search.queryGrep(params as Parameters<RuntimeApi["search"]["queryGrep"]>[0]);
      case "search.recordSelection":
        return this.search.recordSelection(params as Parameters<RuntimeApi["search"]["recordSelection"]>[0]);
      case "search.readTextPreview":
        return this.search.readTextPreview(params as Parameters<RuntimeApi["search"]["readTextPreview"]>[0]);
      case "registers.read":
        return this.registers.read(params as Parameters<RuntimeApi["registers"]["read"]>[0]);
      case "registers.write":
        return this.registers.write(params as Parameters<RuntimeApi["registers"]["write"]>[0]);
      case "clipboard.read":
        return this.clipboard.read(params as Parameters<RuntimeApi["clipboard"]["read"]>[0]);
      case "clipboard.write":
        return this.clipboard.write(params as Parameters<RuntimeApi["clipboard"]["write"]>[0]);
      case "lsp.registerServer":
        return this.lsp.registerServer(params as Parameters<RuntimeApi["lsp"]["registerServer"]>[0]);
      case "lsp.configuredServers":
        return this.lsp.configuredServers();
      case "lsp.startServer":
        return this.lsp.startServer(params as Parameters<RuntimeApi["lsp"]["startServer"]>[0]);
      case "lsp.shutdownServer":
        return this.lsp.shutdownServer(params as Parameters<RuntimeApi["lsp"]["shutdownServer"]>[0]);
      case "lsp.status":
        return this.lsp.status();
      case "lsp.diagnostics":
        return this.lsp.diagnostics();
      case "lsp.logs":
        return this.lsp.logs(params as Parameters<RuntimeApi["lsp"]["logs"]>[0]);
      case "lsp.request":
        return this.lsp.request(params as Parameters<RuntimeApi["lsp"]["request"]>[0]);
      case "lsp.notify":
        return this.lsp.notify(params as Parameters<RuntimeApi["lsp"]["notify"]>[0]);
      case "resources.save":
        return this.resources.save(params as Parameters<RuntimeApi["resources"]["save"]>[0]);
      default:
        throw new RuntimeClientError(`unknown runtime method ${method}`);
    }
  }

  dispose(): void {}
}

function runtimePerfSpan(category: string, phase: string, data: Record<string, unknown> = {}, slowMs = 16) {
  const start = performance.now();
  return {
    end(extra: Record<string, unknown> = {}) {
      const ms = Math.round((performance.now() - start) * 10) / 10;
      if (!runtimePerfEnabled()) return;
      emitRuntimePerfLog(category, phase, { ms, ...data, ...extra });
    },
  };
}

let runtimePerfAnnounced = false;

function runtimePerfEnabled(): boolean {
  const enabled = readRuntimePerfFlag();
  if (enabled && !runtimePerfAnnounced) {
    runtimePerfAnnounced = true;
    emitRuntimePerfLog("runtime", "diagnostics.enabled", { source: runtimePerfFlagSource() });
  }
  return enabled;
}

function readRuntimePerfFlag(): boolean {
  const globalValue = (globalThis as { __THE_EDITOR_PERF__?: unknown }).__THE_EDITOR_PERF__;
  if (globalValue === true || globalValue === "1" || globalValue === "true") return true;
  if (readRuntimePerfBridge()?.perfEnabled === true) return true;
  try {
    const storage = typeof window !== "undefined" ? window.localStorage : undefined;
    if (storage?.getItem("the-editor.perf") === "1" || storage?.getItem("the-editor.perf") === "true") return true;
  } catch {
    // Ignore storage access failures.
  }
  const env = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process?.env;
  return env?.THE_EDITOR_PERF === "1" || env?.THE_EDITOR_PERF === "true";
}

function runtimePerfFlagSource(): string {
  const globalValue = (globalThis as { __THE_EDITOR_PERF__?: unknown }).__THE_EDITOR_PERF__;
  if (globalValue === true || globalValue === "1" || globalValue === "true") return "globalThis.__THE_EDITOR_PERF__";
  if (readRuntimePerfBridge()?.perfEnabled === true) return "window.theEditor.perfEnabled";
  try {
    const storage = typeof window !== "undefined" ? window.localStorage : undefined;
    if (storage?.getItem("the-editor.perf") === "1" || storage?.getItem("the-editor.perf") === "true") return "localStorage.the-editor.perf";
  } catch {
    // Ignore storage access failures.
  }
  return "THE_EDITOR_PERF";
}

function emitRuntimePerfLog(category: string, phase: string, data: Record<string, unknown>): void {
  const bridge = readRuntimePerfBridge();
  if (bridge?.perfLog) {
    bridge.perfLog({ category, phase, data });
    return;
  }
  console.info(`[perf:${category}] ${phase}`, data);
}

function readRuntimePerfBridge(): { perfEnabled?: boolean; perfLog?: (payload: { category: string; phase: string; data?: Record<string, unknown> }) => void } | undefined {
  return typeof window !== "undefined"
    ? (window as unknown as { theEditor?: { perfEnabled?: boolean; perfLog?: (payload: { category: string; phase: string; data?: Record<string, unknown> }) => void } }).theEditor
    : undefined;
}

function surfaceStateFromJson(json: string): SurfaceState {
  return JSON.parse(json) as SurfaceState;
}

function toNativeLspServerConfig(config: LspServerConfig): Record<string, unknown> {
  return {
    id: config.id,
    command: config.command,
    args: config.args ?? [],
    env: config.env ?? {},
    languages: config.languages ?? [],
    file_types: config.fileTypes ?? [],
    initialization_options: config.initializationOptions ?? null,
  };
}

export function createRuntimeClient(options?: RuntimeNativeClientOptions): RuntimeNativeClient {
  return new RuntimeNativeClient(options);
}

export const spawnRuntimeClient = createRuntimeClient;
