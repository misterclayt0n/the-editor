import { describe, expect, it } from "vitest";
import { commandId, createEditorHost, createRuntimeClient } from "../../editor/src/index.js";
import { defaultCommandPalette } from "../src/commandPalette.js";
import themeModule from "../src/theme.js";

async function createThemeEditor() {
  const editor = createEditorHost({ runtime: createRuntimeClient() });
  await defaultCommandPalette()(editor);
  await themeModule(editor);
  return editor;
}

describe("default themes", () => {
  it("opens a theme picker command and activates imported themes", async () => {
    const editor = await createThemeEditor();

    await editor.commands.dispatch({ id: commandId("theme.select") });
    expect(editor.roles.snapshot("commandPalette")?.state).toMatchObject({
      visible: true,
      query: ":theme ",
    });

    const items = await editor.palette.items(":theme dark_plus");
    const darkPlus = items.find((item) => item.command?.id === commandId("theme.set") && (item.command.args as { id: string } | undefined)?.id === "dark_plus");
    expect(darkPlus).toBeTruthy();

    await editor.commands.dispatch({ id: commandId("theme.set"), args: { id: "dark_plus" } });
    expect(editor.themes.active()?.id).toBe("dark_plus");
  });
});
