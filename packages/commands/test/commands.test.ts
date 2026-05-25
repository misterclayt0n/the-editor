import { describe, expect, it } from "@effect/vitest";
import { Context, Effect, Layer } from "effect";
import { definePackage, toPackageDescriptor } from "@the-editor/extensions";
import { PackageActivationError, PackageId, makePackageRuntime } from "@the-editor/kernel";
import {
  CommandExecutionError,
  CommandId,
  CommandNotFoundError,
  Commands,
  makeCommands,
} from "../src/index.js";

const pid = (value: string) => PackageId(value);

class TestCapability extends Context.Tag("test/TestCapability")<
  TestCapability,
  { readonly value: Effect.Effect<number> }
>() {}

describe("Commands", () => {
  it.effect("package registers command through Commands service", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      const pkg = definePackage("pkg", () =>
        Effect.gen(function* () {
          const commands = yield* Commands;
          yield* commands.register("user.sayHello", {
            title: "Say Hello",
            run: () => Effect.void,
          });
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg, Layer.succeed(Commands, commands))]);
      const snapshot = yield* runtime.getPackage(pid("pkg"));
      const list = yield* commands.list;
      const records = yield* runtime.contributionIndex.list;

      expect(list.map((item) => item.contributionId)).toEqual([CommandId("user.sayHello")]);
      expect(list[0]?.owner).toEqual({
        packageId: pid("pkg"),
        activationId: snapshot?.activationId,
      });
      expect(records).toEqual([
        {
          owner: { packageId: pid("pkg"), activationId: snapshot?.activationId },
          registryId: list[0]?.registryId,
          contributionId: CommandId("user.sayHello"),
        },
      ]);
    }),
  );

  it.effect("execute runs command", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      let ran = false;
      const pkg = definePackage("pkg", () =>
        Effect.gen(function* () {
          const commands = yield* Commands;
          yield* commands.register("run", { run: () => Effect.sync(() => (ran = true)) });
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg, Layer.succeed(Commands, commands))]);
      yield* commands.execute("run");

      expect(ran).toBe(true);
    }),
  );

  it.effect("missing command fails", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      const error = yield* Effect.flip(commands.execute("missing"));

      expect(error).toBeInstanceOf(CommandNotFoundError);
      if (error instanceof CommandNotFoundError) {
        expect(error.commandId).toEqual(CommandId("missing"));
      }
    }),
  );

  it.effect("command failure is wrapped", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      const pkg = definePackage("pkg", () =>
        Effect.gen(function* () {
          const commands = yield* Commands;
          yield* commands.register("fail", { run: () => Effect.fail("boom") });
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg, Layer.succeed(Commands, commands))]);
      const error = yield* Effect.flip(commands.execute("fail"));

      expect(error).toBeInstanceOf(CommandExecutionError);
      if (error instanceof CommandExecutionError) {
        expect(error.commandId).toEqual(CommandId("fail"));
        expect(error.cause).toBeDefined();
      }
    }),
  );

  it.effect("duplicate command id fails and keeps first command", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      const layer = Layer.succeed(Commands, commands);
      const packageA = definePackage("A", () =>
        Effect.gen(function* () {
          const commands = yield* Commands;
          yield* commands.register("same", { run: () => Effect.void });
        }),
      );
      const packageB = definePackage("B", () =>
        Effect.gen(function* () {
          const commands = yield* Commands;
          yield* commands.register("same", { run: () => Effect.void });
        }),
      );

      const error = yield* Effect.flip(
        runtime.activateGraph([
          toPackageDescriptor(packageA, layer),
          toPackageDescriptor(packageB, layer),
        ]),
      );

      expect(error).toBeInstanceOf(PackageActivationError);
      expect((yield* commands.list).map((item) => item.owner.packageId)).toEqual([pid("A")]);
      expect((yield* runtime.getPackage(pid("B")))?.status).toBe("failed");
    }),
  );

  it.effect("reload removes old command and registers new one", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      const descriptor = (id: string) =>
        toPackageDescriptor(
          definePackage("pkg", () =>
            Effect.gen(function* () {
              const commands = yield* Commands;
              yield* commands.register(id, { run: () => Effect.void });
            }),
          ),
          Layer.succeed(Commands, commands),
        );

      yield* runtime.activateGraph([descriptor("old")]);
      yield* runtime.reloadPackage(pid("pkg"), descriptor("new"));

      expect(yield* commands.get("old")).toBeUndefined();
      expect((yield* commands.get("new"))?.contributionId).toEqual(CommandId("new"));
    }),
  );

  it.effect("dispose package removes command", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      const pkg = definePackage("pkg", () =>
        Effect.gen(function* () {
          const commands = yield* Commands;
          yield* commands.register("gone", { run: () => Effect.void });
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg, Layer.succeed(Commands, commands))]);
      yield* runtime.disposePackage(pid("pkg"));

      expect(yield* commands.list).toEqual([]);
    }),
  );

  it.effect("command handler can capture host capability at registration time", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const commands = yield* makeCommands(runtime.contributionIndex);
      let seen: number | undefined;
      const pkg = definePackage("pkg", () =>
        Effect.gen(function* () {
          const capability = yield* TestCapability;
          const commands = yield* Commands;
          yield* commands.register("read", {
            run: () =>
              capability.value.pipe(Effect.tap((value) => Effect.sync(() => (seen = value)))),
          });
        }),
      );
      const layer = Layer.merge(
        Layer.succeed(Commands, commands),
        Layer.succeed(TestCapability, { value: Effect.succeed(42) }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);
      yield* commands.execute("read");

      expect(seen).toBe(42);
    }),
  );
});
