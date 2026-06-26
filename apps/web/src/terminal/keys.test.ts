import { describe, expect, test } from "vitest";
import { pierreThemeFamily, resolveTheme, type ThemeContent } from "@the-editor/theme";
import { resolveFontSystem } from "@the-editor/fonts";
import { terminalSearchDecorations } from "../theme/dom";
import { createTerminalOptions, macEditingSequence, terminalKeyHandlerResult, type TerminalKeyboardEventLike } from "./keys";

const key = (overrides: Partial<TerminalKeyboardEventLike>): TerminalKeyboardEventLike => ({
  key: "",
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...overrides,
});

const dark = pierreThemeFamily.themes.find((theme): theme is ThemeContent => theme.appearance === "dark");
if (dark === undefined) throw new Error("Missing bundled dark theme");
const light = pierreThemeFamily.themes.find((theme): theme is ThemeContent => theme.appearance === "light");
if (light === undefined) throw new Error("Missing bundled light theme");

const resolvedTheme = resolveTheme(
  { name: pierreThemeFamily.name, author: pierreThemeFamily.author },
  dark,
  dark,
);
const resolvedLightTheme = resolveTheme(
  { name: pierreThemeFamily.name, author: pierreThemeFamily.author },
  light,
  light,
);

describe("macEditingSequence", () => {
  test("maps native macOS readline chords", () => {
    expect(macEditingSequence(key({ key: "Backspace", metaKey: true }))).toBe("\x15");
    expect(macEditingSequence(key({ key: "ArrowLeft", metaKey: true }))).toBe("\x01");
    expect(macEditingSequence(key({ key: "ArrowRight", metaKey: true }))).toBe("\x05");
    expect(macEditingSequence(key({ key: "Backspace", altKey: true }))).toBe("\x17");
    expect(macEditingSequence(key({ key: "ArrowLeft", altKey: true }))).toBe("\x1bb");
    expect(macEditingSequence(key({ key: "ArrowRight", altKey: true }))).toBe("\x1bf");
    expect(macEditingSequence(key({ key: "Delete", altKey: true }))).toBe("\x1bd");
  });

  test("does not override shifted or controlled keys", () => {
    expect(macEditingSequence(key({ key: "Enter", shiftKey: true }))).toBeNull();
    expect(macEditingSequence(key({ key: "ArrowLeft", altKey: true, shiftKey: true }))).toBeNull();
    expect(macEditingSequence(key({ key: "ArrowRight", metaKey: true, ctrlKey: true }))).toBeNull();
  });
});

describe("terminalKeyHandlerResult", () => {
  test("keeps shift tab inside the terminal", () => {
    expect(terminalKeyHandlerResult(key({ key: "Tab", shiftKey: true }))).toBe("process-prevent-default");
    expect(terminalKeyHandlerResult(key({ key: "Tab" }))).toBe("process");
  });
});

describe("createTerminalOptions", () => {
  test("enables extended keyboard reporting in xterm", () => {
    const options = createTerminalOptions(resolvedTheme) as {
      allowProposedApi?: boolean;
      macOptionIsMeta?: boolean;
      vtExtensions?: { kittyKeyboard?: boolean };
      theme?: Record<string, string>;
    };

    expect(options.allowProposedApi).toBe(true);
    expect(options.macOptionIsMeta).toBe(true);
    expect(options.vtExtensions?.kittyKeyboard).toBe(true);
    expect(options.theme?.background).toBe("#0a0a0a");
    expect(options.theme?.ansiBackground).toBe("#0a0a0a");
    expect(options.theme?.surfaceBackground).toBe("#0a0a0a");
    expect(options.theme?.cursorAccent).toBe("#0a0a0a");
    expect(options.theme?.selectionBackground).toBe("#009fff40");
    expect(options.theme?.selectionInactiveBackground).toBe("#009fff40");
    expect(options.theme?.brightForeground).toBe("#fafafa");
    expect(options.theme?.dimForeground).toBe("#737373");
    expect(options.theme?.dimGreen).toBe("#3e8f50");
  });

  test("uses the editor selection color in light terminal themes", () => {
    const options = createTerminalOptions(resolvedLightTheme) as { theme?: Record<string, string> };

    expect(options.theme?.background).toBe("#ffffff");
    expect(options.theme?.selectionBackground).toBe("#009fff40");
    expect(options.theme?.selectionInactiveBackground).toBe("#009fff40");
  });

  test("uses ANSI background for xterm cells and terminal background for the surface", () => {
    const theme = {
      ...resolvedTheme,
      colors: {
        ...resolvedTheme.colors,
        "terminal.background": "#111111",
        "terminal.ansi.background": "#222222",
      },
    };
    const options = createTerminalOptions(theme) as { theme?: Record<string, string> };

    expect(options.theme?.surfaceBackground).toBe("#111111");
    expect(options.theme?.ansiBackground).toBe("#222222");
    expect(options.theme?.background).toBe("#222222");
    expect(options.theme?.cursorAccent).toBe("#222222");
  });

  test("composites terminal search decorations over the ANSI background", () => {
    const decorations = terminalSearchDecorations({
      ...resolvedTheme,
      colors: {
        ...resolvedTheme.colors,
        "terminal.ansi.background": "#000000",
        "search.match_background": "#ffffff80",
      },
      players: [{ ...resolvedTheme.players[0]!, selection: "#0000ff80" }],
    });

    expect(decorations.matchBackground).toBe("#808080");
    expect(decorations.activeMatchBackground).toBe("#000080");
    expect(decorations.matchOverviewRuler).toBe("#808080");
    expect(decorations.activeMatchColorOverviewRuler).toBe("#000080");
  });

  test("uses resolved terminal font slot", () => {
    const fonts = resolveFontSystem([], [
      {
        id: "test/terminal-font",
        slots: {
          buffer: { family: "Iosevka", fallbacks: ["monospace"], size: 14 },
          terminal: { extends: "buffer", size: 13 },
        },
      },
    ]);
    const options = createTerminalOptions(resolvedTheme, fonts) as {
      fontFamily?: string;
      fontSize?: number;
    };

    expect(options.fontFamily).toBe("Iosevka, monospace");
    expect(options.fontSize).toBe(13);
  });
});
