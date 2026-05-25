import { describe, expect, it } from "@effect/vitest";
import { Context, Effect, Layer } from "effect";
import {
  ContributionId,
  PackageId,
  RegistryId,
  makePackageRuntime,
  makeRegistry,
  type ActivationId,
  type ContributionOwner,
  type IdentifiedContribution,
  type PackageId as KernelPackageId,
} from "@the-editor/kernel";
import { definePackage, toPackageDescriptor } from "../src/index.js";

interface TestContribution extends IdentifiedContribution {
  readonly value: string;
}

const cid = (value: string) => ContributionId(value);
const pid = (value: string) => PackageId(value);

class TestCapability extends Context.Tag("test/TestCapability")<
  TestCapability,
  { readonly value: Effect.Effect<number> }
>() {}

describe("extensions package authoring SDK", () => {
  it("definePackage simple form creates kernel descriptor", () => {
    const pkg = definePackage("user.config", () => Effect.void);
    const descriptor = toPackageDescriptor(pkg);
    expect(pkg.id).toEqual(pid("user.config"));
    expect(descriptor.id).toEqual(pid("user.config"));
    expect(descriptor.dependencies).toBeUndefined();
  });

  it("definePackage options form brands dependencies", () => {
    const pkg = definePackage(
      { id: "user.config", dependencies: ["foundation.a", "foundation.b"] },
      () => Effect.void,
    );
    const descriptor = toPackageDescriptor(pkg);
    expect(pkg.id).toEqual(pid("user.config"));
    expect(descriptor.id).toEqual(pid("user.config"));
    expect(descriptor.dependencies).toEqual([pid("foundation.a"), pid("foundation.b")]);
  });

  it.effect("activation receives ExtensionContext", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      let seen:
        | {
            readonly packageId: KernelPackageId;
            readonly activationId: ActivationId;
            readonly owner: ContributionOwner;
          }
        | undefined;
      const pkg = definePackage("pkg", (ctx) =>
        Effect.sync(() => {
          seen = {
            packageId: ctx.packageId,
            activationId: ctx.activationId,
            owner: ctx.owner,
          };
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg)]);
      const snapshot = yield* runtime.getPackage(pid("pkg"));

      expect(seen?.packageId).toEqual(pid("pkg"));
      expect(seen?.activationId).toEqual(snapshot?.activationId);
      expect(seen?.owner).toEqual({ packageId: pid("pkg"), activationId: snapshot?.activationId });
    }),
  );

  it.effect("package activation can require host-provided Effect services", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      let seen: number | undefined;
      const pkg = definePackage("pkg", () =>
        Effect.gen(function* () {
          const capability = yield* TestCapability;
          seen = yield* capability.value;
        }),
      );
      const descriptor = toPackageDescriptor(
        pkg,
        Layer.succeed(TestCapability, { value: Effect.succeed(42) }),
      );

      yield* runtime.activateGraph([descriptor]);

      expect((yield* runtime.getPackage(pid("pkg")))?.status).toBe("active");
      expect(seen).toBe(42);
    }),
  );

  it.effect("generic contribution registration works", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex: runtime.contributionIndex,
      });
      const pkg = definePackage("pkg", (ctx) =>
        ctx.contributions.register(registry, { id: cid("a"), value: "A" }).pipe(Effect.asVoid),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg)]);
      const snapshot = yield* runtime.getPackage(pid("pkg"));
      const registryItems = yield* registry.list;
      const records = yield* runtime.contributionIndex.list;

      expect(registryItems.map((item) => item.contribution)).toEqual([
        { id: cid("a"), value: "A" },
      ]);
      expect(records).toEqual([
        {
          owner: { packageId: pid("pkg"), activationId: snapshot?.activationId },
          registryId: RegistryId("test.registry"),
          contributionId: cid("a"),
        },
      ]);
    }),
  );

  it.effect("reload disposes old SDK contribution", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const registry = yield* makeRegistry<TestContribution>(RegistryId("test.registry"), {
        contributionIndex: runtime.contributionIndex,
      });
      const descriptor = (id: string) =>
        definePackage("pkg", (ctx) =>
          ctx.contributions.register(registry, { id: cid(id), value: id }).pipe(Effect.asVoid),
        );

      yield* runtime.activateGraph([toPackageDescriptor(descriptor("old"))]);
      yield* runtime.reloadPackage(pid("pkg"), toPackageDescriptor(descriptor("new")));

      expect((yield* registry.list).map((item) => item.contributionId)).toEqual([cid("new")]);
    }),
  );

  it.effect("package-lifetime extension store survives reload", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const seen: number[] = [];
      const pkg = definePackage("pkg", (ctx) =>
        Effect.gen(function* () {
          const store = yield* ctx.stores.package({
            id: "counter",
            version: 1,
            initial: Effect.succeed(0),
          });
          seen.push(yield* store.get);
          yield* store.set(1);
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg)]);
      yield* runtime.reloadPackage(pid("pkg"));
      expect(seen).toEqual([0, 1]);
    }),
  );

  it.effect("activation-lifetime extension store resets on reload", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const seen: number[] = [];
      const pkg = definePackage("pkg", (ctx) =>
        Effect.gen(function* () {
          const store = yield* ctx.stores.activation({
            id: "counter",
            version: 1,
            initial: Effect.succeed(0),
          });
          seen.push(yield* store.get);
          yield* store.set(1);
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(pkg)]);
      yield* runtime.reloadPackage(pid("pkg"));
      expect(seen).toEqual([0, 0]);
    }),
  );

  it.effect("application-lifetime extension store is shared across packages", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      let readByB: number | undefined;
      const definition = { id: "shared", version: 1, initial: Effect.succeed(0) };
      const packageA = definePackage("A", (ctx) =>
        Effect.gen(function* () {
          const store = yield* ctx.stores.application(definition);
          yield* store.set(42);
        }),
      );
      const packageB = definePackage("B", (ctx) =>
        Effect.gen(function* () {
          const store = yield* ctx.stores.application(definition);
          readByB = yield* store.get;
        }),
      );

      yield* runtime.activateGraph([toPackageDescriptor(packageA), toPackageDescriptor(packageB)]);
      expect(readByB).toBe(42);
    }),
  );
});
