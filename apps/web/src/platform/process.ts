/**
 * ProcessPort: the platform seam for external processes (language servers
 * first; formatters and tools later). Capability-gated like every port —
 * no host, no processes, the editor simply has no LSP.
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
