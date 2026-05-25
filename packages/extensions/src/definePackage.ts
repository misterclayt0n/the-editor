import { PackageId, type PackageDescriptor } from "@the-editor/kernel";
import type { Effect, Scope } from "effect";
import type { ExtensionContext } from "./ExtensionContext.js";
import { makeExtensionContext } from "./ExtensionContext.js";

export interface DefinePackageOptions {
  readonly id: string;
  readonly dependencies?: readonly string[];
}

export type PackageActivation = (
  context: ExtensionContext,
) => Effect.Effect<void, unknown, Scope.Scope>;

export function definePackage(id: string, activate: PackageActivation): PackageDescriptor;
export function definePackage(
  options: DefinePackageOptions,
  activate: PackageActivation,
): PackageDescriptor;
export function definePackage(
  input: string | DefinePackageOptions,
  activate: PackageActivation,
): PackageDescriptor {
  const options = typeof input === "string" ? { id: input } : input;
  return {
    id: PackageId(options.id),
    ...(options.dependencies ? { dependencies: options.dependencies.map(PackageId) } : {}),
    module: {
      activate: (context) => activate(makeExtensionContext(context)),
    },
  };
}
