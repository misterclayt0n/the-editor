export type DiagnosticSeverity = "info" | "warn" | "error";

export interface DiagnosticEntry {
  id: string;
  severity: DiagnosticSeverity;
  source: string;
  message: string;
  details?: string;
  packageId?: string;
  target?: string;
  role?: string;
  implementationId?: string;
}

export class DiagnosticRegistry {
  private readonly entries = new Map<string, DiagnosticEntry>();

  report(entry: DiagnosticEntry): DiagnosticEntry {
    this.entries.set(entry.id, entry);
    return entry;
  }

  clear(id: string): void {
    this.entries.delete(id);
  }

  clearPrefix(prefix: string): void {
    for (const id of [...this.entries.keys()]) {
      if (id.startsWith(prefix)) this.entries.delete(id);
    }
  }

  clearWhere(predicate: (entry: DiagnosticEntry) => boolean): void {
    for (const [id, entry] of [...this.entries.entries()]) {
      if (predicate(entry)) this.entries.delete(id);
    }
  }

  all(): DiagnosticEntry[] {
    return [...this.entries.values()].sort((a, b) => a.id.localeCompare(b.id));
  }
}
