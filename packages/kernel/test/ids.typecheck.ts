import { ActivationId, PackageId, RegistryId } from "../src/index.js";

const takesPackageId = (_id: PackageId) => undefined;
const packageId = PackageId("pkg");
const activationId = ActivationId("pkg#1");
const registryId = RegistryId("registry");

takesPackageId(packageId);

// @ts-expect-error ActivationId must not be assignable to PackageId.
takesPackageId(activationId);

// @ts-expect-error RegistryId must not be assignable to PackageId.
takesPackageId(registryId);

// @ts-expect-error Raw strings must not be assignable to PackageId.
takesPackageId("pkg");
