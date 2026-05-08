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
  it("changes the current selection and enters insert mode", async () => {
    const editor = await createTestEditor("hello");

    await editor.commands.dispatch({ id: commandId("change_selection") });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.text).toBe("ello");
    expect(state.lineEnding).toBe("\n");
    expect(getDefaultState(editor).mode).toBe("insert");
    await expect(editor.runtime.registers.read({ name: '"' })).resolves.toMatchObject({ values: ["h"] });
  });

  it("copies the current selection onto the next line", async () => {
    const editor = await createTestEditor("abc\ndef\nghi");
    await editor.commands.dispatch({ id: commandId("select_mode") });
    await editor.commands.dispatch({ id: commandId("extend_next_char") });
    await handleWorkbenchKeyInput(editor, { key: "b", text: "b" });

    await editor.commands.dispatch({ id: commandId("copy_selection_on_next_line") });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 0, head: 2 },
      { anchor: 4, head: 6 },
    ]);
  });

  it("copies a cursor to the next matching visual column skipping short lines", async () => {
    const editor = await createTestEditor("alskd jklasjdlk ajdlksj\n\n\naksdj lajdkla");
    await editor.commands.dispatch({ id: commandId("move_char_right"), args: { count: 8 } });

    await editor.commands.dispatch({ id: commandId("copy_selection_on_next_line") });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 8, head: 8 },
      { anchor: 34, head: 35 },
    ]);
  });

  it("splits selections on newline", async () => {
    const editor = await createTestEditor("alpha\nbeta\ngamma");
    await editor.commands.dispatch({ id: commandId("select_all") });

    await editor.commands.dispatch({ id: commandId("split_selection_on_newline") });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 0, head: 5 },
      { anchor: 6, head: 10 },
      { anchor: 11, head: 16 },
    ]);
  });

  it("matches brackets in normal and select mode", async () => {
    const editor = await createTestEditor("(ab)");

    await editor.commands.dispatch({ id: commandId("match_brackets") });

    let state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 3, head: 3 }]);

    const selectEditor = await createTestEditor("(ab)");
    await selectEditor.commands.dispatch({ id: commandId("select_mode") });
    await selectEditor.commands.dispatch({ id: commandId("extend_match_brackets") });

    state = await selectEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(selectEditor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 4 }]);
  });

  it("adds, deletes, and replaces surrounds through pending input", async () => {
    const addEditor = await createTestEditor("ab");
    await addEditor.commands.dispatch({ id: commandId("select_mode") });
    await addEditor.commands.dispatch({ id: commandId("extend_next_char") });
    await handleWorkbenchKeyInput(addEditor, { key: "b", text: "b" });

    await addEditor.commands.dispatch({ id: commandId("surround_add") });
    await handleWorkbenchKeyInput(addEditor, { key: "(", text: "(" });

    let state = await addEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(addEditor).activeSurfaceId!,
    });
    expect(state.text).toBe("(ab)");
    expect(getDefaultState(addEditor).mode).toBe("normal");

    const editor = await createTestEditor("(ab)");
    await editor.commands.dispatch({ id: commandId("move_char_right") });
    await editor.commands.dispatch({ id: commandId("surround_replace") });
    await handleWorkbenchKeyInput(editor, { key: "(", text: "(" });
    await handleWorkbenchKeyInput(editor, { key: "[", text: "[" });

    state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.text).toBe("[ab]");

    await editor.commands.dispatch({ id: commandId("surround_delete") });
    await handleWorkbenchKeyInput(editor, { key: "[", text: "[" });

    state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.text).toBe("ab");
  });

  it("selects textobjects from pending input", async () => {
    const editor = await createTestEditor("hello world");
    await editor.commands.dispatch({ id: commandId("move_char_right"), args: { count: 1 } });

    await editor.commands.dispatch({ id: commandId("select_textobject_inner") });
    await handleWorkbenchKeyInput(editor, { key: "w", text: "w" });

    let state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 5 }]);

    const pairEditor = await createTestEditor("(ab)");
    await pairEditor.commands.dispatch({ id: commandId("move_char_right") });
    await pairEditor.commands.dispatch({ id: commandId("select_textobject_around") });
    await handleWorkbenchKeyInput(pairEditor, { key: "m", text: "m" });

    state = await pairEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(pairEditor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 4 }]);
  });

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

  it("extends to the next matching char from select mode", async () => {
    const editor = await createTestEditor("hello world");
    await editor.commands.dispatch({ id: commandId("select_mode") });

    await editor.commands.dispatch({ id: commandId("extend_next_char") });
    await handleWorkbenchKeyInput(editor, { key: "o", text: "o" });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 5 }]);
  });

  it("extends till the previous matching char from select mode", async () => {
    const editor = await createTestEditor("kaaaaaaaaaaaakaaaaaaaaaaaaax");
    await editor.commands.dispatch({ id: commandId("goto_line_end") });
    await editor.commands.dispatch({ id: commandId("select_mode") });

    await editor.commands.dispatch({ id: commandId("extend_till_prev_char") });
    await handleWorkbenchKeyInput(editor, { key: "k", text: "k" });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 28, head: 14 }]);
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
