export interface PerfSpan {
  readonly start: number;
  end(extra?: Record<string, unknown>): void;
}

const defaultSlowMs = 16;
let announced = false;

export function perfDiagnosticsEnabled(): boolean {
  const enabled = readPerfFlag();
  if (enabled && !announced) {
    announced = true;
    emitPerfLog("diagnostics", "enabled", { source: perfFlagSource() });
  }
  return enabled;
}

function readPerfFlag(): boolean {
  const globalValue = (globalThis as { __THE_EDITOR_PERF__?: unknown }).__THE_EDITOR_PERF__;
  if (globalValue === true || globalValue === "1" || globalValue === "true") return true;
  if (readPerfBridge()?.perfEnabled === true) return true;
  try {
    const storage = typeof window !== "undefined" ? window.localStorage : undefined;
    if (storage?.getItem("the-editor.perf") === "1" || storage?.getItem("the-editor.perf") === "true") return true;
  } catch {
    // Ignore storage access failures.
  }
  const env = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process?.env;
  return env?.THE_EDITOR_PERF === "1" || env?.THE_EDITOR_PERF === "true";
}

function perfFlagSource(): string {
  const globalValue = (globalThis as { __THE_EDITOR_PERF__?: unknown }).__THE_EDITOR_PERF__;
  if (globalValue === true || globalValue === "1" || globalValue === "true") return "globalThis.__THE_EDITOR_PERF__";
  if (readPerfBridge()?.perfEnabled === true) return "window.theEditor.perfEnabled";
  try {
    const storage = typeof window !== "undefined" ? window.localStorage : undefined;
    if (storage?.getItem("the-editor.perf") === "1" || storage?.getItem("the-editor.perf") === "true") return "localStorage.the-editor.perf";
  } catch {
    // Ignore storage access failures.
  }
  return "THE_EDITOR_PERF";
}

export function perfLog(category: string, phase: string, data: Record<string, unknown> = {}): void {
  if (!perfDiagnosticsEnabled()) return;
  emitPerfLog(category, phase, data);
}

export function perfSpan(category: string, phase: string, data: Record<string, unknown> = {}, slowMs = defaultSlowMs): PerfSpan {
  const start = performance.now();
  return {
    start,
    end(extra: Record<string, unknown> = {}) {
      const ms = Math.round((performance.now() - start) * 10) / 10;
      if (!perfDiagnosticsEnabled()) return;
      emitPerfLog(category, phase, { ms, ...data, ...extra });
    },
  };
}

function emitPerfLog(category: string, phase: string, data: Record<string, unknown>): void {
  const bridge = readPerfBridge();
  if (bridge?.perfLog) {
    bridge.perfLog({ category, phase, data });
    return;
  }
  console.info(`[perf:${category}] ${phase}`, data);
}

function readPerfBridge(): { perfEnabled?: boolean; perfLog?: (payload: { category: string; phase: string; data?: Record<string, unknown> }) => void } | undefined {
  return typeof window !== "undefined"
    ? (window as unknown as { theEditor?: { perfEnabled?: boolean; perfLog?: (payload: { category: string; phase: string; data?: Record<string, unknown> }) => void } }).theEditor
    : undefined;
}
