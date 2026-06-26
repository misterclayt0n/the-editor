import { describe, expect, test } from "vitest";
import { detectTerminalShortcutPlatform, terminalShortcutAction, type TerminalShortcutEventLike } from "./shortcuts";

const key = (overrides: Partial<TerminalShortcutEventLike>): TerminalShortcutEventLike => ({
  key: "",
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...overrides,
});

describe("terminalShortcutAction", () => {
  test("uses native macOS terminal shortcuts", () => {
    expect(terminalShortcutAction(key({ key: "c", metaKey: true }), "darwin")).toBe("copy");
    expect(terminalShortcutAction(key({ key: "v", metaKey: true }), "darwin")).toBe("paste");
    expect(terminalShortcutAction(key({ key: "k", metaKey: true }), "darwin")).toBe("clear");
    expect(terminalShortcutAction(key({ key: "a", metaKey: true }), "darwin")).toBe("select-all");
  });

  test("uses Linux and Windows terminal copy/paste chords without stealing Ctrl+C", () => {
    expect(terminalShortcutAction(key({ key: "c", ctrlKey: true }), "linux")).toBeNull();
    expect(terminalShortcutAction(key({ key: "c", ctrlKey: true, shiftKey: true }), "linux")).toBe("copy");
    expect(terminalShortcutAction(key({ key: "v", ctrlKey: true }), "linux")).toBe("paste");
    expect(terminalShortcutAction(key({ key: "Insert", shiftKey: true }), "win32")).toBe("paste");
  });
});

describe("detectTerminalShortcutPlatform", () => {
  test("maps common user agents to terminal shortcut platforms", () => {
    expect(detectTerminalShortcutPlatform("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)")).toBe("darwin");
    expect(detectTerminalShortcutPlatform("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe("win32");
    expect(detectTerminalShortcutPlatform("Mozilla/5.0 (X11; Linux x86_64)")).toBe("linux");
  });
});
