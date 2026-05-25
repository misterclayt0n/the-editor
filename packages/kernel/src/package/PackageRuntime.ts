import { Context, Effect, Exit, Layer, Ref, Scope } from "effect";
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

export type PackageStatus = "inactive" | "activating" | "active" | "failed" | "disposed";

export interface PackageSnapshot {
  readonly packageId: PackageId;
  readonly activationId?: ActivationId;
  readonly status: PackageStatus;
  readonly dependencies: readonly PackageId[];
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
  readonly stores: StoreRuntime;
}

const sortGraph = (
  descriptors: readonly PackageDescriptor[],
): Effect.Effect<readonly PackageDescriptor[], PackageDependencyCycleError | PackageDependencyMissingError> =>
  Effect.gen(function* () {
    const byId = new Map(descriptors.map((d) => [d.id, d] as const));
    const visiting = new Set<string>();
    const visited = new Set<string>();
    const out: PackageDescriptor[] = [];

    const visit = (
      descriptor: PackageDescriptor,
      stack: readonly PackageId[],
    ): PackageDependencyCycleError | PackageDependencyMissingError | undefined => {
      if (visited.has(descriptor.id)) return undefined;
      if (visiting.has(descriptor.id)) {
        return new PackageDependencyCycleError({ packageIds: [...stack, descriptor.id] });
      }
      visiting.add(descriptor.id);
      for (const dependencyId of descriptor.dependencies ?? []) {
        const dependency = byId.get(dependencyId);
        if (!dependency) {
          return new PackageDependencyMissingError({ packageId: descriptor.id, dependencyId });
        }
        const error = visit(dependency, [...stack, descriptor.id]);
        if (error) return error;
      }
      visiting.delete(descriptor.id);
      visited.add(descriptor.id);
      out.push(descriptor);
      return undefined;
    };

    for (const descriptor of descriptors) {
      const error = visit(descriptor, []);
      if (error) return yield* Effect.fail(error);
    }
    return out;
  });

export const makePackageRuntime = Effect.gen(function* () {
  const stores = yield* makeStoreRuntime();
  const records = yield* Ref.make(new Map<string, ActiveRecord>());
  const snapshots = yield* Ref.make(new Map<string, PackageSnapshot>());
  const generations = yield* Ref.make(new Map<string, number>());

  const nextActivationId = (id: PackageId) =>
    Ref.modify(generations, (m) => {
      const n = (m.get(id) ?? 0) + 1;
      return [ActivationId(`${id}#${n}`), new Map(m).set(id, n)] as const;
    });

  const dispose = (id: PackageId) =>
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
      const disposed: PackageSnapshot = old?.activationId
        ? {
            packageId: id,
            activationId: old.activationId,
            status: "disposed",
            dependencies: old.dependencies,
          }
        : { packageId: id, status: "disposed", dependencies: [] };
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

  const runtime: PackageRuntime = {
    activateGraph: (descriptors) =>
      Effect.gen(function* () {
        for (const d of yield* sortGraph(descriptors)) yield* runtime.activatePackage(d);
      }),
    activatePackage: (descriptor) =>
      Effect.gen(function* () {
        yield* dispose(descriptor.id).pipe(Effect.ignore);
        yield* activateOne(descriptor);
      }),
    reloadPackage: (id, descriptor) =>
      Effect.gen(function* () {
        const old = (yield* Ref.get(records)).get(id);
        if (!descriptor && !old) return yield* new PackageNotFoundError({ packageId: id });
        yield* dispose(id);
        yield* activateOne(descriptor ?? old!.descriptor);
      }),
    disposePackage: dispose,
    listPackages: Ref.get(snapshots).pipe(Effect.map((m) => Array.from(m.values()))),
    getPackage: (id) => Ref.get(snapshots).pipe(Effect.map((m) => m.get(id))),
    stores,
  };
  return runtime;
});

export class PackageRuntimeService extends Context.Tag("@the-editor/kernel/PackageRuntime")<
  PackageRuntimeService,
  PackageRuntime
>() {}
export const PackageRuntimeLive = Layer.effect(PackageRuntimeService, makePackageRuntime);
