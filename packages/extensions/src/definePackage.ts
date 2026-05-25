import {
  PackageId,
  type PackageDescriptor,
  type PackageId as KernelPackageId,
} from "@the-editor/kernel";
import { Effect, type Layer, type Scope } from "effect";
import type { ExtensionContext } from "./ExtensionContext.js";
import { makeExtensionContext, PackageActivationContext } from "./ExtensionContext.js";

export interface DefinePackageOptions {
  readonly id: string;
  readonly dependencies?: readonly string[];
}

export type PackageActivation<Requirements = never> = (
  context: ExtensionContext,
) => Effect.Effect<void, unknown, Scope.Scope | PackageActivationContext | Requirements>;

export interface ExtensionPackage<Requirements = never> {
  readonly id: KernelPackageId;
  readonly dependencies?: readonly KernelPackageId[];
  readonly activate: PackageActivation<Requirements>;
}

export function definePackage(
  id: string,
  activate: PackageActivation<never>,
): ExtensionPackage<never>;
export function definePackage(
  options: DefinePackageOptions,
  activate: PackageActivation<never>,
): ExtensionPackage<never>;
export function definePackage<Requirements>(
  id: string,
  activate: PackageActivation<Requirements>,
): ExtensionPackage<Requirements>;
export function definePackage<Requirements>(
  options: DefinePackageOptions,
  activate: PackageActivation<Requirements>,
): ExtensionPackage<Requirements>;
export function definePackage<Requirements = never>(
  input: string | DefinePackageOptions,
  activate: PackageActivation<Requirements>,
): ExtensionPackage<Requirements> {
  const options = typeof input === "string" ? { id: input } : input;
  return {
    id: PackageId(options.id),
    ...(options.dependencies ? { dependencies: options.dependencies.map(PackageId) } : {}),
    activate,
  };
}

export function toPackageDescriptor(extensionPackage: ExtensionPackage<never>): PackageDescriptor;
export function toPackageDescriptor<Requirements>(
  extensionPackage: ExtensionPackage<Requirements>,
  layer: Layer.Layer<Requirements>,
): PackageDescriptor;
export function toPackageDescriptor<Requirements>(
  extensionPackage: ExtensionPackage<Requirements>,
  layer?: Layer.Layer<Requirements>,
): PackageDescriptor {
  return {
    id: extensionPackage.id,
    ...(extensionPackage.dependencies ? { dependencies: extensionPackage.dependencies } : {}),
    module: {
      activate: (context) => {
        const extensionContext = makeExtensionContext(context);
        const activation = extensionPackage
          .activate(extensionContext)
          .pipe(Effect.provideService(PackageActivationContext, extensionContext));
        // Adapter boundary: the extension package preserves its extra requirements type,
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
