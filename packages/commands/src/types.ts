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

/**
 * Palette participation. Commands opt IN to the palette's primary surface;
 * availability (which commands can run right now) is structural — the
 * palette lists through `listAvailable`, so a command scoped to a surface
 * simply isn't offered when that surface isn't focused.
 */
export type CommandPaletteDefinition = {
  readonly enabled?: boolean;
  /** Exact-match shortcuts in the palette input (e.g. vim-style "w"). */
  readonly aliases?: readonly string[];
  /** Palette section heading; defaults to "Commands". */
  readonly group?: string;
};

export interface CommandDefinition {
  readonly title?: string;
  readonly description?: string;
  /** Where the command exists. Defaults to global. */
  readonly scope?: SurfaceScope;
  /** Opt into the command palette (true, or with aliases/group). */
  readonly palette?: boolean | CommandPaletteDefinition;
  readonly run: CommandHandler;
}

export interface CommandContribution extends IdentifiedContribution {
  readonly title?: string;
  readonly description?: string;
  readonly scope: SurfaceScope;
  readonly palette?: boolean | CommandPaletteDefinition;
  readonly run: CommandHandler;
}
