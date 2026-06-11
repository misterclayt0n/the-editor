import { describe, expect, test } from "vitest";
import { GeometryModel } from "../src/geometry";

describe("GeometryModel", () => {
  test("degenerates to arithmetic with no widgets", () => {
    const model = new GeometryModel(100, 20, 8);
    expect(model.lineTop(0)).toBe(8);
    expect(model.lineTop(50)).toBe(1008);
    expect(model.lineAt(1008)).toBe(50);
    expect(model.lineAt(1027)).toBe(50);
    expect(model.lineAt(0)).toBe(0);
    expect(model.contentHeight()).toBe(8 + 100 * 20 + 8);
  });

  test("widget rows shift lines below them", () => {
    const model = new GeometryModel(100, 20, 8, [
      { afterLine: 10, heightRows: 3 },
      { afterLine: 20, heightRows: 2 },
    ]);
    expect(model.lineTop(10)).toBe(8 + 10 * 20); // before its own widget
    expect(model.lineTop(11)).toBe(8 + (11 + 3) * 20);
    expect(model.lineTop(21)).toBe(8 + (21 + 5) * 20);
    expect(model.contentHeight()).toBe(8 + 105 * 20 + 8);
  });

  test("lineAt resolves widget zones to the widget's line", () => {
    const model = new GeometryModel(100, 20, 8, [{ afterLine: 10, heightRows: 3 }]);
    // Rows 11..13 (visual) are the widget zone after line 10.
    expect(model.lineAt(8 + 10 * 20)).toBe(10); // line 10 itself
    expect(model.lineAt(8 + 11 * 20)).toBe(10); // widget row 1
    expect(model.lineAt(8 + 13 * 20)).toBe(10); // widget row 3
    expect(model.lineAt(8 + 14 * 20)).toBe(11); // first line after the widget
  });

  test("clamps below zero and past the end", () => {
    const model = new GeometryModel(10, 20, 8, [{ afterLine: 9, heightRows: 5 }]);
    expect(model.lineAt(-100)).toBe(0);
    expect(model.lineAt(1e9)).toBe(9);
  });

  test("invalid widgets are ignored", () => {
    const model = new GeometryModel(10, 20, 0, [
      { afterLine: -1, heightRows: 2 },
      { afterLine: 50, heightRows: 2 },
      { afterLine: 3, heightRows: 0 },
    ]);
    expect(model.contentHeight()).toBe(10 * 20);
  });
});
