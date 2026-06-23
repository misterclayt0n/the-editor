import { describe, expect, test } from "vitest";
import { pierreThemeFamily, resolveTheme, type ThemeContent } from "@the-editor/theme";
import { resolveFontSystem } from "@the-editor/fonts";
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

const resolvedTheme = resolveTheme(
  { name: pierreThemeFamily.name, author: pierreThemeFamily.author },
  dark,
  dark,
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
      theme?: { background?: string };
    };

    expect(options.allowProposedApi).toBe(true);
    expect(options.macOptionIsMeta).toBe(true);
    expect(options.vtExtensions?.kittyKeyboard).toBe(true);
    expect(options.theme?.background).toBe("#171717");
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

