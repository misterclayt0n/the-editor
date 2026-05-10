import type { EditorHost } from "./editorHost.js";

export interface WorkingDirectorySnapshot {
  start: string;
  current: string;
  previous?: string;
}

export class WorkingDirectoryRegistry {
  private readonly listeners = new Set<() => void>();
  private state: WorkingDirectorySnapshot;

  constructor(private readonly editor: EditorHost, initialCwd?: string) {
    const start = initialCwd ?? defaultWorkingDirectory();
    this.state = { start, current: start };
  }

  current(): string {
    return this.state.current;
  }

  previous(): string | undefined {
    return this.state.previous;
  }

  start(): string {
    return this.state.start;
  }

  snapshot(): WorkingDirectorySnapshot {
    return { ...this.state };
  }

  setCurrent(path: string): WorkingDirectorySnapshot {
    this.state = {
      start: this.state.start,
      current: path,
      previous: this.state.current,
    };
    this.emit();
    return this.snapshot();
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private emit(): void {
    this.editor.workbench.invalidate("cwd");
    for (const listener of this.listeners) listener();
  }
}

function defaultWorkingDirectory(): string {
  try {
    if (typeof process !== "undefined" && typeof process.cwd === "function") {
      return process.cwd();
    }
  } catch {
    // ignored
  }
  return "/";
}
