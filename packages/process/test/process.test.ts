import { describe, expect, it } from "vitest";
import { Effect, Exit, Scope } from "effect";
import {
  makeProcesses,
  makePtys,
  ProcessUnavailableError,
  type ProcessDriver,
  type ProcessStream,
} from "../src/index.js";

const encoder = new TextEncoder();

/** A fake driver: records spawns, lets tests push data/exit and observe kills. */
function fakeDriver() {
  type Proc = {
    id: number;
    command: string;
    killed: boolean;
    data: (chunk: Uint8Array, stream: ProcessStream) => void;
    exit: (code: number | null) => void;
  };
  const procs = new Map<number, Proc>();
  let nextId = 1;
  let failNext = false;
  const driver: ProcessDriver = {
    spawn: (options) => {
      if (failNext) {
        failNext = false;
        return Promise.resolve(null);
      }
      const id = nextId++;
      procs.set(id, {
        id,
        command: options.command,
        killed: false,
        data: () => {},
        exit: () => {},
      });
      return Promise.resolve(id);
    },
    write: () => {},
    kill: (id) => {
      const p = procs.get(id);
      if (p !== undefined) p.killed = true;
    },
    onData: (id, listener) => {
      const p = procs.get(id);
      if (p !== undefined) p.data = listener;
    },
    onExit: (id, listener) => {
      const p = procs.get(id);
      if (p !== undefined) p.exit = listener;
    },
  };
  return {
    driver,
    proc: (id: number) => procs.get(id),
    failNextSpawn: () => {
      failNext = true;
    },
    count: () => procs.size,
  };
}

describe("makeProcesses", () => {
  it("run collects stdout/stderr and the exit code", async () => {
    const fake = fakeDriver();
    const processes = makeProcesses(fake.driver);
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const promise = Effect.runPromise(processes.run({ command: "git" }));
        // Push output after the run subscribes (next microtask).
        yield* Effect.promise(() => Promise.resolve());
        const proc = fake.proc(1)!;
        proc.data(encoder.encode("hello "), "stdout");
        proc.data(encoder.encode("oops"), "stderr");
        proc.data(encoder.encode("world"), "stdout");
        proc.exit(0);
        return yield* Effect.promise(() => promise);
      }),
    );
    expect(result).toEqual({ code: 0, stdout: "hello world", stderr: "oops" });
    // Self-scoped: killed after completion.
    expect(fake.proc(1)?.killed).toBe(true);
  });

  it("buffers output that arrives before the first listener subscribes", async () => {
    const fake = fakeDriver();
    const processes = makeProcesses(fake.driver);
    // Emit BEFORE run's async subscriber attaches by pushing synchronously in
    // the acquire continuation is impossible here; instead assert via run that
    // the buffer replays: push immediately after spawn resolves.
    const result = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* processes.spawn({ command: "echo" });
          const proc = fake.proc(handle.pid)!;
          // Data arrives before onData — must be buffered and replayed.
          proc.data(encoder.encode("early"), "stdout");
          const seen: string[] = [];
          handle.onData((chunk) => seen.push(new TextDecoder().decode(chunk)));
          proc.data(encoder.encode("-late"), "stdout");
          return seen;
        }),
      ),
    );
    expect(result).toEqual(["early", "-late"]);
  });

  it("spawn kills the process when its scope closes", async () => {
    const fake = fakeDriver();
    const processes = makeProcesses(fake.driver);
    const scope = await Effect.runPromise(Scope.make());
    const handle = await Effect.runPromise(Scope.extend(processes.spawn({ command: "lsp" }), scope));
    expect(fake.proc(handle.pid)?.killed).toBe(false);
    await Effect.runPromise(Scope.close(scope, Exit.void));
    expect(fake.proc(handle.pid)?.killed).toBe(true);
  });

  it("kill is idempotent and blocks writes after", async () => {
    const fake = fakeDriver();
    const processes = makeProcesses(fake.driver);
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* processes.spawn({ command: "x" });
          handle.kill();
          handle.kill(); // no throw
          handle.write(encoder.encode("ignored"));
          expect(fake.proc(handle.pid)?.killed).toBe(true);
        }),
      ),
    );
  });

  it("late onExit subscriber is called immediately with the latched code", async () => {
    const fake = fakeDriver();
    const processes = makeProcesses(fake.driver);
    const code = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* processes.spawn({ command: "x" });
          fake.proc(handle.pid)!.exit(42);
          return yield* Effect.async<number | null>((resume) => {
            handle.onExit((c) => resume(Effect.succeed(c)));
          });
        }),
      ),
    );
    expect(code).toBe(42);
  });

  it("fails with ProcessSpawnError when the driver returns null", async () => {
    const fake = fakeDriver();
    fake.failNextSpawn();
    const processes = makeProcesses(fake.driver);
    const exit = await Effect.runPromiseExit(processes.run({ command: "nope" }));
    expect(Exit.isFailure(exit)).toBe(true);
  });

  it("with no driver, run/spawn fail with ProcessUnavailableError", async () => {
    const processes = makeProcesses(undefined);
    const exit = await Effect.runPromiseExit(processes.run({ command: "git" }));
    expect(Exit.isFailure(exit)).toBe(true);
    const failure = Exit.isFailure(exit) ? exit.cause : null;
    expect(JSON.stringify(failure)).toContain("ProcessUnavailableError");
  });
});

describe("makePtys", () => {
  it("spawns, exposes processName when the driver can, and kills on scope close", async () => {
    let killed = false;
    const driver = {
      spawn: () => Promise.resolve(7),
      write: () => {},
      resize: () => {},
      kill: () => {
        killed = true;
      },
      onData: () => {},
      onExit: () => {},
      processName: () => Promise.resolve("zsh"),
    };
    const ptys = makePtys(driver);
    const name = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* ptys.spawn({ cols: 80, rows: 24 });
          expect(handle.pid).toBe(7);
          return yield* Effect.promise(() => handle.processName!());
        }),
      ),
    );
    expect(name).toBe("zsh");
    expect(killed).toBe(true);
  });

  it("omits processName when the driver lacks it", async () => {
    const ptys = makePtys({
      spawn: () => Promise.resolve(1),
      write: () => {},
      resize: () => {},
      kill: () => {},
      onData: () => {},
      onExit: () => {},
    });
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* ptys.spawn({ cols: 1, rows: 1 });
          expect(handle.processName).toBeUndefined();
        }),
      ),
    );
  });

  it("with no driver, spawn fails unavailable", async () => {
    const ptys = makePtys(undefined);
    const exit = await Effect.runPromiseExit(Effect.scoped(ptys.spawn({ cols: 1, rows: 1 })));
    expect(Exit.isFailure(exit)).toBe(true);
  });
});
