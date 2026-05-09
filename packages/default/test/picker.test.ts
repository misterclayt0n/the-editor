import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { commandId, createEditorHost, createRuntimeClient } from "../../editor/src/index.js";
import { defaultKeymaps } from "../src/keymaps.js";
import { defaultPicker } from "../src/picker.js";
import { defaultScratch } from "../src/scratch.js";
import { defaultViewport } from "../src/scratch.js";
import { getDefaultState } from "../src/state.js";

interface PickerSnapshot {
  visible: boolean;
  query: string;
  selectedIndex: number;
  items: Array<{ id: string; title: string; detail?: string }>;
}

async function createPickerEditor() {
  const editor = createEditorHost({ runtime: createRuntimeClient() });
  await defaultScratch()(editor);
  await defaultPicker()(editor);
  await defaultKeymaps()(editor);
  return editor;
}

describe("default picker", () => {
  it("preserves the current surface viewport when opening a file", async () => {
    const dir = await mkdtemp(join(tmpdir(), "the-editor-viewport-"));
    const path = join(dir, "viewport.txt");
    await writeFile(path, "one\ntwo\nthree\n", "utf8");

    const editor = createEditorHost({ runtime: createRuntimeClient() });
    await defaultScratch()(editor);
    const state = getDefaultState(editor);
    state.surfaceViewport = { ...defaultViewport(), rowCount: 41, wrapWidth: 137 };
    state.surfaceMetrics = { cellWidthPx: 9, rowHeightPx: 22 };
    await editor.commands.dispatch({ id: commandId("file.open"), args: { path } });

    const surface = state.activeSurfaceId!;
    const surfaceState = await editor.runtime.surfaces.readState({ surfaceId: surface });
    expect(surfaceState.viewport.rowCount).toBe(41);
    expect(surfaceState.viewport.wrapWidth).toBe(137);
  });

  it("reopens the file picker after closing while indexing", async () => {
    const dir = await mkdtemp(join(tmpdir(), "the-editor-picker-reopen-"));
    const first = join(dir, "alpha.txt");
    const second = join(dir, "beta-file.txt");
    await writeFile(first, "alpha\n", "utf8");
    await writeFile(second, "beta\npreview\n", "utf8");

    const editor = await createPickerEditor();
    await editor.commands.dispatch({ id: commandId("file.open"), args: { path: first } });
    await editor.commands.dispatch({ id: commandId("file_picker") });
    await editor.commands.dispatch({ id: commandId("picker.close") });
    await editor.commands.dispatch({ id: commandId("file_picker") });

    await waitFor(async () => {
      await editor.commands.dispatch({ id: commandId("picker.setQuery"), args: { query: "beta" } });
      const snapshot = pickerSnapshot(editor);
      return Boolean(snapshot?.visible && snapshot.items.some((item) => item.detail?.includes("beta-file.txt")));
    });

    const snapshot = pickerSnapshot(editor)!;
    expect(snapshot.visible).toBe(true);
    expect(snapshot.items.some((item) => item.detail?.includes("beta-file.txt"))).toBe(true);
  }, 15_000);

  it("opens the file picker and opens a selected file", async () => {
    const dir = await mkdtemp(join(tmpdir(), "the-editor-picker-"));
    const first = join(dir, "alpha.txt");
    const second = join(dir, "beta-file.txt");
    await writeFile(first, "alpha\n", "utf8");
    await writeFile(second, "beta\npreview\n", "utf8");

    const editor = await createPickerEditor();
    await editor.commands.dispatch({ id: commandId("file.open"), args: { path: first } });
    await editor.commands.dispatch({ id: commandId("file_picker") });

    await waitFor(async () => {
      await editor.commands.dispatch({ id: commandId("picker.setQuery"), args: { query: "beta" } });
      const snapshot = pickerSnapshot(editor);
      return Boolean(snapshot?.items.some((item) => item.detail?.includes("beta-file.txt")));
    });

    let snapshot = pickerSnapshot(editor)!;
    const index = snapshot.items.findIndex((item) => item.detail?.includes("beta-file.txt"));
    expect(index).toBeGreaterThanOrEqual(0);

    await editor.commands.dispatch({ id: commandId("picker.selectIndex"), args: { index } });
    snapshot = pickerSnapshot(editor)!;
    expect(snapshot.visible).toBe(true);
    expect(snapshot.items[snapshot.selectedIndex]?.detail).toContain("beta-file.txt");

    await editor.commands.dispatch({ id: commandId("picker.accept") });
    expect(getDefaultState(editor).activePath?.endsWith("/beta-file.txt")).toBe(true);
    expect(pickerSnapshot(editor)?.visible ?? false).toBe(false);
  }, 15_000);
});

function pickerSnapshot(editor: ReturnType<typeof createEditorHost>) {
  return editor.roles.snapshot("picker")?.state as PickerSnapshot | undefined;
}

async function waitFor(predicate: () => boolean | Promise<boolean>, timeoutMs = 5_000): Promise<void> {
  const started = Date.now();
  for (;;) {
    if (await predicate()) return;
    if (Date.now() - started > timeoutMs) throw new Error("timed out waiting for picker state");
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
