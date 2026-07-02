/**
 * ProcessPort: the low-level, HOST-ONLY driver behind the public `Processes`
 * seam (`@the-editor/process`). This is the raw id-multiplexed window bridge;
 * packages never see it — they get `makeProcesses(driver)`'s scoped handles.
 * It stays out of the extension allowlist deliberately (on the host-only list).
 * Structurally a `ProcessDriver`. Capability-gated: no host, no processes.
 *
 * Priority: dev test port (headless verification) > Electron bridge.
 */

export type ProcessSpawnOptions = {
  readonly command: string;
  readonly args?: readonly string[];
  readonly cwd?: string;
};

export interface ProcessPort {
  spawn(options: ProcessSpawnOptions): Promise<number | null>;
  write(id: number, chunk: Uint8Array): void;
  kill(id: number): void;
  onData(id: number, listener: (chunk: Uint8Array, stream: "stdout" | "stderr") => void): void;
  onExit(id: number, listener: (code: number | null) => void): void;
}

export function createProcessPort(): ProcessPort | undefined {
  const w = window as unknown as {
    __testProcessPort?: ProcessPort;
    theEditorProc?: ProcessPort;
  };
  const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
  if (isDev && w.__testProcessPort) return w.__testProcessPort;
  if (w.theEditorProc) return w.theEditorProc;
  return undefined;
}
