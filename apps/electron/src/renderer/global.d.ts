export {};

declare global {
  interface Window {
    theEditor: {
      policyRequest(method: string, params?: unknown): Promise<unknown>;
      onPolicyEvent(handler: (event: unknown) => void): () => void;
    };
  }
}
