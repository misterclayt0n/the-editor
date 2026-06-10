import { describe, expect, test } from "vitest";
import {
  chordFromKeyboardEvent,
  chordKey,
  formatChords,
  InvalidKeysError,
  isTextInputChord,
  parseChord,
  parseKeys,
} from "../src/index";

describe("chord parsing", () => {
  test("single keys and modifiers", () => {
    expect(parseChord("a")).toEqual({
      key: "a",
      ctrl: false,
      alt: false,
      shift: false,
      meta: false,
    });
    expect(parseChord("ctrl+s")).toMatchObject({ key: "s", ctrl: true });
    expect(parseChord("cmd+shift+p")).toMatchObject({ key: "p", meta: true, shift: true });
    expect(parseChord("Ctrl+W")).toMatchObject({ key: "w", ctrl: true });
  });

  test("named keys and aliases", () => {
    expect(parseChord("esc").key).toBe("escape");
    expect(parseChord("return").key).toBe("enter");
    expect(parseChord("space").key).toBe(" ");
    expect(parseChord("ctrl+plus").key).toBe("+");
    expect(parseChord("left").key).toBe("arrowleft");
    expect(parseChord("ArrowDown").key).toBe("arrowdown");
  });

  test("sequences split on whitespace", () => {
    const chords = parseKeys("ctrl+w  h");
    expect(chords).toHaveLength(2);
    expect(chordKey(chords[0]!)).toBe("ctrl+w");
    expect(chordKey(chords[1]!)).toBe("h");
  });

  test("invalid chords are rejected", () => {
    expect(() => parseChord("")).toThrow(InvalidKeysError);
    expect(() => parseChord("ctrl+")).toThrow(InvalidKeysError);
    expect(() => parseChord("ctrl")).toThrow(InvalidKeysError);
    expect(() => parseChord("wat+x")).toThrow(InvalidKeysError);
    expect(() => parseKeys("   ")).toThrow(InvalidKeysError);
  });

  test("formatChords round-trips readably", () => {
    expect(formatChords(parseKeys("ctrl+w h"))).toBe("ctrl+w h");
    expect(formatChords(parseKeys("ctrl+space"))).toBe("ctrl+space");
  });
});

describe("keyboard events", () => {
  const event = (
    key: string,
    mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {},
  ) => ({
    key,
    ctrlKey: mods.ctrlKey ?? false,
    altKey: mods.altKey ?? false,
    shiftKey: mods.shiftKey ?? false,
    metaKey: mods.metaKey ?? false,
  });

  test("maps DOM events to chords", () => {
    expect(chordFromKeyboardEvent(event("H", { shiftKey: true }))).toMatchObject({
      key: "h",
      shift: true,
    });
    expect(chordFromKeyboardEvent(event("ArrowLeft"))).toMatchObject({ key: "arrowleft" });
    expect(chordFromKeyboardEvent(event("Enter"))).toMatchObject({ key: "enter" });
  });

  test("modifier-only keydowns are not chords", () => {
    expect(chordFromKeyboardEvent(event("Shift", { shiftKey: true }))).toBeUndefined();
    expect(chordFromKeyboardEvent(event("Control", { ctrlKey: true }))).toBeUndefined();
  });

  test("matches bindings: shift+h from keyboard equals parsed shift+h", () => {
    const fromEvent = chordFromKeyboardEvent(event("H", { shiftKey: true }))!;
    expect(chordKey(fromEvent)).toBe(chordKey(parseChord("shift+h")));
  });

  test("isTextInputChord marks unmodified printables", () => {
    expect(isTextInputChord(chordFromKeyboardEvent(event("a"))!)).toBe(true);
    expect(isTextInputChord(chordFromKeyboardEvent(event("?", { shiftKey: true }))!)).toBe(true);
    expect(isTextInputChord(chordFromKeyboardEvent(event("a", { ctrlKey: true }))!)).toBe(false);
    expect(isTextInputChord(chordFromKeyboardEvent(event("Enter"))!)).toBe(false);
  });
});
