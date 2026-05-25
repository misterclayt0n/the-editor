import { Brand } from "effect";

export type PackageId = string & Brand.Brand<"PackageId">;
export type ActivationId = string & Brand.Brand<"ActivationId">;
export type RegistryId = string & Brand.Brand<"RegistryId">;
export type ContributionId = string & Brand.Brand<"ContributionId">;
export type StoreId = string & Brand.Brand<"StoreId">;

export const PackageId = Brand.nominal<PackageId>();
export const ActivationId = Brand.nominal<ActivationId>();
export const RegistryId = Brand.nominal<RegistryId>();
export const ContributionId = Brand.nominal<ContributionId>();
export const StoreId = Brand.nominal<StoreId>();
