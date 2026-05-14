import { moduleId, type ModuleId } from "./contributions.js";

export type StoreKind = "host" | "module" | "persisted" | "session";

export interface StoreToken<T> {
  readonly id: string;
  readonly kind: StoreKind;
  readonly label?: string;
  init(): T;
}

export interface PersistedStoreToken<T, TRaw = unknown> extends StoreToken<T> {
  readonly kind: "persisted";
  readonly version: number;
  serialize(state: T): TRaw;
  deserialize(raw: TRaw): T;
}

export interface SessionStoreToken<T> extends StoreToken<T> {
  readonly kind: "session";
}

export function defineHostStore<T>(definition: Omit<StoreToken<T>, "kind">): StoreToken<T> {
  return { ...definition, kind: "host" };
}

export function defineModuleStore<T>(definition: Omit<StoreToken<T>, "kind">): StoreToken<T> {
  return { ...definition, kind: "module" };
}

export function definePersistedStore<T, TRaw = unknown>(definition: Omit<PersistedStoreToken<T, TRaw>, "kind">): PersistedStoreToken<T, TRaw> {
  return { ...definition, kind: "persisted" };
}

export function defineSessionStore<T>(definition: Omit<SessionStoreToken<T>, "kind">): SessionStoreToken<T> {
  return { ...definition, kind: "session" };
}

interface StoreEntry<T = unknown> {
  token: StoreToken<T>;
  value: T;
  moduleId?: ModuleId;
  sessionId?: string;
}

export class StoreRegistry {
  private readonly hostStores = new Map<string, StoreEntry>();
  private readonly moduleStores = new Map<string, StoreEntry>();
  private readonly persistedStores = new Map<string, StoreEntry>();
  private readonly sessionStores = new Map<string, StoreEntry>();

  constructor(private readonly currentModuleId: () => ModuleId) {}

  host<T>(token: StoreToken<T>): T {
    this.assertKind(token, "host");
    return this.getOrInit(this.hostStores, token).value;
  }

  module<T>(token: StoreToken<T>, options?: { moduleId?: ModuleId | string }): T {
    this.assertKind(token, "module");
    const owner = options?.moduleId ? moduleId(options.moduleId) : this.currentModuleId();
    return this.getOrInit(this.moduleStores, token, { moduleId: owner }).value;
  }

  persisted<T>(token: PersistedStoreToken<T>): T {
    this.assertKind(token, "persisted");
    return this.getOrInit(this.persistedStores, token).value;
  }

  session<T>(token: SessionStoreToken<T>, sessionId: string): T {
    this.assertKind(token, "session");
    return this.getOrInit(this.sessionStores, token, { sessionId }).value;
  }

  get<T>(token: StoreToken<T>, options?: { moduleId?: ModuleId | string; sessionId?: string }): T | undefined {
    return this.mapFor(token).get(this.keyFor(token, options))?.value as T | undefined;
  }

  set<T>(token: StoreToken<T>, value: T, options?: { moduleId?: ModuleId | string; sessionId?: string }): T {
    const map = this.mapFor(token);
    const entry: StoreEntry<T> = {
      token,
      value,
      moduleId: options?.moduleId ? moduleId(options.moduleId) : token.kind === "module" ? this.currentModuleId() : undefined,
      sessionId: options?.sessionId,
    };
    map.set(this.keyFor(token, options), entry);
    return value;
  }

  delete(token: StoreToken<unknown>, options?: { moduleId?: ModuleId | string; sessionId?: string }): void {
    this.mapFor(token).delete(this.keyFor(token, options));
  }

  disposeModule(owner: ModuleId | string): void {
    const normalized = moduleId(owner);
    for (const [key, entry] of this.moduleStores.entries()) {
      if (entry.moduleId === normalized) this.moduleStores.delete(key);
    }
  }

  clear(): void {
    this.hostStores.clear();
    this.moduleStores.clear();
    this.persistedStores.clear();
    this.sessionStores.clear();
  }

  private getOrInit<T>(map: Map<string, StoreEntry>, token: StoreToken<T>, options?: { moduleId?: ModuleId; sessionId?: string }): StoreEntry<T> {
    const key = this.keyFor(token, options);
    const existing = map.get(key) as StoreEntry<T> | undefined;
    if (existing) return existing;
    const entry: StoreEntry<T> = { token, value: token.init(), moduleId: options?.moduleId, sessionId: options?.sessionId };
    map.set(key, entry);
    return entry;
  }

  private mapFor(token: StoreToken<unknown>): Map<string, StoreEntry> {
    switch (token.kind) {
      case "host": return this.hostStores;
      case "module": return this.moduleStores;
      case "persisted": return this.persistedStores;
      case "session": return this.sessionStores;
    }
  }

  private keyFor(token: StoreToken<unknown>, options?: { moduleId?: ModuleId | string; sessionId?: string }): string {
    switch (token.kind) {
      case "module":
        return `${options?.moduleId ? moduleId(options.moduleId) : this.currentModuleId()}::${token.id}`;
      case "session":
        if (!options?.sessionId) throw new Error(`session store ${token.id} requires sessionId`);
        return `${options.sessionId}::${token.id}`;
      case "host":
      case "persisted":
        return token.id;
    }
  }

  private assertKind(token: StoreToken<unknown>, kind: StoreKind): void {
    if (token.kind !== kind) throw new Error(`store ${token.id} is ${token.kind}, not ${kind}`);
  }
}
