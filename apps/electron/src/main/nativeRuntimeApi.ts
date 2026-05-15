import type { RuntimeApi } from "@the-editor/contracts/runtime";
import { NativeRuntimeSession } from "@the-editor/runtime-node";
import type {
  CellGridMetrics,
  ClipboardKind,
  CursorSemantics,
  DocumentId,
  DocumentOpened,
  OkResult,
  RenderFrame,
  SurfaceCreated,
  SurfaceEdit,
  SurfaceId,
  SurfaceOperation,
  SurfaceState,
  Viewport,
} from "@the-editor/contracts/runtime";

class RuntimeBrokerError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RuntimeBrokerError";
  }
}

class RuntimeOperationQueue {
  private tail: Promise<unknown> = Promise.resolve();

  run<T>(operation: () => T | Promise<T>): Promise<T> {
    const runAfterTail = this.tail.catch(() => undefined).then(operation);
    this.tail = runAfterTail.catch(() => undefined);
    return runAfterTail;
  }
}

export class MainNativeRuntimeApi implements RuntimeApi {
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
    readViewport: async (params: { surfaceId: SurfaceId }): Promise<{ viewport: Viewport }> => ({
      viewport: this.session.readSurfaceViewport(params.surfaceId),
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
    applyOperation: async (params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<OkResult> => {
      this.session.applySurfaceOperation(params.surfaceId, params.operation);
      return { ok: true };
    },
    readState: async (params: { surfaceId: SurfaceId }): Promise<SurfaceState> => ({
      text: this.session.readSurfaceText(params.surfaceId),
      revision: this.session.readSurfaceRevision(params.surfaceId),
      lineEnding: this.session.readSurfaceLineEnding(params.surfaceId),
      lineCount: this.session.readSurfaceLineCount(params.surfaceId),
      selection: this.session.readSurfaceSelection(params.surfaceId),
      activeCursorId: this.session.readSurfaceActiveCursor(params.surfaceId),
      cursorSemantics: this.session.readSurfaceCursorSemantics(params.surfaceId) as CursorSemantics,
      viewport: this.session.readSurfaceViewport(params.surfaceId),
    }),
    setActiveCursor: async (params: { surfaceId: SurfaceId; cursorId: number | null }): Promise<OkResult> => {
      this.session.setSurfaceActiveCursor(params.surfaceId, params.cursorId);
      return { ok: true };
    },
    applyEdit: async (params: { surfaceId: SurfaceId; edit: SurfaceEdit }): Promise<OkResult> => {
      this.session.applySurfaceEdit(params.surfaceId, params.edit);
      return { ok: true };
    },
    completeWords: async (params: { surfaceId: SurfaceId; cursorOffset: number; query: string; limit?: number }): Promise<Array<{ label: string }>> => (
      JSON.parse(this.session.completeSurfaceWordsJson(params.surfaceId, params.cursorOffset, params.query, params.limit ?? null)) as Array<{ label: string }>
    ),
    commitUndoCheckpoint: async (params: { surfaceId: SurfaceId }): Promise<OkResult> => {
      this.session.commitSurfaceUndoCheckpoint(params.surfaceId);
      return { ok: true };
    },
    undo: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => {
      this.session.undoSurface(params.surfaceId, params.count ?? 1);
      return { ok: true };
    },
    redo: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => {
      this.session.redoSurface(params.surfaceId, params.count ?? 1);
      return { ok: true };
    },
    earlier: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => {
      this.session.earlierSurface(params.surfaceId, params.count ?? 1);
      return { ok: true };
    },
    later: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => {
      this.session.laterSurface(params.surfaceId, params.count ?? 1);
      return { ok: true };
    },
    renderFrame: async (params: { surfaceId: SurfaceId; viewport: Viewport; metrics: CellGridMetrics }): Promise<RenderFrame> => {
      return this.session.renderFrame(params.surfaceId, params.viewport, params.metrics) as RenderFrame;
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

  readonly resources = {
    save: async (params: { surfaceId: SurfaceId }): Promise<OkResult> => {
      this.session.saveSurfaceDocument(params.surfaceId);
      return { ok: true };
    },
  };

  private readonly session = new NativeRuntimeSession();
  private readonly queue = new RuntimeOperationQueue();

  async renderFrame(params: { surfaceId: SurfaceId; viewport: Viewport; metrics: CellGridMetrics }): Promise<RenderFrame> {
    return this.queue.run(() => this.session.renderFrame(params.surfaceId, params.viewport, params.metrics) as RenderFrame);
  }

  async request(method: string, params: unknown = {}): Promise<unknown> {
    return this.queue.run(() => this.dispatchRequest(method, params));
  }

  private async dispatchRequest(method: string, params: unknown = {}): Promise<unknown> {
    switch (method) {
      case "server.ping": return this.server.ping();
      case "syntax.highlightLegend": return this.syntax.highlightLegend();
      case "documents.openFile": return this.documents.openFile(params as Parameters<RuntimeApi["documents"]["openFile"]>[0]);
      case "documents.createScratch": return this.documents.createScratch(params as Parameters<RuntimeApi["documents"]["createScratch"]>[0]);
      case "surfaces.create": return this.surfaces.create(params as Parameters<RuntimeApi["surfaces"]["create"]>[0]);
      case "surfaces.destroy": return this.surfaces.destroy(params as Parameters<RuntimeApi["surfaces"]["destroy"]>[0]);
      case "surfaces.loadFile": return this.surfaces.loadFile(params as Parameters<RuntimeApi["surfaces"]["loadFile"]>[0]);
      case "surfaces.lineCount": return this.surfaces.lineCount(params as Parameters<RuntimeApi["surfaces"]["lineCount"]>[0]);
      case "surfaces.readViewport": return this.surfaces.readViewport(params as Parameters<RuntimeApi["surfaces"]["readViewport"]>[0]);
      case "surfaces.setViewport": return this.surfaces.setViewport(params as Parameters<RuntimeApi["surfaces"]["setViewport"]>[0]);
      case "surfaces.setMetrics": return this.surfaces.setMetrics(params as Parameters<RuntimeApi["surfaces"]["setMetrics"]>[0]);
      case "surfaces.setSoftWrap": return this.surfaces.setSoftWrap(params as Parameters<RuntimeApi["surfaces"]["setSoftWrap"]>[0]);
      case "surfaces.setCursorSemantics": return this.surfaces.setCursorSemantics(params as Parameters<RuntimeApi["surfaces"]["setCursorSemantics"]>[0]);
      case "surfaces.applyOperation": return this.surfaces.applyOperation(params as Parameters<RuntimeApi["surfaces"]["applyOperation"]>[0]);
      case "surfaces.readState": return this.surfaces.readState(params as Parameters<RuntimeApi["surfaces"]["readState"]>[0]);
      case "surfaces.setActiveCursor": return this.surfaces.setActiveCursor(params as Parameters<RuntimeApi["surfaces"]["setActiveCursor"]>[0]);
      case "surfaces.applyEdit": return this.surfaces.applyEdit(params as Parameters<RuntimeApi["surfaces"]["applyEdit"]>[0]);
      case "surfaces.completeWords": return this.surfaces.completeWords(params as Parameters<RuntimeApi["surfaces"]["completeWords"]>[0]);
      case "surfaces.commitUndoCheckpoint": return this.surfaces.commitUndoCheckpoint(params as Parameters<RuntimeApi["surfaces"]["commitUndoCheckpoint"]>[0]);
      case "surfaces.undo": return this.surfaces.undo(params as Parameters<RuntimeApi["surfaces"]["undo"]>[0]);
      case "surfaces.redo": return this.surfaces.redo(params as Parameters<RuntimeApi["surfaces"]["redo"]>[0]);
      case "surfaces.earlier": return this.surfaces.earlier(params as Parameters<RuntimeApi["surfaces"]["earlier"]>[0]);
      case "surfaces.later": return this.surfaces.later(params as Parameters<RuntimeApi["surfaces"]["later"]>[0]);
      case "surfaces.renderFrame": return this.surfaces.renderFrame(params as Parameters<RuntimeApi["surfaces"]["renderFrame"]>[0]);
      case "search.ensureIndex": return this.search.ensureIndex(params as Parameters<RuntimeApi["search"]["ensureIndex"]>[0]);
      case "search.status": return this.search.status(params as Parameters<RuntimeApi["search"]["status"]>[0]);
      case "search.refresh": return this.search.refresh(params as Parameters<RuntimeApi["search"]["refresh"]>[0]);
      case "search.queryFiles": return this.search.queryFiles(params as Parameters<RuntimeApi["search"]["queryFiles"]>[0]);
      case "search.queryDirs": return this.search.queryDirs(params as Parameters<RuntimeApi["search"]["queryDirs"]>[0]);
      case "search.queryMixed": return this.search.queryMixed(params as Parameters<RuntimeApi["search"]["queryMixed"]>[0]);
      case "search.queryGrep": return this.search.queryGrep(params as Parameters<RuntimeApi["search"]["queryGrep"]>[0]);
      case "search.recordSelection": return this.search.recordSelection(params as Parameters<RuntimeApi["search"]["recordSelection"]>[0]);
      case "search.previewPathCacheKey": return this.search.previewPathCacheKey(params as Parameters<RuntimeApi["search"]["previewPathCacheKey"]>[0]);
      case "search.readTextPreview": return this.search.readTextPreview(params as Parameters<RuntimeApi["search"]["readTextPreview"]>[0]);
      case "registers.read": return this.registers.read(params as Parameters<RuntimeApi["registers"]["read"]>[0]);
      case "registers.write": return this.registers.write(params as Parameters<RuntimeApi["registers"]["write"]>[0]);
      case "clipboard.read": return this.clipboard.read(params as Parameters<RuntimeApi["clipboard"]["read"]>[0]);
      case "clipboard.write": return this.clipboard.write(params as Parameters<RuntimeApi["clipboard"]["write"]>[0]);
      case "resources.save": return this.resources.save(params as Parameters<RuntimeApi["resources"]["save"]>[0]);
      default:
        throw new RuntimeBrokerError(`unknown runtime method ${method}`);
    }
  }
}
