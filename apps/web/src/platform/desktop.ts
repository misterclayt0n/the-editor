// The desktop bridge the Electron host injects on `window`. Absent in a plain
// browser. Kept here so the one structural assertion lives in a single place.
const bridge =
  typeof window !== "undefined"
    ? (window as unknown as {
        theEditorDesktop?: { platform: string; homeDir?: string };
      })
    : undefined;

/** macOS Electron host (hiddenInset title bar): reserve traffic-light space. */
export const isMacDesktop = bridge?.theEditorDesktop?.platform === "darwin";

/** Any Electron desktop host (frameless): worth a window-drag handle in chrome. */
export const isDesktop = bridge?.theEditorDesktop !== undefined;

/** The OS home directory, when the Electron bridge provides one. Anchors the
 *  global `~/.the-editor` extension/config directory; absent in a plain
 *  browser (no notion of a home directory to read without a picker). */
export const homeDir = bridge?.theEditorDesktop?.homeDir;
