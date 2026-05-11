import { describe, expect, it } from "vitest";
import { createEditorHost, createRuntimeClient } from "@the-editor/editor";
import { getWorkbenchSessionState } from "@the-editor/editor/workbench";
import { defaultScratch } from "../src/scratch.js";
import { defaultWorkbench } from "../src/workbench.js";

async function createWorkbenchEditor(text = "alpha\nbeta\n") {
  const editor = createEditorHost({ runtime: createRuntimeClient() });
  await defaultScratch()(editor);
  await defaultWorkbench()(editor);
  await editor.commands.dispatch({ id: "scratch.create", args: { text } });
  return editor;
}

describe("workbench splits", () => {
  it("duplicates the active surface into a right split", async () => {
    const editor = await createWorkbenchEditor();

    await editor.commands.dispatch({ id: "workbench.vsplit" });

    const snapshot = await getWorkbenchSessionState(editor);
    expect(snapshot.editorLayout).toMatchObject({
      root: {
        kind: "split",
        axis: "row",
        children: [{ kind: "group" }, { kind: "group" }],
      },
    });
    const surfacePanes = snapshot.editorLayout?.panes.filter((pane) => pane.kind === "surface") ?? [];
    expect(surfacePanes).toHaveLength(2);
    expect(new Set(surfacePanes.map((pane) => pane.documentId))).toEqual(new Set([snapshot.activeDocumentId]));
    expect(surfacePanes[0]?.surfaceId).not.toBe(surfacePanes[1]?.surfaceId);
    expect(new Set(surfacePanes.map((pane) => pane.itemId)).size).toBe(1);
    expect(snapshot.editorLayout?.items.filter((item) => item.kind === "surface")).toHaveLength(1);
  });

  it("collapses empty split groups when their last pane closes", async () => {
    const editor = await createWorkbenchEditor();

    await editor.commands.dispatch({ id: "workbench.vsplit" });
    const split = await getWorkbenchSessionState(editor);
    const activePaneId = split.editorLayout?.activePaneId;
    expect(activePaneId).toBeTruthy();

    await editor.commands.dispatch({ id: "workbench.closePane", args: { paneId: activePaneId } });

    const collapsed = await getWorkbenchSessionState(editor);
    expect(collapsed.editorLayout?.groups).toHaveLength(1);
    expect(collapsed.editorLayout?.panes.filter((pane) => pane.kind === "surface")).toHaveLength(1);
  });

  it("exposes :hsplit and :vsplit in the command palette provider registry", async () => {
    const editor = await createWorkbenchEditor();

    const items = await editor.palette.items(":split");
    const titles = items.map((item) => item.title);
    expect(titles).toContain(":hsplit");
    expect(titles).toContain(":vsplit");
  });
});
