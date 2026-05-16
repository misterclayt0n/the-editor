import { describe, expect, it } from "vitest";
import { createEditorHost } from "../../editor/src/index.js";
import { defaultCommandPalette } from "../src/commandPalette.js";
import { defaultReload } from "../src/reload.js";

import { createTestRuntime } from "./runtime.js";
async function createPaletteEditor() {
  const editor = createEditorHost({ runtime: createTestRuntime() });
  await defaultCommandPalette()(editor);
  await defaultReload()(editor);
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

    await editor.commands.dispatch({ id: "commandPalette.open" as never, args: { initialQuery: "reload default/" } });
    const role = editor.roles.snapshot("commandPalette")?.state as { sessionId?: string } | undefined;
    const session = role?.sessionId ? editor.sessions.snapshot<{ items: Array<{ command?: { args?: unknown } }> }>(role.sessionId)?.state : undefined;
    const items = session?.items ?? [];
    expect(items.some((item) => item.command?.args && typeof item.command.args === "object" && (item.command.args as { specifier?: string }).specifier === "default/ui")).toBe(true);
    expect(items.some((item) => item.command?.args && typeof item.command.args === "object" && (item.command.args as { specifier?: string }).specifier === "default/runtime")).toBe(true);
  });
});
