import { toDisposable, type Disposable } from "./disposable.js";

export interface HookRegistration<THandler> {
  id: string;
  handler: THandler;
  priority?: number;
}

type Entry<THandler> = Required<HookRegistration<THandler>> & { order: number };

export type EventHandler<T> = (value: T) => void | Promise<void>;
export type WaterfallHandler<T> = (value: T) => T | Promise<T>;
export type FirstHandler<T, R> = (value: T) => R | undefined | Promise<R | undefined>;
export type CollectHandler<T, R> = (value: T) => R | undefined | Promise<R | undefined>;
export type ReducerHandler<T> = (value: T) => T | Promise<T>;
export type AroundHandler<T, R> = (value: T, next: () => Promise<R>) => R | Promise<R>;

class HookPoint<THandler> {
  private nextOrder = 0;
  private readonly entries = new Map<string, Entry<THandler>>();

  tap(registration: HookRegistration<THandler>): Disposable {
    const entry: Entry<THandler> = {
      priority: 0,
      ...registration,
      order: this.nextOrder++,
    };
    this.entries.set(entry.id, entry);
    return toDisposable(() => {
      const current = this.entries.get(entry.id);
      if (current === entry) this.entries.delete(entry.id);
    });
  }

  handlers(): THandler[] {
    return [...this.entries.values()]
      .sort((a, b) => b.priority - a.priority || a.order - b.order)
      .map((entry) => entry.handler);
  }
}

export class Hooks {
  private readonly points = new Map<string, HookPoint<unknown>>();

  tap<THandler>(name: string, registration: HookRegistration<THandler>): Disposable {
    return this.point<THandler>(name).tap(registration);
  }

  async event<T>(name: string, value: T): Promise<void> {
    for (const handler of this.handlers<EventHandler<T>>(name)) {
      await handler(value);
    }
  }

  async waterfall<T>(name: string, value: T): Promise<T> {
    let current = value;
    for (const handler of this.handlers<WaterfallHandler<T>>(name)) {
      current = await handler(current);
    }
    return current;
  }

  async first<T, R>(name: string, value: T): Promise<R | undefined> {
    for (const handler of this.handlers<FirstHandler<T, R>>(name)) {
      const result = await handler(value);
      if (result !== undefined) return result;
    }
    return undefined;
  }

  async collect<T, R>(name: string, value: T): Promise<R[]> {
    const results: R[] = [];
    for (const handler of this.handlers<CollectHandler<T, R>>(name)) {
      const result = await handler(value);
      if (result !== undefined) results.push(result);
    }
    return results;
  }

  async reduce<T>(name: string, value: T): Promise<T> {
    let current = value;
    for (const handler of this.handlers<ReducerHandler<T>>(name)) {
      current = await handler(current);
    }
    return current;
  }

  async around<T, R>(name: string, value: T, inner: () => Promise<R>): Promise<R> {
    const handlers = this.handlers<AroundHandler<T, R>>(name);
    let next = inner;
    for (const handler of handlers.reverse()) {
      const previous = next;
      next = () => Promise.resolve(handler(value, previous));
    }
    return next();
  }

  private handlers<THandler>(name: string): THandler[] {
    return this.point<THandler>(name).handlers();
  }

  private point<THandler>(name: string): HookPoint<THandler> {
    let point = this.points.get(name) as HookPoint<THandler> | undefined;
    if (!point) {
      point = new HookPoint<THandler>();
      this.points.set(name, point as HookPoint<unknown>);
    }
    return point;
  }
}
