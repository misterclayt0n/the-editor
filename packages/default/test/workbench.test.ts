import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
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

  it("opens additional files as tabs instead of replacing the active one", async () => {
    const dir = await mkdtemp(join(tmpdir(), "the-editor-tabs-"));
    const first = join(dir, "alpha.txt");
    const second = join(dir, "beta.txt");
    await writeFile(first, "alpha\n", "utf8");
    await writeFile(second, "beta\n", "utf8");

    const editor = createEditorHost({ runtime: createRuntimeClient(), cwd: dir });
    await defaultScratch()(editor);
    await defaultWorkbench()(editor);
    await editor.commands.dispatch({ id: "scratch.create", args: { text: "" } });

    await editor.commands.dispatch({ id: "file.open", args: { path: first } });
    let snapshot = await getWorkbenchSessionState(editor);
    expect(snapshot.editorLayout?.panes.filter((pane) => pane.kind === "surface")).toHaveLength(1);
    expect(snapshot.activePath).toBe(first);

    await editor.commands.dispatch({ id: "file.open", args: { path: second } });
    snapshot = await getWorkbenchSessionState(editor);
    const surfacePanes = snapshot.editorLayout?.panes.filter((pane) => pane.kind === "surface") ?? [];
    expect(surfacePanes).toHaveLength(2);
    expect(surfacePanes.map((pane) => pane.title)).toEqual(["alpha.txt", "beta.txt"]);
    expect(snapshot.activePath).toBe(second);
  });

  it("reveals an already open file instead of opening a duplicate tab", async () => {
    const dir = await mkdtemp(join(tmpdir(), "the-editor-tabs-reveal-"));
    const first = join(dir, "alpha.txt");
    const second = join(dir, "beta.txt");
    await writeFile(first, "alpha\n", "utf8");
    await writeFile(second, "beta\n", "utf8");

    const editor = createEditorHost({ runtime: createRuntimeClient(), cwd: dir });
    await defaultScratch()(editor);
    await defaultWorkbench()(editor);
    await editor.commands.dispatch({ id: "scratch.create", args: { text: "" } });

    await editor.commands.dispatch({ id: "file.open", args: { path: first } });
    const firstOpen = await getWorkbenchSessionState(editor);
    const firstPaneId = firstOpen.editorLayout?.activePaneId;

    await editor.commands.dispatch({ id: "file.open", args: { path: second } });
    await editor.commands.dispatch({ id: "file.open", args: { path: first } });

    const reopened = await getWorkbenchSessionState(editor);
    expect(reopened.editorLayout?.panes.filter((pane) => pane.kind === "surface")).toHaveLength(2);
    expect(reopened.editorLayout?.activePaneId).toBe(firstPaneId);
    expect(reopened.activePath).toBe(first);
  });

  it("moves panes between tab positions and split groups", async () => {
    const editor = await createWorkbenchEditor();
    await editor.commands.dispatch({ id: "workbench.vsplit" });
    let snapshot = await getWorkbenchSessionState(editor);
    const groups = snapshot.editorLayout?.groups ?? [];
    const leftGroup = groups[0]!;
    const rightGroup = groups[1]!;
    const rightPaneId = rightGroup.activePaneId!;

    await editor.commands.dispatch({
      id: "workbench.movePane",
      args: { paneId: rightPaneId, placement: { kind: "tab", group: leftGroup.id, index: 0 } },
    });

    snapshot = await getWorkbenchSessionState(editor);
    const remainingGroup = snapshot.editorLayout?.groups[0];
    expect(snapshot.editorLayout?.groups).toHaveLength(1);
    expect(remainingGroup?.paneIds[0]).toBe(rightPaneId);
    expect(snapshot.editorLayout?.activePaneId).toBe(rightPaneId);

    await editor.commands.dispatch({
      id: "workbench.movePane",
      args: { paneId: rightPaneId, placement: { kind: "split", direction: "right", of: remainingGroup!.id } },
    });
    snapshot = await getWorkbenchSessionState(editor);
    expect(snapshot.editorLayout?.root).toMatchObject({ kind: "split", axis: "row" });
    expect(snapshot.editorLayout?.activePaneId).toBe(rightPaneId);
  });

  it("exposes :hsplit and :vsplit in the command palette provider registry", async () => {
    const editor = await createWorkbenchEditor();

    const items = await editor.palette.items(":split");
    const titles = items.map((item) => item.title);
    expect(titles).toContain(":hsplit");
    expect(titles).toContain(":vsplit");
  });
});
