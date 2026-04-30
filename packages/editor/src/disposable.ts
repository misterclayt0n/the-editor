export interface Disposable {
  dispose(): void;
}

export class DisposableStore implements Disposable {
  private disposed = false;
  private readonly disposables: Disposable[] = [];

  add<T extends Disposable>(disposable: T): T {
    if (this.disposed) {
      disposable.dispose();
      return disposable;
    }
    this.disposables.push(disposable);
    return disposable;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const disposable of this.disposables.splice(0).reverse()) {
      disposable.dispose();
    }
  }
}

export function toDisposable(dispose: () => void): Disposable {
  let disposed = false;
  return {
    dispose() {
      if (disposed) return;
      disposed = true;
      dispose();
    },
  };
}

export function combinedDisposable(disposables: Iterable<Disposable>): Disposable {
  const store = new DisposableStore();
  for (const disposable of disposables) store.add(disposable);
  return store;
}
