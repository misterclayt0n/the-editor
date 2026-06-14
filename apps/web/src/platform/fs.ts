/**
 * FileSystemPort adapters, chosen by environment:
 *
 * 1. A test-injected port (`window.__testFsPort`, dev only) so e2e tests can
 *    drive the whole file pipeline deterministically.
 * 2. The Electron preload bridge (`window.theEditorFs`) — native fs + fs.watch
 *    in the main process. The full experience.
 * 3. The File System Access API (Chromium browsers) — real files via picker
 *    handles; "paths" are synthetic handle keys, and watching is mtime
 *    polling (the API has no watcher).
 *
 * Returns undefined when no adapter applies (the workspace then has no
 * Files service: capability gating, not stubs).
 */

import { Effect, Option } from "effect";
import {
  FileIoError,
  type DirEntry,
  type FileEvent,
  type FileStat,
  type FileSystemPort,
} from "@the-editor/editor";

type Bridge = {
  readFile(path: string): Promise<string>;
  writeFile(path: string, text: string): Promise<{ mtimeMs: number; size: number }>;
  stat(path: string): Promise<{ mtimeMs: number; size: number } | null>;
  watch(path: string, onEvent: (event: { path: string; kind: string }) => void): Promise<number>;
  unwatch(id: number): Promise<void>;
  pickFile(): Promise<string | null>;
  pickFolder?(): Promise<string | null>;
  readDir?(path: string): Promise<Array<{ name: string; kind: "file" | "directory" }>>;
  mkdir?(path: string): Promise<void>;
  watchDir?(
    path: string,
    onEvent: (event: { path: string; kind: string }) => void,
  ): Promise<number>;
};

const io = <A>(
  path: string,
  operation: "read" | "write" | "stat" | "watch" | "pick",
  run: () => Promise<A>,
): Effect.Effect<A, FileIoError> =>
  Effect.tryPromise({
    try: run,
    catch: (cause) => new FileIoError(path, operation, cause),
  });

function bridgePort(bridge: Bridge): FileSystemPort.Service {
  return {
    readFile: (path) => io(path, "read", () => bridge.readFile(path)),
    writeFile: (path, text) => io(path, "write", () => bridge.writeFile(path, text)),
    stat: (path) =>
      io(path, "stat", () => bridge.stat(path)).pipe(Effect.map(Option.fromNullable)),
    watch: (path, onEvent) =>
      io(path, "watch", async () => {
        const id = await bridge.watch(path, (event) =>
          onEvent({ path: event.path, kind: event.kind === "deleted" ? "deleted" : "changed" }),
        );
        return () => void bridge.unwatch(id);
      }),
    pickFile: () => io("", "pick", () => bridge.pickFile()).pipe(Effect.map(Option.fromNullable)),
    ...(bridge.pickFolder !== undefined
      ? {
          pickFolder: () =>
            io("", "pick", () => bridge.pickFolder!()).pipe(Effect.map(Option.fromNullable)),
        }
      : {}),
    ...(bridge.readDir !== undefined
      ? {
          readDir: (path: string): ReturnType<NonNullable<FileSystemPort.Service["readDir"]>> =>
            io(path, "read", () => bridge.readDir!(path)) as ReturnType<
              NonNullable<FileSystemPort.Service["readDir"]>
            >,
        }
      : {}),
    ...(bridge.mkdir !== undefined
      ? { mkdir: (path: string) => io(path, "write", () => bridge.mkdir!(path)).pipe(Effect.asVoid) }
      : {}),
    ...(bridge.watchDir !== undefined
      ? {
          watchDir: (path: string, onEvent: (event: FileEvent) => void) =>
            io(path, "watch", async () => {
              const id = await bridge.watchDir!(path, (event) =>
                onEvent({
                  path: event.path,
                  kind: event.kind === "deleted" ? "deleted" : "changed",
                }),
              );
              return () => void bridge.unwatch(id);
            }),
        }
      : {}),
    capabilities: { watch: true },
  };
}

const POLL_MS = 1500;

/** File System Access fallback: picker handles + mtime polling. */
function fsAccessPort(): FileSystemPort.Service {
  const handles = new Map<string, FileSystemFileHandle>();
  let nextHandle = 1;

  const handleFor = (path: string): Effect.Effect<FileSystemFileHandle, FileIoError> => {
    const handle = handles.get(path);
    return handle !== undefined
      ? Effect.succeed(handle)
      : Effect.fail(new FileIoError(path, "read", "unknown handle"));
  };

  return {
    readFile: (path) =>
      handleFor(path).pipe(
        Effect.flatMap((handle) =>
          io(path, "read", async () => {
            const file = await handle.getFile();
            return file.text();
          }),
        ),
      ),
    writeFile: (path, text) =>
      handleFor(path).pipe(
        Effect.flatMap((handle) =>
          io(path, "write", async (): Promise<FileStat> => {
            const writable = await handle.createWritable();
            await writable.write(text);
            await writable.close();
            const file = await handle.getFile();
            return { mtimeMs: file.lastModified, size: file.size };
          }),
        ),
      ),
    stat: (path) =>
      handleFor(path).pipe(
        Effect.flatMap((handle) =>
          io(path, "stat", async () => {
            const file = await handle.getFile();
            return Option.some({ mtimeMs: file.lastModified, size: file.size });
          }),
        ),
        Effect.orElseSucceed(() => Option.none<FileStat>()),
      ),
    watch: (path, onEvent) =>
      Effect.sync(() => {
        let lastMtime = 0;
        const timer = setInterval(async () => {
          const handle = handles.get(path);
          if (handle === undefined) return;
          try {
            const file = await handle.getFile();
            if (lastMtime !== 0 && file.lastModified > lastMtime) {
              onEvent({ path, kind: "changed" } satisfies FileEvent);
            }
            lastMtime = file.lastModified;
          } catch {
            onEvent({ path, kind: "deleted" } satisfies FileEvent);
          }
        }, POLL_MS);
        return () => clearInterval(timer);
      }),
    pickFile: () =>
      io("", "pick", async () => {
        const picker = (
          window as unknown as {
            showOpenFilePicker?: () => Promise<FileSystemFileHandle[]>;
          }
        ).showOpenFilePicker;
        if (picker === undefined) return null;
        try {
          const [handle] = await picker();
          if (handle === undefined) return null;
          const path = `picked:${nextHandle++}:${handle.name}`;
          handles.set(path, handle);
          return path;
        } catch {
          return null; // user cancelled
        }
      }).pipe(Effect.map(Option.fromNullable)),
    capabilities: { watch: true }, // via polling
  };
}

export function createFileSystemPort(): FileSystemPort.Service | undefined {
  const w = window as unknown as Record<string, unknown>;
  const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
  if (isDev && w.__testFsPort !== undefined) {
    // Test ports speak the promise Bridge protocol (injected via init script).
    return bridgePort(w.__testFsPort as Bridge);
  }
  if (w.theEditorFs !== undefined) {
    return bridgePort(w.theEditorFs as Bridge);
  }
  if ("showOpenFilePicker" in window) {
    return fsAccessPort();
  }
  return undefined;
}
