import { Effect, Duration, Fiber, Ref, Scope } from "effect";
import { spawn, type ChildProcess } from "node:child_process";
import { pathToFileURL } from "node:url";

const WEB_DEV_SERVER_URL = "http://localhost:5173";

type DevMode = "desktop" | "web";

interface WaitForDevServerOptions {
  readonly intervalMs?: number;
  readonly isReady?: (url: string, timeoutMs: number) => Effect.Effect<boolean>;
  readonly timeoutMs?: number;
}

export class DevServerTimeoutError extends Error {
  override readonly name = "DevServerTimeoutError";
  readonly url: string;

  constructor(url: string) {
    super(`Timed out waiting for dev server at ${url}`);
    this.url = url;
  }
}

export class ChildProcessError extends Error {
  override readonly name = "ChildProcessError";
  readonly label: string;
  readonly code: number | null;
  readonly signal: NodeJS.Signals | null;

  constructor(label: string, code: number | null, signal: NodeJS.Signals | null) {
    super(
      `${label} exited with ${code == null ? `signal ${String(signal)}` : `code ${String(code)}`}`,
    );
    this.label = label;
    this.code = code;
    this.signal = signal;
  }
}

export function parseMode(argv: readonly string[]): DevMode {
  const mode = argv[2];
  if (mode == null || mode === "desktop") return "desktop";
  if (mode === "web") return "web";
  throw new Error(`Unknown dev mode: ${mode}. Expected "web" or "desktop".`);
}

function spawnProcess(
  label: string,
  command: string,
  args: readonly string[],
  options: { readonly cwd?: string } = {},
): Effect.Effect<ChildProcess> {
  return Effect.sync(() => {
    const child = spawn(command, [...args], {
      cwd: options.cwd,
      env: process.env,
      stdio: "inherit",
    });

    child.once("spawn", () => {
      console.log(`[dev] ${label} started (pid ${String(child.pid)})`);
    });

    child.once("error", (error) => {
      console.error(`[dev] ${label} failed to start`, error);
    });

    return child;
  });
}

function interruptProcess(child: ChildProcess): Effect.Effect<void> {
  return Effect.async<void>((resume) => {
    if (child.exitCode != null || child.signalCode != null || child.killed) {
      resume(Effect.void);
      return;
    }

    const timeout = setTimeout(() => {
      if (child.exitCode == null && child.signalCode == null) {
        child.kill("SIGKILL");
      }
    }, 2_000);

    child.once("exit", () => {
      clearTimeout(timeout);
      resume(Effect.void);
    });

    child.kill("SIGTERM");
  });
}

export function runChild(
  label: string,
  command: string,
  args: readonly string[],
  options: { readonly cwd?: string } = {},
): Effect.Effect<void, ChildProcessError, Scope.Scope> {
  return Effect.acquireRelease(spawnProcess(label, command, args, options), interruptProcess).pipe(
    Effect.flatMap((child) =>
      Effect.async<void, ChildProcessError>((resume) => {
        child.once("exit", (code, signal) => {
          if (code === 0) {
            resume(Effect.void);
            return;
          }
          resume(Effect.fail(new ChildProcessError(label, code, signal)));
        });
      }),
    ),
  );
}

export function runCommand(
  label: string,
  command: string,
  args: readonly string[],
  options: { readonly cwd?: string } = {},
): Effect.Effect<void, ChildProcessError> {
  return spawnProcess(label, command, args, options).pipe(
    Effect.flatMap((child) =>
      Effect.async<void, ChildProcessError>((resume) => {
        child.once("exit", (code, signal) => {
          if (code === 0) {
            resume(Effect.void);
            return;
          }
          resume(Effect.fail(new ChildProcessError(label, code, signal)));
        });
      }),
    ),
  );
}

export const isDevServerReady = (url: string, timeoutMs = 1_000): Effect.Effect<boolean> =>
  Effect.tryPromise({
    try: async () => {
      const response = await fetch(url, {
        method: "HEAD",
        signal: AbortSignal.timeout(timeoutMs),
      });
      return response.ok;
    },
    catch: () => false,
  }).pipe(Effect.catchAll(() => Effect.succeed(false)));

export function waitForDevServer(
  url: string,
  options: WaitForDevServerOptions = {},
): Effect.Effect<void, DevServerTimeoutError> {
  const startedAt = Date.now();
  const intervalMs = options.intervalMs ?? 200;
  const timeoutMs = options.timeoutMs ?? 30_000;
  const isReady = options.isReady ?? isDevServerReady;

  return Effect.gen(function* () {
    while (Date.now() - startedAt < timeoutMs) {
      if (yield* isReady(url, intervalMs)) return;
      yield* Effect.sleep(Duration.millis(intervalMs));
    }
    return yield* Effect.fail(new DevServerTimeoutError(url));
  });
}

const installSignalHandlers = (
  interrupted: Ref.Ref<boolean>,
  mainFiber: Fiber.RuntimeFiber<void, unknown>,
) =>
  Effect.sync(() => {
    const interrupt = (signal: NodeJS.Signals) => {
      console.log(`[dev] received ${signal}, shutting down...`);
      void Effect.runPromise(
        Ref.set(interrupted, true).pipe(Effect.zipRight(Fiber.interrupt(mainFiber))),
      );
    };

    process.once("SIGINT", interrupt);
    process.once("SIGTERM", interrupt);
  });

const webDev = runChild("web", "pnpm", ["exec", "vite"], { cwd: "apps/web" });

const desktopDev = Effect.scoped(
  Effect.gen(function* () {
    const webFiber = yield* Effect.forkScoped(webDev);
    yield* waitForDevServer(WEB_DEV_SERVER_URL);
    yield* runCommand("desktop build", "pnpm", ["run", "build"], { cwd: "apps/desktop" });
    yield* runChild("desktop", "pnpm", ["exec", "electron", "dist-electron/main.cjs"], {
      cwd: "apps/desktop",
    });
    yield* Fiber.interrupt(webFiber);
  }),
);

export const program = Effect.gen(function* () {
  const mode = parseMode(process.argv);
  const interrupted = yield* Ref.make(false);
  const main = Effect.scoped(mode === "web" ? webDev : desktopDev);
  const fiber = yield* Effect.fork(main);
  yield* installSignalHandlers(interrupted, fiber);
  yield* Fiber.join(fiber);
});

function isMainModule(): boolean {
  const entrypoint = process.argv[1];
  return entrypoint != null && import.meta.url === pathToFileURL(entrypoint).href;
}

if (isMainModule()) {
  Effect.runPromise(program).catch((error: unknown) => {
    if (error instanceof Error) {
      console.error(`[dev] ${error.message}`);
    } else {
      console.error("[dev] failed", error);
    }
    process.exitCode = 1;
  });
}
