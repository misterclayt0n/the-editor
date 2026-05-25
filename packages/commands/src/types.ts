import { ContributionId, type IdentifiedContribution } from "@the-editor/kernel";
import type { Effect } from "effect";

export type CommandId = ContributionId;
export const CommandId = ContributionId;

export interface CommandInvocation {
  readonly id: CommandId;
  readonly args: unknown;
}

export type CommandHandler = (invocation: CommandInvocation) => Effect.Effect<void, unknown>;

export interface CommandDefinition {
  readonly title?: string;
  readonly description?: string;
  readonly run: CommandHandler;
}

export interface CommandContribution extends IdentifiedContribution {
  readonly title?: string;
  readonly description?: string;
  readonly run: CommandHandler;
}
