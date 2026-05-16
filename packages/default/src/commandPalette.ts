import { readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import {
  commandId,
  commandPaletteRoleId,
  dispatch,
  matchesCommandPaletteItem,
  roleImplementationId,
  scoreCommandPaletteItem,
  type Command,
  type CommandArgumentItem,
  type CommandPaletteItem,
  type CommandPaletteRoleState,
  type EditorModule,
  type InteractiveSessionController,
  type InteractiveSessionId,
  defineHostStore,
} from "@the-editor/editor";
import { resolveFilePickerRoot } from "./workingDirectory.js";

interface CommandPaletteSessionState {
  query: string;
  selectedIndex: number;
  placeholder?: string;
  mode: "commands" | "argument";
  commandTitle?: string;
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
        editor.overlays.open(commandPaletteRoleId, { focus: true });
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
      id: commandId("commandPalette.open"),
      title: "Open Command Palette",
      aliases: ["commands"],
      category: "Workbench",
      palette: "hidden",
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
      id: commandId("commandPalette.close"),
      title: "Close Command Palette",
      hidden: true,
      palette: "hidden",
      run() {
        closePalette(editor, true);
      },
    });

    editor.commands.register<{ text: string }, void>({
      id: commandId("commandPalette.appendText"),
      title: "Append Command Palette Text",
      hidden: true,
      palette: "hidden",
      async run(_ctx, args) {
        const session = activeSessionId(editor);
        if (!session) return;
        const snapshot = editor.sessions.snapshot<CommandPaletteSessionState>(session);
        if (!snapshot) return;
        await editor.sessions.dispatch(session, "setQuery", { query: `${snapshot.state.query}${args.text}` });
      },
    });

    editor.commands.register<{ query: string }, void>({
      id: commandId("commandPalette.setQuery"),
      title: "Set Command Palette Query",
      hidden: true,
      palette: "hidden",
      async run(_ctx, args) {
        await dispatchPaletteSession(editor, "setQuery", args);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("commandPalette.backspace"),
      title: "Delete Command Palette Character",
      hidden: true,
      palette: "hidden",
      async run() {
        const sessionId = activeSessionId(editor);
        if (!sessionId) return;
        const snapshot = editor.sessions.snapshot<CommandPaletteSessionState>(sessionId);
        if (!snapshot?.state.query) return;
        await editor.sessions.dispatch(sessionId, "setQuery", { query: snapshot.state.query.slice(0, -1) });
      },
    });

    editor.commands.register<{ delta: number }, void>({
      id: commandId("commandPalette.moveSelection"),
      title: "Move Command Palette Selection",
      hidden: true,
      palette: "hidden",
      async run(_ctx, args) {
        await dispatchPaletteSession(editor, "moveSelection", args);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("commandPalette.accept"),
      title: "Accept Command Palette Selection",
      hidden: true,
      palette: "hidden",
      async run() {
        await dispatchPaletteSession(editor, "accept");
      },
    });

    editor.commands.register<{ index: number }, void>({
      id: commandId("commandPalette.selectIndex"),
      title: "Select Command Palette Item",
      hidden: true,
      palette: "hidden",
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
          esc: "commandPalette.close",
          ret: "commandPalette.accept",
          up: dispatch("commandPalette.moveSelection", { delta: -1 }),
          down: dispatch("commandPalette.moveSelection", { delta: 1 }),
          "C-p": dispatch("commandPalette.moveSelection", { delta: -1 }),
          "C-n": dispatch("commandPalette.moveSelection", { delta: 1 }),
          backspace: "commandPalette.backspace",
        },
      },
    });

    editor.commands.register<{ id: string }, void>({
      id: commandId("command.run"),
      title: "Run Command",
      aliases: ["command"],
      category: "Workbench",
      palette: "topLevel",
      args: [{ name: "id", source: "commands", required: true }],
      async run(_ctx, args) {
        if (!args?.id) return;
        await editor.commands.dispatch({ id: commandId(args.id) });
      },
    });

    editor.commands.registerArgumentProvider({
      id: "commands",
      title: "Commands",
      provide(ctx) {
        return editor.commands.all()
          .filter((command) => paletteVisibility(command) !== "hidden" && command.id !== commandId("command.run"))
          .map((command): CommandArgumentItem<string> => ({
            id: command.id,
            title: command.id,
            detail: command.title,
            value: command.id,
            keywords: [command.id, command.title, ...(command.aliases ?? []), ...(command.keywords ?? [])],
          }))
          .filter((item) => matchesCommandPaletteItem(ctx.query, item));
      },
    });

    editor.commands.registerArgumentProvider({
      id: "files",
      title: "Files",
      provide(ctx) {
        const root = resolveFilePickerRoot(editor);
        const files = listFiles(root, 900);
        return files
          .map((path): CommandArgumentItem<string> => ({
            id: path,
            title: path,
            detail: root,
            value: join(root, path),
            keywords: [path],
          }))
          .filter((item) => matchesCommandPaletteItem(ctx.query, item));
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
          mode: "commands",
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
      editor.overlays.open(commandPaletteRoleId, { focus: true });
      await refresh(session, requestState, ++requestState.version);
    }

    async function refresh(
      session: InteractiveSessionController<CommandPaletteSessionState>,
      requestState: { version: number },
      version: number,
    ): Promise<void> {
      const query = session.snapshot().state.query;
      const resolved = await resolvePaletteItems(editor, query);
      if (activeSessionId(editor) !== session.id || version !== requestState.version) return;
      session.updateState((state) => {
        state.mode = resolved.mode;
        state.commandTitle = resolved.commandTitle;
        state.placeholder = resolved.placeholder;
        state.items = resolved.items;
        state.selectedIndex = clampIndex(state.selectedIndex, state.items.length);
      });
    }
  };
}

async function resolvePaletteItems(editor: Parameters<EditorModule>[0], query: string): Promise<{ mode: "commands" | "argument"; commandTitle?: string; placeholder: string; items: CommandPaletteItem[] }> {
  const parsed = parseCommandInput(query);
  const tokenResolution = parsed.hasTail ? editor.commands.resolveToken(parsed.head) : undefined;
  const command = tokenResolution?.command;
  const firstArg = command?.args?.[0];
  if (command && firstArg) {
    const provider = editor.commands.argumentProvider(firstArg.source);
    const argItems = provider ? await provider.provide({ editor, command, query: parsed.tail }) : [];
    const items = rankArgumentItems(parsed.tail, command, firstArg.name, argItems);
    return {
      mode: "argument",
      commandTitle: command.title,
      placeholder: `${tokenResolution.kind === "alias" ? tokenResolution.alias : command.title} ${firstArg.name}`,
      items,
    };
  }

  return {
    mode: "commands",
    placeholder: "Type a command",
    items: rankCommandItems(editor.commands.all().filter((command) => paletteVisibility(command) === "topLevel"), query),
  };
}

function paletteVisibility(command: Command): "topLevel" | "command" | "hidden" {
  if (command.palette) return command.palette;
  if (command.hidden) return "hidden";
  return "command";
}

function rankCommandItems(commands: Command[], query: string): CommandPaletteItem[] {
  const trimmed = query.trim();
  return commands
    .map((command) => {
      const alias = command.aliases?.[0];
      const item: CommandPaletteItem = {
        id: command.id,
        title: command.title,
        detail: command.category,
        alias,
        group: command.category,
        keywords: [command.id, command.title, ...(command.aliases ?? []), ...(command.keywords ?? [])],
        command: { id: command.id },
      };
      return { item, score: scoreCommand(command, trimmed, item) };
    })
    .filter(({ score }) => !trimmed || score > 0)
    .sort((a, b) => b.score - a.score || a.item.title.localeCompare(b.item.title))
    .map(({ item }) => item);
}

function rankArgumentItems<T>(query: string, command: Command, name: string, items: CommandArgumentItem<T>[]): CommandPaletteItem[] {
  return items
    .map((arg) => ({
      item: {
        id: `${command.id}:${arg.id}`,
        title: arg.title,
        detail: arg.detail,
        keywords: arg.keywords,
        command: { id: command.id, args: { [name]: arg.value } },
      } satisfies CommandPaletteItem,
      score: scoreCommandPaletteItem(query, arg),
    }))
    .filter(({ score }) => !query.trim() || score > 0)
    .sort((a, b) => b.score - a.score || a.item.title.localeCompare(b.item.title))
    .map(({ item }) => item);
}

function scoreCommand(command: Command, query: string, item: CommandPaletteItem): number {
  if (!query) return 1;
  const normalized = query.trim().toLowerCase();
  if (command.aliases?.some((alias) => alias.toLowerCase() === normalized)) return 100_000;
  if (command.title.toLowerCase() === normalized) return 90_000;
  if (command.id.toLowerCase() === normalized) return 80_000;
  return scoreCommandPaletteItem(query, item);
}

function parseCommandInput(query: string): { head: string; tail: string; hasTail: boolean } {
  const match = query.match(/^(\S+)\s+(.*)$/s);
  if (!match) return { head: query.trim(), tail: "", hasTail: false };
  return { head: match[1] ?? "", tail: match[2] ?? "", hasTail: true };
}

function listFiles(root: string, limit: number): string[] {
  const results: string[] = [];
  const ignored = new Set([".git", ".jj", "node_modules", "target", "dist", ".next"]);
  const visit = (dir: string) => {
    if (results.length >= limit) return;
    let entries: string[];
    try {
      entries = readdirSync(dir);
    } catch {
      return;
    }
    for (const entry of entries) {
      if (results.length >= limit || ignored.has(entry)) continue;
      const path = join(dir, entry);
      let stat;
      try {
        stat = statSync(path);
      } catch {
        continue;
      }
      if (stat.isDirectory()) visit(path);
      else if (stat.isFile()) results.push(relative(root, path));
    }
  };
  visit(root);
  return results;
}

function paletteState(editor: Parameters<EditorModule>[0]): CommandPaletteStoredState {
  return editor.stores.host(defineHostStore({ id: commandPaletteStateSlot, init: () => ({} satisfies CommandPaletteStoredState) }));
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
  if (sessionId) editor.sessions.dispose(sessionId);
  paletteState(editor).activeSessionId = undefined;
  editor.overlays.close(commandPaletteRoleId, { restoreFocus });
}

function clampIndex(index: number, length: number): number {
  if (length <= 0) return 0;
  return ((index % length) + length) % length;
}
