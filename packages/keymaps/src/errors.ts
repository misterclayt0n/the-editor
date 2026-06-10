import { Data } from "effect";
import type { RegistryError } from "@the-editor/kernel";

export class InvalidKeysError extends Data.TaggedError("InvalidKeysError")<{
  readonly keys: string;
  readonly reason: string;
}> {}

export type KeymapsError = InvalidKeysError | RegistryError;
