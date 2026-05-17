import { describe, expect, it } from "vitest";
import { commandId, createEditorHost } from "../../editor/src/index.js";
import { defaultCommandPalette } from "../src/commandPalette.js";
import themeModule from "../src/theme.js";

import { createTestRuntime } from "./runtime.js";
async function createThemeEditor() {
  const editor = createEditorHost({ runtime: createTestRuntime() });
  await defaultCommandPalette()(editor);
  await themeModule(editor);
  return editor;
}

describe("default themes", () => {
  it("opens a theme picker command and activates imported themes", async () => {
    const editor = await createThemeEditor();

    await editor.commands.dispatch({ id: commandId("theme.select") });
    const paletteRoleState = editor.roles.snapshot("commandPalette")?.state as { visible?: boolean; sessionId?: string } | undefined;
    const paletteSession = paletteRoleState?.sessionId ? editor.sessions.snapshot<{ query: string }>(paletteRoleState.sessionId)?.state : undefined;
    expect({
      visible: Boolean(paletteRoleState?.visible),
      query: paletteSession?.query,
    }).toMatchObject({
      visible: true,
      query: "theme",
    });

    const role = editor.roles.snapshot("commandPalette")?.state as { sessionId?: string } | undefined;
    if (role?.sessionId) await editor.sessions.dispatch(role.sessionId, "setQuery", { query: "dark_plus" });
    const updatedPaletteSession = role?.sessionId ? editor.sessions.snapshot<{ items: Array<{ command?: { id: unknown; args?: unknown } }> }>(role.sessionId)?.state : undefined;
    const darkPlus = updatedPaletteSession?.items.find((item) => item.command?.id === commandId("theme.select") && (item.command.args as { id: string } | undefined)?.id === "dark_plus");
    expect(darkPlus).toBeTruthy();

    await editor.commands.dispatch({ id: commandId("theme.set"), args: { id: "dark_plus" } });
    expect(editor.themes.active()?.id).toBe("dark_plus");
  });
});
