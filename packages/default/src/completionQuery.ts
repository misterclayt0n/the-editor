import type { CompletionContext, EditorHost } from "@the-editor/editor";
import type { SurfaceId, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import { activeSurfacePane } from "./state.js";
import { completionContextFromSnapshot, isCompletionQueryCharacter as isSnapshotCompletionQueryCharacter, updateSurfaceSnapshot } from "./surfaceSnapshot.js";

export interface RuntimeCompletionContext extends CompletionContext {
  surfaceId: SurfaceId;
  activeCursorId: number | null;
  cursorHead: number;
  selection: SurfaceSelectionRange[];
  revision: number;
  completionRequestSequence?: number;
}

export async function readCompletionContext(
  editor: EditorHost,
  trigger: CompletionContext["trigger"],
  options: {
    allowRuntimeRead?: boolean;
    languageIdForPath: (editor: EditorHost, path: string) => string;
  },
): Promise<RuntimeCompletionContext | undefined> {
  const cached = completionContextFromSnapshot(editor, trigger, options.languageIdForPath);
  if (cached) return { editor, ...cached };
  if (!options.allowRuntimeRead) return undefined;

  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId) return undefined;
  const surface = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
  updateSurfaceSnapshot(editor, pane.surfaceId, surface);
  const refreshed = completionContextFromSnapshot(editor, trigger, options.languageIdForPath);
  return refreshed ? { editor, ...refreshed } : undefined;
}

export function isCompletionQueryCharacter(char: string): boolean {
  return isSnapshotCompletionQueryCharacter(char);
}
