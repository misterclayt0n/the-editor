import { Context, Effect, Exit, Layer, Ref, Scope } from "effect";
import {
  makeContributionIndex,
  type ContributionIndex,
} from "../contribution/ContributionIndex.js";
import { ActivationId, type PackageId } from "../ids.js";
import {
  PackageActivationError,
  PackageDependencyCycleError,
  PackageDependencyMissingError,
  PackageNotFoundError,
  type PackageError,
} from "../errors.js";
import { makeStoreRuntime, type StoreRuntime } from "../store/StoreRuntime.js";
import type { PackageDescriptor } from "./PackageDescriptor.js";

export type PackageStatus =
  | "inactive"
  | "activating"
  | "active"
  | "failed"
  | "blocked"
  | "disposed";

export interface PackageSnapshot {
  readonly packageId: PackageId;
  readonly activationId?: ActivationId;
  readonly status: PackageStatus;
  readonly dependencies: readonly PackageId[];
  readonly blockedBy?: readonly PackageId[];
  readonly failure?: unknown;
}

interface ActiveRecord {
  readonly descriptor: PackageDescriptor;
  readonly scope: Scope.CloseableScope;
  readonly activationId: ActivationId;
}

export interface PackageRuntime {
  readonly activateGraph: (
    descriptors: readonly PackageDescriptor[],
  ) => Effect.Effect<void, PackageError>;
  readonly activatePackage: (descriptor: PackageDescriptor) => Effect.Effect<void, PackageError>;
  readonly reloadPackage: (
    packageId: PackageId,
    descriptor?: PackageDescriptor,
  ) => Effect.Effect<void, PackageError>;
  readonly disposePackage: (packageId: PackageId) => Effect.Effect<void, PackageError>;
  readonly listPackages: Effect.Effect<readonly PackageSnapshot[]>;
  readonly getPackage: (packageId: PackageId) => Effect.Effect<PackageSnapshot | undefined>;
  readonly listDependencies: (packageId: PackageId) => Effect.Effect<readonly PackageId[]>;
  readonly listDependents: (packageId: PackageId) => Effect.Effect<readonly PackageId[]>;
  readonly listAffectedDependents: (packageId: PackageId) => Effect.Effect<readonly PackageId[]>;
  readonly contributionIndex: ContributionIndex;
  readonly stores: StoreRuntime;
}

const sortPackageIds = (
  packageIds: readonly PackageId[],
  descriptors: ReadonlyMap<string, PackageDescriptor>,
): Effect.Effect<
  readonly PackageId[],
  PackageDependencyCycleError | PackageDependencyMissingError
> =>
  Effect.gen(function* () {
    const targets = new Set<string>(packageIds);
    const visiting = new Set<string>();
    const visited = new Set<string>();
    const out: PackageId[] = [];

    const visit = (
      packageId: PackageId,
      stack: readonly PackageId[],
    ): PackageDependencyCycleError | PackageDependencyMissingError | undefined => {
      if (visited.has(packageId)) return undefined;
      if (visiting.has(packageId)) {
        return new PackageDependencyCycleError({ packageIds: [...stack, packageId] });
      }
      const descriptor = descriptors.get(packageId);
      if (!descriptor) {
        const parent = stack[stack.length - 1] ?? packageId;
        return new PackageDependencyMissingError({ packageId: parent, dependencyId: packageId });
      }
      visiting.add(packageId);
      for (const dependencyId of descriptor.dependencies ?? []) {
        if (!descriptors.has(dependencyId)) {
          return new PackageDependencyMissingError({ packageId, dependencyId });
        }
        const error = visit(dependencyId, [...stack, packageId]);
        if (error) return error;
      }
      visiting.delete(packageId);
      visited.add(packageId);
      if (targets.has(packageId)) out.push(packageId);
      return undefined;
    };

    for (const packageId of packageIds) {
      const error = visit(packageId, []);
      if (error) return yield* Effect.fail(error);
    }
    return out;
  });

const dependentClosure = (
  packageId: PackageId,
  descriptors: ReadonlyMap<string, PackageDescriptor>,
): readonly PackageId[] => {
  const out: PackageId[] = [];
  const seen = new Set<string>();
  const visit = (id: PackageId) => {
    for (const descriptor of descriptors.values()) {
      if (seen.has(descriptor.id) || !(descriptor.dependencies ?? []).includes(id)) continue;
      seen.add(descriptor.id);
      out.push(descriptor.id);
      visit(descriptor.id);
    }
  };
  visit(packageId);
  return out;
};

export const makePackageRuntime = Effect.gen(function* () {
  const contributionIndex = yield* makeContributionIndex;
  const stores = yield* makeStoreRuntime();
  const knownDescriptors = yield* Ref.make(new Map<string, PackageDescriptor>());
  const records = yield* Ref.make(new Map<string, ActiveRecord>());
  const snapshots = yield* Ref.make(new Map<string, PackageSnapshot>());
  const generations = yield* Ref.make(new Map<string, number>());

  const nextActivationId = (id: PackageId) =>
    Ref.modify(generations, (m) => {
      const n = (m.get(id) ?? 0) + 1;
      return [ActivationId(`${id}#${n}`), new Map(m).set(id, n)] as const;
    });

  const disposeOne = (id: PackageId) =>
    Effect.gen(function* () {
      const record = (yield* Ref.get(records)).get(id);
      if (record) {
        yield* Scope.close(record.scope, Exit.void);
        yield* Ref.update(records, (m) => {
          const n = new Map(m);
          n.delete(id);
          return n;
        });
      }
      const old = (yield* Ref.get(snapshots)).get(id);
      const descriptor = (yield* Ref.get(knownDescriptors)).get(id);
      const disposed: PackageSnapshot = old?.activationId
        ? {
            packageId: id,
            activationId: old.activationId,
            status: "disposed",
            dependencies: descriptor?.dependencies ?? old.dependencies,
          }
        : { packageId: id, status: "disposed", dependencies: descriptor?.dependencies ?? [] };
      yield* Ref.update(snapshots, (m) => new Map(m).set(id, disposed));
    });

  const activateOne = (descriptor: PackageDescriptor) =>
    Effect.gen(function* () {
      const activationId = yield* nextActivationId(descriptor.id);
      yield* Ref.update(snapshots, (m) =>
        new Map(m).set(descriptor.id, {
          packageId: descriptor.id,
          activationId,
          status: "activating",
          dependencies: descriptor.dependencies ?? [],
        }),
      );
      const scope = yield* Scope.make();
      const context = {
        packageId: descriptor.id,
        activationId,
        owner: { packageId: descriptor.id, activationId },
        stores,
      };
      const result = yield* descriptor.module
        .activate(context)
        .pipe(Effect.provideService(Scope.Scope, scope), Effect.exit);
      if (Exit.isFailure(result)) {
        yield* Scope.close(scope, result);
        const error = new PackageActivationError({
          packageId: descriptor.id,
          activationId,
          cause: result.cause,
        });
        yield* Ref.update(snapshots, (m) =>
          new Map(m).set(descriptor.id, {
            packageId: descriptor.id,
            activationId,
            status: "failed",
            dependencies: descriptor.dependencies ?? [],
            failure: error,
          }),
        );
        return yield* Effect.fail(error);
      }
      yield* Ref.update(records, (m) =>
        new Map(m).set(descriptor.id, { descriptor, scope, activationId }),
      );
      yield* Ref.update(snapshots, (m) =>
        new Map(m).set(descriptor.id, {
          packageId: descriptor.id,
          activationId,
          status: "active",
          dependencies: descriptor.dependencies ?? [],
        }),
      );
    });

  const activateSorted = (ids: readonly PackageId[]) =>
    Effect.gen(function* () {
      let firstFailure: PackageError | undefined;
      for (const id of ids) {
        const descriptors = yield* Ref.get(knownDescriptors);
        const descriptor = descriptors.get(id);
        if (!descriptor) return yield* new PackageNotFoundError({ packageId: id });
        const snapshotMap = yield* Ref.get(snapshots);
        const blockedBy = (descriptor.dependencies ?? []).filter(
          (dependencyId) => snapshotMap.get(dependencyId)?.status !== "active",
        );
        if (blockedBy.length > 0) {
          yield* Ref.update(snapshots, (m) =>
            new Map(m).set(id, {
              packageId: id,
              status: "blocked",
              dependencies: descriptor.dependencies ?? [],
              blockedBy,
            }),
          );
          continue;
        }
        const result = yield* activateOne(descriptor).pipe(Effect.exit);
        if (Exit.isFailure(result) && !firstFailure) {
          const snapshot = (yield* Ref.get(snapshots)).get(id);
          firstFailure = snapshot?.failure as PackageError | undefined;
        }
      }
      if (firstFailure) return yield* Effect.fail(firstFailure);
    });

  const affectedSubgraph = (id: PackageId) =>
    Effect.gen(function* () {
      const descriptors = yield* Ref.get(knownDescriptors);
      return [id, ...dependentClosure(id, descriptors)] as const;
    });

  const sortedKnown = (ids: readonly PackageId[]) =>
    Effect.gen(function* () {
      const descriptors = yield* Ref.get(knownDescriptors);
      return yield* sortPackageIds(ids, descriptors);
    });

  const disposeMany = (ids: readonly PackageId[]) =>
    Effect.gen(function* () {
      const sorted = yield* sortedKnown(ids);
      for (const id of [...sorted].reverse()) yield* disposeOne(id);
    });

  const runtime: PackageRuntime = {
    activateGraph: (descriptors) =>
      Effect.gen(function* () {
        yield* Ref.update(knownDescriptors, (m) => {
          const next = new Map(m);
          for (const descriptor of descriptors) next.set(descriptor.id, descriptor);
          return next;
        });
        const sorted = yield* sortedKnown(descriptors.map((descriptor) => descriptor.id));
        yield* activateSorted(sorted);
      }),
    activatePackage: (descriptor) =>
      Effect.gen(function* () {
        yield* Ref.update(knownDescriptors, (m) => new Map(m).set(descriptor.id, descriptor));
        yield* disposeOne(descriptor.id).pipe(Effect.ignore);
        const sorted = yield* sortedKnown([descriptor.id]);
        yield* activateSorted(sorted);
      }),
    reloadPackage: (id, descriptor) =>
      Effect.gen(function* () {
        const descriptors = yield* Ref.get(knownDescriptors);
        if (!descriptor && !descriptors.has(id))
          return yield* new PackageNotFoundError({ packageId: id });
        if (descriptor) {
          yield* Ref.update(knownDescriptors, (m) => new Map(m).set(id, descriptor));
        }
        const affected = yield* affectedSubgraph(id);
        const sorted = yield* sortedKnown(affected);
        yield* disposeMany(sorted);
        yield* activateSorted(sorted);
      }),
    disposePackage: (id) =>
      Effect.gen(function* () {
        const descriptors = yield* Ref.get(knownDescriptors);
        if (!descriptors.has(id) && !(yield* Ref.get(snapshots)).has(id)) {
          return yield* new PackageNotFoundError({ packageId: id });
        }
        const affected = descriptors.has(id) ? yield* affectedSubgraph(id) : [id];
        yield* disposeMany(affected);
      }),
    listPackages: Ref.get(snapshots).pipe(Effect.map((m) => Array.from(m.values()))),
    getPackage: (id) => Ref.get(snapshots).pipe(Effect.map((m) => m.get(id))),
    listDependencies: (id) =>
      Ref.get(knownDescriptors).pipe(Effect.map((m) => m.get(id)?.dependencies ?? [])),
    listDependents: (id) =>
      Ref.get(knownDescriptors).pipe(
        Effect.map((m) =>
          Array.from(m.values())
            .filter((descriptor) => (descriptor.dependencies ?? []).includes(id))
            .map((descriptor) => descriptor.id),
        ),
      ),
    listAffectedDependents: (id) =>
      Ref.get(knownDescriptors).pipe(Effect.map((m) => dependentClosure(id, m))),
    contributionIndex,
    stores,
  };
  return runtime;
});

export class PackageRuntimeService extends Context.Tag("@the-editor/kernel/PackageRuntime")<
  PackageRuntimeService,
  PackageRuntime
>() {}
export const PackageRuntimeLive = Layer.effect(PackageRuntimeService, makePackageRuntime);
