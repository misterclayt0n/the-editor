import { contributionKind } from "./contributions.js";
import type { CommandDispatch } from "./commands.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";
import { roleId } from "./roles.js";

export const commandPaletteRoleId = roleId("commandPalette");

export type CommandPaletteIcon =
  | { kind: "lucide"; name: string }
  | { kind: "file"; path: string; entryKind?: "file" | "directory" }
  | { kind: "text"; value: string }
  | { kind: "svg"; source: string; viewBox?: string }
  | { kind: "uri"; src: string };

export interface CommandPaletteFrameTarget<TArgs = unknown> {
  title: string;
  provider: string;
  placeholder?: string;
  query?: string;
  args?: TArgs;
}

export interface CommandPaletteItem {
  id: string;
  title: string;
  detail?: string;
  group?: string;
  alias?: string;
  keybinding?: string;
  keywords?: string[];
  icon?: CommandPaletteIcon;
  command?: CommandDispatch;
  frame?: CommandPaletteFrameTarget;
  query?: string;
}

export interface CommandPaletteProviderContext<TArgs = unknown> {
  editor: EditorHost;
  query: string;
  frame?: CommandPaletteFrameTarget<TArgs>;
}

export type CommandPaletteProviderVisibility = "global" | "developer" | "hidden";

export interface CommandPaletteProvider<TArgs = unknown> {
  id: string;
  title?: string;
  priority?: number;
  visibility?: CommandPaletteProviderVisibility;
  provide(ctx: CommandPaletteProviderContext<TArgs>): CommandPaletteItem[] | Promise<CommandPaletteItem[]>;
}

interface CommandPaletteProviderEntry extends CommandPaletteProvider {
  order: number;
}

export interface CommandPaletteRoleState {
  visible: boolean;
  query: string;
  placeholder?: string;
  mode?: "commands" | "argument";
  commandTitle?: string;
  canGoBack?: boolean;
  items: CommandPaletteItem[];
}

export class CommandPaletteRegistry {
  private nextOrder = 0;
  private readonly providers = new Map<string, CommandPaletteProviderEntry>();

  constructor(private readonly editor: EditorHost) {}

  registerProvider<TArgs = unknown>(provider: CommandPaletteProvider<TArgs>, moduleId = this.editor.currentModuleId()): Disposable {
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

  async items(query: string, frame?: CommandPaletteFrameTarget): Promise<CommandPaletteItem[]> {
    const providers = [...this.providers.values()].sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.order - b.order);
    const selectedProviders = frame
      ? providers.filter((provider) => provider.id === frame.provider)
      : providers.filter((provider) => (provider.visibility ?? "global") === "global");
    const items: CommandPaletteItem[] = [];
    for (const provider of selectedProviders) {
      items.push(...await provider.provide({ editor: this.editor, query, frame }));
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

  const collapsedQuery = collapsePaletteText(normalizedQuery);
  const haystacks = [item.title, item.detail, ...(item.keywords ?? [])]
    .filter(Boolean)
    .map((value) => normalizePaletteText(value!));

  let best = 0;
  for (const haystack of haystacks) {
    best = Math.max(best, directPaletteScore(normalizedQuery, haystack));
    best = Math.max(best, fuzzyPaletteScore(normalizedQuery, haystack));

    if (!collapsedQuery) continue;
    const collapsedHaystack = collapsePaletteText(haystack);
    best = Math.max(best, directPaletteScore(collapsedQuery, collapsedHaystack));
    best = Math.max(best, fuzzyPaletteScore(collapsedQuery, collapsedHaystack));
  }

  return best;
}

function directPaletteScore(query: string, haystack: string): number {
  if (!query || !haystack) return 0;
  if (haystack === query) return 1200 - haystack.length;
  if (haystack.startsWith(query)) return 900 - haystack.length;
  if (haystack.includes(query)) return 700 - haystack.length;

  const words = query.split(/\s+/).filter(Boolean);
  if (words.length && words.every((word) => haystack.includes(word))) {
    return 500 - haystack.length;
  }

  return 0;
}

function fuzzyPaletteScore(query: string, haystack: string): number {
  if (!query || !haystack) return 0;

  let queryIndex = 0;
  let firstMatch = -1;
  let score = 0;
  let consecutive = 0;

  for (let haystackIndex = 0; haystackIndex < haystack.length && queryIndex < query.length; haystackIndex += 1) {
    if (haystack[haystackIndex] !== query[queryIndex]) {
      consecutive = 0;
      continue;
    }

    if (firstMatch < 0) firstMatch = haystackIndex;
    score += 10;
    if (isPaletteWordStart(haystack, haystackIndex)) score += 18;
    if (consecutive > 0) score += 14 + consecutive * 3;
    consecutive += 1;
    queryIndex += 1;
  }

  if (queryIndex !== query.length) return 0;
  score -= Math.max(0, haystack.length - query.length);
  if (firstMatch > 0) score -= Math.min(firstMatch, 40);
  return Math.max(score, 1);
}

function isPaletteWordStart(value: string, index: number): boolean {
  if (index <= 0) return true;
  return isPaletteSeparator(value[index - 1]);
}

function collapsePaletteText(value: string): string {
  return value.replace(/[^a-z0-9]+/g, "");
}

function isPaletteSeparator(char: string): boolean {
  return /[^a-z0-9]/.test(char);
}

function normalizePaletteText(value: string | undefined): string {
  return (value ?? "").trim().toLowerCase().replace(/\s+/g, " ");
}
