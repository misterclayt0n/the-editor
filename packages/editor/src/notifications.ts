import type { EditorHost } from "./editorHost.js";

export type NotificationSeverity = "info" | "warn" | "error";

export interface NotificationEntry {
  id: string;
  source?: string;
  severity: NotificationSeverity;
  message: string;
  details?: string;
  createdAt: number;
}

export interface NotificationOptions {
  id?: string;
  source?: string;
  severity?: NotificationSeverity;
  message: string;
  details?: string;
  durationMs?: number;
}

export class NotificationRegistry {
  private readonly entries = new Map<string, NotificationEntry>();
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  private readonly listeners = new Set<() => void>();
  private sequence = 0;

  constructor(private readonly editor: EditorHost) {}

  show(options: NotificationOptions): NotificationEntry {
    const id = options.id ?? `${options.source ?? "notification"}:${++this.sequence}`;
    const entry: NotificationEntry = {
      id,
      source: options.source,
      severity: options.severity ?? "info",
      message: options.message,
      details: options.details,
      createdAt: Date.now(),
    };

    this.entries.delete(id);
    this.entries.set(id, entry);
    while (this.entries.size > 8) {
      const oldest = this.entries.keys().next().value as string | undefined;
      if (!oldest) break;
      this.dismiss(oldest);
    }

    this.clearTimer(id);
    const durationMs = options.durationMs ?? defaultNotificationDuration(entry.severity);
    if (durationMs > 0) {
      this.timers.set(id, setTimeout(() => {
        this.dismiss(id);
      }, durationMs));
    }

    this.emit();
    return entry;
  }

  info(source: string, message: string, details?: string, durationMs?: number): NotificationEntry {
    return this.show({ id: source, source, severity: "info", message, details, durationMs });
  }

  warn(source: string, message: string, details?: string, durationMs?: number): NotificationEntry {
    return this.show({ id: source, source, severity: "warn", message, details, durationMs });
  }

  error(source: string, message: string, details?: string, durationMs?: number): NotificationEntry {
    return this.show({ id: source, source, severity: "error", message, details, durationMs });
  }

  dismiss(id: string): void {
    const deleted = this.entries.delete(id);
    this.clearTimer(id);
    if (!deleted) return;
    this.emit();
  }

  clear(): void {
    if (!this.entries.size) return;
    for (const id of [...this.entries.keys()]) {
      this.clearTimer(id);
    }
    this.entries.clear();
    this.emit();
  }

  snapshot(): NotificationEntry[] {
    return [...this.entries.values()].sort((a, b) => b.createdAt - a.createdAt);
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  dispose(): void {
    for (const id of [...this.timers.keys()]) {
      this.clearTimer(id);
    }
    this.listeners.clear();
    this.entries.clear();
  }

  private clearTimer(id: string): void {
    const timer = this.timers.get(id);
    if (!timer) return;
    clearTimeout(timer);
    this.timers.delete(id);
  }

  private emit(): void {
    this.editor.workbench.invalidate("notifications");
    for (const listener of this.listeners) listener();
  }
}

function defaultNotificationDuration(severity: NotificationSeverity): number {
  switch (severity) {
    case "info":
      return 4_000;
    case "warn":
      return 6_000;
    case "error":
      return 8_000;
  }
}
