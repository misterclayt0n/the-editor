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
  type CommandPaletteFrameTarget,
  type CommandPaletteItem,
  type CommandPaletteRoleState,
  type EditorModule,
  type InteractiveSessionController,
  type InteractiveSessionId,
  defineHostStore,
} from "@the-editor/editor";
import { resolveFilePickerRoot } from "./workingDirectory.js";

type PathPaletteMode = "file" | "directory" | "both";

interface PathPaletteFrameArgs {
  root: string;
  baseRoot: string;
  mode: PathPaletteMode;
  commandId: string;
  argumentName: string;
}

interface CommandPaletteFrameState {
  query: string;
  placeholder?: string;
  mode: "commands" | "argument";
  commandId?: string;
  commandTitle?: string;
  argumentName?: string;
  providerFrame?: CommandPaletteFrameTarget;
}

interface CommandPaletteSessionState extends CommandPaletteFrameState {
  items: CommandPaletteItem[];
  stack: CommandPaletteFrameState[];
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
          placeholder: session?.state.placeholder,
          mode: session?.state.mode,
          commandTitle: session?.state.commandTitle,
          canGoBack: Boolean(session?.state.stack.length),
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
        if (!snapshot?.state.query) {
          await editor.sessions.dispatch(sessionId, "back");
          return;
        }
        await editor.sessions.dispatch(sessionId, "setQuery", { query: snapshot.state.query.slice(0, -1) });
      },
    });

    editor.commands.register<{ id: string }, void>({
      id: commandId("command.run"),
      title: "Run Editor Command",
      aliases: ["command"],
      category: "Workbench",
      palette: "topLevel",
      icon: { kind: "lucide", name: "keyboard" },
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
            icon: command.icon,
          }))
          .filter((item) => matchesCommandPaletteItem(ctx.query, item));
      },
    });

    editor.commands.registerArgumentProvider({
      id: "files",
      title: "Files",
      provide(ctx) {
        return pathArgumentItems(editor, ctx.query, {
          root: resolveFilePickerRoot(editor),
          baseRoot: resolveFilePickerRoot(editor),
          mode: "both",
          commandId: ctx.command.id,
          argumentName: ctx.command.args?.[0]?.name ?? "path",
        });
      },
    });

    editor.commands.registerArgumentProvider({
      id: "directories",
      title: "Directories",
      provide(ctx) {
        return pathArgumentItems(editor, ctx.query, {
          root: resolveFilePickerRoot(editor),
          baseRoot: resolveFilePickerRoot(editor),
          mode: "directory",
          commandId: ctx.command.id,
          argumentName: ctx.command.args?.[0]?.name ?? "path",
        });
      },
    });

    editor.palette.registerProvider<PathPaletteFrameArgs>({
      id: "default.paths",
      title: "Paths",
      visibility: "hidden",
      provide(ctx) {
        const args = ctx.frame?.args;
        if (!args) return [];
        return pathPaletteItems(editor, ctx.query, args);
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
          placeholder: "Type a command",
          mode: "commands",
          items: [],
          stack: [],
        },
        methods: {
          setQuery: async ({ session }, args) => {
            const query = typeof (args as { query?: unknown } | undefined)?.query === "string"
              ? (args as { query: string }).query
              : "";
            session.updateState((state) => {
              state.query = query;
            });
            await refresh(session, requestState, ++requestState.version);
          },
          acceptItem: async (_ctx, args) => {
            const itemId = typeof (args as { id?: unknown } | undefined)?.id === "string"
              ? (args as { id: string }).id
              : "";
            const snapshot = session.snapshot().state;
            const item = snapshot.items.find((candidate) => candidate.id === itemId);
            if (!item) return;
            if (typeof item.query === "string") {
              session.updateState((state) => {
                state.query = item.query ?? "";
              });
              await refresh(session, requestState, ++requestState.version);
              return;
            }
            if (item.frame) {
              enterProviderFrame(session, item.frame);
              await refresh(session, requestState, ++requestState.version);
              return;
            }
            if (!item.command) return;
            const command = editor.commands.get(item.command.id);
            const firstArg = command?.args?.[0];
            if (command && firstArg && !item.command.args) {
              await enterArgumentFrame(editor, session, requestState, ++requestState.version, command, firstArg.name, firstArg.source);
              return;
            }
            closePalette(editor, true);
            await editor.commands.dispatch(item.command);
          },
          back: async ({ session }) => {
            if (!goBack(session)) return;
            await refresh(session, requestState, ++requestState.version);
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
      const snapshot = session.snapshot().state;
      const resolved = await resolvePaletteItems(editor, snapshot);
      if (activeSessionId(editor) !== session.id || version !== requestState.version) return;
      session.updateState((state) => {
        state.mode = resolved.mode;
        state.commandTitle = resolved.commandTitle;
        state.placeholder = resolved.placeholder;
        state.items = resolved.items;
      });
    }
  };
}

async function resolvePaletteItems(editor: Parameters<EditorModule>[0], state: CommandPaletteFrameState): Promise<{ mode: "commands" | "argument"; commandTitle?: string; placeholder: string; items: CommandPaletteItem[] }> {
  if (state.mode === "argument" && state.providerFrame) {
    return {
      mode: "argument",
      commandTitle: state.providerFrame.title,
      placeholder: state.providerFrame.placeholder ?? "Search...",
      items: await editor.palette.items(state.query, { ...state.providerFrame, query: state.query }),
    };
  }

  if (state.mode === "argument" && state.commandId && state.argumentName) {
    const command = editor.commands.get(commandId(state.commandId));
    if (!command) return { mode: "commands", placeholder: "Type a command", items: [] };
    const firstArg = command.args?.[0];
    const provider = firstArg ? editor.commands.argumentProvider(firstArg.source) : undefined;
    const argItems = provider ? await provider.provide({ editor, command, query: state.query }) : [];
    return {
      mode: "argument",
      commandTitle: command.title,
      placeholder: state.placeholder ?? `Search ${firstArg?.name ?? "argument"}...`,
      items: rankArgumentItems(state.query, command, state.argumentName, argItems),
    };
  }

  return {
    mode: "commands",
    placeholder: "Type a command",
    items: await resolveGlobalPaletteItems(editor, state.query),
  };
}

async function resolveGlobalPaletteItems(editor: Parameters<EditorModule>[0], query: string): Promise<CommandPaletteItem[]> {
  const commandItems = rankCommandItems(editor.commands.all().filter((command) => paletteVisibility(command) === "topLevel"), query);
  if (!query.trim()) return commandItems;
  return [...commandItems, ...await editor.palette.items(query)];
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
        group: command.category ?? "Commands",
        keywords: [command.id, command.title, ...(command.aliases ?? []), ...(command.keywords ?? [])],
        icon: command.icon,
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
        group: command.title,
        keywords: arg.keywords,
        icon: arg.icon,
        command: arg.frame ? undefined : { id: command.id, args: { [name]: arg.value } },
        frame: arg.frame,
        query: arg.query,
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

async function enterArgumentFrame(
  editor: Parameters<EditorModule>[0],
  session: InteractiveSessionController<CommandPaletteSessionState>,
  requestState: { version: number },
  version: number,
  command: Command,
  argumentName: string,
  argumentSource: string,
): Promise<void> {
  session.updateState((state) => {
    state.stack.push(currentFrame(state));
    state.query = "";
    state.mode = "argument";
    state.commandId = command.id;
    state.commandTitle = command.title;
    state.argumentName = argumentName;
    state.providerFrame = undefined;
    state.placeholder = `Search ${argumentSource}...`;
    state.items = [];
  });
  // Resolve through the same versioned refresh path as text changes so stale async providers cannot win.
  const snapshot = session.snapshot().state;
  const resolved = await resolvePaletteItems(editor, snapshot);
  if (version !== requestState.version) return;
  session.updateState((state) => {
    state.mode = resolved.mode;
    state.commandTitle = resolved.commandTitle;
    state.placeholder = resolved.placeholder;
    state.items = resolved.items;
  });
}

function enterProviderFrame(session: InteractiveSessionController<CommandPaletteSessionState>, frame: CommandPaletteFrameTarget): void {
  session.updateState((state) => {
    state.stack.push(currentFrame(state));
    state.query = frame.query ?? "";
    state.mode = "argument";
    state.commandId = undefined;
    state.commandTitle = frame.title;
    state.argumentName = undefined;
    state.providerFrame = frame;
    state.placeholder = frame.placeholder ?? "Search...";
    state.items = [];
  });
}

function goBack(session: InteractiveSessionController<CommandPaletteSessionState>): boolean {
  let didGoBack = false;
  session.updateState((state) => {
    const previous = state.stack.pop();
    if (!previous) return;
    state.query = previous.query;
    state.placeholder = previous.placeholder;
    state.mode = previous.mode;
    state.commandId = previous.commandId;
    state.commandTitle = previous.commandTitle;
    state.argumentName = previous.argumentName;
    state.providerFrame = previous.providerFrame;
    state.items = [];
    didGoBack = true;
  });
  return didGoBack;
}

function currentFrame(state: CommandPaletteSessionState): CommandPaletteFrameState {
  return {
    query: state.query,
    placeholder: state.placeholder,
    mode: state.mode,
    commandId: state.commandId,
    commandTitle: state.commandTitle,
    argumentName: state.argumentName,
    providerFrame: state.providerFrame,
  };
}

function pathArgumentItems(editor: Parameters<EditorModule>[0], query: string, args: PathPaletteFrameArgs): CommandArgumentItem<string>[] {
  return pathPaletteItems(editor, query, args)
    .map((item): CommandArgumentItem<string> => ({
      id: item.id,
      title: item.title,
      detail: item.detail,
      value: ((item.command?.args as Record<string, unknown> | undefined)?.[args.argumentName] as string | undefined) ?? join(args.root, item.title.replace(/\/$/, "")),
      keywords: item.keywords,
      icon: item.icon,
      frame: item.frame,
      query: item.query,
    }));
}

function pathPaletteItems(_editor: Parameters<EditorModule>[0], query: string, args: PathPaletteFrameArgs): CommandPaletteItem[] {
  const parsed = parsePathQuery(query);
  const currentRoot = join(args.root, parsed.dir);
  const entries = listDirectoryEntries(currentRoot)
    .filter((entry) => args.mode === "both" || (args.mode === "file" ? !entry.isDirectory : entry.isDirectory))
    .map((entry): CommandPaletteItem => {
      const absolute = join(currentRoot, entry.name);
      const relativePath = relative(args.baseRoot, absolute) || entry.name;
      const displayTitle = entry.isDirectory ? `${entry.name}/` : entry.name;
      const keywords = [entry.name, relativePath];
      if (entry.isDirectory && args.mode !== "directory") {
        return {
          id: `path:${relativePath}`,
          title: displayTitle,
          detail: relativePath,
          group: "Open File",
          keywords,
          icon: { kind: "file", path: absolute, entryKind: "directory" },
          query: `${parsed.dir}${entry.name}/`,
        };
      }
      return {
        id: `path:${relativePath}`,
        title: displayTitle,
        detail: relativePath,
        group: args.mode === "directory" ? "Directories" : "Open File",
        keywords,
        icon: { kind: "file", path: absolute, entryKind: entry.isDirectory ? "directory" : "file" },
        command: { id: commandId(args.commandId), args: { [args.argumentName]: absolute } },
      };
    });
  return entries.filter((item) => matchesCommandPaletteItem(parsed.leaf, item));
}

function parsePathQuery(query: string): { dir: string; leaf: string } {
  const normalized = query.replace(/\\+/g, "/");
  const slash = normalized.lastIndexOf("/");
  if (slash < 0) return { dir: "", leaf: normalized };
  return { dir: normalized.slice(0, slash + 1), leaf: normalized.slice(slash + 1) };
}

function listDirectoryEntries(root: string): Array<{ name: string; isDirectory: boolean }> {
  let entries: string[];
  try {
    entries = readdirSync(root);
  } catch {
    return [];
  }
  return entries
    .map((name) => {
      try {
        return { name, isDirectory: statSync(join(root, name)).isDirectory() };
      } catch {
        return undefined;
      }
    })
    .filter((entry): entry is { name: string; isDirectory: boolean } => Boolean(entry))
    .sort((a, b) => Number(b.isDirectory) - Number(a.isDirectory) || a.name.localeCompare(b.name));
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
