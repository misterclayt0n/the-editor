import { commandId, type CommandDispatch, type CommandId } from "./commands.js";
import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export interface KeyInput {
  key: string;
  code?: string;
  text?: string;
}

export interface KeymapContext {
  editor: EditorHost;
}

export interface KeymapNodeSpec {
  name: string;
  sticky?: boolean;
  bindings: Record<string, KeymapBindingValue>;
}

export type KeymapBindingValue =
  | string
  | CommandDispatch
  | readonly (string | CommandDispatch)[]
  | KeymapNodeSpec;

export interface KeymapRegistration {
  id: string;
  scope: string;
  tree: KeymapNodeSpec;
  priority?: number;
  when?: (ctx: KeymapContext) => boolean;
}

export interface KeymapPendingState {
  scope: string;
  keys: readonly string[];
  name: string;
  sticky: boolean;
  nextKeys: readonly string[];
}

export type KeymapPressResult =
  | { kind: "matched"; dispatches: CommandDispatch[] }
  | { kind: "pending"; pending: KeymapPendingState }
  | { kind: "not-found" }
  | { kind: "cancelled"; keys: readonly string[] };

interface KeymapRegistrationEntry extends KeymapRegistration {
  order: number;
}

interface KeyTrieNode {
  name: string;
  sticky: boolean;
  map: Map<string, KeyTrie>;
}

type KeyTrie =
  | { kind: "node"; node: KeyTrieNode }
  | { kind: "dispatches"; dispatches: CommandDispatch[] };

const modifierOrder = ["C", "A", "M", "S"] as const;
const specialKeyNames = new Map<string, string>([
  ["escape", "esc"],
  ["esc", "esc"],
  ["enter", "ret"],
  ["return", "ret"],
  ["ret", "ret"],
  ["backspace", "backspace"],
  ["delete", "del"],
  ["del", "del"],
  ["arrowleft", "left"],
  ["left", "left"],
  ["arrowright", "right"],
  ["right", "right"],
  ["arrowup", "up"],
  ["up", "up"],
  ["arrowdown", "down"],
  ["down", "down"],
  ["pageup", "pageup"],
  ["pagedown", "pagedown"],
  ["home", "home"],
  ["end", "end"],
  ["tab", "tab"],
  ["space", "space"],
  [" ", "space"],
  ["minus", "minus"],
  ["underscore", "_"],
]);

export class KeymapRegistry {
  private nextOrder = 0;
  private readonly registrations = new Map<string, KeymapRegistrationEntry>();
  private readonly roots = new Map<string, KeyTrieNode>();
  private pendingKeys: string[] = [];
  private stickyNode: KeyTrieNode | undefined;
  private activeScope: string | undefined;

  constructor(private readonly editor: EditorHost) {}

  register(registration: KeymapRegistration, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: KeymapRegistrationEntry = { ...registration, order: this.nextOrder++ };
    this.registrations.set(registration.id, entry);
    this.rebuildRoots();

    const unregister = toDisposable(() => {
      const current = this.registrations.get(registration.id);
      if (current === entry) {
        this.registrations.delete(registration.id);
        this.rebuildRoots();
      }
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("keybinding"),
      localId: registration.id,
      label: `${registration.scope} keymap`,
      disposable: unregister,
    });
  }

  reset(): void {
    this.pendingKeys = [];
    this.stickyNode = undefined;
    this.activeScope = undefined;
  }

  pending(): KeymapPendingState | undefined {
    if (!this.pendingKeys.length) return undefined;
    const node = this.stickyNode ?? (this.activeScope ? this.roots.get(this.activeScope) : undefined);
    if (!node || !this.activeScope) return undefined;
    const current = searchNode(node, this.pendingKeys);
    if (!current || current.kind !== "node") return undefined;
    return pendingState(this.activeScope, this.pendingKeys, current.node);
  }

  async press(scope: string, input: KeyInput): Promise<KeymapPressResult> {
    const hookResult = await this.editor.hooks.first<{ scope: string; input: KeyInput }, KeymapPressResult>(
      "keymaps.press",
      { scope, input },
    );
    if (hookResult) return hookResult;

    if (scope !== this.activeScope) {
      this.pendingKeys = [];
      this.stickyNode = undefined;
      this.activeScope = scope;
    }

    const key = normalizeKeyInput(input);
    if (!key) return { kind: "not-found" };

    if (key === "esc") {
      if (this.pendingKeys.length) {
        const cancelled = [...this.pendingKeys];
        this.pendingKeys = [];
        this.stickyNode = undefined;
        return { kind: "cancelled", keys: cancelled };
      }
      this.stickyNode = undefined;
    }

    const root = this.stickyNode ?? this.roots.get(scope);
    if (!root) return { kind: "not-found" };

    const first = this.pendingKeys[0] ?? key;
    const firstMatch = root.map.get(first);
    if (!firstMatch) return { kind: "not-found" };
    if (firstMatch.kind === "dispatches") return finishMatched(this, firstMatch.dispatches);

    this.pendingKeys.push(key);
    const match = searchNode(firstMatch.node, this.pendingKeys.slice(1));
    if (!match) {
      const cancelled = [...this.pendingKeys];
      this.pendingKeys = [];
      return { kind: "cancelled", keys: cancelled };
    }

    if (match.kind === "dispatches") return finishMatched(this, match.dispatches);

    if (match.node.sticky) {
      this.pendingKeys = [];
      this.stickyNode = match.node;
    }

    return {
      kind: "pending",
      pending: pendingState(scope, this.pendingKeys, match.node),
    };
  }

  async pressScopes(scopes: readonly string[], input: KeyInput): Promise<KeymapPressResult> {
    const filtered = scopes.filter((scope) => scope.trim().length > 0);
    if (!filtered.length) return { kind: "not-found" };

    if (this.pendingKeys.length && this.activeScope && filtered.includes(this.activeScope)) {
      return this.press(this.activeScope, input);
    }

    if (this.pendingKeys.length && this.activeScope && !filtered.includes(this.activeScope)) {
      this.reset();
    }

    for (const scope of filtered) {
      const result = await this.press(scope, input);
      if (result.kind === "not-found") continue;
      return result;
    }

    return { kind: "not-found" };
  }

  private rebuildRoots(): void {
    this.roots.clear();
    const ctx: KeymapContext = { editor: this.editor };
    const entries = [...this.registrations.values()]
      .filter((entry) => entry.when?.(ctx) ?? true)
      .sort((a, b) => (a.priority ?? 0) - (b.priority ?? 0) || a.order - b.order);

    for (const entry of entries) {
      const existing = this.roots.get(entry.scope);
      const tree = materializeNode(entry.tree);
      if (existing) mergeNode(existing, tree);
      else this.roots.set(entry.scope, tree);
    }
  }
}

export function keymap(name: string, bindings: Record<string, KeymapBindingValue>, options?: { sticky?: boolean }): KeymapNodeSpec {
  return {
    name,
    sticky: options?.sticky ?? false,
    bindings,
  };
}

export function dispatch(id: string | CommandId, args?: unknown): CommandDispatch {
  return {
    id: typeof id === "string" ? commandId(id) : id,
    args,
  };
}

export function normalizeKeyInput(input: KeyInput): string {
  return normalizeKeyDescriptor(input.key);
}

export function normalizeKeyDescriptor(key: string): string {
  const trimmed = key.trim();
  if (!trimmed) return "";
  if (trimmed.includes("+")) return normalizePlusKeyDescriptor(trimmed);

  const rawParts = trimmed.split("-").map((part) => part.trim()).filter(Boolean);
  const modifiers: string[] = [];
  while (rawParts.length > 1 && isModifierToken(rawParts[0]!)) {
    modifiers.push(normalizeModifier(rawParts.shift()!));
  }

  const normalizedKey = normalizeKeyAtom(rawParts.join("-"));
  return formatKey(modifiers, normalizedKey);
}

function normalizePlusKeyDescriptor(key: string): string {
  const parts = key.split("+").map((part) => part.trim()).filter(Boolean);
  const rawKey = parts.pop() ?? "";
  const modifiers = parts.map(normalizeModifier);
  return formatKey(modifiers, normalizeKeyAtom(rawKey));
}

function formatKey(modifiers: string[], key: string): string {
  if (!key) return "";
  const deduped = [...new Set(modifiers)];
  if (key.length === 1) {
    const shiftIndex = deduped.indexOf("S");
    if (shiftIndex >= 0) deduped.splice(shiftIndex, 1);
  }
  const ordered = modifierOrder.filter((modifier) => deduped.includes(modifier));
  return ordered.length ? `${ordered.join("-")}-${key}` : key;
}

function normalizeModifier(modifier: string): string {
  switch (modifier.toLowerCase()) {
    case "c":
    case "ctrl":
    case "control":
      return "C";
    case "a":
    case "alt":
    case "option":
      return "A";
    case "m":
    case "meta":
    case "cmd":
    case "command":
      return "M";
    case "s":
    case "shift":
      return "S";
    default:
      return modifier.toUpperCase();
  }
}

function isModifierToken(value: string): boolean {
  return ["c", "ctrl", "control", "a", "alt", "option", "m", "meta", "cmd", "command", "s", "shift"]
    .includes(value.toLowerCase());
}

function normalizeKeyAtom(key: string): string {
  const trimmed = key.trim();
  if (!trimmed) return "";
  if (trimmed.length === 1) {
    return trimmed === " " ? "space" : trimmed;
  }
  return specialKeyNames.get(trimmed.toLowerCase()) ?? trimmed.toLowerCase();
}

function splitBindingKeys(rawKeys: string): string[] {
  return rawKeys.includes(" | ") ? rawKeys.split(/\s+\|\s+/) : [rawKeys];
}

function materializeNode(spec: KeymapNodeSpec): KeyTrieNode {
  const node: KeyTrieNode = {
    name: spec.name,
    sticky: spec.sticky ?? false,
    map: new Map(),
  };

  for (const [rawKeys, value] of Object.entries(spec.bindings)) {
    const keys = splitBindingKeys(rawKeys).map((key) => normalizeKeyDescriptor(key)).filter(Boolean);
    const trie = materializeValue(value);
    for (const key of keys) node.map.set(key, cloneTrie(trie));
  }

  return node;
}

function materializeValue(value: KeymapBindingValue): KeyTrie {
  if (isKeymapNodeSpec(value)) return { kind: "node", node: materializeNode(value) };
  const values = Array.isArray(value) ? value : [value];
  return {
    kind: "dispatches",
    dispatches: values.map((entry) =>
      typeof entry === "string" ? dispatch(entry) : { id: typeof entry.id === "string" ? commandId(entry.id) : entry.id, args: entry.args },
    ),
  };
}

function isKeymapNodeSpec(value: KeymapBindingValue): value is KeymapNodeSpec {
  return typeof value === "object" && value !== null && !Array.isArray(value) && "bindings" in value;
}

function cloneTrie(trie: KeyTrie): KeyTrie {
  if (trie.kind === "dispatches") return { kind: "dispatches", dispatches: trie.dispatches.map((dispatch) => ({ ...dispatch })) };
  return { kind: "node", node: cloneNode(trie.node) };
}

function cloneNode(node: KeyTrieNode): KeyTrieNode {
  return {
    name: node.name,
    sticky: node.sticky,
    map: new Map([...node.map.entries()].map(([key, trie]) => [key, cloneTrie(trie)])),
  };
}

function mergeNode(into: KeyTrieNode, other: KeyTrieNode): void {
  for (const [key, trie] of other.map.entries()) {
    const current = into.map.get(key);
    if (current?.kind === "node" && trie.kind === "node") mergeNode(current.node, trie.node);
    else into.map.set(key, cloneTrie(trie));
  }
}

function searchNode(node: KeyTrieNode, keys: readonly string[]): KeyTrie | undefined {
  let current: KeyTrie = { kind: "node", node };
  for (const key of keys) {
    if (current.kind !== "node") return undefined;
    current = current.node.map.get(key)!;
    if (!current) return undefined;
  }
  return current;
}

function pendingState(scope: string, keys: readonly string[], node: KeyTrieNode): KeymapPendingState {
  return {
    scope,
    keys: [...keys],
    name: node.name,
    sticky: node.sticky,
    nextKeys: [...node.map.keys()],
  };
}

function finishMatched(registry: KeymapRegistry, dispatches: CommandDispatch[]): KeymapPressResult {
  registry.reset();
  return {
    kind: "matched",
    dispatches: dispatches.map((dispatch) => ({ ...dispatch })),
  };
}
