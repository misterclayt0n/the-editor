import {
  commandId,
  commandPaletteRoleId,
  dispatch,
  matchesCommandPaletteItem,
  roleImplementationId,
  scoreCommandPaletteItem,
  type CommandPaletteItem,
  type CommandPaletteRoleState,
  type EditorModule,
  type InteractiveSessionController,
  type InteractiveSessionId,
} from "@the-editor/editor";

interface CommandPaletteSessionState {
  query: string;
  selectedIndex: number;
  placeholder?: string;
  items: CommandPaletteItem[];
}

interface CommandPaletteStoredState {
  activeSessionId?: InteractiveSessionId;
}

const commandPaletteImplementationId = roleImplementationId("default.commandPalette");
const commandPaletteStateSlot = "default.commandPalette.state";

export function defaultCommandPalette(): EditorModule {
  return (editor) => {
    const moduleId = editor.currentModuleId();

    editor.roles.setFallback(commandPaletteRoleId, commandPaletteImplementationId);

    editor.roles.register<{ visible: boolean; sessionId?: string }>({
      role: commandPaletteRoleId,
      implementationId: commandPaletteImplementationId,
      title: "Default Command Palette",
      async open() {
        await openPalette();
      },
      close() {
        closePalette(editor, true);
      },
      async focus() {
        editor.focus.setRole(commandPaletteRoleId);
        if (!activeSessionId(editor)) await openPalette();
      },
      snapshot(): CommandPaletteRoleState & { sessionId?: string } {
        const sessionId = activeSessionId(editor);
        const session = sessionId ? editor.sessions.snapshot<CommandPaletteSessionState>(sessionId) : undefined;
        return {
          visible: Boolean(sessionId),
          sessionId,
          query: session?.state.query ?? "",
          selectedIndex: session?.state.selectedIndex ?? 0,
          placeholder: session?.state.placeholder,
          items: session?.state.items ?? [],
        };
      },
    });

    editor.commands.register<{ initialQuery?: string } | undefined, void>({
      id: commandId("command_palette.open"),
      title: "Command Palette",
      category: "Workbench",
      keywords: ["palette", "commands", "reload"],
      async run(_ctx, args) {
        const initialQuery = typeof args === "string"
          ? args
          : typeof (args as { initialQuery?: unknown } | undefined)?.initialQuery === "string"
            ? (args as { initialQuery: string }).initialQuery
            : "";
        await openPalette(initialQuery);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("command_palette.close"),
      title: "Close Command Palette",
      hidden: true,
      run() {
        closePalette(editor, true);
      },
    });

    editor.commands.register<{ text: string }, void>({
      id: commandId("command_palette.appendText"),
      title: "Append Command Palette Text",
      hidden: true,
      async run(_ctx, args) {
        const session = activeSessionId(editor);
        if (!session) return;
        const snapshot = editor.sessions.snapshot<CommandPaletteSessionState>(session);
        if (!snapshot) return;
        await editor.sessions.dispatch(session, "setQuery", { query: `${snapshot.state.query}${args.text}` });
      },
    });

    editor.commands.register<{ query: string }, void>({
      id: commandId("command_palette.setQuery"),
      title: "Set Command Palette Query",
      hidden: true,
      async run(_ctx, args) {
        await dispatchPaletteSession(editor, "setQuery", args);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("command_palette.backspace"),
      title: "Delete Command Palette Character",
      hidden: true,
      async run() {
        const sessionId = activeSessionId(editor);
        if (!sessionId) return;
        const snapshot = editor.sessions.snapshot<CommandPaletteSessionState>(sessionId);
        if (!snapshot?.state.query) return;
        await editor.sessions.dispatch(sessionId, "setQuery", { query: snapshot.state.query.slice(0, -1) });
      },
    });

    editor.commands.register<{ delta: number }, void>({
      id: commandId("command_palette.moveSelection"),
      title: "Move Command Palette Selection",
      hidden: true,
      async run(_ctx, args) {
        await dispatchPaletteSession(editor, "moveSelection", args);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("command_palette.accept"),
      title: "Accept Command Palette Selection",
      hidden: true,
      async run() {
        await dispatchPaletteSession(editor, "accept");
      },
    });

    editor.commands.register<{ index: number }, void>({
      id: commandId("command_palette.selectIndex"),
      title: "Select Command Palette Item",
      hidden: true,
      async run(_ctx, args) {
        await dispatchPaletteSession(editor, "selectIndex", args);
      },
    });

    editor.keymaps.register({
      id: "default.commandPalette.scope",
      scope: "role.commandPalette",
      tree: {
        name: "Command Palette",
        bindings: {
          esc: "command_palette.close",
          ret: "command_palette.accept",
          up: dispatch("command_palette.moveSelection", { delta: -1 }),
          down: dispatch("command_palette.moveSelection", { delta: 1 }),
          "C-p": dispatch("command_palette.moveSelection", { delta: -1 }),
          "C-n": dispatch("command_palette.moveSelection", { delta: 1 }),
          backspace: "command_palette.backspace",
        },
      },
    });

    editor.palette.registerProvider({
      id: "default.commands",
      title: "Commands",
      async provide(ctx, query) {
        return ctx.editor.commands
          .all()
          .filter((command) => !command.hidden)
          .map((command) => ({
            id: command.id,
            title: command.category ? `${command.category}: ${command.title ?? command.id}` : (command.title ?? command.id),
            detail: command.id,
            group: command.category,
            keywords: [command.id, ...(command.keywords ?? [])],
            command: { id: command.id },
          }))
          .filter((item) => matchesCommandPaletteItem(query, item));
      },
    });

    editor.palette.registerProvider({
      id: "default.reload",
      title: "Reload",
      priority: 100,
      provide(ctx, query) {
        const trimmed = query.trim();
        if (!(trimmed.startsWith(":") || trimmed.startsWith("reload"))) return [];
        const packages = ctx.editor.packages.snapshot();
        const items: CommandPaletteItem[] = [];
        if (":reload".startsWith(trimmed) || "reload".startsWith(trimmed) || trimmed === ":reload" || trimmed === "reload") {
          items.push({
            id: "reload:all",
            title: ":reload",
            detail: "Reload all loaded packages",
            keywords: ["reload", ":reload"],
            command: { id: commandId("editor.reload") },
          });
        }

        const prefix = trimmed.startsWith(":reload") ? trimmed.slice(7).trim() : trimmed.startsWith("reload") ? trimmed.slice(6).trim() : "";
        for (const pkg of packages) {
          if (!prefix || pkg.id.includes(prefix)) {
            items.push({
              id: `reload:${pkg.id}`,
              title: `:reload ${pkg.id}`,
              detail: pkg.loadedTargets.length ? `Targets: ${pkg.loadedTargets.join(", ")}` : "Registered package",
              keywords: [":reload", "reload", pkg.id],
              command: { id: commandId("editor.reload"), args: { specifier: pkg.id } },
            });
          }
          for (const target of pkg.loadedTargets) {
            const specifier = `${pkg.id}/${target}`;
            if (prefix && !specifier.includes(prefix)) continue;
            items.push({
              id: `reload:${specifier}`,
              title: `:reload ${specifier}`,
              detail: `Reload ${target} target`,
              keywords: [":reload", "reload", specifier, pkg.id, target],
              command: { id: commandId("editor.reload"), args: { specifier } },
            });
          }
        }

        return items;
      },
    });

    async function openPalette(initialQuery = ""): Promise<void> {
      closePalette(editor, false);
      const query = typeof initialQuery === "string" ? initialQuery : "";
      const requestState = { version: 0 };
      const session = editor.sessions.create<CommandPaletteSessionState>({
        kind: "commandPalette",
        role: commandPaletteRoleId,
        title: "Command Palette",
        moduleId,
        state: {
          query,
          selectedIndex: 0,
          placeholder: "Type a command",
          items: [],
        },
        methods: {
          setQuery: async ({ session }, args) => {
            const query = typeof (args as { query?: unknown } | undefined)?.query === "string"
              ? (args as { query: string }).query
              : "";
            session.updateState((state) => {
              state.query = query;
              state.selectedIndex = 0;
              state.items = [];
            });
            await refresh(session, requestState, ++requestState.version);
          },
          moveSelection: ({ session }, args) => {
            const delta = typeof (args as { delta?: unknown } | undefined)?.delta === "number"
              ? (args as { delta: number }).delta
              : 0;
            session.updateState((state) => {
              if (!state.items.length) return;
              state.selectedIndex = clampIndex(state.selectedIndex + delta, state.items.length);
            });
          },
          selectIndex: ({ session }, args) => {
            const index = typeof (args as { index?: unknown } | undefined)?.index === "number"
              ? (args as { index: number }).index
              : 0;
            session.updateState((state) => {
              if (!state.items.length) return;
              state.selectedIndex = clampIndex(index, state.items.length);
            });
          },
          accept: async ({ session }) => {
            const snapshot = session.snapshot().state;
            const item = snapshot.items[snapshot.selectedIndex];
            if (!item?.command) return;
            closePalette(editor, true);
            await editor.commands.dispatch(item.command);
          },
          close: () => {
            closePalette(editor, true);
          },
        },
      });

      paletteState(editor).activeSessionId = session.id;
      editor.focus.setRole(commandPaletteRoleId);
      editor.workbench.invalidate("roles", "focus");
      await refresh(session, requestState, ++requestState.version);
    }

    async function refresh(
      session: InteractiveSessionController<CommandPaletteSessionState>,
      requestState: { version: number },
      version: number,
    ): Promise<void> {
      const query = session.snapshot().state.query;
      const items = await editor.palette.items(query);
      if (activeSessionId(editor) !== session.id || version !== requestState.version) return;
      const scored = items
        .map((item) => ({ item, score: scoreCommandPaletteItem(query, item) }))
        .filter(({ score }) => !query.trim() || score > 0)
        .sort((a, b) => b.score - a.score || a.item.title.localeCompare(b.item.title))
        .map(({ item }) => item);
      session.updateState((state) => {
        state.items = scored;
        state.selectedIndex = clampIndex(state.selectedIndex, state.items.length);
      });
    }

  };
}

function paletteState(editor: Parameters<EditorModule>[0]): CommandPaletteStoredState {
  return editor.stateSlot(commandPaletteStateSlot, () => ({} satisfies CommandPaletteStoredState));
}

function activeSessionId(editor: Parameters<EditorModule>[0]): InteractiveSessionId | undefined {
  const sessionId = paletteState(editor).activeSessionId;
  if (!sessionId) return undefined;
  if (editor.sessions.snapshot(sessionId)) return sessionId;
  paletteState(editor).activeSessionId = undefined;
  return undefined;
}

async function dispatchPaletteSession(editor: Parameters<EditorModule>[0], method: string, args?: unknown): Promise<void> {
  const sessionId = activeSessionId(editor);
  if (!sessionId) return;
  await editor.sessions.dispatch(sessionId, method, args);
}

function closePalette(editor: Parameters<EditorModule>[0], restoreFocus: boolean): void {
  const sessionId = activeSessionId(editor);
  paletteState(editor).activeSessionId = undefined;
  if (sessionId) editor.sessions.dispose(sessionId);
  if (restoreFocus) editor.focus.restore();
  editor.workbench.invalidate("roles", "focus");
}

function clampIndex(index: number, length: number): number {
  if (!length) return 0;
  if (index < 0) return length - 1;
  if (index >= length) return 0;
  return index;
}
