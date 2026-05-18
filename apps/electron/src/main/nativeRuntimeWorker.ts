import { parentPort } from "node:worker_threads";
import { NativeRuntimeSession } from "@the-editor/runtime-node";
import type { RuntimeApi } from "@the-editor/contracts/runtime";
import type { CellGridMetrics, CursorSemantics, DocumentId, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";

interface RuntimeWorkerRequest {
  id: number;
  method: string;
  params?: unknown;
}

interface RuntimeWorkerResponse {
  id: number;
  result?: unknown;
  error?: { name?: string; message: string; stack?: string };
}

class RuntimeWorkerError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RuntimeWorkerError";
  }
}

const session = new NativeRuntimeSession();

async function dispatch(method: string, params: unknown = {}): Promise<unknown> {
  switch (method) {
    case "server.ping": return { ok: true };
    case "syntax.highlightLegend": {
      const read = (session as { readSyntaxHighlightLegendJson?: () => string }).readSyntaxHighlightLegendJson;
      if (typeof read !== "function") return {};
      return JSON.parse(read.call(session)) as Record<number, string>;
    }
    case "syntax.drainWork": {
      const p = params as Parameters<RuntimeApi["syntax"]["drainWork"]>[0] | undefined;
      const drain = (session as { drainSyntaxWork?: (maxDocuments?: number, budgetMs?: number) => number }).drainSyntaxWork;
      if (typeof drain !== "function") return { advanced: 0 };
      return { advanced: drain.call(session, p?.maxDocuments, p?.budgetMs) };
    }
    case "documents.openFile": {
      const p = params as Parameters<RuntimeApi["documents"]["openFile"]>[0];
      return { documentId: session.openFile(p.path) };
    }
    case "documents.createScratch": {
      const p = params as Parameters<RuntimeApi["documents"]["createScratch"]>[0];
      return { documentId: session.createScratch(p.text ?? "") };
    }
    case "workspace.addRoot": {
      const p = params as Parameters<RuntimeApi["workspace"]["addRoot"]>[0];
      return JSON.parse(session.addWorkspaceRootJson(p.path));
    }
    case "workspace.snapshot": {
      const p = params as Parameters<RuntimeApi["workspace"]["snapshot"]>[0];
      return JSON.parse(session.readWorkspaceSnapshotJson(p.rootId));
    }
    case "workspace.refreshRoot": {
      const p = params as Parameters<RuntimeApi["workspace"]["refreshRoot"]>[0];
      return JSON.parse(session.refreshWorkspaceRootJson(p.rootId));
    }
    case "workspace.refresh": return JSON.parse(session.refreshWorkspaceRootsJson());
    case "surfaces.create": {
      const p = params as Parameters<RuntimeApi["surfaces"]["create"]>[0];
      return { surfaceId: session.createSurface(p.documentId, p.viewport ?? null) };
    }
    case "surfaces.destroy": {
      const p = params as Parameters<RuntimeApi["surfaces"]["destroy"]>[0];
      session.destroySurface(p.surfaceId); return { ok: true };
    }
    case "surfaces.loadFile": {
      const p = params as Parameters<RuntimeApi["surfaces"]["loadFile"]>[0];
      session.loadSurfaceFile(p.surfaceId, p.path); return { ok: true };
    }
    case "surfaces.lineCount": {
      const p = params as { surfaceId: SurfaceId }; return { lineCount: session.readSurfaceLineCount(p.surfaceId) };
    }
    case "surfaces.readViewport": {
      const p = params as { surfaceId: SurfaceId }; return { viewport: session.readSurfaceViewport(p.surfaceId) };
    }
    case "surfaces.setViewport": {
      const p = params as { surfaceId: SurfaceId; viewport: Viewport }; session.setSurfaceViewport(p.surfaceId, p.viewport); return { ok: true };
    }
    case "surfaces.setMetrics": {
      const p = params as { surfaceId: SurfaceId; metrics: CellGridMetrics }; session.setSurfaceMeasurements(p.surfaceId, p.metrics); return { ok: true };
    }
    case "surfaces.setSoftWrap": {
      const p = params as Parameters<RuntimeApi["surfaces"]["setSoftWrap"]>[0]; session.setSurfaceSoftWrap(p.surfaceId, p.softWrap); return { ok: true };
    }
    case "surfaces.setCursorSemantics": {
      const p = params as { surfaceId: SurfaceId; semantics: CursorSemantics }; session.setSurfaceCursorSemantics(p.surfaceId, p.semantics); return { ok: true };
    }
    case "surfaces.applyOperation": {
      const p = params as Parameters<RuntimeApi["surfaces"]["applyOperation"]>[0]; session.applySurfaceOperation(p.surfaceId, p.operation); return { ok: true };
    }
    case "surfaces.readState": {
      const p = params as { surfaceId: SurfaceId };
      return {
        text: session.readSurfaceText(p.surfaceId),
        revision: session.readSurfaceRevision(p.surfaceId),
        lineEnding: session.readSurfaceLineEnding(p.surfaceId),
        lineCount: session.readSurfaceLineCount(p.surfaceId),
        selection: session.readSurfaceSelection(p.surfaceId),
        activeCursorId: session.readSurfaceActiveCursor(p.surfaceId),
        cursorSemantics: session.readSurfaceCursorSemantics(p.surfaceId),
        viewport: session.readSurfaceViewport(p.surfaceId),
      };
    }
    case "surfaces.readText": {
      const p = params as { surfaceId: SurfaceId };
      return { text: session.readSurfaceText(p.surfaceId) };
    }
    case "surfaces.readRevision": {
      const p = params as { surfaceId: SurfaceId };
      return { revision: session.readSurfaceRevision(p.surfaceId) };
    }
    case "surfaces.readLineEnding": {
      const p = params as { surfaceId: SurfaceId };
      return { lineEnding: session.readSurfaceLineEnding(p.surfaceId) };
    }
    case "surfaces.readSelection": {
      const p = params as { surfaceId: SurfaceId };
      return { selection: session.readSurfaceSelection(p.surfaceId), activeCursorId: session.readSurfaceActiveCursor(p.surfaceId) };
    }
    case "surfaces.readCursorSemantics": {
      const p = params as { surfaceId: SurfaceId };
      return { cursorSemantics: session.readSurfaceCursorSemantics(p.surfaceId) };
    }
    case "surfaces.setActiveCursor": {
      const p = params as Parameters<RuntimeApi["surfaces"]["setActiveCursor"]>[0]; session.setSurfaceActiveCursor(p.surfaceId, p.cursorId); return { ok: true };
    }
    case "surfaces.applyEdit": {
      const p = params as Parameters<RuntimeApi["surfaces"]["applyEdit"]>[0]; session.applySurfaceEdit(p.surfaceId, p.edit); return { ok: true };
    }
    case "surfaces.completeWords": {
      const p = params as Parameters<RuntimeApi["surfaces"]["completeWords"]>[0]; return JSON.parse(session.completeSurfaceWordsJson(p.surfaceId, p.cursorOffset, p.query, p.limit ?? null));
    }
    case "surfaces.commitUndoCheckpoint": {
      const p = params as { surfaceId: SurfaceId }; session.commitSurfaceUndoCheckpoint(p.surfaceId); return { ok: true };
    }
    case "surfaces.undo": {
      const p = params as Parameters<RuntimeApi["surfaces"]["undo"]>[0]; session.undoSurface(p.surfaceId, p.count ?? 1); return { ok: true };
    }
    case "surfaces.redo": {
      const p = params as Parameters<RuntimeApi["surfaces"]["redo"]>[0]; session.redoSurface(p.surfaceId, p.count ?? 1); return { ok: true };
    }
    case "surfaces.earlier": {
      const p = params as Parameters<RuntimeApi["surfaces"]["earlier"]>[0]; session.earlierSurface(p.surfaceId, p.count ?? 1); return { ok: true };
    }
    case "surfaces.later": {
      const p = params as Parameters<RuntimeApi["surfaces"]["later"]>[0]; session.laterSurface(p.surfaceId, p.count ?? 1); return { ok: true };
    }
    case "surfaces.renderFrame": {
      const p = params as { surfaceId: SurfaceId; viewport: Viewport; metrics: CellGridMetrics }; return session.renderFrame(p.surfaceId, p.viewport, p.metrics) as RenderFrame;
    }
    case "search.ensureIndex": { const p = params as Parameters<RuntimeApi["search"]["ensureIndex"]>[0]; return JSON.parse(session.ensureSearchIndexJson(p.rootPath)); }
    case "search.status": { const p = params as Parameters<RuntimeApi["search"]["status"]>[0]; return JSON.parse(session.readSearchIndexStatusJson(p.indexId)); }
    case "search.refresh": { const p = params as Parameters<RuntimeApi["search"]["refresh"]>[0]; session.refreshSearchIndex(p.indexId); return { ok: true }; }
    case "search.queryFiles": { const p = params as Parameters<RuntimeApi["search"]["queryFiles"]>[0]; return JSON.parse(session.querySearchFilesJson(p.indexId, p.query, p.offset ?? null, p.limit ?? null, p.currentPath ?? null)); }
    case "search.queryDirs": { const p = params as Parameters<RuntimeApi["search"]["queryDirs"]>[0]; return JSON.parse(session.querySearchDirsJson(p.indexId, p.query, p.offset ?? null, p.limit ?? null, p.currentPath ?? null)); }
    case "search.queryMixed": { const p = params as Parameters<RuntimeApi["search"]["queryMixed"]>[0]; return JSON.parse(session.querySearchMixedJson(p.indexId, p.query, p.offset ?? null, p.limit ?? null, p.currentPath ?? null)); }
    case "search.queryGrep": {
      const p = params as Parameters<RuntimeApi["search"]["queryGrep"]>[0];
      return JSON.parse(session.querySearchGrepJson(p.indexId, p.query, p.offset ?? null, p.limit ?? null, p.currentPath ?? null, p.beforeContext ?? null, p.afterContext ?? null));
    }
    case "search.recordSelection": { const p = params as Parameters<RuntimeApi["search"]["recordSelection"]>[0]; session.recordSearchSelection(p.indexId, p.query, p.path); return { ok: true }; }
    case "search.previewPathCacheKey": { const p = params as Parameters<RuntimeApi["search"]["previewPathCacheKey"]>[0]; return session.previewPathCacheKey(p.path); }
    case "search.readTextPreview": {
      const p = params as Parameters<RuntimeApi["search"]["readTextPreview"]>[0];
      return JSON.parse(session.readTextPreviewJson(p.path, p.maxBytes ?? null, p.maxLines ?? null, p.aroundLine ?? null, p.contextLines ?? null));
    }
    case "vcs.queryHunks": {
      const p = params as Parameters<RuntimeApi["vcs"]["queryHunks"]>[0];
      return { hunks: JSON.parse(session.queryVcsHunksJson(p.surfaceId, p.rootPath, p.relativePath)) };
    }
    case "registers.read": { const p = params as Parameters<RuntimeApi["registers"]["read"]>[0]; return { values: session.readRegister(p.name) ?? null }; }
    case "registers.write": { const p = params as Parameters<RuntimeApi["registers"]["write"]>[0]; session.writeRegister(p.name, p.values); return { ok: true }; }
    case "clipboard.read": { const p = params as Parameters<RuntimeApi["clipboard"]["read"]>[0]; return { value: session.readClipboard(p.kind) ?? null }; }
    case "clipboard.write": { const p = params as Parameters<RuntimeApi["clipboard"]["write"]>[0]; session.writeClipboard(p.value, p.kind); return { ok: true }; }
    case "resources.status": { const p = params as Parameters<RuntimeApi["resources"]["status"]>[0]; return JSON.parse(session.readResourceStatusJson(p.surfaceId)); }
    case "resources.save": { const p = params as Parameters<RuntimeApi["resources"]["save"]>[0]; session.saveSurfaceDocument(p.surfaceId, p.conflict === "overwrite"); return { ok: true }; }
    case "resources.reload": { const p = params as Parameters<RuntimeApi["resources"]["reload"]>[0]; session.reloadSurfaceDocument(p.surfaceId); return { ok: true }; }
    default: throw new RuntimeWorkerError(`unknown runtime method ${method}`);
  }
}

function serializeError(error: unknown): RuntimeWorkerResponse["error"] {
  if (error instanceof Error) return { name: error.name, message: error.message, stack: error.stack };
  return { message: String(error) };
}

parentPort?.on("message", (request: RuntimeWorkerRequest) => {
  void dispatch(request.method, request.params).then(
    (result) => parentPort?.postMessage({ id: request.id, result } satisfies RuntimeWorkerResponse),
    (error) => parentPort?.postMessage({ id: request.id, error: serializeError(error) } satisfies RuntimeWorkerResponse),
  );
});
