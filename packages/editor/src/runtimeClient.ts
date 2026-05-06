import type {
  CellGridMetrics,
  ClipboardKind,
  DocumentId,
  DocumentOpened,
  OkResult,
  RenderFrame,
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
    applyOperation: async (params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<OkResult> => {
      this.session.applySurfaceOperation(params.surfaceId, params.operation);
      return { ok: true };
    },
    readState: async (params: { surfaceId: SurfaceId }): Promise<SurfaceState> => ({
      text: this.session.readSurfaceText(params.surfaceId),
      lineEnding: this.session.readSurfaceLineEnding(params.surfaceId),
      selection: this.session.readSurfaceSelection(params.surfaceId),
    }),
    applyEdit: async (params: { surfaceId: SurfaceId; edit: SurfaceEdit }): Promise<OkResult> => {
      this.session.applySurfaceEdit(params.surfaceId, params.edit);
      return { ok: true };
    },
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
    renderFrame: async (params: {
      surfaceId: SurfaceId;
      viewport?: Viewport;
    }): Promise<RenderFrame> => {
      const json = this.session.renderFrameJson(params.surfaceId, params.viewport ?? null);
      return JSON.parse(json) as RenderFrame;
    },
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
      case "surfaces.setViewport":
        return this.surfaces.setViewport(params as Parameters<RuntimeApi["surfaces"]["setViewport"]>[0]);
      case "surfaces.setMetrics":
        return this.surfaces.setMetrics(params as Parameters<RuntimeApi["surfaces"]["setMetrics"]>[0]);
      case "surfaces.setSoftWrap":
        return this.surfaces.setSoftWrap(params as Parameters<RuntimeApi["surfaces"]["setSoftWrap"]>[0]);
      case "surfaces.applyOperation":
        return this.surfaces.applyOperation(params as Parameters<RuntimeApi["surfaces"]["applyOperation"]>[0]);
      case "surfaces.readState":
        return this.surfaces.readState(params as Parameters<RuntimeApi["surfaces"]["readState"]>[0]);
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
      case "registers.read":
        return this.registers.read(params as Parameters<RuntimeApi["registers"]["read"]>[0]);
      case "registers.write":
        return this.registers.write(params as Parameters<RuntimeApi["registers"]["write"]>[0]);
      case "clipboard.read":
        return this.clipboard.read(params as Parameters<RuntimeApi["clipboard"]["read"]>[0]);
      case "clipboard.write":
        return this.clipboard.write(params as Parameters<RuntimeApi["clipboard"]["write"]>[0]);
      case "resources.save":
        return this.resources.save(params as Parameters<RuntimeApi["resources"]["save"]>[0]);
      default:
        throw new RuntimeClientError(`unknown runtime method ${method}`);
    }
  }

  dispose(): void {}
}

export function createRuntimeClient(options?: RuntimeNativeClientOptions): RuntimeNativeClient {
  return new RuntimeNativeClient(options);
}

export const spawnRuntimeClient = createRuntimeClient;
