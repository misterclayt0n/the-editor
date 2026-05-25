import { describe, expect, it } from "@effect/vitest";
import { Effect, Exit, Queue, Scope } from "effect";
import {
  ActivationId,
  ContributionId,
  ContributionIndexDuplicateRecordError,
  PackageDependencyCycleError,
  PackageDependencyMissingError,
  PackageId,
  RegistryDuplicateContributionError,
  RegistryId,
  StoreId,
  makeContributionIndex,
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

  it.effect("contribution index records scoped registration", () =>
    Effect.gen(function* () {
      const contributionIndex = yield* makeContributionIndex;
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex,
      });
      const scope = yield* Scope.make();
      yield* registry
        .register(
          { packageId: pid("pkg"), activationId: aid("pkg#1") },
          { id: cid("a"), value: "A" },
        )
        .pipe(Effect.provideService(Scope.Scope, scope));

      expect((yield* registry.list).map((x) => x.contributionId)).toEqual([cid("a")]);
      expect(yield* contributionIndex.list).toEqual([
        {
          owner: { packageId: pid("pkg"), activationId: aid("pkg#1") },
          registryId: RegistryId("test.registry"),
          contributionId: cid("a"),
        },
      ]);

      yield* Scope.close(scope, Exit.void);
      expect(yield* registry.list).toEqual([]);
      expect(yield* contributionIndex.list).toEqual([]);
    }),
  );

  it.effect("contribution index queries by package activation and registry", () =>
    Effect.gen(function* () {
      const contributionIndex = yield* makeContributionIndex;
      const registryA = yield* makeRegistry<TestContribution>(RegistryId("registry.a"), {
        contributionIndex,
      });
      const registryB = yield* makeRegistry<TestContribution>(RegistryId("registry.b"), {
        contributionIndex,
      });
      const scope = yield* Scope.make();
      yield* registryA
        .register(
          { packageId: pid("pkg.a"), activationId: aid("pkg.a#1") },
          { id: cid("a1"), value: "A1" },
        )
        .pipe(Effect.provideService(Scope.Scope, scope));
      yield* registryA
        .register(
          { packageId: pid("pkg.b"), activationId: aid("pkg.b#1") },
          { id: cid("a2"), value: "A2" },
        )
        .pipe(Effect.provideService(Scope.Scope, scope));
      yield* registryB
        .register(
          { packageId: pid("pkg.a"), activationId: aid("pkg.a#2") },
          { id: cid("b1"), value: "B1" },
        )
        .pipe(Effect.provideService(Scope.Scope, scope));

      expect(
        (yield* contributionIndex.listByPackage(pid("pkg.a"))).map((r) => r.contributionId),
      ).toEqual([cid("a1"), cid("b1")]);
      expect(
        (yield* contributionIndex.listByActivation(aid("pkg.a#1"))).map((r) => r.contributionId),
      ).toEqual([cid("a1")]);
      expect(
        (yield* contributionIndex.listByRegistry(RegistryId("registry.a"))).map(
          (r) => r.contributionId,
        ),
      ).toEqual([cid("a1"), cid("a2")]);
      yield* Scope.close(scope, Exit.void);
    }),
  );

  it.effect("same registry duplicate contribution is rejected", () =>
    Effect.gen(function* () {
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"));
      const scopeA = yield* Scope.make();
      const scopeB = yield* Scope.make();
      yield* registry
        .register(
          { packageId: pid("pkg.a"), activationId: aid("pkg.a#1") },
          { id: cid("same"), value: "A" },
        )
        .pipe(Effect.provideService(Scope.Scope, scopeA));

      const error = yield* Effect.flip(
        registry
          .register(
            { packageId: pid("pkg.b"), activationId: aid("pkg.b#1") },
            { id: cid("same"), value: "B" },
          )
          .pipe(Effect.provideService(Scope.Scope, scopeB)),
      );

      expect(error).toBeInstanceOf(RegistryDuplicateContributionError);
      expect((yield* registry.list).map((entry) => entry.contribution.value)).toEqual(["A"]);
      yield* Scope.close(scopeB, Exit.void);
      expect((yield* registry.list).map((entry) => entry.contribution.value)).toEqual(["A"]);
      yield* Scope.close(scopeA, Exit.void);
      expect(yield* registry.list).toEqual([]);
    }),
  );

  it.effect("index failure rolls back registry contribution", () =>
    Effect.gen(function* () {
      const contributionIndex = yield* makeContributionIndex;
      const registryA = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex,
      });
      const registryB = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex,
      });
      const scopeA = yield* Scope.make();
      const scopeB = yield* Scope.make();
      yield* registryA
        .register(
          { packageId: pid("pkg.a"), activationId: aid("pkg.a#1") },
          { id: cid("same"), value: "A" },
        )
        .pipe(Effect.provideService(Scope.Scope, scopeA));

      const error = yield* Effect.flip(
        registryB
          .register(
            { packageId: pid("pkg.b"), activationId: aid("pkg.b#1") },
            { id: cid("same"), value: "B" },
          )
          .pipe(Effect.provideService(Scope.Scope, scopeB)),
      );

      expect(error).toBeInstanceOf(ContributionIndexDuplicateRecordError);
      expect(yield* registryB.list).toEqual([]);
      expect((yield* contributionIndex.list).map((r) => r.owner.packageId)).toEqual([pid("pkg.a")]);
      yield* Scope.close(scopeB, Exit.void);
      yield* Scope.close(scopeA, Exit.void);
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

  it.effect("package reload removes old ownership records", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex: runtime.contributionIndex,
      });
      const mod = (value: string) =>
        descriptor("pkg", (ctx) =>
          registry.register(ctx.owner, { id: cid(value), value }).pipe(Effect.asVoid),
        );

      yield* runtime.activateGraph([mod("old")]);
      const first = yield* runtime.getPackage(pid("pkg"));
      expect((yield* runtime.contributionIndex.list).map((r) => r.contributionId)).toEqual([
        cid("old"),
      ]);

      yield* runtime.reloadPackage(pid("pkg"), mod("new"));
      const second = yield* runtime.getPackage(pid("pkg"));
      const records = yield* runtime.contributionIndex.list;
      expect(records.map((r) => r.contributionId)).toEqual([cid("new")]);
      expect(records[0]?.owner.activationId).toEqual(second?.activationId);
      expect(records[0]?.owner.activationId).not.toEqual(first?.activationId);
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

  it.effect("failed activation does not leak index records", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex: runtime.contributionIndex,
      });
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
      expect(yield* runtime.contributionIndex.listByPackage(pid("bad"))).toEqual([]);
    }),
  );

  it.effect("contribution index emits registry add and remove events", () =>
    Effect.gen(function* () {
      const contributionIndex = yield* makeContributionIndex;
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex,
      });
      const subscriptionScope = yield* Scope.make();
      const changes = yield* contributionIndex.changes.pipe(
        Effect.provideService(Scope.Scope, subscriptionScope),
      );
      const registrationScope = yield* Scope.make();

      yield* registry
        .register(
          { packageId: pid("pkg"), activationId: aid("pkg#1") },
          { id: cid("a"), value: "A" },
        )
        .pipe(Effect.provideService(Scope.Scope, registrationScope));
      const added = yield* Queue.take(changes);
      expect(added).toEqual({
        type: "added",
        record: {
          owner: { packageId: pid("pkg"), activationId: aid("pkg#1") },
          registryId: RegistryId("test.registry"),
          contributionId: cid("a"),
        },
      });

      yield* Scope.close(registrationScope, Exit.void);
      const removed = yield* Queue.take(changes);
      expect(removed).toEqual({
        type: "removed",
        record: added.record,
      });
      yield* Scope.close(subscriptionScope, Exit.void);
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
