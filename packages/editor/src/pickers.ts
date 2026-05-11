import type { RenderFrame, Viewport } from "@the-editor/contracts/runtime";
import { contributionKind, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";
import { roleId } from "./roles.js";

export const pickerRoleId = roleId("picker");

export interface PickerItem {
  id: string;
  title: string;
  subtitle?: string;
  detail?: string;
  keywords?: string[];
  payload?: unknown;
}

export interface PickerStatusLine {
  primary?: string;
  secondary?: string;
}

export interface PickerPreviewRow {
  text: string;
  lineNumber?: number | null;
}

export interface PickerPreviewSyntaxSpan {
  row: number;
  startCol: number;
  endCol: number;
  kind: number;
  scope?: string;
}

export type PickerPreview =
  | {
      kind: "text";
      title?: string;
      detail?: string;
      text: string;
      startLine?: number;
      endLine?: number;
      truncated?: boolean;
      binary?: boolean;
      rows?: PickerPreviewRow[];
      syntax?: PickerPreviewSyntaxSpan[];
    }
  | {
      kind: "surface";
      title?: string;
      detail?: string;
      path: string;
      frame?: RenderFrame;
      viewport?: Viewport;
      lineCount?: number;
      syntaxLegend?: Record<number, string>;
    }
  | {
      kind: "empty";
      title?: string;
      detail?: string;
    };

export interface PickerPage {
  items: PickerItem[];
  totalCount?: number;
  status?: PickerStatusLine;
  emptyText?: string;
  refreshAfterMs?: number;
}

export interface PickerSession {
  search(query: string, page: { offset: number; limit: number }): PickerPage | Promise<PickerPage>;
  preview?(item: PickerItem): PickerPreview | null | Promise<PickerPreview | null>;
  accept(item: PickerItem): void | Promise<void>;
  dispose?(): void;
}

export interface PickerProviderContext {
  editor: EditorHost;
}

export interface PickerProvider {
  id: string;
  title?: string;
  placeholder?: string;
  createSession(ctx: PickerProviderContext, args?: unknown): PickerSession | Promise<PickerSession>;
}

export interface PickerProviderEntry extends PickerProvider {
  moduleId: ModuleId;
  order: number;
}

export class PickerRegistry {
  private nextOrder = 0;
  private readonly providers = new Map<string, PickerProviderEntry>();

  constructor(private readonly editor: EditorHost) {}

  registerProvider(provider: PickerProvider, moduleId = this.editor.currentModuleId()): Disposable {
    const entry: PickerProviderEntry = {
      ...provider,
      moduleId,
      order: this.nextOrder++,
    };
    this.providers.set(provider.id, entry);

    const unregister = toDisposable(() => {
      const current = this.providers.get(provider.id);
      if (current === entry) this.providers.delete(provider.id);
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("pickerProvider"),
      localId: provider.id,
      label: provider.title,
      disposable: unregister,
    });
  }

  entry(id: string): PickerProviderEntry | undefined {
    return this.providers.get(id);
  }

  all(): PickerProviderEntry[] {
    return [...this.providers.values()].sort((a, b) => a.order - b.order || a.id.localeCompare(b.id));
  }
}
