import { PackageActivationContext } from "@the-editor/extensions";
import { Context, Effect, Exit, Layer, Option, type Scope } from "effect";
import {
  ContributionId,
  RegistryId,
  makeRegistry,
  PackageRuntimeService,
  type ContributionIndex,
  type OwnedContribution,
} from "@the-editor/kernel";
import {
  scopeApplies,
  SurfaceScope,
  Workbench,
  type Panel,
  type PanelId,
} from "@the-editor/workbench";
import {
  CommandExecutionError,
  CommandNotApplicableError,
  CommandNotFoundError,
  CommandTargetNotFoundError,
  type CommandError,
} from "./errors.js";
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
    /**
     * Execute at the current focus: the command must be applicable to the
     * focused panel (or be global), and its handler runs with that panel's
     * merged context provided.
     */
    readonly execute: (id: string, args?: unknown) => Effect.Effect<void, CommandError>;
    /** Execute on behalf of an explicit panel (palette-style origin targets). */
    readonly executeIn: (
      panelId: PanelId,
      id: string,
      args?: unknown,
    ) => Effect.Effect<void, CommandError>;
    readonly get: (id: string) => Effect.Effect<OwnedContribution<CommandContribution> | undefined>;
    readonly list: Effect.Effect<readonly OwnedContribution<CommandContribution>[]>;
    /** Commands applicable at a target panel (or at the focused panel when omitted). */
    readonly listAvailable: (
      panelId?: PanelId,
    ) => Effect.Effect<readonly OwnedContribution<CommandContribution>[]>;
  }
}

export class Commands extends Context.Tag("@the-editor/commands/Commands")<
  Commands,
  Commands.Service
>() {}

export type MakeCommandsOptions = {
  /**
   * When present, execution resolves targets and contexts through the
   * workbench. Without it only global commands run, with an empty context —
   * enough for headless/kernel-only use.
   */
  readonly workbench?: Workbench.Service;
};

export const makeCommands = (
  contributionIndex: ContributionIndex,
  options?: MakeCommandsOptions,
): Effect.Effect<Commands.Service> =>
  Effect.gen(function* () {
    const registry = yield* makeRegistry<CommandContribution>(RegistryId("the-editor.commands"), {
      contributionIndex,
    });
    const workbench = options?.workbench;

    const commandId = (id: string) => CommandId(id);

    const targetPanel = (panelId?: PanelId): Effect.Effect<Option.Option<Panel>> => {
      if (!workbench) return Effect.succeed(Option.none());
      if (panelId === undefined) return workbench.focusedPanel;
      return workbench.panel(panelId);
    };

    const contextFor = (panel: Option.Option<Panel>): Effect.Effect<Context.Context<never>> => {
      if (!workbench) return Effect.succeed(Context.empty());
      return Option.match(panel, {
        onNone: () => workbench.focusedContext, // falls back to base context
        onSome: (p) => workbench.contextAt(p.id).pipe(Effect.orDie),
      });
    };

    const runCommand = (
      command: OwnedContribution<CommandContribution>,
      panel: Option.Option<Panel>,
      args: unknown,
    ): Effect.Effect<void, CommandError> =>
      Effect.gen(function* () {
        const id = command.contributionId as CommandId;
        const focused = Option.getOrUndefined(panel);
        if (!scopeApplies(command.contribution.scope, focused)) {
          return yield* new CommandNotApplicableError({
            commandId: id,
            ...(focused ? { panelId: focused.id } : {}),
          });
        }
        const context = yield* contextFor(panel);
        const result = yield* command.contribution
          .run({ id, args })
          .pipe(Effect.provide(context as Context.Context<any>), Effect.exit) as Effect.Effect<
          Exit.Exit<void, unknown>
        >;
        if (Exit.isFailure(result)) {
          return yield* new CommandExecutionError({ commandId: id, cause: result.cause });
        }
      });

    const executeAt = (
      id: string,
      args: unknown,
      panelId?: PanelId,
    ): Effect.Effect<void, CommandError> =>
      Effect.gen(function* () {
        const id_ = commandId(id);
        const command = yield* registry.get(id_);
        if (!command) return yield* new CommandNotFoundError({ commandId: id_ });
        const panel = yield* targetPanel(panelId);
        if (panelId !== undefined && Option.isNone(panel)) {
          return yield* new CommandTargetNotFoundError({ commandId: id_, panelId });
        }
        return yield* runCommand(command, panel, args);
      });

    return {
      register: (id, definition) =>
        Effect.gen(function* () {
          const extension = yield* PackageActivationContext;
          const { scope, ...rest } = definition;
          return yield* extension.contributions.register(registry, {
            id: ContributionId(id),
            scope: scope ?? SurfaceScope.global,
            ...rest,
          });
        }),
      execute: (id, args) => executeAt(id, args),
      executeIn: (panelId, id, args) => executeAt(id, args, panelId),
      get: (id) => registry.get(commandId(id)),
      list: registry.list,
      listAvailable: (panelId) =>
        Effect.gen(function* () {
          const panel = yield* targetPanel(panelId);
          const focused = Option.getOrUndefined(panel);
          const all = yield* registry.list;
          return all.filter((command) => scopeApplies(command.contribution.scope, focused));
        }),
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
