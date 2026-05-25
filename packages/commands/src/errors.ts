import { Data } from "effect";
import type { RegistryError } from "@the-editor/kernel";
import type { CommandId } from "./types.js";

export class CommandNotFoundError extends Data.TaggedError("CommandNotFoundError")<{
  readonly commandId: CommandId;
}> {}

export class CommandExecutionError extends Data.TaggedError("CommandExecutionError")<{
  readonly commandId: CommandId;
  readonly cause: unknown;
}> {}

export type CommandError = CommandNotFoundError | CommandExecutionError | RegistryError;
