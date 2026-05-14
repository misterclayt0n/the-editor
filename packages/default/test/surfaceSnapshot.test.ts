import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";
import { commandId, createEditorHost, createRuntimeClient, defineHostStore } from "../../editor/src/index.js";
import { defaultCompletion } from "../src/completion.js";
import { defaultEditing } from "../src/editing.js";
import { defaultScratch } from "../src/scratch.js";
import { defaultWorkbench } from "../src/workbench.js";
import { completionContextFromSnapshot, surfaceSnapshot } from "../src/surfaceSnapshot.js";
import { getDefaultState } from "../src/state.js";

async function createTestEditor(text: string) {
  const editor = createEditorHost({ runtime: createRuntimeClient() });
  await defaultScratch()(editor);
  await defaultEditing()(editor);
  await defaultWorkbench()(editor);
  await editor.commands.dispatch({ id: commandId("scratch.create"), args: { text } });
  await editor.commands.dispatch({ id: commandId("insert_mode") });
  return editor;
}

describe("surface snapshots", () => {
  it("are updated from editing command mutation results", async () => {
    const editor = await createTestEditor("");
    const surfaceId = getDefaultState(editor).activeSurfaceId!;

    await editor.commands.dispatch({ id: commandId("edit.insertText"), args: { text: "std" } });

    const snapshot = surfaceSnapshot(editor, surfaceId);
    expect(snapshot?.text).toBe("std");
    const context = completionContextFromSnapshot(editor, { kind: "input", character: "d" }, (_editor, path) => path);
    expect(context?.query).toBe("std");
    expect(context?.text).toBe("std");
  });

  it("serve automatic completion without reading runtime surface state", async () => {
    const dir = await mkdtemp(join(tmpdir(), "the-editor-completion-snapshot-"));
    const path = join(dir, "main.snaptest");
    await writeFile(path, "");
    const editor = createEditorHost({ runtime: createRuntimeClient() });
    await defaultScratch()(editor);
    await defaultEditing()(editor);
    await defaultWorkbench()(editor);
    await defaultCompletion()(editor);
    await editor.commands.dispatch({ id: commandId("file.open"), args: { path } });
    await editor.commands.dispatch({ id: commandId("insert_mode") });

    const lsp = editor.lsp as any;
    lsp.serverIdsForPath = () => ["snapshot-lsp"];
    lsp.languageForPath = () => ({ id: "snaptest", languageId: "snaptest", serverIds: ["snapshot-lsp"], fileTypes: ["snaptest"] });
    lsp.request = vi.fn().mockResolvedValue({ isIncomplete: false, items: [{ label: "stdio" }] });
    editor.stores.host(defineHostStore<{ statuses: any[]; updatedAt: number }>({ id: "default.lsp.statuses", init: () => ({ statuses: [], updatedAt: 0 }) })).statuses = [{
      id: "snapshot-lsp",
      handle: 1,
      state: "running",
      completionProvider: true,
      completionTriggerCharacters: [],
    }];

    const readState = vi.spyOn(editor.runtime.surfaces, "readState");
    await editor.commands.dispatch({ id: commandId("edit.insertText"), args: { text: "s" } });
    await editor.commands.dispatch({ id: commandId("edit.insertText"), args: { text: "t" } });
    await new Promise((resolve) => setTimeout(resolve, 230));

    expect(readState).not.toHaveBeenCalled();
    expect(lsp.request).toHaveBeenCalledWith(expect.objectContaining({ method: "textDocument/completion" }));
  });
});
