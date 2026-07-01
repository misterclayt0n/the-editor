/**
 * The OS home directory, when the Electron bridge provides one. Anchors
 * `~/.the-editor` (global extensions + user config): capability-gated like
 * every other port — no bridge, no home directory, no global extensions
 * (project-local ones still work off the already-picked workspace root).
 */
import { homeDir as bridgeHomeDir } from "./desktop.js";

export function getHomeDir(): string | undefined {
  return bridgeHomeDir;
}

/** Join a home-relative path without assuming a path separator convention
 *  beyond POSIX (Electron here targets macOS/Linux paths; Windows joins the
 *  same way since forward slashes are broadly accepted). */
export function homePath(...segments: readonly string[]): string | undefined {
  const home = getHomeDir();
  if (home === undefined) return undefined;
  const base = home.replace(/\/+$/, "");
  return [base, ...segments].join("/");
}
