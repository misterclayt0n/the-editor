import type { CellGridMetrics, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";

export {};

declare global {
  interface Window {
    theEditorRender: {
      renderFrame(request: { surfaceId: SurfaceId; viewport: Viewport; metrics: CellGridMetrics }): Promise<RenderFrame>;
    };
    theEditor: {
      openFileDialog(): Promise<string | null>;
      policyRequest(method: string, params?: unknown): Promise<unknown>;
      openExternal(url: string): Promise<unknown>;
      perfEnabled: boolean;
      perfLogPath: string;
      perfLog(payload: { category: string; phase: string; data?: Record<string, unknown> }): void;
      onPolicyEvent(handler: (event: unknown) => void): () => void;
      onAppCommand(handler: (event: unknown) => void): () => void;
      readExtensionFile(path: string): string;
      resolveExtensionPath(specifier: string, importerPath: string): string | null;
    };
  }
}
