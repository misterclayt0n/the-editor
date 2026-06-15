import type { ResolvedTheme } from "@the-editor/theme";
import type { ITerminalOptions } from "@xterm/xterm";
import { terminalTheme } from "../theme/dom";

export type TerminalKeyHandlerResult = "process" | "process-prevent-default" | "skip";

export interface TerminalKeyboardEventLike {
  readonly key: string;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
}


export const TERMINAL_FONT_FAMILY = '"SF Mono", Menlo, Monaco, "Cascadia Code", monospace';

export function createTerminalOptions(theme: ResolvedTheme): ITerminalOptions {
  return {
    allowProposedApi: true,
    fontSize: 12.5,
    fontFamily: TERMINAL_FONT_FAMILY,
    cursorBlink: true,
    macOptionIsMeta: true,
    theme: terminalTheme(theme),
    vtExtensions: {
      kittyKeyboard: true,
    },
  } satisfies ITerminalOptions;
}

/**
 * macOS "natural text editing": Cmd/Option line- and word-motions that every
 * native terminal (Terminal.app, iTerm) maps to readline control bytes. xterm
 * sends nothing for some of these, so without this the terminal feels dead to
 * ⌘⌫ etc. Returns the bytes to write to the PTY, or null to leave the key.
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
