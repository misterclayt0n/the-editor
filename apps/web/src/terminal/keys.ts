import type { ResolvedTheme } from "@the-editor/theme";
import { resolveFontSystem, FONT_SLOT_TERMINAL, type ResolvedFontSystem } from "@the-editor/fonts";
import { terminalTheme } from "../theme/dom";

/**
 * Engine-neutral terminal options: a superset both engines accept. xterm.js
 * reads all of them; ghostty-web ignores the xterm-specific ones (kitty
 * keyboard state lives in the Ghostty emulator itself, and macOS "Option as
 * Meta" is wired to its key encoder by the backend).
 */
export type TerminalOptions = {
  readonly allowProposedApi?: boolean;
  readonly fontSize?: number;
  readonly fontFamily?: string;
  readonly macOptionIsMeta?: boolean;
  readonly theme?: Readonly<Record<string, string>>;
  readonly vtExtensions?: { readonly kittyKeyboard?: boolean };
};

export type TerminalKeyHandlerResult = "process" | "process-prevent-default" | "skip";

export interface TerminalKeyboardEventLike {
  readonly key: string;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
}

const defaultFontSystem = resolveFontSystem([], []);

export function createTerminalOptions(
  theme: ResolvedTheme,
  fonts: ResolvedFontSystem = defaultFontSystem,
): TerminalOptions {
  const terminalFont = fonts.slots[FONT_SLOT_TERMINAL] ?? defaultFontSystem.slots[FONT_SLOT_TERMINAL]!;
  return {
    allowProposedApi: true,
    fontSize: terminalFont.size,
    fontFamily: terminalFont.cssFamily,
    macOptionIsMeta: true,
    theme: terminalTheme(theme),
    vtExtensions: {
      kittyKeyboard: true,
    },
  } satisfies TerminalOptions;
}

/**
 * macOS "natural text editing": Cmd/Option line- and word-motions that every
 * native terminal (Terminal.app, iTerm, Ghostty) maps to readline control
 * bytes. The browser key encoder sends nothing for some of these, so without
 * this the terminal feels dead to ⌘⌫ etc. Returns the bytes to write to the
 * PTY, or null to leave the key.
 */
export function macEditingSequence(event: TerminalKeyboardEventLike): string | null {
  if (event.ctrlKey || event.shiftKey) return null;
  if (event.metaKey && !event.altKey) {
    switch (event.key) {
      case "Backspace":
        return "\x15"; // Ctrl+U — delete to start of line
      case "ArrowLeft":
        return "\x01"; // Ctrl+A — start of line
      case "ArrowRight":
        return "\x05"; // Ctrl+E — end of line
      default:
        return null;
    }
  }
  if (event.altKey && !event.metaKey) {
    switch (event.key) {
      case "Backspace":
        return "\x17"; // Ctrl+W — delete word backward
      case "ArrowLeft":
        return "\x1bb"; // Esc b — word backward
      case "ArrowRight":
        return "\x1bf"; // Esc f — word forward
      case "Delete":
        return "\x1bd"; // Esc d — delete word forward
      default:
        return null;
    }
  }
  return null;
}

export function terminalKeyHandlerResult(event: TerminalKeyboardEventLike): TerminalKeyHandlerResult {
  if (event.key === "Tab" && event.shiftKey) return "process-prevent-default";
  return "process";
}
