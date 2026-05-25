import type { Effect, Scope } from "effect";
import type { PackageError } from "../errors.js";
import type { PackageId } from "../ids.js";
import type { PackageContext } from "./PackageContext.js";

export interface PackageModule {
  readonly activate: (context: PackageContext) => Effect.Effect<void, PackageError, Scope.Scope>;
}

export interface PackageDescriptor {
  readonly id: PackageId;
  readonly dependencies?: readonly PackageId[];
  readonly module: PackageModule;
}
