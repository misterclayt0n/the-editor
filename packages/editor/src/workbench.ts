import type { CellGridMetrics, DocumentId, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration } from "./decorations.js";
import type { DiagnosticEntry } from "./diagnostics.js";
import type { EditorHost } from "./editorHost.js";
import type { FocusTarget } from "./focus.js";
import type { RoleSnapshot } from "./roles.js";
import type { ToolSnapshot } from "./tools.js";

export interface WorkbenchKeyInput {
  key: string;
  text?: string;
}

export interface WorkbenchSurfaceConfigureRequest {
  viewport?: Viewport;
  metrics?: CellGridMetrics;
}

export type WorkbenchInvalidationKind =
  | "all"
  | "frame"
  | "roles"
  | "tools"
  | "decorations"
  | "focus"
  | "interaction";

export interface WorkbenchSessionState {
  mode?: string;
  activeDocumentId?: DocumentId;
  activeSurfaceId?: SurfaceId;
  activePath?: string;
  softWrap?: boolean;
  frame?: RenderFrame;
  roles?: Record<string, RoleSnapshot>;
  tools?: ToolSnapshot[];
  decorations?: BufferDecoration[];
  diagnostics?: DiagnosticEntry[];
  focus?: FocusTarget;
}

export class WorkbenchController {
  private readonly listeners = new Set<(kinds: readonly WorkbenchInvalidationKind[]) => void>();

  constructor(_editor: EditorHost) {}

  invalidate(...kinds: WorkbenchInvalidationKind[]): void {
    const nextKinds = [...new Set<WorkbenchInvalidationKind>(kinds.length ? kinds : ["all"])];
    for (const listener of this.listeners) listener(nextKinds);
  }

  subscribe(listener: (kinds: readonly WorkbenchInvalidationKind[]) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
}

export async function bootWorkbench(editor: EditorHost): Promise<void> {
  await editor.hooks.event("workbench.boot", undefined);
}

export async function handleWorkbenchKeyInput(editor: EditorHost, input: WorkbenchKeyInput): Promise<boolean> {
  return (await editor.hooks.first<WorkbenchKeyInput, boolean>("workbench.keyInput", input)) ?? false;
}

export async function configureWorkbenchSurface(editor: EditorHost, request: WorkbenchSurfaceConfigureRequest): Promise<boolean> {
  return (await editor.hooks.first<WorkbenchSurfaceConfigureRequest, boolean>("workbench.surfaceConfigure", request)) ?? false;
}

export async function getWorkbenchSessionState(editor: EditorHost): Promise<WorkbenchSessionState> {
  return (await editor.hooks.first<void, WorkbenchSessionState>("workbench.sessionState", undefined)) ?? {};
}
