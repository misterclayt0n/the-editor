import { describe, expect, it } from "@effect/vitest";
import { Effect, Exit, Scope } from "effect";
import {
  ActivationId,
  ContributionId,
  PackageDependencyCycleError,
  PackageDependencyMissingError,
  PackageId,
  RegistryId,
  StoreId,
  makePackageRuntime,
  makeRegistry,
  type IdentifiedContribution,
  type PackageContext,
  type PackageDescriptor,
  type StoreDefinition,
} from "../src/index.js";

interface TestContribution extends IdentifiedContribution {
  readonly value: string;
}
const aid = (s: string) => ActivationId(s);
const cid = (s: string) => ContributionId(s);
const pid = (s: string) => PackageId(s);

const descriptor = (
  id: string,
  activate: (context: PackageContext) => Effect.Effect<void, unknown, Scope.Scope>,
  dependencies?: readonly PackageId[],
): PackageDescriptor => ({
  id: pid(id),
  ...(dependencies ? { dependencies } : {}),
  module: { activate },
});

describe("kernel", () => {
  it.effect("registry scope cleanup", () =>
    Effect.gen(function* () {
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"));
      const scope = yield* Scope.make();
      yield* registry
        .register(
          { packageId: pid("pkg"), activationId: aid("pkg#1") },
          { id: cid("a"), value: "A" },
        )
        .pipe(Effect.provideService(Scope.Scope, scope));
      expect((yield* registry.list).map((x) => x.contributionId)).toEqual([cid("a")]);
      yield* Scope.close(scope, Exit.void);
      expect(yield* registry.list).toEqual([]);
    }),
  );

  it.effect("package activation registers contribution", () =>
    Effect.gen(function* () {
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"));
      const runtime = yield* makePackageRuntime;
      const pkg = descriptor("pkg", (ctx) =>
        registry.register(ctx.owner, { id: cid("a"), value: "A" }).pipe(Effect.asVoid),
      );
      yield* runtime.activateGraph([pkg]);
      const snap = yield* runtime.getPackage(pid("pkg"));
      const items = yield* registry.list;
      expect(snap?.status).toBe("active");
      expect(items[0]?.owner.packageId).toEqual(pid("pkg"));
      expect(items[0]?.owner.activationId).toEqual(snap?.activationId);
    }),
  );

  it.effect("package reload disposes old generation", () =>
    Effect.gen(function* () {
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"));
      const runtime = yield* makePackageRuntime;
      yield* runtime.activateGraph([
        descriptor("pkg", (ctx) =>
          registry.register(ctx.owner, { id: cid("old"), value: "old" }).pipe(Effect.asVoid),
        ),
      ]);
      const first = yield* runtime.getPackage(pid("pkg"));
      yield* runtime.reloadPackage(
        pid("pkg"),
        descriptor("pkg", (ctx) =>
          registry.register(ctx.owner, { id: cid("new"), value: "new" }).pipe(Effect.asVoid),
        ),
      );
      const second = yield* runtime.getPackage(pid("pkg"));
      const items = yield* registry.list;
      expect(items.map((x) => x.contributionId)).toEqual([cid("new")]);
      expect(second?.activationId).not.toEqual(first?.activationId);
    }),
  );

  it.effect("failed activation rolls back contributions", () =>
    Effect.gen(function* () {
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"));
      const runtime = yield* makePackageRuntime;
      yield* runtime
        .activateGraph([
          descriptor("bad", (ctx) =>
            Effect.gen(function* () {
              yield* registry.register(ctx.owner, { id: cid("a"), value: "A" });
              return yield* Effect.fail("boom");
            }),
          ),
        ])
        .pipe(Effect.exit);
      expect((yield* runtime.getPackage(pid("bad")))?.status).toBe("failed");
      expect(yield* registry.list).toEqual([]);
      yield* runtime.activateGraph([descriptor("good", () => Effect.void)]);
      expect((yield* runtime.getPackage(pid("good")))?.status).toBe("active");
    }),
  );

  it.effect("dependency order", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const order: string[] = [];
      yield* runtime.activateGraph([
        descriptor("B", () => Effect.sync(() => order.push("B")), [pid("A")]),
        descriptor("A", () => Effect.sync(() => order.push("A"))),
      ]);
      expect(order).toEqual(["A", "B"]);
      expect((yield* runtime.listPackages).map((s) => s.status)).toEqual(["active", "active"]);
    }),
  );

  it.effect("missing dependency", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const error = yield* Effect.flip(
        runtime.activateGraph([descriptor("B", () => Effect.void, [pid("A")])]),
      );
      expect(error).toBeInstanceOf(PackageDependencyMissingError);
      expect(yield* runtime.listPackages).toEqual([]);
    }),
  );

  it.effect("cycle detection", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const error = yield* Effect.flip(
        runtime.activateGraph([
          descriptor("A", () => Effect.void, [pid("B")]),
          descriptor("B", () => Effect.void, [pid("A")]),
        ]),
      );
      expect(error).toBeInstanceOf(PackageDependencyCycleError);
      expect(yield* runtime.listPackages).toEqual([]);
    }),
  );

  it.effect("store package lifetime survives reload", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const store: StoreDefinition<number> = {
        id: StoreId("counter"),
        version: 1,
        lifetime: "package",
        initial: Effect.succeed(0),
      };
      const seen: number[] = [];
      const mod = descriptor("pkg", (ctx) =>
        Effect.gen(function* () {
          const s = yield* ctx.stores.getOrCreate(store, ctx);
          seen.push(yield* s.get);
          yield* s.set(1);
        }),
      );
      yield* runtime.activateGraph([mod]);
      yield* runtime.reloadPackage(pid("pkg"), mod);
      expect(seen).toEqual([0, 1]);
    }),
  );

  it.effect("activation-lifetime store dies on reload", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const store: StoreDefinition<number> = {
        id: StoreId("counter"),
        version: 1,
        lifetime: "activation",
        initial: Effect.succeed(0),
      };
      const seen: number[] = [];
      const mod = descriptor("pkg", (ctx) =>
        Effect.gen(function* () {
          const s = yield* ctx.stores.getOrCreate(store, ctx);
          seen.push(yield* s.get);
          yield* s.set(1);
        }),
      );
      yield* runtime.activateGraph([mod]);
      yield* runtime.reloadPackage(pid("pkg"), mod);
      expect(seen).toEqual([0, 0]);
    }),
  );
});
