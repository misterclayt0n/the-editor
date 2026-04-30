import { commandId, type CommandDispatch, type CommandId } from "./commands.js";
import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export interface KeyInput {
  key: string;
  code?: string;
  modifiers?: readonly string[];
  text?: string;
}

export interface Keybinding {
  id: string;
  key: string;
  command: CommandId | string;
  args?: unknown;
  when?: (ctx: KeymapContext) => boolean;
  priority?: number;
}

export interface KeymapContext {
  editor: EditorHost;
}

type KeybindingEntry = Keybinding & { order: number };

export class KeymapRegistry {
  private nextOrder = 0;
  private readonly keybindings = new Map<string, KeybindingEntry>();

  constructor(private readonly editor: EditorHost) {}

  bind(binding: Keybinding, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: KeybindingEntry = { ...binding, order: this.nextOrder++ };
    this.keybindings.set(binding.id, entry);

    const unregister = toDisposable(() => {
      const current = this.keybindings.get(binding.id);
      if (current === entry) this.keybindings.delete(binding.id);
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("keybinding"),
      localId: binding.id,
      label: `${binding.key} -> ${binding.command}`,
      disposable: unregister,
    });
  }

  all(): Keybinding[] {
    return [...this.keybindings.values()]
      .sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.order - b.order)
      .map(({ order: _order, ...binding }) => binding);
  }

  async resolve(input: KeyInput): Promise<CommandDispatch | undefined> {
    const hookResult = await this.editor.hooks.first<KeyInput, CommandDispatch>("keymaps.resolve", input);
    if (hookResult) return hookResult;

    const key = normalizeKeyInput(input);
    const ctx: KeymapContext = { editor: this.editor };
    const matches = [...this.keybindings.values()]
      .filter((binding) => normalizeKey(binding.key) === key)
      .filter((binding) => binding.when?.(ctx) ?? true)
      .sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.order - b.order);

    const binding = matches[0];
    if (!binding) return undefined;
    return {
      id: typeof binding.command === "string" ? commandId(binding.command) : binding.command,
      args: binding.args,
    };
  }
}

export function normalizeKeyInput(input: KeyInput): string {
  const modifiers = [...(input.modifiers ?? [])].map((modifier) => modifier.toLowerCase()).sort();
  return normalizeKey([...modifiers, input.key].join("+"));
}

export function normalizeKey(key: string): string {
  return key
    .split("+")
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean)
    .join("+");
}
