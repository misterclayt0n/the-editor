import { PackageActivationContext } from "@the-editor/extensions";
import { Context, Effect, Exit, Layer, type Scope } from "effect";
import {
  ContributionId,
  RegistryId,
  makeRegistry,
  PackageRuntimeService,
  type ContributionIndex,
  type OwnedContribution,
} from "@the-editor/kernel";
import { CommandExecutionError, CommandNotFoundError, type CommandError } from "./errors.js";
import { CommandId, type CommandContribution, type CommandDefinition } from "./types.js";

export declare namespace Commands {
  export interface Service {
    readonly register: (
      id: string,
      definition: CommandDefinition,
    ) => Effect.Effect<
      OwnedContribution<CommandContribution>,
      CommandError,
      Scope.Scope | PackageActivationContext
    >;
    readonly execute: (id: string, args?: unknown) => Effect.Effect<void, CommandError>;
    readonly get: (id: string) => Effect.Effect<OwnedContribution<CommandContribution> | undefined>;
    readonly list: Effect.Effect<readonly OwnedContribution<CommandContribution>[]>;
  }
}

export class Commands extends Context.Tag("@the-editor/commands/Commands")<
  Commands,
  Commands.Service
>() {}

export const makeCommands = (
  contributionIndex: ContributionIndex,
): Effect.Effect<Commands.Service> =>
  Effect.gen(function* () {
    const registry = yield* makeRegistry<CommandContribution>(RegistryId("the-editor.commands"), {
      contributionIndex,
    });

    const commandId = (id: string) => CommandId(id);

    return {
      register: (id, definition) =>
        Effect.gen(function* () {
          const extension = yield* PackageActivationContext;
          return yield* extension.contributions.register(registry, {
            id: ContributionId(id),
            ...definition,
          });
        }),
      execute: (id, args) =>
        Effect.gen(function* () {
          const id_ = commandId(id);
          const command = yield* registry.get(id_);
          if (!command) return yield* new CommandNotFoundError({ commandId: id_ });
          const result = yield* command.contribution.run({ id: id_, args }).pipe(Effect.exit);
          if (Exit.isFailure(result)) {
            return yield* new CommandExecutionError({ commandId: id_, cause: result.cause });
          }
        }),
      get: (id) => registry.get(commandId(id)),
      list: registry.list,
    } satisfies Commands.Service;
  });

export const CommandsLiveFromRuntime = Layer.effect(
  Commands,
  Effect.gen(function* () {
    const runtime = yield* PackageRuntimeService;
    return yield* makeCommands(runtime.contributionIndex);
  }),
);

export const CommandsLive = CommandsLiveFromRuntime;
