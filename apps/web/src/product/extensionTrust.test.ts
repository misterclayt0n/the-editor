import { describe, expect, it } from "@effect/vitest";
import { Effect, Option, Stream } from "effect";
import { makeAttention } from "@the-editor/kernel";
import { FileIoError, type FileSystemPort, type FileStat } from "@the-editor/editor";
import { makeExtensionTrust } from "./extensionTrust";

/** Same tiny in-memory FileSystemPort shape used in userExtensions.test.ts,
 *  trimmed to what trust persistence actually touches (read/write/mkdir). */
function makeFakeFs() {
  const files = new Map<string, { text: string; mtimeMs: number }>();
  let clock = 1;

  const port: FileSystemPort.Service = {
    readFile: (path) => {
      const entry = files.get(path);
      return entry !== undefined
        ? Effect.succeed(entry.text)
        : Effect.fail(new FileIoError(path, "read", "not found"));
    },
    writeFile: (path, text) => {
      files.set(path, { text, mtimeMs: ++clock });
      return Effect.succeed({ mtimeMs: files.get(path)!.mtimeMs, size: text.length });
    },
    stat: (path) => {
      const entry = files.get(path);
      const stat: FileStat | null =
        entry !== undefined ? { mtimeMs: entry.mtimeMs, size: entry.text.length } : null;
      return Effect.succeed(Option.fromNullable(stat));
    },
    watch: () => Effect.succeed(() => {}),
    pickFile: () => Effect.succeed(Option.none()),
    mkdir: () => Effect.succeed(undefined),
    capabilities: { watch: false },
  };

  return { port, files };
}

describe("makeExtensionTrust", () => {
  it.live("asks Attention at most once per root and remembers the answer", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        const attention = yield* makeAttention();
        let asked = 0;

        yield* attention.registerResponder;
        yield* Effect.forkScoped(
          Stream.runForEach(attention.requestChanges, (pending) =>
            Effect.forEach(
              pending,
              (p) =>
                Effect.sync(() => {
                  asked++;
                }).pipe(Effect.zipRight(p.respond("trust"))),
              { discard: true },
            ),
          ),
        );

        const trust = makeExtensionTrust(fs.port, attention, "/home/user");
        const first = yield* trust.ensureTrust("/workspace");
        const second = yield* trust.ensureTrust("/workspace");

        expect(first).toBe(true);
        expect(second).toBe(true);
        expect(asked).toBe(1);
      }),
    ),
  );

  it.live("persists trust decisions and a fresh instance reads them back", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        const attention = yield* makeAttention();
        yield* attention.registerResponder;
        yield* Effect.forkScoped(
          Stream.runForEach(attention.requestChanges, (pending) =>
            Effect.forEach(pending, (p) => p.respond("trust"), { discard: true }),
          ),
        );

        const first = makeExtensionTrust(fs.port, attention, "/home/user");
        yield* first.ensureTrust("/workspace");

        expect(fs.files.has("/home/user/.the-editor/trust.json")).toBe(true);

        // A brand-new instance (simulating a restart) reads the persisted
        // decision back and never asks again — no responder needed this time.
        const second = makeExtensionTrust(fs.port, attention, "/home/user");
        const remembered = yield* second.ensureTrust("/workspace");
        expect(remembered).toBe(true);
      }),
    ),
  );

  it.live("defaults to denied with no responder mounted", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        const attention = yield* makeAttention(); // no registerResponder
        const trust = makeExtensionTrust(fs.port, attention, "/home/user");
        const trusted = yield* trust.ensureTrust("/workspace");
        expect(trusted).toBe(false);
      }),
    ),
  );

  it.live("never touches the filesystem without a home directory", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        const attention = yield* makeAttention();
        yield* attention.registerResponder;
        yield* Effect.forkScoped(
          Stream.runForEach(attention.requestChanges, (pending) =>
            Effect.forEach(pending, (p) => p.respond("trust"), { discard: true }),
          ),
        );

        const trust = makeExtensionTrust(fs.port, attention, undefined);
        const trusted = yield* trust.ensureTrust("/workspace");
        expect(trusted).toBe(true);
        expect(fs.files.size).toBe(0);
      }),
    ),
  );
});
