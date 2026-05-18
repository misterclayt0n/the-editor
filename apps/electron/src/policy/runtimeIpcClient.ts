import type { RuntimeApi } from "@the-editor/contracts/runtime";

interface RuntimeIpcRequest {
  type: "runtime.request";
  id: number;
  method: string;
  params?: unknown;
}

interface RuntimeIpcResponse {
  type: "runtime.response";
  id: number;
  result?: unknown;
  error?: unknown;
}

export interface RuntimeIpcTransport {
  postMessage(message: RuntimeIpcRequest): void;
  onMessage(listener: (message: unknown) => void): void;
}

export class RuntimeIpcClient implements RuntimeApi {
  readonly server = {
    ping: () => this.request("server.ping") as ReturnType<RuntimeApi["server"]["ping"]>,
  };

  readonly syntax = {
    highlightLegend: () => this.request("syntax.highlightLegend") as ReturnType<RuntimeApi["syntax"]["highlightLegend"]>,
    drainWork: (params: Parameters<RuntimeApi["syntax"]["drainWork"]>[0] = {}) => this.request("syntax.drainWork", params) as ReturnType<RuntimeApi["syntax"]["drainWork"]>,
  };

  readonly documents = {
    openFile: (params: Parameters<RuntimeApi["documents"]["openFile"]>[0]) => this.request("documents.openFile", params) as ReturnType<RuntimeApi["documents"]["openFile"]>,
    createScratch: (params: Parameters<RuntimeApi["documents"]["createScratch"]>[0]) => this.request("documents.createScratch", params) as ReturnType<RuntimeApi["documents"]["createScratch"]>,
  };

  readonly workspace = {
    addRoot: (params: Parameters<RuntimeApi["workspace"]["addRoot"]>[0]) => this.request("workspace.addRoot", params) as ReturnType<RuntimeApi["workspace"]["addRoot"]>,
    snapshot: (params: Parameters<RuntimeApi["workspace"]["snapshot"]>[0]) => this.request("workspace.snapshot", params) as ReturnType<RuntimeApi["workspace"]["snapshot"]>,
    refreshRoot: (params: Parameters<RuntimeApi["workspace"]["refreshRoot"]>[0]) => this.request("workspace.refreshRoot", params) as ReturnType<RuntimeApi["workspace"]["refreshRoot"]>,
    refresh: (params: Parameters<RuntimeApi["workspace"]["refresh"]>[0] = {}) => this.request("workspace.refresh", params) as ReturnType<RuntimeApi["workspace"]["refresh"]>,
  };

  readonly surfaces = {
    create: (params: Parameters<RuntimeApi["surfaces"]["create"]>[0]) => this.request("surfaces.create", params) as ReturnType<RuntimeApi["surfaces"]["create"]>,
    destroy: (params: Parameters<RuntimeApi["surfaces"]["destroy"]>[0]) => this.request("surfaces.destroy", params) as ReturnType<RuntimeApi["surfaces"]["destroy"]>,
    loadFile: (params: Parameters<RuntimeApi["surfaces"]["loadFile"]>[0]) => this.request("surfaces.loadFile", params) as ReturnType<RuntimeApi["surfaces"]["loadFile"]>,
    lineCount: (params: Parameters<RuntimeApi["surfaces"]["lineCount"]>[0]) => this.request("surfaces.lineCount", params) as ReturnType<RuntimeApi["surfaces"]["lineCount"]>,
    readViewport: (params: Parameters<RuntimeApi["surfaces"]["readViewport"]>[0]) => this.request("surfaces.readViewport", params) as ReturnType<RuntimeApi["surfaces"]["readViewport"]>,
    setViewport: (params: Parameters<RuntimeApi["surfaces"]["setViewport"]>[0]) => this.request("surfaces.setViewport", params) as ReturnType<RuntimeApi["surfaces"]["setViewport"]>,
    setMetrics: (params: Parameters<RuntimeApi["surfaces"]["setMetrics"]>[0]) => this.request("surfaces.setMetrics", params) as ReturnType<RuntimeApi["surfaces"]["setMetrics"]>,
    setSoftWrap: (params: Parameters<RuntimeApi["surfaces"]["setSoftWrap"]>[0]) => this.request("surfaces.setSoftWrap", params) as ReturnType<RuntimeApi["surfaces"]["setSoftWrap"]>,
    setCursorSemantics: (params: Parameters<RuntimeApi["surfaces"]["setCursorSemantics"]>[0]) => this.request("surfaces.setCursorSemantics", params) as ReturnType<RuntimeApi["surfaces"]["setCursorSemantics"]>,
    applyOperation: (params: Parameters<RuntimeApi["surfaces"]["applyOperation"]>[0]) => this.request("surfaces.applyOperation", params) as ReturnType<RuntimeApi["surfaces"]["applyOperation"]>,
    readState: (params: Parameters<RuntimeApi["surfaces"]["readState"]>[0]) => this.request("surfaces.readState", params) as ReturnType<RuntimeApi["surfaces"]["readState"]>,
    readText: (params: Parameters<RuntimeApi["surfaces"]["readText"]>[0]) => this.request("surfaces.readText", params) as ReturnType<RuntimeApi["surfaces"]["readText"]>,
    readRevision: (params: Parameters<RuntimeApi["surfaces"]["readRevision"]>[0]) => this.request("surfaces.readRevision", params) as ReturnType<RuntimeApi["surfaces"]["readRevision"]>,
    readLineEnding: (params: Parameters<RuntimeApi["surfaces"]["readLineEnding"]>[0]) => this.request("surfaces.readLineEnding", params) as ReturnType<RuntimeApi["surfaces"]["readLineEnding"]>,
    readSelection: (params: Parameters<RuntimeApi["surfaces"]["readSelection"]>[0]) => this.request("surfaces.readSelection", params) as ReturnType<RuntimeApi["surfaces"]["readSelection"]>,
    readCursorSemantics: (params: Parameters<RuntimeApi["surfaces"]["readCursorSemantics"]>[0]) => this.request("surfaces.readCursorSemantics", params) as ReturnType<RuntimeApi["surfaces"]["readCursorSemantics"]>,
    completeWords: (params: Parameters<RuntimeApi["surfaces"]["completeWords"]>[0]) => this.request("surfaces.completeWords", params) as ReturnType<RuntimeApi["surfaces"]["completeWords"]>,
    setActiveCursor: (params: Parameters<RuntimeApi["surfaces"]["setActiveCursor"]>[0]) => this.request("surfaces.setActiveCursor", params) as ReturnType<RuntimeApi["surfaces"]["setActiveCursor"]>,
    applyEdit: (params: Parameters<RuntimeApi["surfaces"]["applyEdit"]>[0]) => this.request("surfaces.applyEdit", params) as ReturnType<RuntimeApi["surfaces"]["applyEdit"]>,
    commitUndoCheckpoint: (params: Parameters<RuntimeApi["surfaces"]["commitUndoCheckpoint"]>[0]) => this.request("surfaces.commitUndoCheckpoint", params) as ReturnType<RuntimeApi["surfaces"]["commitUndoCheckpoint"]>,
    undo: (params: Parameters<RuntimeApi["surfaces"]["undo"]>[0]) => this.request("surfaces.undo", params) as ReturnType<RuntimeApi["surfaces"]["undo"]>,
    redo: (params: Parameters<RuntimeApi["surfaces"]["redo"]>[0]) => this.request("surfaces.redo", params) as ReturnType<RuntimeApi["surfaces"]["redo"]>,
    earlier: (params: Parameters<RuntimeApi["surfaces"]["earlier"]>[0]) => this.request("surfaces.earlier", params) as ReturnType<RuntimeApi["surfaces"]["earlier"]>,
    later: (params: Parameters<RuntimeApi["surfaces"]["later"]>[0]) => this.request("surfaces.later", params) as ReturnType<RuntimeApi["surfaces"]["later"]>,
    renderFrame: (params: Parameters<RuntimeApi["surfaces"]["renderFrame"]>[0]) => this.request("surfaces.renderFrame", params) as ReturnType<RuntimeApi["surfaces"]["renderFrame"]>,
  };

  readonly vcs = {
    queryHunks: (params: Parameters<RuntimeApi["vcs"]["queryHunks"]>[0]) => this.request("vcs.queryHunks", params) as ReturnType<RuntimeApi["vcs"]["queryHunks"]>,
  };

  readonly registers = {
    read: (params: Parameters<RuntimeApi["registers"]["read"]>[0]) => this.request("registers.read", params) as ReturnType<RuntimeApi["registers"]["read"]>,
    write: (params: Parameters<RuntimeApi["registers"]["write"]>[0]) => this.request("registers.write", params) as ReturnType<RuntimeApi["registers"]["write"]>,
  };

  readonly clipboard = {
    read: (params: Parameters<RuntimeApi["clipboard"]["read"]>[0]) => this.request("clipboard.read", params) as ReturnType<RuntimeApi["clipboard"]["read"]>,
    write: (params: Parameters<RuntimeApi["clipboard"]["write"]>[0]) => this.request("clipboard.write", params) as ReturnType<RuntimeApi["clipboard"]["write"]>,
  };

  readonly search = {
    ensureIndex: (params: Parameters<RuntimeApi["search"]["ensureIndex"]>[0]) => this.request("search.ensureIndex", params) as ReturnType<RuntimeApi["search"]["ensureIndex"]>,
    status: (params: Parameters<RuntimeApi["search"]["status"]>[0]) => this.request("search.status", params) as ReturnType<RuntimeApi["search"]["status"]>,
    refresh: (params: Parameters<RuntimeApi["search"]["refresh"]>[0]) => this.request("search.refresh", params) as ReturnType<RuntimeApi["search"]["refresh"]>,
    queryFiles: (params: Parameters<RuntimeApi["search"]["queryFiles"]>[0]) => this.request("search.queryFiles", params) as ReturnType<RuntimeApi["search"]["queryFiles"]>,
    queryDirs: (params: Parameters<RuntimeApi["search"]["queryDirs"]>[0]) => this.request("search.queryDirs", params) as ReturnType<RuntimeApi["search"]["queryDirs"]>,
    queryMixed: (params: Parameters<RuntimeApi["search"]["queryMixed"]>[0]) => this.request("search.queryMixed", params) as ReturnType<RuntimeApi["search"]["queryMixed"]>,
    queryGrep: (params: Parameters<RuntimeApi["search"]["queryGrep"]>[0]) => this.request("search.queryGrep", params) as ReturnType<RuntimeApi["search"]["queryGrep"]>,
    recordSelection: (params: Parameters<RuntimeApi["search"]["recordSelection"]>[0]) => this.request("search.recordSelection", params) as ReturnType<RuntimeApi["search"]["recordSelection"]>,
    previewPathCacheKey: (params: Parameters<RuntimeApi["search"]["previewPathCacheKey"]>[0]) => this.request("search.previewPathCacheKey", params) as ReturnType<RuntimeApi["search"]["previewPathCacheKey"]>,
    readTextPreview: (params: Parameters<RuntimeApi["search"]["readTextPreview"]>[0]) => this.request("search.readTextPreview", params) as ReturnType<RuntimeApi["search"]["readTextPreview"]>,
  };

  readonly resources = {
    save: (params: Parameters<RuntimeApi["resources"]["save"]>[0]) => this.request("resources.save", params) as ReturnType<RuntimeApi["resources"]["save"]>,
  };

  private nextId = 1;
  private readonly pending = new Map<number, { resolve(value: unknown): void; reject(error: unknown): void }>();

  constructor(private readonly transport: RuntimeIpcTransport) {
    transport.onMessage((message) => this.handleMessage(message));
  }

  private request(method: string, params?: unknown): Promise<unknown> {
    const id = this.nextId++;
    this.transport.postMessage({ type: "runtime.request", id, method, params });
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
  }

  private handleMessage(message: unknown): void {
    if (!isRuntimeResponse(message)) return;
    const pending = this.pending.get(message.id);
    if (!pending) return;
    this.pending.delete(message.id);
    if (message.error) pending.reject(deserializeError(message.error));
    else pending.resolve(message.result);
  }
}

export function createRuntimeIpcClient(transport: RuntimeIpcTransport): RuntimeIpcClient {
  return new RuntimeIpcClient(transport);
}

function isRuntimeResponse(value: unknown): value is RuntimeIpcResponse {
  return typeof value === "object"
    && value !== null
    && (value as { type?: unknown }).type === "runtime.response"
    && typeof (value as { id?: unknown }).id === "number";
}

function deserializeError(error: unknown): Error {
  if (typeof error === "object" && error !== null && typeof (error as { message?: unknown }).message === "string") {
    const next = new Error((error as { message: string }).message);
    if (typeof (error as { stack?: unknown }).stack === "string") next.stack = (error as { stack: string }).stack;
    return next;
  }
  return new Error(String(error));
}
