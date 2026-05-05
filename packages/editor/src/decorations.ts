import type { DocumentId, RenderFrame, SurfaceId } from "@the-editor/contracts/runtime";
import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export interface RangeDecoration {
  kind: "range";
  id: string;
  surfaceRow: number;
  startCol: number;
  endCol: number;
  background?: string;
  underline?: string;
}

export interface InlineTextDecoration {
  kind: "inlineText";
  id: string;
  surfaceRow: number;
  col: number;
  text: string;
  foreground?: string;
  opacity?: number;
}

export type BufferDecoration = RangeDecoration | InlineTextDecoration;

export interface DecorationProviderContext {
  editor: EditorHost;
  mode?: string;
  activeDocumentId?: DocumentId;
  activeSurfaceId?: SurfaceId;
  activePath?: string;
  frame?: RenderFrame;
}

export interface DecorationProvider {
  id: string;
  title?: string;
  priority?: number;
  provide(ctx: DecorationProviderContext): BufferDecoration[] | Promise<BufferDecoration[]>;
}

interface DecorationProviderEntry extends DecorationProvider {
  order: number;
}

export class DecorationRegistry {
  private nextOrder = 0;
  private readonly providers = new Map<string, DecorationProviderEntry>();

  constructor(private readonly editor: EditorHost) {}

  registerProvider(provider: DecorationProvider, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: DecorationProviderEntry = {
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
      kind: contributionKind("decorationProvider"),
      localId: provider.id,
      label: provider.title,
      disposable: unregister,
    });
  }

  async collect(context: Omit<DecorationProviderContext, "editor">): Promise<BufferDecoration[]> {
    const ctx: DecorationProviderContext = {
      editor: this.editor,
      ...context,
    };
    const providers = [...this.providers.values()]
      .sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.order - b.order);
    const decorations: BufferDecoration[] = [];
    for (const provider of providers) {
      decorations.push(...await provider.provide(ctx));
    }
    return decorations;
  }
}
