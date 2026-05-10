import { mkdtemp, mkdir, realpath, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { commandId, createEditorHost, createRuntimeClient } from "../../editor/src/index.js";
import { defaultCommandPalette } from "../src/commandPalette.js";
import { defaultScratch } from "../src/scratch.js";
import { defaultWorkingDirectory } from "../src/workingDirectory.js";
import { getDefaultState } from "../src/state.js";

async function createWorkingDirectoryEditor(cwd: string) {
  const editor = createEditorHost({ runtime: createRuntimeClient(), cwd });
  await defaultWorkingDirectory()(editor);
  await defaultCommandPalette()(editor);
  await defaultScratch()(editor);
  return editor;
}

describe("working directory", () => {
  it("shows and changes the current working directory", async () => {
    const root = await realpath(await mkdtemp(join(tmpdir(), "the-editor-cwd-")));
    const nested = join(root, "nested");
    await mkdir(nested);
    const canonicalNested = await realpath(nested);

    const editor = await createWorkingDirectoryEditor(root);

    await editor.commands.dispatch({ id: commandId("editor.showWorkingDirectory") });
    expect(editor.notifications.snapshot()[0]?.message).toBe(`current working directory: ${root}`);

    await editor.commands.dispatch({ id: commandId("editor.changeWorkingDirectory"), args: { path: canonicalNested } });
    expect(editor.cwd.current()).toBe(canonicalNested);
    expect(editor.cwd.previous()).toBe(root);
    expect(editor.notifications.snapshot()[0]?.message).toBe(`current working directory: ${canonicalNested}`);

    await editor.commands.dispatch({ id: commandId("editor.changeWorkingDirectory"), args: { path: "-" } });
    expect(editor.cwd.current()).toBe(root);
    expect(editor.notifications.snapshot()[0]?.message).toBe(`current working directory: ${root}`);
  });

  it("offers cd and pwd items in the command palette", async () => {
    const root = await realpath(await mkdtemp(join(tmpdir(), "the-editor-cwd-palette-")));
    const editor = await createWorkingDirectoryEditor(root);

    const items = await editor.palette.items(":");
    expect(items.some((item) => item.command?.id === commandId("editor.changeWorkingDirectory"))).toBe(true);
    expect(items.some((item) => item.command?.id === commandId("editor.showWorkingDirectory"))).toBe(true);
  });

  it("resolves relative file opens from the current working directory", async () => {
    const root = await realpath(await mkdtemp(join(tmpdir(), "the-editor-cwd-open-")));
    await writeFile(join(root, "note.txt"), "hello\n", "utf8");

    const editor = await createWorkingDirectoryEditor(root);
    await editor.commands.dispatch({ id: commandId("file.open"), args: { path: "note.txt" } });

    expect(getDefaultState(editor).activePath).toBe(join(root, "note.txt"));
  });
});
