import { contributionKind, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";
import { roleId } from "./roles.js";

export const completionMenuRoleId = roleId("completionMenu");

export type CompletionInsertTextFormat = "plain" | "snippet";

export interface CompletionTextEdit {
  start: number;
  end: number;
  text: string;
}

export interface CompletionItem {
  id: string;
  label: string;
  sourceId?: string;
  filterText?: string;
  sortText?: string;
  detail?: string;
  documentation?: string;
  kind?: string;
  badge?: string;
  labelDetail?: string;
  labelDescription?: string;
  replace?: { start: number; end: number };
  insertText?: string;
  insertTextFormat?: CompletionInsertTextFormat;
  additionalTextEdits?: CompletionTextEdit[];
  commitCharacters?: string[];
  deprecated?: boolean;
  payload?: unknown;
}

export interface CompletionTrigger {
  kind: "manual" | "input" | "delete" | "triggerCharacter" | "retrigger" | "incomplete" | "accept";
  character?: string;
}

export interface CompletionContext {
  editor: EditorHost;
  surfaceId: number;
  path?: string;
  languageId?: string;
  trigger: CompletionTrigger;
  query: string;
  cursorOffset: number;
  replaceStart: number;
  replaceEnd: number;
  linePrefix: string;
  lineSuffix: string;
  text: string;
}

export interface CompletionResult {
  items: CompletionItem[];
  isIncomplete?: boolean;
  display?: CompletionDisplayOptions;
}

export interface CompletionDisplayOptions {
  dynamicWidth?: boolean;
  showDocumentation?: boolean;
}

export interface CompletionSource {
  id: string;
  title?: string;
  priority?: number;
  shouldTrigger?(ctx: CompletionContext): boolean;
  provide(ctx: CompletionContext): CompletionResult | null | undefined | Promise<CompletionResult | null | undefined>;
  resolve?(item: CompletionItem, ctx: CompletionContext): void | Partial<CompletionItem> | CompletionItem | Promise<void | Partial<CompletionItem> | CompletionItem>;
  accept?(item: CompletionItem, ctx: CompletionContext): void | Promise<void>;
}

export interface CompletionPresentationContext {
  editor: EditorHost;
  source: CompletionSourceEntry;
  item: CompletionItem;
  path?: string;
  languageId?: string;
  query: string;
}

export interface CompletionPresentation {
  id: string;
  priority?: number;
  when?(ctx: CompletionPresentationContext): boolean;
  present(item: CompletionItem, ctx: CompletionPresentationContext): void | Partial<CompletionItem>;
}

export interface CompletionSourceEntry extends CompletionSource {
  moduleId: ModuleId;
  order: number;
}

export interface CompletionPresentationEntry extends CompletionPresentation {
  moduleId: ModuleId;
  order: number;
}

export class CompletionRegistry {
  private nextSourceOrder = 0;
  private nextPresentationOrder = 0;
  private readonly sourcesById = new Map<string, CompletionSourceEntry>();
  private readonly presentationsById = new Map<string, CompletionPresentationEntry>();

  constructor(private readonly editor: EditorHost) {}

  registerSource(source: CompletionSource, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: CompletionSourceEntry = {
      ...source,
      moduleId,
      order: this.nextSourceOrder++,
    };
    this.sourcesById.set(source.id, entry);

    const unregister = toDisposable(() => {
      const current = this.sourcesById.get(source.id);
      if (current === entry) this.sourcesById.delete(source.id);
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("completionSource"),
      localId: source.id,
      label: source.title,
      disposable: unregister,
    });
  }

  registerPresentation(presentation: CompletionPresentation, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: CompletionPresentationEntry = {
      ...presentation,
      moduleId,
      order: this.nextPresentationOrder++,
    };
    this.presentationsById.set(presentation.id, entry);

    const unregister = toDisposable(() => {
      const current = this.presentationsById.get(presentation.id);
      if (current === entry) this.presentationsById.delete(presentation.id);
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("completionPresentation"),
      localId: presentation.id,
      label: presentation.id,
      disposable: unregister,
    });
  }

  source(id: string): CompletionSourceEntry | undefined {
    return this.sourcesById.get(id);
  }

  sources(): CompletionSourceEntry[] {
    return [...this.sourcesById.values()].sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.order - b.order || a.id.localeCompare(b.id));
  }

  presentations(): CompletionPresentationEntry[] {
    return [...this.presentationsById.values()].sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.order - b.order || a.id.localeCompare(b.id));
  }

  present(item: CompletionItem, source: CompletionSourceEntry, context: Omit<CompletionPresentationContext, "item" | "source">): CompletionItem {
    let current = item;
    for (const presentation of this.presentations()) {
      const input: CompletionPresentationContext = {
        ...context,
        source,
        item: current,
      };
      if (!(presentation.when?.(input) ?? true)) continue;
      const patch = presentation.present(current, input);
      if (!patch) continue;
      current = { ...current, ...patch };
    }
    return current;
  }
}
