import type { RuntimeApi } from "@the-editor/contracts/runtime";

export interface Disposable {
  dispose(): void;
}

export type EditorModule = (editor: EditorHost) => void | Disposable | Promise<void | Disposable>;

export interface EditorHostOptions {
  runtime: RuntimeApi;
}

export class DisposableStore implements Disposable {
  private readonly disposables: Disposable[] = [];

  add<T extends Disposable>(disposable: T): T {
    this.disposables.push(disposable);
    return disposable;
  }

  dispose(): void {
    for (const disposable of this.disposables.splice(0).reverse()) {
      disposable.dispose();
    }
  }
}

export class EditorHost {
  readonly runtime: RuntimeApi;
  private readonly modules = new DisposableStore();

  constructor(options: EditorHostOptions) {
    this.runtime = options.runtime;
  }

  async use(module: EditorModule): Promise<void> {
    const disposable = await module(this);
    if (disposable) this.modules.add(disposable);
  }

  dispose(): void {
    this.modules.dispose();
  }
}

export function createEditorHost(options: EditorHostOptions): EditorHost {
  return new EditorHost(options);
}
