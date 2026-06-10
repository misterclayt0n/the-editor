import { Data } from "effect";
import type { RegistryError } from "@the-editor/kernel";
import type { PanelId } from "@the-editor/workbench";
import type { CommandId } from "./types.js";

export class CommandNotFoundError extends Data.TaggedError("CommandNotFoundError")<{
  readonly commandId: CommandId;
}> {}

export class CommandExecutionError extends Data.TaggedError("CommandExecutionError")<{
  readonly commandId: CommandId;
  readonly cause: unknown;
}> {}

/** The command exists, but its scope does not apply at the execution target. */
export class CommandNotApplicableError extends Data.TaggedError("CommandNotApplicableError")<{
  readonly commandId: CommandId;
  readonly panelId?: PanelId;
}> {}

export class CommandTargetNotFoundError extends Data.TaggedError("CommandTargetNotFoundError")<{
  readonly commandId: CommandId;
  readonly panelId: PanelId;
}> {}

export type CommandError =
  | CommandNotFoundError
  | CommandExecutionError
  | CommandNotApplicableError
  | CommandTargetNotFoundError
  | RegistryError;
