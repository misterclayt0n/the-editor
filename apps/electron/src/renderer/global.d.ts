export {};

declare global {
  interface Window {
    theEditor: {
      openFileDialog(): Promise<string | null>;
      policyRequest(method: string, params?: unknown): Promise<unknown>;
      onPolicyEvent(handler: (event: unknown) => void): () => void;
      readExtensionFile(path: string): string;
      resolveExtensionPath(specifier: string, importerPath: string): string | null;
    };
  }
}
