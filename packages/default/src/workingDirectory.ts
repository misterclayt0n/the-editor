import { existsSync, realpathSync, statSync } from "node:fs";
import os from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { commandId, roleImplementationId, type EditorHost, type EditorModule } from "@the-editor/editor";

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
      title: "Change Working Directory",
      aliases: ["cd"],
      category: "Workspace",
      palette: "topLevel",
      icon: { kind: "lucide", name: "folder-open" },
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
      title: "Show Current Directory",
      aliases: ["pwd"],
      category: "Workspace",
      palette: "topLevel",
      icon: { kind: "lucide", name: "folder" },
      run() {
        const cwd = editor.cwd.current();
        const message = `current working directory: ${cwd}`;
        if (existsSync(cwd)) editor.notifications.info("cwd", message);
        else editor.notifications.error("cwd", `${message} (deleted)`);
      },
    });

  };
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
