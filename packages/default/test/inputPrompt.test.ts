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
});
