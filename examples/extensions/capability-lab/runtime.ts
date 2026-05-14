import {
  commandId,
  dispatch,
  matchesCommandPaletteItem,
  roleImplementationId,
  toolTypeId,
  toDisposable,
  type CommandPaletteItem,
  type EditorHost,
  type EditorModule,
} from "@the-editor/editor";

const capabilityToolTypeId = toolTypeId("capability-lab.panel");
const capabilityStatuslineImplementationId = roleImplementationId("capability-lab.statusline");
const capabilityDiagnosticId = "capability-lab:diagnostic";
const capabilityStateSlot = "capability-lab.state";

interface CapabilityLabState {
  counter: number;
  ghostTextEnabled: boolean;
  highlightEnabled: boolean;
  tickerHandle: ReturnType<typeof setInterval> | undefined;
  lastEvent: string;
  toolOpenedCount: number;
}

const capabilityLabRuntime: EditorModule = (editor) => {
  editor.tools.registerType({
    id: capabilityToolTypeId,
    title: "Capability Lab",
    root: "bottomPanel",
    instancePolicy: "singleton",
    closeable: true,
    persistenceKey: "capability-lab.panel",
    snapshot: ({ editor: host, tool }) => snapshotCapabilityToolState(host, tool.id),
  });

  editor.roles.register({
    role: "statusline",
    implementationId: capabilityStatuslineImplementationId,
    title: "Capability Lab Statusline",
    snapshot() {
      const state = getCapabilityState(editor);
      return {
        counter: state.counter,
        ghostTextEnabled: state.ghostTextEnabled,
        highlightEnabled: state.highlightEnabled,
        tickerRunning: Boolean(state.tickerHandle),
        lastEvent: state.lastEvent,
        focus: describeFocus(editor),
      };
    },
  });

  editor.decorations.registerProvider({
    id: "capability-lab.decorations",
    title: "Capability Lab Decorations",
    priority: 40,
    provide(ctx) {
      const state = getCapabilityState(ctx.editor);
      const rows = ctx.frame?.rows ?? [];
      if (!rows.length) return [];

      const decorations: Array<
        | {
          kind: "range";
          id: string;
          surfaceRow: number;
          startCol: number;
          endCol: number;
          background: string;
          underline: string;
        }
        | {
          kind: "inlineText";
          id: string;
          surfaceRow: number;
          col: number;
          text: string;
          foreground: string;
          opacity: number;
        }
      > = [];
      if (state.highlightEnabled) {
        decorations.push(buildHighlightDecoration(rows, state.counter));
      }
      if (state.ghostTextEnabled) {
        const row = rows[Math.min(rows.length - 1, state.counter % rows.length)] ?? rows[0]!;
        decorations.push({
          kind: "inlineText",
          id: "capability-lab.ghost",
          surfaceRow: row.surfaceRow,
          col: row.text.length,
          text: `  // capability-lab ${state.counter}`,
          foreground: "#7dd3fc",
          opacity: 0.85,
        });
      }
      return decorations;
    },
  });

  editor.keymaps.register({
    id: "capability-lab.tool-scope",
    scope: "tool-kind.capability-lab.panel",
    tree: {
      name: "Capability Lab",
      bindings: {
        g: "capability_lab.toggleGhostText",
        h: "capability_lab.toggleHighlight",
        t: "capability_lab.toggleTicker",
        n: "capability_lab.bumpCounter",
        r: "capability_lab.resetCounter",
        e: "capability_lab.focusEditor",
        ":": dispatch("command_palette.open", { initialQuery: "capability " }),
      },
    },
  });

  editor.commands.register<void, string>({
    id: commandId("capability_lab.open"),
    title: "Open Capability Lab",
    category: "Extensions",
    keywords: ["capability", "lab", "tool", "mock"],
    run() {
      const state = getCapabilityState(editor);
      state.toolOpenedCount += 1;
      state.lastEvent = `opened tool (${state.toolOpenedCount})`;
      const instance = editor.tools.open(capabilityToolTypeId, "Capability Lab");
      editor.workbench.invalidate("tools", "focus", "roles");
      return instance.id;
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.close"),
    title: "Close Capability Lab",
    category: "Extensions",
    hidden: true,
    run() {
      closeCapabilityTool(editor);
      setLastEvent(editor, "closed tool");
      editor.workbench.invalidate("tools", "focus", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.focusTool"),
    title: "Focus Capability Lab",
    category: "Extensions",
    hidden: true,
    run() {
      const instance = ensureCapabilityTool(editor);
      editor.tools.focus(instance.id);
      setLastEvent(editor, `focused ${instance.title}`);
      editor.workbench.invalidate("tools", "focus", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.focusEditor"),
    title: "Focus Editor",
    category: "Extensions",
    hidden: true,
    run() {
      editor.input.focusEditor({ reason: "capabilityLab.focusEditor" });
      setLastEvent(editor, "focused editor");
      editor.workbench.invalidate("focus", "tools", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.focusPalette"),
    title: "Focus Command Palette",
    category: "Extensions",
    hidden: true,
    async run() {
      setLastEvent(editor, "focused command palette");
      await editor.commands.dispatch({
        id: commandId("command_palette.open"),
        args: { initialQuery: "capability " },
      });
      editor.workbench.invalidate("focus", "roles", "tools");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.bumpCounter"),
    title: "Capability Lab: Increment Counter",
    category: "Extensions",
    hidden: true,
    run() {
      const state = getCapabilityState(editor);
      state.counter += 1;
      state.lastEvent = `counter -> ${state.counter}`;
      editor.workbench.invalidate("tools", "decorations", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.resetCounter"),
    title: "Capability Lab: Reset Counter",
    category: "Extensions",
    hidden: true,
    run() {
      const state = getCapabilityState(editor);
      state.counter = 0;
      state.lastEvent = "counter reset";
      editor.workbench.invalidate("tools", "decorations", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.toggleGhostText"),
    title: "Toggle Capability Ghost Text",
    category: "Extensions",
    hidden: true,
    run() {
      const state = getCapabilityState(editor);
      state.ghostTextEnabled = !state.ghostTextEnabled;
      state.lastEvent = state.ghostTextEnabled ? "ghost text enabled" : "ghost text disabled";
      editor.workbench.invalidate("decorations", "tools", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.toggleHighlight"),
    title: "Toggle Capability Highlight",
    category: "Extensions",
    hidden: true,
    run() {
      const state = getCapabilityState(editor);
      state.highlightEnabled = !state.highlightEnabled;
      state.lastEvent = state.highlightEnabled ? "highlight enabled" : "highlight disabled";
      editor.workbench.invalidate("decorations", "tools", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.toggleTicker"),
    title: "Toggle Capability Ticker",
    category: "Extensions",
    hidden: true,
    run() {
      const state = getCapabilityState(editor);
      if (state.tickerHandle) {
        stopTicker(editor);
        state.lastEvent = "ticker stopped";
      } else {
        state.tickerHandle = setInterval(() => {
          const current = getCapabilityState(editor);
          current.counter += 1;
          current.lastEvent = `tick ${current.counter}`;
          editor.workbench.invalidate("tools", "decorations", "roles");
        }, 1000);
        state.lastEvent = "ticker started";
      }
      editor.workbench.invalidate("tools", "decorations", "roles");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.kickAsyncUpdate"),
    title: "Capability Lab: Async Ping",
    category: "Extensions",
    hidden: true,
    run() {
      setLastEvent(editor, "queued async ping");
      editor.workbench.invalidate("tools", "roles");
      setTimeout(() => {
        const state = getCapabilityState(editor);
        state.counter += 10;
        state.lastEvent = `async ping -> ${state.counter}`;
        editor.workbench.invalidate("tools", "decorations", "roles");
      }, 600);
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.selectStatusline"),
    title: "Use Capability Lab Statusline",
    category: "Extensions",
    hidden: true,
    run() {
      editor.roles.activate("statusline", capabilityStatuslineImplementationId);
      setLastEvent(editor, "selected capability statusline");
      editor.workbench.invalidate("roles", "tools");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.clearStatuslineSelection"),
    title: "Use Automatic Statusline",
    category: "Extensions",
    hidden: true,
    run() {
      editor.roles.clearSelection("statusline");
      setLastEvent(editor, "cleared statusline selection");
      editor.workbench.invalidate("roles", "tools");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.reportDiagnostic"),
    title: "Report Capability Diagnostic",
    category: "Extensions",
    hidden: true,
    run() {
      editor.diagnostics.report({
        id: capabilityDiagnosticId,
        severity: "warn",
        source: "capability-lab",
        packageId: "capability-lab",
        message: "mock capability-lab diagnostic",
        details: "This is only here to prove background invalidation and inspector plumbing.",
      });
      setLastEvent(editor, "reported diagnostic");
      editor.workbench.invalidate("all");
    },
  });

  editor.commands.register<void, void>({
    id: commandId("capability_lab.clearDiagnostic"),
    title: "Clear Capability Diagnostic",
    category: "Extensions",
    hidden: true,
    run() {
      editor.diagnostics.clear(capabilityDiagnosticId);
      setLastEvent(editor, "cleared diagnostic");
      editor.workbench.invalidate("all");
    },
  });

  editor.palette.registerProvider({
    id: "capability-lab.commands",
    title: "Capability Lab",
    priority: 30,
    provide(_ctx, query) {
      return capabilityPaletteItems().filter((item) => matchesCommandPaletteItem(query, item));
    },
  });

  return toDisposable(() => {
    stopTicker(editor);
    editor.diagnostics.clear(capabilityDiagnosticId);
  });
};

export default capabilityLabRuntime;

function capabilityPaletteItems(): CommandPaletteItem[] {
  return [
    paletteItem({
      id: "capability-lab:open",
      title: "Extensions: Open Capability Lab",
      detail: "capability_lab.open",
      keywords: ["capability", "lab", "tool", "mock"],
      command: { id: commandId("capability_lab.open") },
    }),
    paletteItem({
      id: "capability-lab:statusline",
      title: "Extensions: Use Capability Lab Statusline",
      detail: "capability_lab.selectStatusline",
      keywords: ["capability", "statusline", "role"],
      command: { id: commandId("capability_lab.selectStatusline") },
    }),
    paletteItem({
      id: "capability-lab:automatic-statusline",
      title: "Extensions: Use Automatic Statusline",
      detail: "capability_lab.clearStatuslineSelection",
      keywords: ["capability", "statusline", "automatic", "role"],
      command: { id: commandId("capability_lab.clearStatuslineSelection") },
    }),
    paletteItem({
      id: "capability-lab:diagnostic",
      title: "Extensions: Report Capability Diagnostic",
      detail: "capability_lab.reportDiagnostic",
      keywords: ["capability", "diagnostic", "warning", "inspector"],
      command: { id: commandId("capability_lab.reportDiagnostic") },
    }),
  ];
}

function buildHighlightDecoration(rows: Array<{ surfaceRow: number; text: string }>, counter: number) {
  const row = rows[counter % rows.length] ?? rows[0]!;
  const width = Math.max(1, Math.min(Math.max(row.text.length, 1), 4 + (counter % 8)));
  return {
    kind: "range" as const,
    id: "capability-lab.highlight",
    surfaceRow: row.surfaceRow,
    startCol: 0,
    endCol: width,
    background: "rgba(56, 189, 248, 0.18)",
    underline: "#38bdf8",
  };
}

function ensureCapabilityTool(editor: EditorHost) {
  const existing = editor.tools.allInstances().find((tool) => tool.typeId === capabilityToolTypeId);
  if (existing) return existing;
  const state = getCapabilityState(editor);
  state.toolOpenedCount += 1;
  return editor.tools.open(capabilityToolTypeId, "Capability Lab");
}

function closeCapabilityTool(editor: EditorHost): void {
  for (const tool of editor.tools.allInstances()) {
    if (tool.typeId === capabilityToolTypeId) editor.tools.close(tool.id);
  }
}

function getCapabilityState(editor: EditorHost): CapabilityLabState {
  return editor.stateSlot(capabilityStateSlot, () => ({
    counter: 0,
    ghostTextEnabled: true,
    highlightEnabled: true,
    tickerHandle: undefined,
    lastEvent: "ready",
    toolOpenedCount: 0,
  }));
}

function snapshotCapabilityToolState(editor: EditorHost, toolId: string) {
  const state = getCapabilityState(editor);
  const focus = editor.input.current();
  return {
    counter: state.counter,
    ghostTextEnabled: state.ghostTextEnabled,
    highlightEnabled: state.highlightEnabled,
    tickerRunning: Boolean(state.tickerHandle),
    lastEvent: state.lastEvent,
    focus: describeFocus(editor),
    selectedStatusline: editor.roles.snapshot("statusline")?.selectedImplementationId,
    thisToolFocused: focus.kind === "tool" && focus.toolId === toolId,
  };
}

function stopTicker(editor: EditorHost): void {
  const state = getCapabilityState(editor);
  if (!state.tickerHandle) return;
  clearInterval(state.tickerHandle);
  state.tickerHandle = undefined;
}

function setLastEvent(editor: EditorHost, lastEvent: string): void {
  getCapabilityState(editor).lastEvent = lastEvent;
}

function describeFocus(editor: EditorHost): string {
  const focus = editor.input.current();
  if (focus.kind === "editor") return "editor";
  if (focus.kind === "role") return `role:${focus.role}`;
  return `tool:${focus.toolId}`;
}

function paletteItem(item: CommandPaletteItem): CommandPaletteItem {
  return item;
}
