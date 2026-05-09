import { describe, expect, it } from "vitest";
import { handleWorkbenchKeyInput } from "../../editor/src/workbench.js";
import { commandId, createEditorHost, createRuntimeClient } from "../../editor/src/index.js";
import { defaultEditing } from "../src/editing.js";
import { defaultInputPrompt } from "../src/inputPrompt.js";
import { defaultKeymaps } from "../src/keymaps.js";
import { defaultScratch } from "../src/scratch.js";
import { getDefaultState } from "../src/state.js";
import { defaultWorkbench } from "../src/workbench.js";

async function createTestEditor(text: string, options?: { keymaps?: boolean }) {
  const editor = createEditorHost({ runtime: createRuntimeClient() });
  await defaultScratch()(editor);
  await defaultEditing()(editor);
  await defaultInputPrompt()(editor);
  if (options?.keymaps) await defaultKeymaps()(editor);
  await defaultWorkbench()(editor);
  await editor.commands.dispatch({
    id: commandId("scratch.create"),
    args: { text },
  });
  return editor;
}

describe("input prompt", () => {
  it("opens a select regex prompt, previews matches, and restores on cancel", async () => {
    const editor = await createTestEditor("one two one");
    await editor.commands.dispatch({ id: commandId("select_all") });

    await editor.commands.dispatch({ id: commandId("select_regex") });
    await editor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "one" } });

    let snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({
      visible: true,
      title: "SELECT",
      query: "one",
      placeholder: "Regular expression",
    });

    let state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 0, head: 3 },
      { anchor: 8, head: 11 },
    ]);

    await editor.commands.dispatch({ id: commandId("input_prompt.close") });

    snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({ visible: false, query: "" });

    state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 11 }]);
  });

  it("keeps the prompt open on invalid regex and closes on accept", async () => {
    const editor = await createTestEditor("one two one");
    await editor.commands.dispatch({ id: commandId("select_all") });
    await editor.commands.dispatch({ id: commandId("select_regex") });

    await editor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "(" } });

    let snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({ visible: true });
    expect((snapshot?.state as { error?: string } | undefined)?.error).toBeTruthy();

    await editor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "one" } });
    await editor.commands.dispatch({ id: commandId("input_prompt.accept") });

    snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({ visible: false, query: "" });

    const state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 0, head: 3 },
      { anchor: 8, head: 11 },
    ]);
  });

  it("opens / and ? searches and repeats with n and N", async () => {
    const editor = await createTestEditor("one two one", { keymaps: true });

    await handleWorkbenchKeyInput(editor, { key: "/", text: "/" });
    await editor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "one" } });

    let snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({
      visible: true,
      title: "/",
      query: "one",
      placeholder: "Search",
    });

    let state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 3 }]);

    await editor.commands.dispatch({ id: commandId("input_prompt.accept") });

    const { values } = await editor.runtime.registers.read({ name: "/" });
    expect(values).toEqual(["one"]);

    await handleWorkbenchKeyInput(editor, { key: "n", text: "n" });
    state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 8, head: 11 }]);

    await handleWorkbenchKeyInput(editor, { key: "N", text: "N" });
    state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 3 }]);

    await editor.commands.dispatch({ id: commandId("goto_line_end") });
    await handleWorkbenchKeyInput(editor, { key: "?", text: "?" });
    await editor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "one" } });

    snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({ visible: true, title: "?", query: "one" });

    state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 8, head: 11 }]);
  });

  it("splits selections on regex matches through the input prompt", async () => {
    const editor = await createTestEditor("alpha,beta,gamma");
    await editor.commands.dispatch({ id: commandId("select_all") });
    await editor.commands.dispatch({ id: commandId("split_selection") });
    await editor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "," } });

    let snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({ visible: true, title: "SPLIT", query: "," });

    let state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 0, head: 5 },
      { anchor: 6, head: 10 },
      { anchor: 11, head: 16 },
    ]);

    await editor.commands.dispatch({ id: commandId("input_prompt.accept") });

    snapshot = editor.roles.snapshot("inputPrompt");
    expect(snapshot?.state).toMatchObject({ visible: false, query: "" });

    state = await editor.runtime.surfaces.readState({
      surfaceId: getDefaultState(editor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 0, head: 5 },
      { anchor: 6, head: 10 },
      { anchor: 11, head: 16 },
    ]);
  });

  it("keeps and removes selections through regex prompts", async () => {
    const keepEditor = await createTestEditor("alpha\nbeta\ngamma");
    await keepEditor.commands.dispatch({ id: commandId("select_all") });
    await keepEditor.commands.dispatch({ id: commandId("split_selection_on_newline") });
    await keepEditor.commands.dispatch({ id: commandId("keep_selections") });
    await keepEditor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "^a" } });
    await keepEditor.commands.dispatch({ id: commandId("input_prompt.accept") });

    let state = await keepEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(keepEditor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 5 }]);

    const removeEditor = await createTestEditor("alpha\nbeta\ngamma");
    await removeEditor.commands.dispatch({ id: commandId("select_all") });
    await removeEditor.commands.dispatch({ id: commandId("split_selection_on_newline") });
    await removeEditor.commands.dispatch({ id: commandId("remove_selections") });
    await removeEditor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "^a" } });
    await removeEditor.commands.dispatch({ id: commandId("input_prompt.accept") });

    state = await removeEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(removeEditor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([
      { anchor: 6, head: 10 },
      { anchor: 11, head: 16 },
    ]);
  });

  it("runs shell prompt workflows", async () => {
    const pipeEditor = await createTestEditor("alpha");
    await pipeEditor.commands.dispatch({ id: commandId("select_all") });
    await pipeEditor.commands.dispatch({ id: commandId("shell_pipe") });
    await pipeEditor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "tr a-z A-Z" } });
    await pipeEditor.commands.dispatch({ id: commandId("input_prompt.accept") });

    let state = await pipeEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(pipeEditor).activeSurfaceId!,
    });
    expect(state.text).toBe("ALPHA");
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 5 }]);

    const insertEditor = await createTestEditor("alpha");
    await insertEditor.commands.dispatch({ id: commandId("select_all") });
    await insertEditor.commands.dispatch({ id: commandId("shell_insert_output") });
    await insertEditor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "printf X" } });
    await insertEditor.commands.dispatch({ id: commandId("input_prompt.accept") });

    state = await insertEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(insertEditor).activeSurfaceId!,
    });
    expect(state.text).toBe("Xalpha");
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 1 }]);

    const keepPipeEditor = await createTestEditor("alpha\nbeta\ngamma");
    await keepPipeEditor.commands.dispatch({ id: commandId("select_all") });
    await keepPipeEditor.commands.dispatch({ id: commandId("split_selection_on_newline") });
    await keepPipeEditor.commands.dispatch({ id: commandId("shell_keep_pipe") });
    await keepPipeEditor.commands.dispatch({ id: commandId("input_prompt.setQuery"), args: { query: "grep -q '^a'" } });
    await keepPipeEditor.commands.dispatch({ id: commandId("input_prompt.accept") });

    state = await keepPipeEditor.runtime.surfaces.readState({
      surfaceId: getDefaultState(keepPipeEditor).activeSurfaceId!,
    });
    expect(state.selection.map(({ anchor, head }) => ({ anchor, head }))).toEqual([{ anchor: 0, head: 5 }]);
  });
});
