import { ContributionId, type IdentifiedContribution } from "@the-editor/kernel";
import type { SurfaceScope } from "@the-editor/workbench";
import type { Effect } from "effect";

export type CommandId = ContributionId;
export const CommandId = ContributionId;

export interface CommandInvocation {
  readonly id: CommandId;
  readonly args: unknown;
}

/**
 * Handlers may require services (`R = any` at this boundary): when executed,
 * the merged context of the targeted panel (workbench base ⊕ panel context)
 * is provided. A handler that needs TextDocument simply `yield*`s it — it can
 * only run where the focused surface supplies one.
 */
export type CommandHandler = (invocation: CommandInvocation) => Effect.Effect<void, unknown, any>;

export interface CommandDefinition {
  readonly title?: string;
  readonly description?: string;
  /** Where the command exists. Defaults to global. */
  readonly scope?: SurfaceScope;
  readonly run: CommandHandler;
}

export interface CommandContribution extends IdentifiedContribution {
  readonly title?: string;
  readonly description?: string;
  readonly scope: SurfaceScope;
  readonly run: CommandHandler;
}
