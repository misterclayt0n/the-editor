export type TerminalShortcutPlatform = "darwin" | "linux" | "win32";

export type TerminalShortcutAction = "copy" | "paste" | "clear" | "select-all";

export interface TerminalShortcutEventLike {
  readonly key: string;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
}

export const detectTerminalShortcutPlatform = (
  userAgent = navigator.userAgent,
): TerminalShortcutPlatform => {
  if (/\bMacintosh\b|\bMac OS\b|\bMac\b/i.test(userAgent)) return "darwin";
  if (/\bWindows\b|\bWin64\b|\bWin32\b/i.test(userAgent)) return "win32";
  return "linux";
};

export function terminalShortcutAction(
  event: TerminalShortcutEventLike,
  platform: TerminalShortcutPlatform = detectTerminalShortcutPlatform(),
): TerminalShortcutAction | null {
  const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
  const isMac = platform === "darwin";

  if (isMac) {
    if (event.metaKey && !event.ctrlKey && !event.altKey) {
      if (key === "c") return "copy";
      if (key === "v") return "paste";
      if (key === "k" && !event.shiftKey) return "clear";
      if (key === "a" && !event.shiftKey) return "select-all";
    }
    return null;
  }

  if (event.key === "Insert" && event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey) {
    return "paste";
  }
  if (!event.ctrlKey || event.metaKey || event.altKey) return null;
  if (key === "v") return "paste";
  if (key === "c" && event.shiftKey) return "copy";
  if (key === "k" && event.shiftKey) return "clear";
  if (key === "a" && event.shiftKey) return "select-all";
  return null;
}
