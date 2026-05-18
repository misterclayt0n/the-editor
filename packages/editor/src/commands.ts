import { contributionKind, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import type { CommandPaletteFrameTarget, CommandPaletteIcon } from "./palette.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type CommandId = string & { readonly __brand?: "CommandId" };

export interface CommandContext {
  editor: EditorHost;
}

export interface CommandArgumentProviderContext {
  editor: EditorHost;
  command: Command;
  query: string;
}

export interface CommandArgumentItem<TValue = unknown> {
  id: string;
  title: string;
  detail?: string;
  value: TValue;
  keywords?: string[];
  icon?: CommandPaletteIcon;
  frame?: CommandPaletteFrameTarget;
  query?: string;
}

export interface CommandArgumentProvider<TValue = unknown> {
  id: string;
  title?: string;
  provide(ctx: CommandArgumentProviderContext): CommandArgumentItem<TValue>[] | Promise<CommandArgumentItem<TValue>[]>;
}

export interface CommandArgumentSpec {
  name: string;
  source: string;
  required?: boolean;
}

export type CommandPaletteVisibility = "topLevel" | "command" | "hidden";

export interface Command<TArgs = unknown, TResult = unknown> {
  id: CommandId;
  title: string;
  aliases?: string[];
  args?: CommandArgumentSpec[];
  category?: string;
  hidden?: boolean;
  palette?: CommandPaletteVisibility;
  keywords?: string[];
  icon?: CommandPaletteIcon;
  run(ctx: CommandContext, args: TArgs): TResult | Promise<TResult>;
}

export interface CommandDispatch<TArgs = unknown> {
  id: CommandId;
  args?: TArgs;
}

export type CommandTokenResolution =
  | { kind: "alias"; command: Command; alias: string }
  | { kind: "title"; command: Command }
  | { kind: "id"; command: Command };

export class CommandRegistry {
  private readonly commands = new Map<CommandId, Command>();
  private readonly aliases = new Map<string, CommandId>();
  private readonly argumentProviders = new Map<string, CommandArgumentProvider>();

  constructor(private readonly editor: EditorHost) {}

  register<TArgs, TResult>(command: Command<TArgs, TResult>, moduleId = this.editor.currentModuleId()): Disposable {
    const normalizedAliases = [...new Set((command.aliases ?? []).map(normalizeCommandAlias).filter(Boolean))];
    for (const alias of normalizedAliases) {
      const owner = this.aliases.get(alias);
      if (owner && owner !== command.id) throw new Error(`command alias '${alias}' already registered by ${owner}`);
    }
    const registered = { ...command, aliases: normalizedAliases } as Command;
    const previous = this.commands.get(command.id);
    this.commands.set(command.id, registered);
    for (const alias of normalizedAliases) this.aliases.set(alias, command.id);

    const unregister = toDisposable(() => {
      const current = this.commands.get(command.id);
      if (current === registered) {
        for (const alias of normalizedAliases) {
          if (this.aliases.get(alias) === command.id) this.aliases.delete(alias);
        }
        if (previous) {
          this.commands.set(command.id, previous);
          for (const alias of previous.aliases ?? []) this.aliases.set(normalizeCommandAlias(alias), previous.id);
        } else this.commands.delete(command.id);
      }
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("command"),
      localId: command.id,
      label: command.title,
      disposable: unregister,
    });
  }

  get(id: CommandId): Command | undefined {
    return this.commands.get(id);
  }

  all(): Command[] {
    return [...this.commands.values()].sort((a, b) => a.title.localeCompare(b.title) || a.id.localeCompare(b.id));
  }

  registerArgumentProvider(provider: CommandArgumentProvider, moduleId = this.editor.currentModuleId()): Disposable {
    this.argumentProviders.set(provider.id, provider);
    const unregister = toDisposable(() => {
      if (this.argumentProviders.get(provider.id) === provider) this.argumentProviders.delete(provider.id);
    });
    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("commandArgumentProvider"),
      localId: provider.id,
      label: provider.title,
      disposable: unregister,
    });
  }

  argumentProvider(id: string): CommandArgumentProvider | undefined {
    return this.argumentProviders.get(id);
  }

  resolveToken(token: string): CommandTokenResolution | undefined {
    const normalized = normalizeCommandAlias(token);
    if (!normalized) return undefined;
    const aliasOwner = this.aliases.get(normalized);
    if (aliasOwner) {
      const command = this.commands.get(aliasOwner);
      if (command) return { kind: "alias", command, alias: normalized };
    }
    for (const command of this.commands.values()) {
      if (normalizeCommandAlias(command.title) === normalized) return { kind: "title", command };
    }
    const byId = this.commands.get(token as CommandId);
    if (byId) return { kind: "id", command: byId };
    return undefined;
  }

  aliasesFor(commandId: CommandId): string[] {
    return this.commands.get(commandId)?.aliases ?? [];
  }

  async dispatch<TResult = unknown>(dispatch: CommandDispatch): Promise<TResult> {
    return this.editor.hooks.around("commands.dispatch", dispatch, async () => {
      const command = this.commands.get(dispatch.id);
      if (!command) throw new Error(`unknown command: ${dispatch.id}`);
      await this.editor.hooks.event("commands.beforeDispatch", dispatch);
      const result = await command.run({ editor: this.editor }, dispatch.args);
      await this.editor.hooks.event("commands.afterDispatch", { dispatch, result });
      return result as TResult;
    });
  }
}

export function commandId(id: string): CommandId {
  return id as CommandId;
}

function normalizeCommandAlias(alias: string): string {
  return alias.trim().toLowerCase();
}

export type { ModuleId };
