import { defineHostStore } from "./stores.js";
import type { EditorHost } from "./editorHost.js";

interface PerfTraceState {
  id: number;
  cause: string;
  startedAt: number;
  data?: Record<string, unknown>;
}

const perfTraceStore = defineHostStore<PerfTraceState | undefined>({
  id: "editor.perfTrace.current",
  init: () => undefined,
});

let nextPerfTraceId = 1;

export function beginPerfTrace(editor: EditorHost, cause: string, data?: Record<string, unknown>): PerfTraceState {
  const trace = {
    id: nextPerfTraceId++,
    cause,
    startedAt: performance.now(),
    data,
  } satisfies PerfTraceState;
  editor.stores.set(perfTraceStore, trace);
  return trace;
}

export function currentPerfTrace(editor: EditorHost): PerfTraceState | undefined {
  return editor.stores.get(perfTraceStore);
}

export function endPerfTrace(editor: EditorHost): PerfTraceState | undefined {
  const trace = currentPerfTrace(editor);
  editor.stores.delete(perfTraceStore);
  return trace;
}

export function perfTraceLog(editor: EditorHost, category: string, phase: string, data: Record<string, unknown> = {}): void {
  const trace = currentPerfTrace(editor);
  const payload = trace ? { traceId: trace.id, traceCause: trace.cause, ...trace.data, ...data } : data;
  const bridge = typeof window !== "undefined"
    ? (window as unknown as { theEditor?: { perfEnabled?: boolean; perfLog?: (payload: { category: string; phase: string; data?: Record<string, unknown> }) => void } }).theEditor
    : undefined;
  if (bridge?.perfEnabled && bridge.perfLog) {
    bridge.perfLog({ category, phase, data: payload });
  }
}
