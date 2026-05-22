import { describe, expect, it } from "@effect/vitest";
import { createServer, type Server } from "node:http";
import { Effect, Scope } from "effect";
import {
  ChildProcessError,
  DevServerTimeoutError,
  isDevServerReady,
  parseMode,
  runCommand,
  waitForDevServer,
} from "./dev.ts";

const nodeArgs = (source: string) => ["--eval", source] as const;

describe("dev runner", () => {
  it("parses the default desktop mode", () => {
    expect(parseMode(["node", "scripts/dev.ts"])).toBe("desktop");
  });

  it("parses explicit web mode", () => {
    expect(parseMode(["node", "scripts/dev.ts", "web"])).toBe("web");
  });

  it.effect("runs a successful command", () =>
    runCommand("success", process.execPath, nodeArgs("process.exit(0)")),
  );

  it.effect("turns a failed command into a typed error", () =>
    Effect.gen(function* () {
      const error = yield* Effect.flip(
        runCommand("failure", process.execPath, nodeArgs("process.exit(7)")),
      );

      expect(error).toBeInstanceOf(ChildProcessError);
      expect(error.label).toBe("failure");
      expect(error.code).toBe(7);
    }),
  );

  it.effect("detects a ready dev server", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const server = yield* makeHttpServer();
        const ready = yield* isDevServerReady(server.url);

        expect(ready).toBe(true);
      }),
    ),
  );

  it.effect("times out while waiting for a missing dev server", () =>
    Effect.gen(function* () {
      const error = yield* Effect.flip(
        waitForDevServer("http://missing-dev-server.test", {
          intervalMs: 1,
          isReady: () => Effect.succeed(false),
          timeoutMs: 0,
        }),
      );

      expect(error).toBeInstanceOf(DevServerTimeoutError);
    }),
  );
});

interface TestHttpServer {
  readonly server: Server;
  readonly url: string;
}

function makeHttpServer(): Effect.Effect<TestHttpServer, never, Scope.Scope> {
  return Effect.acquireRelease(
    Effect.async<TestHttpServer>((resume) => {
      const server = createServer((_request, response) => {
        response.statusCode = 204;
        response.end();
      });

      server.listen(0, "127.0.0.1", () => {
        const address = server.address();
        if (address == null || typeof address === "string") {
          resume(Effect.die(new Error("Expected TCP server address")));
          return;
        }

        resume(
          Effect.succeed({
            server,
            url: `http://127.0.0.1:${String(address.port)}`,
          }),
        );
      });
    }),
    ({ server }) =>
      Effect.async<void>((resume) => {
        server.close(() => resume(Effect.void));
      }),
  );
}
