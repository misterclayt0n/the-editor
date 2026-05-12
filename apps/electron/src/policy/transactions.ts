import type { EditorHost } from "@the-editor/editor";
import type { WorkbenchInvalidationKind, WorkbenchSessionState } from "@the-editor/editor/workbench";

interface PolicyTransactionControllerOptions<TPayload> {
  editor: EditorHost;
  buildState(): Promise<WorkbenchSessionState>;
  publish(payload: TPayload): void;
  toPayload(state: WorkbenchSessionState): TPayload;
  onError(error: unknown): void;
}

export class PolicyTransactionController<TPayload> {
  private readonly pendingKinds = new Set<WorkbenchInvalidationKind>();
  private inTransaction = 0;
  private scheduled = false;
  private flushing = false;
  private lastPayload: TPayload | undefined;

  constructor(private readonly options: PolicyTransactionControllerOptions<TPayload>) {
    options.editor.workbench.subscribe((kinds) => {
      for (const kind of kinds) this.pendingKinds.add(kind);
      if (!this.inTransaction) this.schedule();
    });
  }

  async run<T>(action: () => Promise<T>): Promise<{ result: T; payload: TPayload }> {
    this.inTransaction += 1;
    try {
      const result = await action();
      const payload = await this.flush({ force: true });
      return { result, payload };
    } finally {
      this.inTransaction -= 1;
      if (!this.inTransaction && this.pendingKinds.size) this.schedule();
    }
  }

  async publishNow(): Promise<TPayload> {
    return this.flush({ force: true });
  }

  private schedule(): void {
    if (this.scheduled || this.flushing) return;
    this.scheduled = true;
    queueMicrotask(() => {
      this.scheduled = false;
      void this.flush().catch((error) => {
        this.options.onError(error);
      });
    });
  }

  private async flush(options: { force?: boolean } = {}): Promise<TPayload> {
    if (this.flushing) return this.lastPayload ?? this.options.toPayload(await this.options.buildState());
    if (!options.force && !this.pendingKinds.size) {
      return this.lastPayload ?? this.options.toPayload(await this.options.buildState());
    }

    this.flushing = true;
    this.pendingKinds.clear();
    try {
      const state = await this.options.buildState();
      const payload = this.options.toPayload(state);
      this.lastPayload = payload;
      this.options.publish(payload);
      return payload;
    } finally {
      this.flushing = false;
      if (this.pendingKinds.size && !this.inTransaction) this.schedule();
    }
  }
}
