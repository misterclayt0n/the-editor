// The desktop bridge the Electron host injects on `window`. Absent in a plain
// browser. Kept here so the one structural assertion lives in a single place.
const bridge =
  typeof window !== "undefined"
    ? (window as unknown as { theEditorDesktop?: { platform: string } })
    : undefined;

/** macOS Electron host (hiddenInset title bar): reserve traffic-light space. */
export const isMacDesktop = bridge?.theEditorDesktop?.platform === "darwin";
