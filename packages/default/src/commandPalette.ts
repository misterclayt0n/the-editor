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
} from "@the-editor/editor";

interface CommandPaletteState {
  visible: boolean;
  query: string;
  selectedIndex: number;
  items: CommandPaletteItem[];
}

const commandPaletteImplementationId = roleImplementationId("default.commandPalette");

export function defaultCommandPalette(): EditorModule {
  return (editor) => {
    const state: CommandPaletteState = {
      visible: false,
      query: "",
      selectedIndex: 0,
      items: [],
    };

    async function refresh(): Promise<void> {
      const query = state.query;
      const items = await editor.palette.items(query);
      const scored = items
        .map((item) => ({ item, score: scoreCommandPaletteItem(query, item) }))
        .filter(({ score }) => !query.trim() || score > 0)
        .sort((a, b) => b.score - a.score || a.item.title.localeCompare(b.item.title))
        .map(({ item }) => item);
      state.items = scored;
      state.selectedIndex = clampIndex(state.selectedIndex, state.items.length);
    }

    async function open(initialQuery = ""): Promise<void> {
      state.visible = true;
      state.query = initialQuery;
      state.selectedIndex = 0;
      await refresh();
    }

    function close(): void {
      state.visible = false;
      state.query = "";
      state.selectedIndex = 0;
      state.items = [];
    }

    editor.roles.setFallback(commandPaletteRoleId, commandPaletteImplementationId);

    editor.roles.register({
      role: commandPaletteRoleId,
      implementationId: commandPaletteImplementationId,
      title: "Default Command Palette",
      async open() {
        editor.focus.setRole(commandPaletteRoleId);
        await open();
      },
      close() {
        close();
        editor.focus.restore();
      },
      async focus() {
        editor.focus.setRole(commandPaletteRoleId);
        if (!state.visible) await open();
      },
      snapshot(): CommandPaletteRoleState {
        return {
          visible: state.visible,
          query: state.query,
          selectedIndex: state.selectedIndex,
          placeholder: "Type a command",
          items: state.items,
        };
      },
    });

    editor.commands.register<{ initialQuery?: string } | undefined, void>({
      id: commandId("command_palette.open"),
      title: "Command Palette",
      category: "Workbench",
      keywords: ["palette", "commands", "reload"],
      async run(_ctx, args) {
        editor.focus.setRole(commandPaletteRoleId);
        await open(args?.initialQuery ?? "");
      },
    });

    editor.commands.register<void, void>({
      id: commandId("command_palette.close"),
      title: "Close Command Palette",
      hidden: true,
      run() {
        close();
        editor.focus.restore();
      },
    });

    editor.commands.register<{ text: string }, void>({
      id: commandId("command_palette.appendText"),
      title: "Append Command Palette Text",
      hidden: true,
      async run(_ctx, args) {
        state.query += args.text;
        state.selectedIndex = 0;
        await refresh();
      },
    });

    editor.commands.register<{ query: string }, void>({
      id: commandId("command_palette.setQuery"),
      title: "Set Command Palette Query",
      hidden: true,
      async run(_ctx, args) {
        state.query = args.query;
        state.selectedIndex = 0;
        await refresh();
      },
    });

    editor.commands.register<void, void>({
      id: commandId("command_palette.backspace"),
      title: "Delete Command Palette Character",
      hidden: true,
      async run() {
        if (!state.query) return;
        state.query = state.query.slice(0, -1);
        state.selectedIndex = 0;
        await refresh();
      },
    });

    editor.commands.register<{ delta: number }, void>({
      id: commandId("command_palette.moveSelection"),
      title: "Move Command Palette Selection",
      hidden: true,
      run(_ctx, args) {
        if (!state.items.length) return;
        state.selectedIndex = clampIndex(state.selectedIndex + args.delta, state.items.length);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("command_palette.accept"),
      title: "Accept Command Palette Selection",
      hidden: true,
      async run() {
        const item = state.items[state.selectedIndex];
        if (!item?.command) return;
        close();
        editor.focus.restore();
        await editor.commands.dispatch(item.command);
      },
    });

    editor.commands.register<{ index: number }, void>({
      id: commandId("command_palette.selectIndex"),
      title: "Select Command Palette Item",
      hidden: true,
      run(_ctx, args) {
        if (!state.items.length) return;
        state.selectedIndex = clampIndex(args.index, state.items.length);
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
  };
}

function clampIndex(index: number, length: number): number {
  if (!length) return 0;
  if (index < 0) return length - 1;
  if (index >= length) return 0;
  return index;
}
