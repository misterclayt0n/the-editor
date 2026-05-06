import { describe, expect, it } from "vitest";
import { commandId, createEditorHost, createRuntimeClient } from "../../editor/src/index.js";
import { handleWorkbenchKeyInput } from "../../editor/src/workbench.js";
import { defaultEditing } from "../src/editing.js";
import { defaultScratch } from "../src/scratch.js";
import { getDefaultState } from "../src/state.js";
import { defaultWorkbench } from "../src/workbench.js";

async function createTestEditor(text: string) {
  const editor = createEditorHost({ runtime: createRuntimeClient() });
  await defaultScratch()(editor);
  await defaultEditing()(editor);
  await defaultWorkbench()(editor);
  await editor.commands.dispatch({
    id: commandId("scratch.create"),
    args: { text },
  });
  return editor;
}

describe("default editing commands", () => {
  it("replaces the grapheme under the cursor from pending replace input", async () => {
    const editor = await createTestEditor("hello");

    await editor.commands.dispatch({ id: commandId("replace") });
    await handleWorkbenchKeyInput(editor, { key: "x", text: "x" });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.text).toBe("xello");
    expect(state.lineEnding).toBe("\n");
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 0 }]);
    expect(getDefaultState(editor).mode).toBe("normal");
  });

  it("replaces with the first yanked register value", async () => {
    const editor = await createTestEditor("hello");
    await editor.runtime.registers.write({ name: '"', values: ["ZZ"] });

    await editor.commands.dispatch({ id: commandId("replace_with_yanked") });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.text).toBe("ZZello");
    expect(state.lineEnding).toBe("\n");
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 0 }]);
    expect(getDefaultState(editor).mode).toBe("normal");
  });

  it("repeats the last recorded motion command", async () => {
    const editor = await createTestEditor("hello");

    await editor.commands.dispatch({ id: commandId("move_char_right") });
    await editor.commands.dispatch({ id: commandId("repeat_last_motion") });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.text).toBe("hello");
    expect(state.lineEnding).toBe("\n");
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 2, head: 2 }]);
  });

  it("handles pending find-next-char input", async () => {
    const editor = await createTestEditor("hello world");

    await editor.commands.dispatch({ id: commandId("find_next_char") });
    await handleWorkbenchKeyInput(editor, { key: "o", text: "o" });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.text).toBe("hello world");
    expect(state.lineEnding).toBe("\n");
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 5 }]);
  });

  it("repeats till-char motion from the new cursor position", async () => {
    const editor = await createTestEditor("aaaaaaaaaaakaaaaaaaaaaaaak");

    await editor.commands.dispatch({ id: commandId("find_till_char") });
    await handleWorkbenchKeyInput(editor, { key: "k", text: "k" });
    await editor.commands.dispatch({ id: commandId("repeat_last_motion") });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 10, head: 25 }]);
  });

  it("repeats backward char motions from the new cursor position", async () => {
    const editor = await createTestEditor("kaaaaaaaaaaaakaaaaaaaaaaaaax");
    await editor.commands.dispatch({ id: commandId("goto_line_end") });
    await editor.commands.dispatch({ id: commandId("find_prev_char") });
    await handleWorkbenchKeyInput(editor, { key: "k", text: "k" });
    await editor.commands.dispatch({ id: commandId("repeat_last_motion") });

    let state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 14, head: 0 }]);

    const editorTill = await createTestEditor("kaaaaaaaaaaaakaaaaaaaaaaaaax");
    await editorTill.commands.dispatch({ id: commandId("goto_line_end") });
    await editorTill.commands.dispatch({ id: commandId("till_prev_char") });
    await handleWorkbenchKeyInput(editorTill, { key: "k", text: "k" });
    await editorTill.commands.dispatch({ id: commandId("repeat_last_motion") });

    state = await editorTill.runtime.surfaces.readState({
      surfaceId: getDefaultState(editorTill).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 15, head: 1 }]);
  });
});
