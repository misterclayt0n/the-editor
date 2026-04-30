import { contributionKind, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type CommandId = string & { readonly __brand?: "CommandId" };

export interface CommandContext {
  editor: EditorHost;
}

export interface Command<TArgs = unknown, TResult = unknown> {
  id: CommandId;
  title?: string;
  run(ctx: CommandContext, args: TArgs): TResult | Promise<TResult>;
}

export interface CommandDispatch<TArgs = unknown> {
  id: CommandId;
  args?: TArgs;
}

export class CommandRegistry {
  private readonly commands = new Map<CommandId, Command>();

  constructor(private readonly editor: EditorHost) {}

  register<TArgs, TResult>(command: Command<TArgs, TResult>, moduleId = this.editor.currentModuleId()): Disposable {
    const previous = this.commands.get(command.id);
    this.commands.set(command.id, command as Command);

    const unregister = toDisposable(() => {
      const current = this.commands.get(command.id);
      if (current === command) {
        if (previous) this.commands.set(command.id, previous);
        else this.commands.delete(command.id);
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
    return [...this.commands.values()].sort((a, b) => a.id.localeCompare(b.id));
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

export type { ModuleId };
