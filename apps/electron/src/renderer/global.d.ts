export {};

declare global {
  interface Window {
    theEditor: {
      runtimeRequest(method: string, params?: unknown): Promise<unknown>;
      policyRequest(method: string, params?: unknown): Promise<unknown>;
      onPolicyEvent(handler: (event: unknown) => void): () => void;
    };
  }
}
