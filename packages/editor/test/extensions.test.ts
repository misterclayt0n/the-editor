import { describe, expect, it } from "vitest";
import type { RuntimeApi } from "@the-editor/contracts/runtime";
import { commandId, createEditorHost, defineEditorPackage, defineTheme, resolveThemeStyle, roleImplementationId, toolTypeId } from "../src/index.js";

function createRuntimeStub(): RuntimeApi {
  return {
    server: { ping: async () => ({ ok: true }) },
    syntax: {
      highlightLegend: async () => ({}),
    },
    documents: {
      openFile: async () => ({ documentId: 1 as never }),
      createScratch: async () => ({ documentId: 1 as never }),
    },
    surfaces: {
      create: async () => ({ surfaceId: 1 as never }),
      setViewport: async () => ({ ok: true }),
      setMetrics: async () => ({ ok: true }),
      setSoftWrap: async () => ({ ok: true }),
      applyOperation: async () => ({ ok: true }),
      readState: async () => ({ text: "", lineEnding: "\n", selection: [{ cursorId: 1, anchor: 0, head: 0 }] }),
      applyEdit: async () => ({ ok: true }),
      commitUndoCheckpoint: async () => ({ ok: true }),
      undo: async () => ({ ok: true }),
      redo: async () => ({ ok: true }),
      earlier: async () => ({ ok: true }),
      later: async () => ({ ok: true }),
      renderFrame: async () => ({ surfaceId: 1 as never, rows: [], selections: [], cursors: [], syntax: [] }),
    },
    registers: {
      read: async () => ({ values: null }),
      write: async () => ({ ok: true }),
    },
    clipboard: {
      read: async () => ({ value: null }),
      write: async () => ({ ok: true }),
    },
    resources: {
      save: async () => ({ ok: true }),
    },
  };
}

describe("extension registries", () => {
  it("reloads package targets and restores commands", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });
    let loads = 0;

    editor.packages.register(defineEditorPackage({
      id: "demo",
      entrypoints: {
        runtime: (host) => {
          loads += 1;
          host.commands.register({
            id: commandId("demo.hello"),
            title: "Hello",
            run: () => `hello-${loads}`,
          });
        },
      },
    }));

    await editor.packages.load("demo");
    await expect(editor.commands.dispatch({ id: commandId("demo.hello") })).resolves.toBe("hello-1");

    await editor.packages.reload("demo/runtime");
    await expect(editor.commands.dispatch({ id: commandId("demo.hello") })).resolves.toBe("hello-2");
  });

  it("tracks active role snapshots", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });

    await editor.use({
      id: "roles.demo",
      activate: (host) => {
        host.roles.register({
          role: "commandPalette",
          implementationId: "demo.palette",
          snapshot: () => ({ visible: true, query: ":reload", selectedIndex: 0, items: [] }),
        });
      },
    });

    expect(editor.roles.snapshot("commandPalette")).toEqual({
      implementationId: "demo.palette",
      selectedImplementationId: undefined,
      selectionSource: "implicit",
      title: undefined,
      moduleId: "roles.demo",
      packageId: undefined,
      fallbackReason: undefined,
      state: { visible: true, query: ":reload", selectedIndex: 0, items: [] },
      available: [
        {
          implementationId: "demo.palette",
          title: undefined,
          moduleId: "roles.demo",
          packageId: undefined,
        },
      ],
    });
  });

  it("keeps explicit role selections across later registrations", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });

    await editor.use({
      id: "roles.one",
      activate: (host) => {
        host.roles.register({ role: "commandPalette", implementationId: "one.palette" });
      },
    });
    editor.roles.activate("commandPalette", "one.palette");

    await editor.use({
      id: "roles.two",
      activate: (host) => {
        host.roles.register({ role: "commandPalette", implementationId: "two.palette" });
      },
    });

    expect(editor.roles.snapshot("commandPalette")?.implementationId).toBe("one.palette");
    expect(editor.roles.snapshot("commandPalette")?.selectedImplementationId).toBe("one.palette");
  });

  it("tracks tool snapshots and focus", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });

    await editor.use({
      id: "tools.demo",
      activate: (host) => {
        host.tools.registerType({
          id: toolTypeId("demo.tool"),
          title: "Demo Tool",
          root: "bottomPanel",
          snapshot: () => ({ visible: true }),
        });
      },
    });

    const tool = editor.tools.open("demo.tool");
    expect(editor.focus.current()).toEqual({ kind: "tool", toolId: tool.id });
    expect(editor.tools.snapshotAll()).toEqual([
      {
        ...tool,
        state: { visible: true },
      },
    ]);
  });

  it("opens tool panes inside the editor split tree", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });

    await editor.use({
      id: "tools.demo.editor",
      activate: (host) => {
        host.tools.registerType({
          id: toolTypeId("demo.editor.tool"),
          title: "Editor Tool",
          root: "editor",
          instancePolicy: "singleton",
        });
      },
    });

    editor.workbench.openSurface({
      title: "Scratch",
      surfaceId: 1 as never,
      documentId: 1 as never,
      closeable: true,
    });
    const tool = editor.workbench.openTool("demo.editor.tool", {
      title: "Demo Tool",
      input: { url: "https://example.com" },
      placement: { kind: "split", direction: "right" },
    });

    expect(editor.focus.current()).toEqual({ kind: "tool", toolId: tool.id });
    expect(editor.workbench.snapshotEditorLayout()).toMatchObject({
      root: {
        kind: "split",
        axis: "row",
        children: [
          { kind: "group" },
          { kind: "group" },
        ],
      },
      groups: [
        { paneIds: [expect.any(String)] },
        { paneIds: [expect.any(String)] },
      ],
      items: [
        { kind: "surface", title: "Scratch", typeId: "surface" },
        { kind: "tool", title: "Demo Tool", toolId: tool.id, input: { url: "https://example.com" } },
      ],
      panes: [
        { kind: "surface", title: "Scratch", itemId: expect.any(String) },
        { kind: "tool", title: "Demo Tool", toolId: tool.id, input: { url: "https://example.com" }, itemId: expect.any(String) },
      ],
    });
  });

  it("restores open tools across runtime reloads", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });
    let loads = 0;

    editor.packages.register(defineEditorPackage({
      id: "demo.tools",
      entrypoints: {
        runtime: (host) => {
          loads += 1;
          host.roles.register({
            role: "statusline",
            implementationId: roleImplementationId("demo.statusline"),
          });
          host.tools.registerType({
            id: toolTypeId("demo.tools.panel"),
            title: `Panel ${loads}`,
            root: "bottomPanel",
            instancePolicy: "singleton",
            persistenceKey: "demo.tools.panel",
            snapshot: () => ({ loads }),
          });
        },
      },
    }));

    await editor.packages.load("demo.tools");
    const tool = editor.tools.open("demo.tools.panel");
    expect(editor.focus.current()).toEqual({ kind: "tool", toolId: tool.id });

    await editor.packages.reload("demo.tools/runtime");

    const tools = editor.tools.snapshotAll();
    expect(tools).toHaveLength(1);
    expect(tools[0]?.typeId).toBe("demo.tools.panel");
    expect(tools[0]?.state).toEqual({ loads: 2 });
    expect(editor.focus.current()).toEqual({ kind: "tool", toolId: tools[0]?.id });
  });

  it("bumps ui target load versions for renderer remounts", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });

    editor.packages.register(defineEditorPackage({
      id: "demo.ui",
      entrypoints: {
        ui: "/tmp/demo-ui.tsx",
      },
      defaultTargets: ["ui"],
    }));

    await editor.packages.load("demo.ui");
    expect(editor.packages.snapshot()).toEqual([
      {
        id: "demo.ui",
        title: undefined,
        loadedTargets: ["ui"],
        availableTargets: ["ui"],
        entrypointPaths: { ui: "/tmp/demo-ui.tsx" },
        loadVersions: { ui: 1 },
      },
    ]);

    await editor.packages.reload("demo.ui/ui");
    expect(editor.packages.snapshot()[0]?.loadVersions).toEqual({ ui: 2 });
  });

  it("resolves theme palette entries and dotted fallback styles", async () => {
    const editor = createEditorHost({ runtime: createRuntimeStub() });

    await editor.use({
      id: "themes.demo",
      activate: (host) => {
        host.themes.register(defineTheme({
          id: "demo.dark",
          palette: {
            text: "#dddddd",
            panel: "#111111",
            accent: "#ffcc66",
          },
          styles: {
            "ui.menu": { bg: "panel", fg: "text" },
            "ui.menu.selected": { bg: "accent" },
          },
        }));
      },
    });

    editor.themes.activate("demo.dark");
    const active = editor.themes.active();
    expect(active?.id).toBe("demo.dark");
    expect(editor.themes.getColor("accent")).toBe("#ffcc66");
    expect(resolveThemeStyle(active!, "ui.menu.selected")).toEqual({
      bg: "#ffcc66",
      fg: "#dddddd",
    });
  });
});
