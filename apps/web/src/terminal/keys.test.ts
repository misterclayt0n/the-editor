import { describe, expect, test } from "vitest";
import { oneThemeFamily, resolveTheme, type ThemeContent } from "@the-editor/theme";
import { createTerminalOptions, macEditingSequence, terminalKeyHandlerResult, type TerminalKeyboardEventLike } from "./keys";
import { ghosttyUnsupportedQueryResponses } from "./queryResponses";

const key = (overrides: Partial<TerminalKeyboardEventLike>): TerminalKeyboardEventLike => ({
  key: "",
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...overrides,
});

const dark = oneThemeFamily.themes.find((theme): theme is ThemeContent => theme.appearance === "dark");
if (dark === undefined) throw new Error("Missing bundled dark theme");

const resolvedTheme = resolveTheme(
  { name: oneThemeFamily.name, author: oneThemeFamily.author },
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
    expect(options.theme?.background).toBe("#282c34");
  });
});

describe("ghosttyUnsupportedQueryResponses", () => {
  const esc = "\x1b";
  const st = `${esc}\\`;

  test("answers terminal identity and color probes that ghostty-web does not answer", () => {
    const responses = ghosttyUnsupportedQueryResponses(
      `${esc}[c${esc}[>c${esc}]10;?${st}${esc}]11;?${st}`,
      { foreground: "#abb2bf", background: "#282c34" },
    );

    expect(responses).toContain(`${esc}[?1;2c`);
    expect(responses).toContain(`${esc}[>0;100;0c`);
    expect(responses).toContain(`${esc}]10;rgb:abab/b2b2/bfbf${st}`);
    expect(responses).toContain(`${esc}]11;rgb:2828/2c2c/3434${st}`);
  });
});
