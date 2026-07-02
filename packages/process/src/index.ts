/**
 * Process ownership: the public capability seam for spawning external
 * programs. Two tags — `Processes` (stdout/stderr pipes: language servers,
 * git, formatters, linters, agents) and `Ptys` (interactive terminals) —
 * hand packages the ability to run local programs without reaching a
 * privileged host object.
 *
 * The shape is deliberately NOT the raw platform port. A raw port is an
 * id-multiplexed global: any holder could `write`/`kill` any id, and nothing
 * ties a process to the lifetime of whoever started it. Here, `spawn` is
 * Scope-bound and returns a HANDLE — you can only touch what you spawned, and
 * when the acquiring scope closes (package dispose/reload, or an explicit
 * `Effect.scoped` block) the process is killed. Ownership is lifetime, the
 * same rule every registry in the editor already follows.
 *
 * Trust posture: extensions are trusted local code (see EXTENSIONS.md §20 and
 * the project-extension trust gate). Holding `Processes` is consent granted at
 * load, not per spawn — registering a `LanguageServers` entry already causes
 * the host to start an arbitrary command, so a per-spawn prompt would be a
 * fiction. Capability-gated: on a host with no process bridge (plain web,
 * headless), the service is still present but `spawn`/`run` fail with
 * `ProcessUnavailableError` rather than the tag being absent — so a
 * process-using extension always activates and degrades at the call, instead
 * of failing to load.
 *
 * Byte streaming stays plain callbacks (the hot-path rule): no Stream wrapper
 * around every chunk.
 */

import { Context, Effect, Scope } from "effect";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class ProcessSpawnError extends Error {
  readonly _tag = "ProcessSpawnError";
  constructor(
    readonly command: string,
    message: string,
  ) {
    super(message);
    this.name = "ProcessSpawnError";
  }
}

/** No process capability on this host (plain web / headless). */
export class ProcessUnavailableError extends ProcessSpawnError {
  constructor(command: string) {
    super(command, `no process host available to run ${JSON.stringify(command)}`);
    this.name = "ProcessUnavailableError";
  }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export type ProcessSpawnOptions = {
  readonly command: string;
  readonly args?: readonly string[];
  readonly cwd?: string;
  /** Extra environment, merged OVER the host environment (never replacing it). */
  readonly env?: Readonly<Record<string, string>>;
  /** Attribution for host process listings / debugging. */
  readonly label?: string;
};

export type PtySpawnOptions = {
  readonly cols: number;
  readonly rows: number;
  readonly cwd?: string;
  /** Shell command; the host picks $SHELL when omitted. */
  readonly shell?: string;
  readonly env?: Readonly<Record<string, string>>;
  readonly label?: string;
};

// ---------------------------------------------------------------------------
// Handles — the only thing a spawner gets back. Scoped to the spawn.
// ---------------------------------------------------------------------------

export type ProcessStream = "stdout" | "stderr";

export interface ProcessHandle {
  readonly pid: number;
  write(chunk: Uint8Array): void;
  /** Idempotent: safe to call explicitly and again on scope close. */
  kill(): void;
  /** Subscribe to output; returns an unsubscribe. Buffered until the first
   *  listener attaches, then live (no unbounded replay for long streams). */
  onData(listener: (chunk: Uint8Array, stream: ProcessStream) => void): () => void;
  /** Fires once with the exit code (null if unknown). Late subscribers after
   *  exit are called immediately. */
  onExit(listener: (code: number | null) => void): () => void;
}

export interface PtyHandle {
  readonly pid: number;
  write(data: string): void;
  resize(cols: number, rows: number): void;
  kill(): void;
  onData(listener: (data: string) => void): () => void;
  onExit(listener: (code: number | null) => void): () => void;
  /** The PTY's foreground process name, when the host can inspect the tty. */
  processName?(): Promise<string | null>;
}

export type RunResult = {
  readonly code: number | null;
  readonly stdout: string;
  readonly stderr: string;
};

// ---------------------------------------------------------------------------
// Services
// ---------------------------------------------------------------------------

export class Processes extends Context.Tag("@the-editor/process/Processes")<
  Processes,
  Processes.Service
>() {}

export namespace Processes {
  export interface Service {
    /**
     * Start a long-lived process. Scope-bound: killed when the acquiring
     * scope closes (a package's activation scope, or an `Effect.scoped`
     * block) — or earlier via `handle.kill()`.
     */
    readonly spawn: (
      options: ProcessSpawnOptions,
    ) => Effect.Effect<ProcessHandle, ProcessSpawnError, Scope.Scope>;
    /**
     * Run to completion, collecting stdout/stderr. Self-scoped — no Scope
     * requirement; the process is always cleaned up. For git, formatters,
     * one-shot tools.
     */
    readonly run: (options: ProcessSpawnOptions) => Effect.Effect<RunResult, ProcessSpawnError>;
  }
}

export class Ptys extends Context.Tag("@the-editor/process/Ptys")<Ptys, Ptys.Service>() {}

export namespace Ptys {
  export interface Service {
    readonly spawn: (
      options: PtySpawnOptions,
    ) => Effect.Effect<PtyHandle, ProcessSpawnError, Scope.Scope>;
  }
}

// ---------------------------------------------------------------------------
// Drivers — the low-level, host-only shape the services wrap. Structurally
// satisfied by the app's window bridges (createProcessPort / createTerminalPort).
// ---------------------------------------------------------------------------

export interface ProcessDriver {
  spawn(options: ProcessSpawnOptions): Promise<number | null>;
  write(id: number, chunk: Uint8Array): void;
  kill(id: number): void;
  onData(id: number, listener: (chunk: Uint8Array, stream: ProcessStream) => void): void;
  onExit(id: number, listener: (code: number | null) => void): void;
}

export interface PtyDriver {
  spawn(options: PtySpawnOptions): Promise<number | null>;
  write(id: number, data: string): void;
  resize(id: number, cols: number, rows: number): void;
  kill(id: number): void;
  onData(id: number, listener: (data: string) => void): void;
  onExit(id: number, listener: (code: number | null) => void): void;
  processName?(id: number): Promise<string | null>;
}

// ---------------------------------------------------------------------------
// Handle construction — buffer-until-first-listener, idempotent kill.
// ---------------------------------------------------------------------------

/** Shared streaming bookkeeping for a single spawned id. Buffers output until
 *  the first listener attaches (so a run() that subscribes a microtask after
 *  spawn never drops the child's first write), then goes live. Exit is latched
 *  and replayed to late subscribers. */
function makeStreamState<Chunk>() {
  let listener: ((chunk: Chunk, stream: ProcessStream) => void) | null = null;
  const buffer: Array<{ chunk: Chunk; stream: ProcessStream }> = [];
  let exited = false;
  let exitCode: number | null = null;
  const exitListeners = new Set<(code: number | null) => void>();

  return {
    pushData(chunk: Chunk, stream: ProcessStream) {
      if (listener !== null) listener(chunk, stream);
      else buffer.push({ chunk, stream });
    },
    pushExit(code: number | null) {
      if (exited) return;
      exited = true;
      exitCode = code;
      for (const l of exitListeners) l(code);
    },
    onData(next: (chunk: Chunk, stream: ProcessStream) => void): () => void {
      listener = next;
      if (buffer.length > 0) {
        const drained = buffer.splice(0, buffer.length);
        for (const item of drained) next(item.chunk, item.stream);
      }
      return () => {
        if (listener === next) listener = null;
      };
    },
    onExit(next: (code: number | null) => void): () => void {
      if (exited) {
        next(exitCode);
        return () => {};
      }
      exitListeners.add(next);
      return () => exitListeners.delete(next);
    },
  };
}

// ---------------------------------------------------------------------------
// Factories — build a service from a driver (or none = capability absent).
// ---------------------------------------------------------------------------

export function makeProcesses(driver: ProcessDriver | undefined): Processes.Service {
  const spawn = (options: ProcessSpawnOptions) =>
    Effect.acquireRelease(
      Effect.gen(function* () {
        if (driver === undefined) return yield* Effect.fail(new ProcessUnavailableError(options.command));
        const id = yield* Effect.tryPromise({
          try: () => driver.spawn(options),
          catch: (cause) => new ProcessSpawnError(options.command, String(cause)),
        });
        if (id === null) {
          return yield* Effect.fail(new ProcessSpawnError(options.command, "process failed to start"));
        }
        const state = makeStreamState<Uint8Array>();
        driver.onData(id, (chunk, stream) => state.pushData(chunk, stream));
        driver.onExit(id, (code) => state.pushExit(code));
        let killed = false;
        const handle: ProcessHandle = {
          pid: id,
          write: (chunk) => {
            if (!killed) driver.write(id, chunk);
          },
          kill: () => {
            if (killed) return;
            killed = true;
            driver.kill(id);
          },
          onData: (listener) => state.onData(listener),
          onExit: (listener) => state.onExit(listener),
        };
        return handle;
      }),
      (handle) => Effect.sync(() => handle.kill()),
    );

  const run = (options: ProcessSpawnOptions) =>
    Effect.scoped(
      Effect.gen(function* () {
        const handle = yield* spawn(options);
        return yield* Effect.async<RunResult>((resume) => {
          const out: Uint8Array[] = [];
          const err: Uint8Array[] = [];
          handle.onData((chunk, stream) => (stream === "stdout" ? out : err).push(chunk));
          handle.onExit((code) => {
            resume(
              Effect.succeed({
                code,
                stdout: decode(out),
                stderr: decode(err),
              }),
            );
          });
        });
      }),
    );

  return { spawn, run };
}

export function makePtys(driver: PtyDriver | undefined): Ptys.Service {
  const spawn = (options: PtySpawnOptions) =>
    Effect.acquireRelease(
      Effect.gen(function* () {
        if (driver === undefined) return yield* Effect.fail(new ProcessUnavailableError("pty"));
        const id = yield* Effect.tryPromise({
          try: () => driver.spawn(options),
          catch: (cause) => new ProcessSpawnError("pty", String(cause)),
        });
        if (id === null) return yield* Effect.fail(new ProcessSpawnError("pty", "pty failed to start"));
        const state = makeStreamState<string>();
        driver.onData(id, (data) => state.pushData(data, "stdout"));
        driver.onExit(id, (code) => state.pushExit(code));
        let killed = false;
        const handle: PtyHandle = {
          pid: id,
          write: (data) => {
            if (!killed) driver.write(id, data);
          },
          resize: (cols, rows) => {
            if (!killed) driver.resize(id, cols, rows);
          },
          kill: () => {
            if (killed) return;
            killed = true;
            driver.kill(id);
          },
          onData: (listener) => state.onData((chunk) => listener(chunk)),
          onExit: (listener) => state.onExit(listener),
          ...(driver.processName !== undefined
            ? { processName: () => driver.processName!(id) }
            : {}),
        };
        return handle;
      }),
      (handle) => Effect.sync(() => handle.kill()),
    );

  return { spawn };
}

const textDecoder = new TextDecoder();
const decode = (chunks: readonly Uint8Array[]): string => {
  if (chunks.length === 0) return "";
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const merged = new Uint8Array(total);
  let at = 0;
  for (const chunk of chunks) {
    merged.set(chunk, at);
    at += chunk.length;
  }
  return textDecoder.decode(merged);
};
