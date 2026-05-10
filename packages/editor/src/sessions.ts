import { contributionKind, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type InteractiveSessionId = string & { readonly __brand?: "InteractiveSessionId" };

export interface InteractiveSessionSnapshot<TState = unknown> {
  id: InteractiveSessionId;
  kind: string;
  role?: string;
  title?: string;
  revision: number;
  state: TState;
}

export interface InteractiveSessionController<TState = unknown> {
  readonly id: InteractiveSessionId;
  readonly kind: string;
  readonly role?: string;
  readonly title?: string;
  snapshot(): InteractiveSessionSnapshot<TState>;
  setState(next: TState): void;
  updateState(mutator: (state: TState) => void): void;
  invalidate(): void;
}

export interface InteractiveSessionMethodContext<TState = unknown> {
  editor: EditorHost;
  session: InteractiveSessionController<TState>;
}

export interface InteractiveSessionSpec<TState = unknown> {
  kind: string;
  role?: string;
  title?: string;
  state: TState;
  moduleId?: ModuleId;
  methods?: Record<string, (ctx: InteractiveSessionMethodContext<TState>, args: unknown) => unknown | Promise<unknown>>;
  dispose?(): void;
}

export type InteractiveSessionEvent =
  | { type: "updated"; session: InteractiveSessionSnapshot }
  | { type: "disposed"; sessionId: InteractiveSessionId };

interface StoredSession<TState = unknown> {
  id: InteractiveSessionId;
  kind: string;
  role?: string;
  title?: string;
  revision: number;
  state: TState;
  moduleId?: ModuleId;
  methods: Record<string, (ctx: InteractiveSessionMethodContext<TState>, args: unknown) => unknown | Promise<unknown>>;
  dispose?(): void;
}

export class InteractiveSessionRegistry {
  private nextId = 0;
  private readonly sessions = new Map<InteractiveSessionId, StoredSession>();
  private readonly listeners = new Set<(event: InteractiveSessionEvent) => void>();

  constructor(private readonly editor: EditorHost) {}

  create<TState>(spec: InteractiveSessionSpec<TState>): InteractiveSessionController<TState> {
    const id = interactiveSessionId(`session:${++this.nextId}`);
    const session: StoredSession<TState> = {
      id,
      kind: spec.kind,
      role: spec.role,
      title: spec.title,
      revision: 0,
      state: spec.state,
      moduleId: spec.moduleId,
      methods: spec.methods ?? {},
      dispose: spec.dispose,
    };
    this.sessions.set(id, session as StoredSession);
    this.emit({ type: "updated", session: this.snapshot(id)! });

    const controller: InteractiveSessionController<TState> = {
      id,
      kind: session.kind,
      role: session.role,
      title: session.title,
      snapshot: () => this.snapshot(id) as InteractiveSessionSnapshot<TState>,
      setState: (next) => {
        const current = this.sessions.get(id) as StoredSession<TState> | undefined;
        if (!current) return;
        current.state = next;
        current.revision += 1;
        this.emit({ type: "updated", session: this.snapshot(id)! });
      },
      updateState: (mutator) => {
        const current = this.sessions.get(id) as StoredSession<TState> | undefined;
        if (!current) return;
        mutator(current.state);
        current.revision += 1;
        this.emit({ type: "updated", session: this.snapshot(id)! });
      },
      invalidate: () => {
        const current = this.sessions.get(id);
        if (!current) return;
        current.revision += 1;
        this.emit({ type: "updated", session: this.snapshot(id)! });
      },
    };

    if (spec.moduleId) {
      this.editor.contributions.add({
        moduleId: spec.moduleId,
        kind: contributionKind("interactiveSession"),
        localId: id,
        label: spec.title ?? spec.kind,
        disposable: toDisposable(() => {
          this.dispose(id);
        }),
      });
    }

    return controller;
  }

  snapshot<TState = unknown>(id: InteractiveSessionId | string): InteractiveSessionSnapshot<TState> | undefined {
    const session = this.sessions.get(interactiveSessionId(id));
    if (!session) return undefined;
    return {
      id: session.id,
      kind: session.kind,
      role: session.role,
      title: session.title,
      revision: session.revision,
      state: session.state,
    } as InteractiveSessionSnapshot<TState>;
  }

  snapshotAll(): InteractiveSessionSnapshot[] {
    return [...this.sessions.values()].map((session) => ({
      id: session.id,
      kind: session.kind,
      role: session.role,
      title: session.title,
      revision: session.revision,
      state: session.state,
    }));
  }

  async dispatch(id: InteractiveSessionId | string, method: string, args?: unknown): Promise<unknown> {
    const session = this.sessions.get(interactiveSessionId(id));
    if (!session) throw new Error(`unknown interactive session ${id}`);
    const handler = session.methods[method];
    if (!handler) throw new Error(`unknown interactive session method ${method} for ${id}`);
    const controller = this.controllerFor(session) as InteractiveSessionController<unknown>;
    return handler({ editor: this.editor, session: controller }, args);
  }

  dispose(id: InteractiveSessionId | string): void {
    const normalized = interactiveSessionId(id);
    const session = this.sessions.get(normalized);
    if (!session) return;
    this.sessions.delete(normalized);
    session.dispose?.();
    this.emit({ type: "disposed", sessionId: normalized });
  }

  subscribe(listener: (event: InteractiveSessionEvent) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private controllerFor<TState>(session: StoredSession<TState>): InteractiveSessionController<TState> {
    return {
      id: session.id,
      kind: session.kind,
      role: session.role,
      title: session.title,
      snapshot: () => this.snapshot(session.id) as InteractiveSessionSnapshot<TState>,
      setState: (next) => {
        const current = this.sessions.get(session.id) as StoredSession<TState> | undefined;
        if (!current) return;
        current.state = next;
        current.revision += 1;
        this.emit({ type: "updated", session: this.snapshot(session.id)! });
      },
      updateState: (mutator) => {
        const current = this.sessions.get(session.id) as StoredSession<TState> | undefined;
        if (!current) return;
        mutator(current.state);
        current.revision += 1;
        this.emit({ type: "updated", session: this.snapshot(session.id)! });
      },
      invalidate: () => {
        const current = this.sessions.get(session.id);
        if (!current) return;
        current.revision += 1;
        this.emit({ type: "updated", session: this.snapshot(session.id)! });
      },
    };
  }

  private emit(event: InteractiveSessionEvent): void {
    for (const listener of this.listeners) listener(event);
  }
}

export function interactiveSessionId(id: string): InteractiveSessionId {
  return id as InteractiveSessionId;
}
