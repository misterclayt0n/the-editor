import { describe, expect, it } from "@effect/vitest";
import { Effect, Layer, Option, Stream } from "effect";
import * as EffectNamespace from "effect";
import {
  Attention,
  makeAttention,
  makePackageRuntime,
  makeStateCell,
  PackageRuntimeService,
} from "@the-editor/kernel";
import { Commands, makeCommands } from "@the-editor/commands";
import * as CommandsNamespace from "@the-editor/commands";
import { toPackageDescriptor } from "@the-editor/extensions";
import * as ExtensionsNamespace from "@the-editor/extensions";
import {
  FileIoError,
  FileSystemPort,
  WorkspaceRoots,
  type DirEntry,
  type FileStat,
} from "@the-editor/editor";
import { makeUserExtensionsPackage } from "./userExtensions";

const testVirtualModules = {
  effect: EffectNamespace,
  "@the-editor/extensions": ExtensionsNamespace,
  "@the-editor/commands": CommandsNamespace,
};

/** A minimal in-memory FileSystemPort: just enough readDir/readFile/stat for
 *  discovery, keyed by full path (no real directory semantics beyond one
 *  level, which is all discovery needs). `delayMs` inserts a real async gap
 *  before every read so concurrent callers actually interleave instead of
 *  each reconcile pass running to completion before the next one starts —
 *  without it, an in-memory fake resolves synchronously-fast enough that a
 *  missing semaphore never gets exercised. */
function makeFakeFs(delayMs = 0) {
  const files = new Map<string, { text: string; mtimeMs: number }>();
  let clock = 1;
  const maybeDelay = <A, E, R>(effect: Effect.Effect<A, E, R>) =>
    delayMs > 0 ? Effect.delay(effect, `${delayMs} millis`) : effect;

  const write = (path: string, text: string): void => {
    files.set(path, { text, mtimeMs: ++clock });
  };
  const remove = (path: string): void => {
    files.delete(path);
  };

  const port: FileSystemPort.Service = {
    readFile: (path) => {
      const entry = files.get(path);
      return maybeDelay(
        entry !== undefined
          ? Effect.succeed(entry.text)
          : Effect.fail(new FileIoError(path, "read", "not found")),
      );
    },
    writeFile: (path, text) => {
      write(path, text);
      const entry = files.get(path)!;
      return Effect.succeed({ mtimeMs: entry.mtimeMs, size: text.length });
    },
    stat: (path) => {
      const entry = files.get(path);
      const stat: FileStat | null =
        entry !== undefined ? { mtimeMs: entry.mtimeMs, size: entry.text.length } : null;
      return maybeDelay(Effect.succeed(Option.fromNullable(stat)));
    },
    watch: () => Effect.succeed(() => {}),
    pickFile: () => Effect.succeed(Option.none()),
    readDir: (dir) => {
      const prefix = dir.endsWith("/") ? dir : `${dir}/`;
      const seen = new Set<string>();
      const entries: DirEntry[] = [];
      for (const path of files.keys()) {
        if (!path.startsWith(prefix)) continue;
        const rest = path.slice(prefix.length);
        const head = rest.split("/")[0];
        if (head === undefined || head === "" || seen.has(head)) continue;
        seen.add(head);
        entries.push({ name: head, kind: rest.includes("/") ? "directory" : "file" });
      }
      return Effect.succeed(entries);
    },
    mkdir: () => Effect.succeed(undefined),
    capabilities: { watch: false },
  };

  return { port, write, remove };
}

/** Poll a boolean-producing Effect until it settles true — the package's
 *  discovery/load work runs in forked background fibers, so tests observe it
 *  settle rather than awaiting a handle directly. */
const waitUntil = <E, R>(
  check: Effect.Effect<boolean, E, R>,
  attempts = 200,
): Effect.Effect<void, E, R> =>
  Effect.gen(function* () {
    for (let i = 0; i < attempts; i++) {
      if (yield* check) return;
      yield* Effect.sleep("5 millis");
    }
    yield* Effect.dieMessage("waitUntil: condition never became true");
  });

const identityTranspiler = { transform: async (source: string) => source };

function packageSource(
  id: string,
  commandId: string,
  title: string,
  options?: { readonly countActivationsAs?: string },
): string {
  const countLine =
    options?.countActivationsAs !== undefined
      ? `require(${JSON.stringify(options.countActivationsAs)}).increment();`
      : "";
  return `
    var definePackage = require("@the-editor/extensions").definePackage;
    var Commands = require("@the-editor/commands").Commands;
    var Effect = require("effect").Effect;
    module.exports.default = definePackage(${JSON.stringify(id)}, function () {
      ${countLine}
      return Effect.flatMap(Commands, function (commands) {
        return commands.register(${JSON.stringify(commandId)}, {
          title: ${JSON.stringify(title)},
          run: function () { return Effect.void; },
        });
      });
    });
  `;
}

describe("makeUserExtensionsPackage", () => {
  it.live("loads a global extension at startup and reloads it after an edit", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/home/user/.the-editor/extensions/hello.ts",
          packageSource("acme.hello", "acme.hello", "Hello v1"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: "/home/user",
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        yield* waitUntil(commands.get("acme.hello").pipe(Effect.map((c) => c !== undefined)));
        const loaded = yield* commands.get("acme.hello");
        expect(loaded?.contribution.title).toEqual("Hello v1");

        // Edit the file (bumps mtime) and reload.
        fs.write(
          "/home/user/.the-editor/extensions/hello.ts",
          packageSource("acme.hello", "acme.hello", "Hello v2"),
        );
        yield* commands.execute("reload");
        yield* waitUntil(
          commands.get("acme.hello").pipe(Effect.map((c) => c?.contribution.title === "Hello v2")),
        );

        // Delete the file and reload: the command should disappear.
        fs.remove("/home/user/.the-editor/extensions/hello.ts");
        yield* commands.execute("reload");
        yield* waitUntil(commands.get("acme.hello").pipe(Effect.map((c) => c === undefined)));
      }),
    ),
  );

  it.live("refuses to load an extension whose id collides with an active package", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/home/user/.the-editor/extensions/a.ts",
          packageSource("acme.shared", "acme.a", "From A"),
        );
        fs.write(
          "/home/user/.the-editor/extensions/b.ts",
          packageSource("acme.shared", "acme.b", "From B"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: "/home/user",
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        // Exactly one of the two same-id extensions wins; the other is
        // refused (surfaced as an attention notice, not a crash).
        yield* waitUntil(
          Effect.all([commands.get("acme.a"), commands.get("acme.b")]).pipe(
            Effect.map(([a, b]) => (a !== undefined) !== (b !== undefined)),
          ),
        );
      }),
    ),
  );

  it.live("does not load project-local extensions without trust", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/workspace/.the-editor/extensions/proj.ts",
          packageSource("acme.project", "acme.project", "Project ext"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention(); // no responder registered
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: undefined, // no global dir either: only the project path is exercised
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        yield* workspaceRoots.set(["/workspace"]);
        // With no responder, `ask`'s `default: "skip"` resolves immediately —
        // give the reactive fiber a moment to run, then assert nothing loaded.
        yield* Effect.sleep("30 millis");
        const command = yield* commands.get("acme.project");
        expect(command).toBeUndefined();
      }),
    ),
  );

  it.live("loads project-local extensions once the workspace is trusted", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/workspace/.the-editor/extensions/proj.ts",
          packageSource("acme.project", "acme.project", "Project ext"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: undefined,
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        // A responder that always answers "trust", mirroring the app's real
        // trust dialog answering yes. Registered in this same scope, so it's
        // torn down automatically when the test's `Effect.scoped` closes.
        yield* attention.registerResponder;
        yield* Effect.forkScoped(
          Stream.runForEach(attention.requestChanges, (pending) =>
            Effect.forEach(pending, (p) => p.respond("trust"), { discard: true }),
          ),
        );

        yield* workspaceRoots.set(["/workspace"]);
        yield* waitUntil(commands.get("acme.project").pipe(Effect.map((c) => c !== undefined)));
      }),
    ),
  );

  it.live(
    "survives concurrent reload command invocations and a root change racing each other",
    () =>
      Effect.scoped(
        Effect.gen(function* () {
          const fs = makeFakeFs(3);
          const helloActivations = { count: 0, increment: () => void helloActivations.count++ };
          fs.write(
            "/home/user/.the-editor/extensions/hello.ts",
            packageSource("acme.hello", "acme.hello", "Hello", {
              countActivationsAs: "test:hello-counter",
            }),
          );
          fs.write(
            "/workspace/.the-editor/extensions/proj.ts",
            packageSource("acme.project", "acme.project", "Project ext"),
          );

          const runtime = yield* makePackageRuntime;
          const attention = yield* makeAttention();
          const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
          const commands = yield* makeCommands(runtime.contributionIndex);

          const servicesForExtensions = Layer.mergeAll(
            Layer.succeed(Commands, commands),
            Layer.succeed(FileSystemPort, fs.port),
            Layer.succeed(PackageRuntimeService, runtime),
            Layer.succeed(Attention, attention),
            Layer.succeed(WorkspaceRoots, workspaceRoots),
          );

          const userExtensions = makeUserExtensionsPackage({
            servicesForExtensions,
            transpiler: identityTranspiler,
            virtualModules: { ...testVirtualModules, "test:hello-counter": helloActivations },
            homeDir: "/home/user",
          });

          yield* attention.registerResponder;
          const failures: string[] = [];
          yield* Effect.forkScoped(
            Stream.runForEach(attention.notices, (notice) =>
              Effect.sync(() => {
                if (notice.level === "error") failures.push(JSON.stringify(notice.payload));
              }),
            ),
          );
          yield* Effect.forkScoped(
            Stream.runForEach(attention.requestChanges, (pending) =>
              Effect.forEach(pending, (p) => p.respond("trust"), { discard: true }),
            ),
          );

          // Package activation registers the command synchronously and only
          // FORKS its first discover+reconcile pass — nothing is loaded yet
          // when this resolves.
          yield* runtime.activateGraph([
            toPackageDescriptor(userExtensions, servicesForExtensions),
          ]);

          // Fire the whole storm immediately: the boot fork's own first
          // reconcile pass races directly against several manual reloads and
          // a workspace-root change, all trying to be the first to discover
          // and activate "acme.hello" — exactly the boot-fork / reactive-
          // subscription / command triangle `reloadAll`'s semaphore exists to
          // serialize.
          yield* Effect.all(
            [
              ...Array.from({ length: 8 }, () => commands.execute("reload")),
              workspaceRoots.set(["/workspace"]),
              commands.execute("reload"),
            ],
            { concurrency: "unbounded", discard: true },
          );

          // 10 producers, fully serialized through one semaphore permit,
          // each paying the fake fs's 10ms delay several times over — give
          // this a wider poll budget than the single-producer tests above.
          yield* waitUntil(
            commands.get("acme.project").pipe(Effect.map((c) => c !== undefined)),
            1000,
          );
          yield* waitUntil(
            commands.get("acme.hello").pipe(Effect.map((c) => c !== undefined)),
            1000,
          );

          // Exactly one live registration each — no duplicate-activation
          // collisions, no dispose/activate failures surfaced. The activation
          // counter is the direct signal: without `reloadAll`'s semaphore,
          // several concurrent reconcile passes each see "acme.hello" as
          // not-yet-active and independently activate it.
          const hello = yield* commands.get("acme.hello");
          const project = yield* commands.get("acme.project");
          expect(hello?.contribution.title).toEqual("Hello");
          expect(project?.contribution.title).toEqual("Project ext");
          expect(failures).toEqual([]);
          expect(helloActivations.count).toBe(1);
        }),
      ),
    10_000,
  );
});
