/**
 * TerminalPort: the low-level, HOST-ONLY driver behind the public `Ptys` seam
 * (`@the-editor/process`). Structurally a `PtyDriver`; packages get
 * `makePtys(driver)`'s scoped handles, never this raw bridge. A PTY is a single
 * merged byte stream with a window size, distinct from ProcessPort's
 * stdout/stderr pipes. Capability-gated: no host, no terminal.
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
  /** The PTY's foreground process name. Capability-gated: absent on hosts that
   *  can't inspect the tty (plain web, test port). */
  processName?(id: number): Promise<string | null>;
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
