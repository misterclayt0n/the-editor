import { existsSync } from "node:fs";
import { Worker } from "node:worker_threads";
import { fileURLToPath } from "node:url";
import type { RuntimeApi } from "@the-editor/contracts/runtime";
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
  SurfaceSelectionRange,
  SurfaceId,
  SurfaceOperation,
  SurfaceState,
  Viewport,
  WorkspaceEntriesChanged,
  WorkspaceRootId,
  WorkspaceSnapshot,
} from "@the-editor/contracts/runtime";

class RuntimeBrokerError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RuntimeBrokerError";
  }
}

class RuntimeWorkerError extends Error {
  constructor(message: string, options?: { name?: string; stack?: string }) {
    super(message);
    this.name = options?.name ?? "RuntimeWorkerError";
    if (options?.stack) this.stack = options.stack;
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

interface RuntimeWorkerResponse {
  id: number;
  result?: unknown;
  error?: { name?: string; message: string; stack?: string };
}

function nativeRuntimeWorkerPath(): string {
  const bundled = fileURLToPath(new URL("./nativeRuntimeWorker.cjs", import.meta.url));
  if (existsSync(bundled)) return bundled;
  const builtFromSource = fileURLToPath(new URL("../../dist/main/nativeRuntimeWorker.cjs", import.meta.url));
  if (existsSync(builtFromSource)) return builtFromSource;
  return bundled;
}

class NativeRuntimeWorkerClient {
  private readonly worker = new Worker(nativeRuntimeWorkerPath());
  private nextId = 1;
  private readonly pending = new Map<number, { resolve(value: unknown): void; reject(reason?: unknown): void }>();

  constructor() {
    this.worker.on("message", (response: RuntimeWorkerResponse) => this.receive(response));
    this.worker.on("error", (error) => this.rejectAll(error));
    this.worker.on("exit", (code) => {
      if (code !== 0) this.rejectAll(new RuntimeBrokerError(`runtime worker exited with code ${code}`));
    });
  }

  request(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ id, method, params });
    });
  }

  async dispose(): Promise<void> {
    await this.worker.terminate();
  }

  private receive(response: RuntimeWorkerResponse): void {
    const pending = this.pending.get(response.id);
    if (!pending) return;
    this.pending.delete(response.id);
    if (response.error) {
      pending.reject(new RuntimeWorkerError(response.error.message, response.error));
      return;
    }
    pending.resolve(response.result);
  }

  private rejectAll(error: unknown): void {
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }
}

export class MainNativeRuntimeApi implements RuntimeApi {
  private readonly queue = new RuntimeOperationQueue();
  private readonly runtime = new NativeRuntimeWorkerClient();

  readonly server = {
    ping: async (): Promise<OkResult> => this.call("server.ping", {}),
  };

  readonly syntax = {
    highlightLegend: async (): Promise<Record<number, string>> => this.call("syntax.highlightLegend", {}),
    drainWork: async (params: Parameters<RuntimeApi["syntax"]["drainWork"]>[0] = {}): ReturnType<RuntimeApi["syntax"]["drainWork"]> => this.call("syntax.drainWork", params),
  };

  readonly documents = {
    openFile: async (params: { path: string }): Promise<DocumentOpened> => this.call("documents.openFile", params),
    createScratch: async (params: { text?: string }): Promise<DocumentOpened> => this.call("documents.createScratch", params),
  };

  readonly workspace = {
    addRoot: async (params: { path: string }): Promise<WorkspaceSnapshot> => this.call("workspace.addRoot", params),
    snapshot: async (params: { rootId: WorkspaceRootId }): Promise<WorkspaceSnapshot> => this.call("workspace.snapshot", params),
    refreshRoot: async (params: { rootId: WorkspaceRootId }): Promise<WorkspaceEntriesChanged> => this.call("workspace.refreshRoot", params),
    refresh: async (): Promise<WorkspaceEntriesChanged[]> => this.call("workspace.refresh", {}),
  };

  readonly surfaces = {
    create: async (params: { documentId: DocumentId; viewport?: Viewport }): Promise<SurfaceCreated> => this.call("surfaces.create", params),
    destroy: async (params: { surfaceId: SurfaceId }): Promise<OkResult> => this.call("surfaces.destroy", params),
    loadFile: async (params: { surfaceId: SurfaceId; path: string }): Promise<OkResult> => this.call("surfaces.loadFile", params),
    lineCount: async (params: { surfaceId: SurfaceId }): Promise<{ lineCount: number }> => this.call("surfaces.lineCount", params),
    readViewport: async (params: { surfaceId: SurfaceId }): Promise<{ viewport: Viewport }> => this.call("surfaces.readViewport", params),
    setViewport: async (params: { surfaceId: SurfaceId; viewport: Viewport }): Promise<OkResult> => this.call("surfaces.setViewport", params),
    setMetrics: async (params: { surfaceId: SurfaceId; metrics: CellGridMetrics }): Promise<OkResult> => this.call("surfaces.setMetrics", params),
    setSoftWrap: async (params: { surfaceId: SurfaceId; softWrap: boolean }): Promise<OkResult> => this.call("surfaces.setSoftWrap", params),
    setCursorSemantics: async (params: { surfaceId: SurfaceId; semantics: CursorSemantics }): Promise<OkResult> => this.call("surfaces.setCursorSemantics", params),
    applyOperation: async (params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<OkResult> => this.call("surfaces.applyOperation", params),
    readState: async (params: { surfaceId: SurfaceId }): Promise<SurfaceState> => this.call("surfaces.readState", params),
    readText: async (params: { surfaceId: SurfaceId }): Promise<{ text: string }> => this.call("surfaces.readText", params),
    readRevision: async (params: { surfaceId: SurfaceId }): Promise<{ revision: number }> => this.call("surfaces.readRevision", params),
    readLineEnding: async (params: { surfaceId: SurfaceId }): Promise<{ lineEnding: string }> => this.call("surfaces.readLineEnding", params),
    readSelection: async (params: { surfaceId: SurfaceId }): Promise<{ selection: SurfaceSelectionRange[]; activeCursorId: number | null }> => this.call("surfaces.readSelection", params),
    readCursorSemantics: async (params: { surfaceId: SurfaceId }): Promise<{ cursorSemantics: CursorSemantics }> => this.call("surfaces.readCursorSemantics", params),
    setActiveCursor: async (params: { surfaceId: SurfaceId; cursorId: number | null }): Promise<OkResult> => this.call("surfaces.setActiveCursor", params),
    applyEdit: async (params: { surfaceId: SurfaceId; edit: SurfaceEdit }): Promise<OkResult> => this.call("surfaces.applyEdit", params),
    completeWords: async (params: { surfaceId: SurfaceId; cursorOffset: number; query: string; limit?: number }): Promise<Array<{ label: string }>> => this.call("surfaces.completeWords", params),
    commitUndoCheckpoint: async (params: { surfaceId: SurfaceId }): Promise<OkResult> => this.call("surfaces.commitUndoCheckpoint", params),
    undo: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => this.call("surfaces.undo", params),
    redo: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => this.call("surfaces.redo", params),
    earlier: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => this.call("surfaces.earlier", params),
    later: async (params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult> => this.call("surfaces.later", params),
    renderFrame: async (params: { surfaceId: SurfaceId; viewport: Viewport; metrics: CellGridMetrics }): Promise<RenderFrame> => this.call("surfaces.renderFrame", params),
  };

  readonly search = {
    ensureIndex: async (params: Parameters<RuntimeApi["search"]["ensureIndex"]>[0]) => this.call("search.ensureIndex", params) as ReturnType<RuntimeApi["search"]["ensureIndex"]>,
    status: async (params: Parameters<RuntimeApi["search"]["status"]>[0]) => this.call("search.status", params) as ReturnType<RuntimeApi["search"]["status"]>,
    refresh: async (params: Parameters<RuntimeApi["search"]["refresh"]>[0]) => this.call("search.refresh", params) as ReturnType<RuntimeApi["search"]["refresh"]>,
    queryFiles: async (params: Parameters<RuntimeApi["search"]["queryFiles"]>[0]) => this.call("search.queryFiles", params) as ReturnType<RuntimeApi["search"]["queryFiles"]>,
    queryDirs: async (params: Parameters<RuntimeApi["search"]["queryDirs"]>[0]) => this.call("search.queryDirs", params) as ReturnType<RuntimeApi["search"]["queryDirs"]>,
    queryMixed: async (params: Parameters<RuntimeApi["search"]["queryMixed"]>[0]) => this.call("search.queryMixed", params) as ReturnType<RuntimeApi["search"]["queryMixed"]>,
    queryGrep: async (params: Parameters<RuntimeApi["search"]["queryGrep"]>[0]) => this.call("search.queryGrep", params) as ReturnType<RuntimeApi["search"]["queryGrep"]>,
    recordSelection: async (params: Parameters<RuntimeApi["search"]["recordSelection"]>[0]) => this.call("search.recordSelection", params) as ReturnType<RuntimeApi["search"]["recordSelection"]>,
    previewPathCacheKey: async (params: Parameters<RuntimeApi["search"]["previewPathCacheKey"]>[0]) => this.call("search.previewPathCacheKey", params) as ReturnType<RuntimeApi["search"]["previewPathCacheKey"]>,
    readTextPreview: async (params: Parameters<RuntimeApi["search"]["readTextPreview"]>[0]) => this.call("search.readTextPreview", params) as ReturnType<RuntimeApi["search"]["readTextPreview"]>,
  };

  readonly vcs = {
    queryHunks: async (params: Parameters<RuntimeApi["vcs"]["queryHunks"]>[0]) => this.call("vcs.queryHunks", params) as ReturnType<RuntimeApi["vcs"]["queryHunks"]>,
  };

  readonly registers = {
    read: async (params: { name: string }): Promise<{ values: string[] | null }> => this.call("registers.read", params),
    write: async (params: { name: string; values: string[] }): Promise<OkResult> => this.call("registers.write", params),
  };

  readonly clipboard = {
    read: async (params: { kind: ClipboardKind }): Promise<{ value: string | null }> => this.call("clipboard.read", params),
    write: async (params: { kind: ClipboardKind; value: string }): Promise<OkResult> => this.call("clipboard.write", params),
  };

  readonly resources = {
    status: async (params: Parameters<RuntimeApi["resources"]["status"]>[0]) => this.call("resources.status", params) as ReturnType<RuntimeApi["resources"]["status"]>,
    save: async (params: Parameters<RuntimeApi["resources"]["save"]>[0]): Promise<OkResult> => this.call("resources.save", params),
    reload: async (params: Parameters<RuntimeApi["resources"]["reload"]>[0]): Promise<OkResult> => this.call("resources.reload", params),
  };

  async renderFrame(params: { surfaceId: SurfaceId; viewport: Viewport; metrics: CellGridMetrics }): Promise<RenderFrame> {
    return this.call("surfaces.renderFrame", params);
  }

  async request(method: string, params: unknown = {}): Promise<unknown> {
    return this.call(method, params);
  }

  async dispose(): Promise<void> {
    await this.runtime.dispose();
  }

  private call<T>(method: string, params: unknown): Promise<T> {
    return this.queue.run(() => this.runtime.request(method, params) as Promise<T>);
  }
}
