import { existsSync, realpathSync, statSync } from "node:fs";
import os from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { commandId, matchesCommandPaletteItem, roleImplementationId, type CommandPaletteItem, type EditorHost, type EditorModule } from "@the-editor/editor";

interface NotificationsRoleState {
  items: ReturnType<EditorHost["notifications"]["snapshot"]>;
}

interface ChangeWorkingDirectoryArgs {
  path?: string;
}

const notificationsImplementationId = roleImplementationId("default.notifications");

export function defaultWorkingDirectory(): EditorModule {
  return (editor) => {
    editor.roles.setFallback("notifications", notificationsImplementationId);

    editor.roles.register<NotificationsRoleState>({
      role: "notifications",
      implementationId: notificationsImplementationId,
      title: "Default Notifications",
      snapshot() {
        return { items: editor.notifications.snapshot() };
      },
    });

    editor.commands.register<ChangeWorkingDirectoryArgs | undefined, void>({
      id: commandId("editor.changeWorkingDirectory"),
      title: ":cd",
      category: "Workbench",
      hidden: true,
      async run(_ctx, args) {
        const requested = args?.path;
        const dir = requested === "-"
          ? editor.cwd.previous() ?? missingPreviousDirectory()
          : requested ? resolveDirectoryCommandPath(editor, requested) : homeDirectory();
        let cwd: string;
        try {
          cwd = realpathSync(dir);
        } catch (error) {
          throw new Error(`failed to change directory to '${dir}': ${error instanceof Error ? error.message : String(error)}`);
        }
        editor.cwd.setCurrent(cwd);
        editor.notifications.info("cwd", `current working directory: ${cwd}`);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("editor.showWorkingDirectory"),
      title: ":pwd",
      category: "Workbench",
      hidden: true,
      run() {
        const cwd = editor.cwd.current();
        const message = `current working directory: ${cwd}`;
        if (existsSync(cwd)) editor.notifications.info("cwd", message);
        else editor.notifications.error("cwd", `${message} (deleted)`);
      },
    });

    editor.palette.registerProvider({
      id: "default.cwd",
      title: "Working Directory",
      priority: 100,
      provide(ctx, query) {
        const trimmed = query.trim();
        const items: CommandPaletteItem[] = [];

        if (matchesPwdQuery(trimmed)) {
          const cwd = ctx.editor.cwd.current();
          items.push({
            id: "cwd:pwd",
            title: ":pwd",
            detail: cwd,
            group: "Working Directory",
            keywords: [":pwd", "pwd", "cwd", cwd],
            command: { id: commandId("editor.showWorkingDirectory") },
          });
        }

        if (matchesCdQuery(trimmed)) {
          const path = cdQueryPath(trimmed);
          const detail = describeChangeDirectoryTarget(ctx.editor, path);
          items.push({
            id: `cwd:cd:${path ?? "default"}`,
            title: path ? `:cd ${path}` : ":cd",
            detail,
            group: "Working Directory",
            keywords: [":cd", "cd", "cwd", path ?? "", ctx.editor.cwd.current()],
            command: path
              ? { id: commandId("editor.changeWorkingDirectory"), args: { path } }
              : { id: commandId("editor.changeWorkingDirectory") },
          });
        }

        return items.filter((item) => matchesCommandPaletteItem(query, item));
      },
    });
  };
}

function matchesPwdQuery(query: string): boolean {
  if (!query) return false;
  return query === ":pwd"
    || query === "pwd"
    || ":pwd".startsWith(query)
    || "pwd".startsWith(query);
}

function matchesCdQuery(query: string): boolean {
  if (!query) return false;
  return query === ":cd"
    || query === "cd"
    || query.startsWith(":cd ")
    || query.startsWith("cd ")
    || ":cd".startsWith(query)
    || "cd".startsWith(query);
}

function cdQueryPath(query: string): string | undefined {
  if (query.startsWith(":cd ")) return query.slice(4).trim() || undefined;
  if (query.startsWith("cd ")) return query.slice(3).trim() || undefined;
  return undefined;
}

function describeChangeDirectoryTarget(editor: EditorHost, path: string | undefined): string {
  try {
    if (path === "-") {
      const previous = editor.cwd.previous();
      return previous ? `Previous: ${previous}` : "No previous working directory";
    }
    const resolved = path ? resolveDirectoryCommandPath(editor, path) : homeDirectory();
    return `Change current working directory to ${resolved}`;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

function resolveDirectoryCommandPath(editor: EditorHost, input: string): string {
  const expanded = expandTilde(input);
  if (isAbsolute(expanded)) return expanded;
  return resolve(editor.cwd.current(), expanded);
}

function homeDirectory(): string {
  const home = expandTilde("~");
  if (home === "~") throw new Error("home directory is not available");
  return home;
}

function missingPreviousDirectory(): never {
  throw new Error("no previous working directory");
}

function expandTilde(input: string): string {
  const home = os.homedir();
  if (!home) return input;
  if (input === "~") return home;
  if (input.startsWith("~/") || input.startsWith("~\\")) return join(home, input.slice(2));
  return input;
}

export function resolveFilePathFromWorkingDirectory(editor: EditorHost, input: string): string {
  const expanded = expandTilde(input);
  if (isAbsolute(expanded)) return expanded;
  return resolve(editor.cwd.current(), expanded);
}

export function resolveFilePickerRoot(editor: EditorHost): string {
  return findProjectRoot(editor.cwd.current()) ?? editor.cwd.current();
}

function findProjectRoot(start: string): string | undefined {
  let current = existingDirectory(start) ?? start;
  let found: string | undefined;
  while (true) {
    if (hasProjectMarker(current)) found = current;
    const parent = resolve(current, "..");
    if (parent === current) return found;
    current = parent;
  }
}

function hasProjectMarker(dir: string): boolean {
  return [".git", "package.json", "Cargo.toml", ".the-editor"].some((name) => existsSync(join(dir, name)));
}

function existingDirectory(path: string): string | undefined {
  try {
    if (!existsSync(path)) return undefined;
    return statSync(path).isDirectory() ? path : undefined;
  } catch {
    return undefined;
  }
}
