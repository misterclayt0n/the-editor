/**
 * TerminalPort: the platform seam for interactive PTYs (the terminal panel).
 * Capability-gated like every port — no host (plain web, headless), no
 * terminal. Distinct from ProcessPort (LSP): a PTY is a single merged byte
 * stream with a window size, not stdout/stderr pipes.
 *
 * Priority: dev test port (headless verification) > Electron bridge.
 */

export type TerminalSpawnOptions = {
  readonly cols: number;
  readonly rows: number;
  readonly cwd?: string;
  /** Shell command; the host picks a sane default ($SHELL) when omitted. */
  readonly shell?: string;
};

export interface TerminalPort {
  /** Start a shell in a PTY; returns its id, or null if none could start. */
  spawn(options: TerminalSpawnOptions): Promise<number | null>;
  write(id: number, data: string): void;
  resize(id: number, cols: number, rows: number): void;
  kill(id: number): void;
  onData(id: number, listener: (data: string) => void): void;
  onExit(id: number, listener: (code: number | null) => void): void;
}

export function createTerminalPort(): TerminalPort | undefined {
  const w = window as unknown as {
    __testTerminalPort?: TerminalPort;
    theEditorTerminal?: TerminalPort;
  };
  const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
  if (isDev && w.__testTerminalPort) return w.__testTerminalPort;
  if (w.theEditorTerminal) return w.theEditorTerminal;
  return undefined;
}
