import type { DocumentId, RenderFrame, SurfaceId } from "@the-editor/contracts/runtime";
import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type GutterTextAlign = "left" | "right" | "center";
export type GutterSide = "left" | "right";

export interface GutterLaneSpec {
  id: string;
  side?: GutterSide;
  order?: number;
  widthCells?: number;
  widthPx?: number;
  paddingLeftPx?: number;
  paddingRightPx?: number;
}

export interface GutterLine {
  id: string;
  surfaceRow: number;
  text?: string;
  title?: string;
  align?: GutterTextAlign;
  foreground?: string;
  background?: string;
  opacity?: number;
}

export interface GutterProviderContext {
  editor: EditorHost;
  mode?: string;
  activeDocumentId?: DocumentId;
  activeSurfaceId?: SurfaceId;
  activePath?: string;
  frame?: RenderFrame;
}

export interface GutterProvider {
  id: string;
  title?: string;
  priority?: number;
  lane: string;
  mode?: "append" | "replace";
  laneSpec?: GutterLaneSpec;
  provide(ctx: GutterProviderContext): GutterLine[] | Promise<GutterLine[]>;
}

export interface GutterLaneSnapshot extends Required<Omit<GutterLaneSpec, "widthCells" | "widthPx">> {
  widthCells?: number;
  widthPx?: number;
}

export interface GutterSnapshot {
  providerId: string;
  lane: GutterLaneSnapshot;
  lines: GutterLine[];
}

interface GutterProviderEntry extends GutterProvider {
  order: number;
}

export class GutterRegistry {
  private nextOrder = 0;
  private readonly providers = new Map<string, GutterProviderEntry>();

  constructor(private readonly editor: EditorHost) {}

  registerProvider(provider: GutterProvider, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: GutterProviderEntry = {
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
      kind: contributionKind("gutterProvider"),
      localId: provider.id,
      label: provider.title,
      disposable: unregister,
    });
  }

  async collect(context: Omit<GutterProviderContext, "editor">): Promise<GutterSnapshot[]> {
    const ctx: GutterProviderContext = {
      editor: this.editor,
      ...context,
    };
    const providers = [...this.providers.values()]
      .sort((a, b) => (a.priority ?? 0) - (b.priority ?? 0) || a.order - b.order);
    const gutters: GutterSnapshot[] = [];
    for (const provider of providers) {
      const lane = normalizeLane(provider.lane, provider.laneSpec, provider.order);
      if (provider.mode === "replace") {
        for (let index = gutters.length - 1; index >= 0; index -= 1) {
          if (gutters[index]?.lane.id === lane.id) gutters.splice(index, 1);
        }
      }
      gutters.push({
        providerId: provider.id,
        lane,
        lines: await provider.provide(ctx),
      });
    }
    return gutters;
  }
}

function normalizeLane(laneId: string, spec: GutterLaneSpec | undefined, fallbackOrder: number): GutterLaneSnapshot {
  return {
    id: spec?.id ?? laneId,
    side: spec?.side ?? "left",
    order: spec?.order ?? fallbackOrder,
    widthCells: spec?.widthCells,
    widthPx: spec?.widthPx,
    paddingLeftPx: spec?.paddingLeftPx ?? 0,
    paddingRightPx: spec?.paddingRightPx ?? 0,
  };
}
