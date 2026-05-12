export type HealthDiagnosticSeverity = "info" | "warn" | "error";

export interface HealthDiagnosticEntry {
  id: string;
  severity: HealthDiagnosticSeverity;
  source: string;
  message: string;
  details?: string;
  packageId?: string;
  target?: string;
  role?: string;
  implementationId?: string;
}

export class HealthDiagnosticRegistry {
  private readonly entries = new Map<string, HealthDiagnosticEntry>();

  report(entry: HealthDiagnosticEntry): HealthDiagnosticEntry {
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

  clearWhere(predicate: (entry: HealthDiagnosticEntry) => boolean): void {
    for (const [id, entry] of [...this.entries.entries()]) {
      if (predicate(entry)) this.entries.delete(id);
    }
  }

  all(): HealthDiagnosticEntry[] {
    return [...this.entries.values()].sort((a, b) => a.id.localeCompare(b.id));
  }
}
