import { contributionKind } from "./contributions.js";
import type { CommandDispatch } from "./commands.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";
import { roleId } from "./roles.js";

export const commandPaletteRoleId = roleId("commandPalette");

export interface CommandPaletteItem {
  id: string;
  title: string;
  detail?: string;
  group?: string;
  keywords?: string[];
  command?: CommandDispatch;
}

export interface CommandPaletteProviderContext {
  editor: EditorHost;
}

export interface CommandPaletteProvider {
  id: string;
  title?: string;
  priority?: number;
  provide(ctx: CommandPaletteProviderContext, query: string): CommandPaletteItem[] | Promise<CommandPaletteItem[]>;
}

interface CommandPaletteProviderEntry extends CommandPaletteProvider {
  order: number;
}

export interface CommandPaletteRoleState {
  visible: boolean;
  query: string;
  selectedIndex: number;
  placeholder?: string;
  items: CommandPaletteItem[];
}

export class CommandPaletteRegistry {
  private nextOrder = 0;
  private readonly providers = new Map<string, CommandPaletteProviderEntry>();

  constructor(private readonly editor: EditorHost) {}

  registerProvider(provider: CommandPaletteProvider, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: CommandPaletteProviderEntry = {
      ...provider,
      order: this.nextOrder++,
    };
    this.providers.set(provider.id, entry);

    const unregister = toDisposable(() => {
      const current = this.providers.get(provider.id);
      if (current === entry) this.providers.delete(provider.id);
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("commandPaletteProvider"),
      localId: provider.id,
      label: provider.title,
      disposable: unregister,
    });
  }

  async items(query: string): Promise<CommandPaletteItem[]> {
    const ctx: CommandPaletteProviderContext = { editor: this.editor };
    const providers = [...this.providers.values()].sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.order - b.order);
    const items: CommandPaletteItem[] = [];
    for (const provider of providers) {
      items.push(...await provider.provide(ctx, query));
    }
    return items;
  }
}

export function matchesCommandPaletteItem(query: string, item: Pick<CommandPaletteItem, "title" | "detail" | "keywords">): boolean {
  const normalizedQuery = normalizePaletteText(query);
  if (!normalizedQuery) return true;
  return scoreCommandPaletteItem(query, item) > 0;
}

export function scoreCommandPaletteItem(query: string, item: Pick<CommandPaletteItem, "title" | "detail" | "keywords">): number {
  const normalizedQuery = normalizePaletteText(query);
  if (!normalizedQuery) return 1;

  const haystacks = [item.title, item.detail, ...(item.keywords ?? [])]
    .filter(Boolean)
    .map((value) => normalizePaletteText(value!));

  let best = 0;
  for (const haystack of haystacks) {
    if (haystack === normalizedQuery) best = Math.max(best, 1000);
    else if (haystack.startsWith(normalizedQuery)) best = Math.max(best, 500 - haystack.length);
    else if (haystack.includes(normalizedQuery)) best = Math.max(best, 250 - haystack.length);
    else {
      const words = normalizedQuery.split(/\s+/).filter(Boolean);
      if (words.length && words.every((word) => haystack.includes(word))) {
        best = Math.max(best, 100 - haystack.length);
      }
    }
  }

  return best;
}

function normalizePaletteText(value: string | undefined): string {
  return (value ?? "").trim().toLowerCase();
}
