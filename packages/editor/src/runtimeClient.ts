import type {
  DocumentId,
  DocumentOpened,
  OkResult,
  RenderFrameDelta,
  RuntimeApi,
  SelectionSpec,
  SurfaceCreated,
  SurfaceId,
  SurfaceSnapshot,
  TransactionSpec,
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
    snapshot: async (params: { surfaceId: SurfaceId }): Promise<SurfaceSnapshot> => {
      return this.session.snapshotSurfaceState(params.surfaceId) as SurfaceSnapshot;
    },
    setSelection: async (params: { surfaceId: SurfaceId; selection: SelectionSpec }): Promise<OkResult> => {
      this.session.setSurfaceSelection(params.surfaceId, params.selection);
      return { ok: true };
    },
    applyTransaction: async (params: {
      surfaceId: SurfaceId;
      transaction: TransactionSpec;
      selection: SelectionSpec;
    }): Promise<OkResult> => {
      this.session.applySurfaceTransaction(params.surfaceId, params.transaction, params.selection);
      return { ok: true };
    },
    renderFrame: async (params: {
      surfaceId: SurfaceId;
      viewport?: Viewport;
      sinceGeneration?: number | null;
    }): Promise<RenderFrameDelta> => {
      const json = this.session.renderFrameJson(
        params.surfaceId,
        params.viewport ?? null,
        params.sinceGeneration ?? null,
      );
      return JSON.parse(json) as RenderFrameDelta;
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
      case "documents.openFile":
        return this.documents.openFile(params as Parameters<RuntimeApi["documents"]["openFile"]>[0]);
      case "documents.createScratch":
        return this.documents.createScratch(params as Parameters<RuntimeApi["documents"]["createScratch"]>[0]);
      case "surfaces.create":
        return this.surfaces.create(params as Parameters<RuntimeApi["surfaces"]["create"]>[0]);
      case "surfaces.setViewport":
        return this.surfaces.setViewport(params as Parameters<RuntimeApi["surfaces"]["setViewport"]>[0]);
      case "surfaces.snapshot":
        return this.surfaces.snapshot(params as Parameters<RuntimeApi["surfaces"]["snapshot"]>[0]);
      case "surfaces.setSelection":
        return this.surfaces.setSelection(params as Parameters<RuntimeApi["surfaces"]["setSelection"]>[0]);
      case "surfaces.applyTransaction":
        return this.surfaces.applyTransaction(params as Parameters<RuntimeApi["surfaces"]["applyTransaction"]>[0]);
      case "surfaces.renderFrame":
        return this.surfaces.renderFrame(params as Parameters<RuntimeApi["surfaces"]["renderFrame"]>[0]);
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
