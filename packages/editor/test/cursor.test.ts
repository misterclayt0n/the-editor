import { describe, expect, test } from "vitest";
import { DEFAULT_CURSOR_STYLE, makeViewCursor } from "../src/cursor";

describe("ViewCursor", () => {
  test("starts with the default bar cursor", () => {
    const cursor = makeViewCursor();
    expect(cursor.getStyle()).toEqual(DEFAULT_CURSOR_STYLE);
  });

  test("normalizes blink to true when a style omits it", () => {
    const cursor = makeViewCursor();
    cursor.setStyle({ shape: "block" });
    expect(cursor.getStyle()).toEqual({ shape: "block", blink: true });
  });

  test("notifies subscribers on style and effect changes", () => {
    const cursor = makeViewCursor();
    let calls = 0;
    const unsubscribe = cursor.subscribe(() => calls++);
    cursor.setStyle({ shape: "underline", blink: false });
    cursor.spawnEffect({ className: "boom" });
    unsubscribe();
    cursor.setStyle({ shape: "bar" });
    expect(calls).toBe(2);
  });

  test("drains effects exactly once", () => {
    const cursor = makeViewCursor();
    cursor.spawnEffect({ className: "a" });
    cursor.spawnEffect({ className: "b" });
    expect(cursor.drainEffects().map((effect) => effect.className)).toEqual(["a", "b"]);
    expect(cursor.drainEffects()).toEqual([]);
  });

  test("reset restores the default style", () => {
    const cursor = makeViewCursor();
    cursor.setStyle({ shape: "block", blink: false, className: "vim" });
    cursor.resetStyle();
    expect(cursor.getStyle()).toEqual(DEFAULT_CURSOR_STYLE);
  });
});
