import {
  PackageId,
  type PackageDescriptor,
  type PackageId as KernelPackageId,
} from "@the-editor/kernel";
import { Effect, type Layer, type Scope } from "effect";
import type { ExtensionContext } from "./ExtensionContext.js";
import { makeExtensionContext } from "./ExtensionContext.js";

export interface DefinePackageOptions {
  readonly id: string;
  readonly dependencies?: readonly string[];
}

export type PackageActivation<R = Scope.Scope> = (
  context: ExtensionContext,
) => Effect.Effect<void, unknown, R>;

export interface ExtensionPackage<R = Scope.Scope> {
  readonly id: KernelPackageId;
  readonly dependencies?: readonly KernelPackageId[];
  readonly activate: PackageActivation<R>;
}

export function definePackage<R = Scope.Scope>(
  id: string,
  activate: PackageActivation<R>,
): ExtensionPackage<R>;
export function definePackage<R = Scope.Scope>(
  options: DefinePackageOptions,
  activate: PackageActivation<R>,
): ExtensionPackage<R>;
export function definePackage<R = Scope.Scope>(
  input: string | DefinePackageOptions,
  activate: PackageActivation<R>,
): ExtensionPackage<R> {
  const options = typeof input === "string" ? { id: input } : input;
  return {
    id: PackageId(options.id),
    ...(options.dependencies ? { dependencies: options.dependencies.map(PackageId) } : {}),
    activate,
  };
}

export function toPackageDescriptor<R extends Scope.Scope>(
  extensionPackage: ExtensionPackage<R>,
): PackageDescriptor;
export function toPackageDescriptor<R>(
  extensionPackage: ExtensionPackage<Scope.Scope | R>,
  layer: Layer.Layer<R>,
): PackageDescriptor;
export function toPackageDescriptor<R>(
  extensionPackage: ExtensionPackage<Scope.Scope | R>,
  layer?: Layer.Layer<R>,
): PackageDescriptor {
  return {
    id: extensionPackage.id,
    ...(extensionPackage.dependencies ? { dependencies: extensionPackage.dependencies } : {}),
    module: {
      activate: (context) => {
        const activation = extensionPackage.activate(makeExtensionContext(context));
        // Adapter boundary: the extension package preserves its required environment type,
        // while the kernel descriptor can only expose Scope. Host composition provides the
        // extra requirements here before handing the descriptor to the kernel.
        return (layer ? Effect.provide(activation, layer) : activation) as Effect.Effect<
          void,
          unknown,
          Scope.Scope
        >;
      },
    },
  };
}
