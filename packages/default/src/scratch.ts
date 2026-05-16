import type { SurfaceId, SurfaceSelectionRange, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule, type WorkbenchPaneDescriptor } from "@the-editor/editor";
import {
  activeSurfacePane,
  getDefaultState,
  refreshActiveSurface,
  setEditorMode,
  upsertSurfacePaneState,
  type DefaultEditorState,
} from "./state.js";
import { resolveFilePathFromWorkingDirectory } from "./workingDirectory.js";

export interface CreateScratchArgs {
  text?: string;
  viewport?: Viewport;
}

export interface OpenFileArgs {
  path: string;
  viewport?: Viewport;
  line?: number;
  column?: number;
}

export function defaultScratch(): EditorModule {
  return (editor) => {
    editor.commands.register<CreateScratchArgs | undefined, DefaultEditorState>({
      id: commandId("scratch.create"),
      title: "Create Scratch Buffer",
      async run(_ctx, args) {
        const state = getDefaultState(editor);
        const activePane = activeSurfacePane(editor);
        const viewport = args?.viewport ?? activePane?.viewport ?? state.surfaceViewport ?? defaultViewport();
        const metrics = activePane?.metrics ?? state.surfaceMetrics;
        const document = await editor.runtime.documents.createScratch({
          text: args?.text ?? "",
        });
        const surface = await editor.runtime.surfaces.create({
          documentId: document.documentId,
          viewport,
        });
        if (metrics) {
          await editor.runtime.surfaces.setMetrics({
            surfaceId: surface.surfaceId,
            metrics,
          });
        }
        if (state.softWrap) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: surface.surfaceId, softWrap: true });
        }

        const pane = editor.workbench.openSurface({
          title: "Scratch",
          surfaceId: surface.surfaceId,
          documentId: document.documentId,
          path: undefined,
          closeable: true,
        });
        upsertSurfacePaneState(editor, {
          paneId: pane.id,
          surfaceId: surface.surfaceId,
          documentId: document.documentId,
          path: undefined,
          title: "Scratch",
          viewport,
          metrics,
        });
        setEditorMode(editor, "normal");
        editor.input.focusEditor({ reason: "scratch.fileSurface" });
        await refreshActiveSurface(editor, viewport);
        editor.workbench.invalidate("frame", "focus", "layout", "tabs");
        return state;
      },
    });

    editor.commands.register<OpenFileArgs, DefaultEditorState>({
      id: commandId("file.open"),
      title: "Open File",
      aliases: ["open", "o"],
      args: [{ name: "path", source: "files", required: true }],
      async run(_ctx, args) {
        const state = getDefaultState(editor);
        const activePane = activeSurfacePane(editor);
        const viewport = args.viewport ?? activePane?.viewport ?? state.surfaceViewport ?? defaultViewport();
        const metrics = activePane?.metrics ?? state.surfaceMetrics;
        const path = resolveFilePathFromWorkingDirectory(editor, args.path);
        const existingPane = findOpenFilePane(editor, path);
        if (existingPane) {
          editor.workbench.focusPane(existingPane.id);
          setEditorMode(editor, "normal");
          editor.input.focusEditor({ reason: "scratch.fileSurface" });
          const existingViewport = args.line ? viewportForLocation(viewport, args.line) : undefined;
          if (args.line) {
            const paneState = getDefaultState(editor).surfacePanes.get(existingPane.id);
            if (paneState && existingViewport) paneState.viewport = existingViewport;
            await navigateSurfaceToLocation(editor, existingPane.surfaceId, { line: args.line, column: args.column, viewport: existingViewport });
          }
          await refreshActiveSurface(editor, existingViewport);
          editor.workbench.invalidate("frame", "focus", "layout", "tabs");
          return state;
        }

        const document = await editor.runtime.documents.openFile({ path });
        const surface = await editor.runtime.surfaces.create({
          documentId: document.documentId,
          viewport,
        });
        if (metrics) {
          await editor.runtime.surfaces.setMetrics({
            surfaceId: surface.surfaceId,
            metrics,
          });
        }
        if (state.softWrap) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: surface.surfaceId, softWrap: true });
        }

        const title = fileTitle(path);
        const pane = await shouldReplaceInitialScratch(editor)
          ? editor.workbench.replaceActiveSurface({
            title,
            surfaceId: surface.surfaceId,
            documentId: document.documentId,
            path,
            closeable: true,
          })
          : editor.workbench.openSurface({
            title,
            surfaceId: surface.surfaceId,
            documentId: document.documentId,
            path,
            closeable: true,
          });
        upsertSurfacePaneState(editor, {
          paneId: pane.id,
          surfaceId: surface.surfaceId,
          documentId: document.documentId,
          path,
          title,
          viewport,
          metrics,
        });
        setEditorMode(editor, "normal");
        editor.input.focusEditor({ reason: "scratch.fileSurface" });
        const nextViewport = args.line ? viewportForLocation(viewport, args.line) : viewport;
        if (args.line) {
          const paneState = getDefaultState(editor).surfacePanes.get(pane.id);
          if (paneState) paneState.viewport = nextViewport;
          await navigateSurfaceToLocation(editor, surface.surfaceId, { line: args.line, column: args.column, viewport: nextViewport });
        }
        await refreshActiveSurface(editor, nextViewport);
        editor.workbench.invalidate("frame", "focus", "layout", "tabs");
        return state;
      },
    });

    editor.commands.register<undefined, DefaultEditorState>({
      id: commandId("view.toggleSoftWrap"),
      title: "Toggle Soft Wrap",
      async run() {
        const state = getDefaultState(editor);
        if (!state.surfacePanes.size) throw new Error("no active surface");
        state.softWrap = !state.softWrap;
        for (const pane of state.surfacePanes.values()) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: pane.surfaceId, softWrap: state.softWrap });
        }
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame");
        return state;
      },
    });

  };
}

export function defaultViewport(): Viewport {
  return {
    firstRow: 0,
    rowCount: 24,
    firstCol: 0,
    wrapWidth: 80,
  };
}

async function shouldReplaceInitialScratch(editor: EditorHost): Promise<boolean> {
  const activePane = activeSurfacePane(editor);
  if (!activePane || activePane.path || activePane.title !== "Scratch") return false;
  const layout = editor.workbench.snapshotEditorLayout();
  if (layout.panes.length !== 1) return false;
  const { text } = await editor.runtime.surfaces.readText({ surfaceId: activePane.surfaceId });
  return text.length === 0;
}

function findOpenFilePane(editor: EditorHost, path: string): Extract<WorkbenchPaneDescriptor, { kind: "surface" }> | undefined {
  const item = editor.workbench.allItems().find((candidate) => candidate.kind === "surface" && candidate.path === path);
  if (!item) return undefined;
  return editor.workbench.findPanesForItem(item.id).find((pane): pane is Extract<WorkbenchPaneDescriptor, { kind: "surface" }> => pane.kind === "surface");
}

function fileTitle(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const slash = normalized.lastIndexOf("/");
  return slash >= 0 ? normalized.slice(slash + 1) : normalized;
}

async function navigateSurfaceToLocation(
  editor: EditorHost,
  surfaceId: SurfaceId,
  location: { line: number; column?: number; viewport?: Viewport },
): Promise<void> {
  const [{ text }, selectionState] = await Promise.all([
    editor.runtime.surfaces.readText({ surfaceId }),
    editor.runtime.surfaces.readSelection({ surfaceId }),
  ]);
  const cursor = offsetForLineColumn(text, location.line, location.column);
  const cursorId = Math.max(1, (selectionState.activeCursorId ?? selectionState.selection[0]?.cursorId ?? 0));
  const selection: SurfaceSelectionRange[] = [{ cursorId, anchor: cursor, head: cursor }];
  if (location.viewport) {
    await editor.runtime.surfaces.setViewport({ surfaceId, viewport: location.viewport });
  }
  await editor.runtime.surfaces.applyEdit({ surfaceId, edit: { selection } });
  await editor.runtime.surfaces.setActiveCursor({ surfaceId, cursorId });
}

function viewportForLocation(viewport: Viewport, line: number): Viewport {
  const firstRow = Math.max(0, line - 3);
  return { ...viewport, firstRow };
}

function offsetForLineColumn(text: string, line: number, column = 1): number {
  const targetLine = Math.max(1, line);
  const targetColumn = Math.max(1, column);
  let currentLine = 1;
  let index = 0;
  while (currentLine < targetLine && index < text.length) {
    if (text.charCodeAt(index) === 10) currentLine += 1;
    index += 1;
  }
  const lineStart = index;
  let lineEnd = text.indexOf("\n", lineStart);
  if (lineEnd < 0) lineEnd = text.length;
  return Math.min(lineStart + targetColumn - 1, lineEnd);
}
