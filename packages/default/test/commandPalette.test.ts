import { describe, expect, it } from "vitest";
import { createEditorHost, createRuntimeClient } from "../../editor/src/index.js";
import { defaultCommandPalette } from "../src/commandPalette.js";

async function createPaletteEditor() {
  const editor = createEditorHost({ runtime: createRuntimeClient() });
  await defaultCommandPalette()(editor);
  return editor;
}

describe("default command palette", () => {
  it("offers reload target items for package/target path queries", async () => {
    const editor = await createPaletteEditor();

    editor.packages.register({
      id: "default",
      entrypoints: {
        runtime: () => undefined,
        interaction: () => undefined,
        ui: () => undefined,
        theme: () => undefined,
      },
      defaultTargets: ["runtime", "interaction", "ui", "theme"],
    });
    await editor.packages.load("default");

    const items = await editor.palette.items("reload default/");
    expect(items.some((item) => item.command?.args && typeof item.command.args === "object" && (item.command.args as { specifier?: string }).specifier === "default/ui")).toBe(true);
    expect(items.some((item) => item.command?.args && typeof item.command.args === "object" && (item.command.args as { specifier?: string }).specifier === "default/runtime")).toBe(true);
  });
});
