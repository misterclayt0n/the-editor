export {};

declare global {
  interface Window {
    theEditor: {
      openFileDialog(): Promise<string | null>;
      policyRequest(method: string, params?: unknown): Promise<unknown>;
      openExternal(url: string): Promise<unknown>;
      perfEnabled: boolean;
      perfLogPath: string;
      perfLog(payload: { category: string; phase: string; data?: Record<string, unknown> }): void;
      onPolicyEvent(handler: (event: unknown) => void): () => void;
      readExtensionFile(path: string): string;
      resolveExtensionPath(specifier: string, importerPath: string): string | null;
    };
  }
}
