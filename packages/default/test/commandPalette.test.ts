import { describe, expect, it } from "vitest";
import { createEditorHost } from "../../editor/src/index.js";
import { defaultCommandPalette } from "../src/commandPalette.js";
import { defaultReload } from "../src/reload.js";
import { commandId } from "@the-editor/editor";

import { createTestRuntime } from "./runtime.js";
async function createPaletteEditor() {
  const editor = createEditorHost({ runtime: createTestRuntime() });
  await defaultCommandPalette()(editor);
  await defaultReload()(editor);
  return editor;
}

describe("default command palette", () => {
  it("shows curated commands by default and raw commands behind command", async () => {
    const editor = await createPaletteEditor();

    editor.commands.register({ id: commandId("append_mode"), title: "Append Mode", run: () => undefined });

    await editor.commands.dispatch({ id: "commandPalette.open" as never });
    const role = editor.roles.snapshot("commandPalette")?.state as { sessionId?: string } | undefined;
    const session = role?.sessionId ? editor.sessions.snapshot<{ items: Array<{ id: string; title: string }> }>(role.sessionId)?.state : undefined;
    const items = session?.items ?? [];
    expect(items.some((item) => item.id === "editor.reload")).toBe(true);
    expect(items.some((item) => item.id === "command.run")).toBe(true);
    expect(items.some((item) => item.id === "append_mode")).toBe(false);

    await editor.sessions.dispatch(role!.sessionId!, "setQuery", { query: "command" });
    await editor.sessions.dispatch(role!.sessionId!, "acceptItem", { id: "command.run" });
    await editor.sessions.dispatch(role!.sessionId!, "setQuery", { query: "append" });
    const commandSession = editor.sessions.snapshot<{ mode: string; items: Array<{ id: string; title: string; command?: { id: string; args?: unknown } }> }>(role!.sessionId!)?.state;
    expect(commandSession?.mode).toBe("argument");
    expect(commandSession?.items.some((item) => item.title === "append_mode" && (item.command?.args as { id?: string } | undefined)?.id === "append_mode")).toBe(true);
  });

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

    await editor.commands.dispatch({ id: "commandPalette.open" as never, args: { initialQuery: "reload" } });
    const role = editor.roles.snapshot("commandPalette")?.state as { sessionId?: string } | undefined;
    await editor.sessions.dispatch(role!.sessionId!, "acceptItem", { id: "editor.reload" });
    await editor.sessions.dispatch(role!.sessionId!, "setQuery", { query: "default/" });
    const session = role?.sessionId ? editor.sessions.snapshot<{ mode: string; items: Array<{ command?: { args?: unknown } }> }>(role.sessionId)?.state : undefined;
    expect(session?.mode).toBe("argument");
    const items = session?.items ?? [];
    expect(items.some((item) => item.command?.args && typeof item.command.args === "object" && (item.command.args as { specifier?: string }).specifier === "default/ui")).toBe(true);
    expect(items.some((item) => item.command?.args && typeof item.command.args === "object" && (item.command.args as { specifier?: string }).specifier === "default/runtime")).toBe(true);
  });
});
