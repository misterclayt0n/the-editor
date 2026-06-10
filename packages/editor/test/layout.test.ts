import { describe, expect, test } from "vitest";
import { TextBuffer } from "@the-editor/engine";
import { hitTestTextPosition, layoutViewport } from "../src/index";

const text = (s: string) => TextBuffer.fromString(s);

describe("layout viewport", () => {
  test("basic visible rows", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb\nccc"),
      version: 1,
      startLine: 1,
      lineCount: 2,
    });
    expect(snapshot.rows).toEqual([
      { kind: "text", row: 0, line: 1, from: 2, to: 4, text: "bb", top: 1, left: 0, height: 1 },
      { kind: "text", row: 1, line: 2, from: 5, to: 8, text: "ccc", top: 2, left: 0, height: 1 },
    ]);
  });

  test("range placement maps to row and columns", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb\nccc"),
      version: 1,
      startLine: 1,
      lineCount: 2,
      anchors: [{ kind: "range", id: "r", from: 3, to: 4, payload: "p" }],
    });
    expect(snapshot.placements).toEqual([
      {
        kind: "range",
        id: "r",
        row: 0,
        line: 1,
        from: 3,
        to: 4,
        columnStart: 1,
        columnEnd: 2,
        rects: [{ top: 1, left: 1, width: 1, height: 1 }],
        payload: "p",
      },
    ]);
  });

  test("range spanning visible lines creates multiple placements", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb\nccc"),
      version: 1,
      startLine: 1,
      lineCount: 2,
      anchors: [{ kind: "range", id: "r", from: 3, to: 7, payload: null }],
    });
    expect(snapshot.placements).toEqual([
      {
        kind: "range",
        id: "r",
        row: 0,
        line: 1,
        from: 3,
        to: 4,
        columnStart: 1,
        columnEnd: 2,
        rects: [{ top: 1, left: 1, width: 1, height: 1 }],
        payload: null,
      },
      {
        kind: "range",
        id: "r",
        row: 1,
        line: 2,
        from: 5,
        to: 7,
        columnStart: 0,
        columnEnd: 2,
        rects: [{ top: 2, left: 0, width: 2, height: 1 }],
        payload: null,
      },
    ]);
  });

  test("line placement maps to visible row", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb\nccc"),
      version: 1,
      startLine: 1,
      lineCount: 2,
      anchors: [{ kind: "line", id: "l", line: 2, payload: 1 }],
    });
    expect(snapshot.placements).toEqual([{ kind: "line", id: "l", row: 1, line: 2, payload: 1 }]);
  });

  test("block rows insert and shift later rows", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb\nccc"),
      version: 1,
      startLine: 0,
      lineCount: 3,
      anchors: [{ kind: "block", id: "b", afterLine: 1, heightRows: 2, payload: "b" }],
    });
    expect(snapshot.rows.map((r) => [r.kind, r.row])).toEqual([
      ["text", 0],
      ["text", 1],
      ["block", 2],
      ["text", 4],
    ]);
    expect(snapshot.placements).toEqual([
      {
        kind: "block",
        id: "b",
        row: 2,
        afterLine: 1,
        heightRows: 2,
        rect: { top: 2, left: 0, width: 0, height: 2 },
        payload: "b",
      },
    ]);
  });

  test("overlay placement maps to row and column", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb\nccc"),
      version: 1,
      startLine: 1,
      lineCount: 2,
      anchors: [{ kind: "overlay", id: "o", at: 6, payload: "o" }],
    });
    expect(snapshot.placements).toEqual([
      {
        kind: "overlay",
        id: "o",
        row: 1,
        line: 2,
        at: 6,
        column: 1,
        rect: { top: 2, left: 1, width: 1, height: 1 },
        payload: "o",
      },
    ]);
  });

  test("layout rows and placements include render geometry from metrics", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb"),
      version: 1,
      startLine: 0,
      lineCount: 2,
      lineHeight: 20,
      charWidth: 10,
      paddingTop: 5,
      paddingLeft: 8,
      anchors: [
        { kind: "overlay", id: "caret", at: 3, payload: null },
        { kind: "range", id: "sel", from: 2, to: 4, payload: null },
      ],
    });
    expect(snapshot.rows[1]).toMatchObject({ kind: "text", top: 25, left: 8, height: 20 });
    expect(snapshot.placements).toContainEqual(
      expect.objectContaining({
        kind: "overlay",
        rect: { top: 25, left: 18, width: 1, height: 20 },
      }),
    );
    expect(snapshot.placements).toContainEqual(
      expect.objectContaining({
        kind: "range",
        rects: [{ top: 25, left: 8, width: 20, height: 20 }],
      }),
    );
  });

  test("range rects use visual columns for tabs and wide graphemes", () => {
    const snapshot = layoutViewport({
      text: text("a\t😀"),
      version: 1,
      startLine: 0,
      lineCount: 1,
      tabWidth: 4,
      lineHeight: 20,
      charWidth: 10,
      paddingTop: 5,
      paddingLeft: 8,
      anchors: [{ kind: "range", id: "sel", from: 1, to: 4, payload: null }],
    });
    expect(snapshot.placements).toEqual([
      expect.objectContaining({
        kind: "range",
        columnStart: 1,
        columnEnd: 6,
        rects: [{ top: 5, left: 18, width: 50, height: 20 }],
      }),
    ]);
  });

  test("hit testing converts viewport pixels to text position", () => {
    const buffer = text("a\nbb\nccc");
    expect(
      hitTestTextPosition({
        text: buffer,
        x: 18,
        y: 25,
        lineHeight: 20,
        charWidth: 10,
        paddingLeft: 8,
        paddingTop: 5,
        tabWidth: 4,
        bias: "nearest",
      }),
    ).toEqual({ line: 1, column: 1, offset: 3 });
  });

  test("nearest hit testing chooses closest boundary inside tabs", () => {
    const buffer = text("a\tb");
    expect(
      hitTestTextPosition({
        text: buffer,
        x: 22,
        y: 0,
        lineHeight: 20,
        charWidth: 10,
        paddingLeft: 0,
        paddingTop: 0,
        tabWidth: 4,
        bias: "nearest",
      }),
    ).toEqual({ line: 0, column: 1, offset: 1 });
    expect(
      hitTestTextPosition({
        text: buffer,
        x: 34,
        y: 0,
        lineHeight: 20,
        charWidth: 10,
        paddingLeft: 0,
        paddingTop: 0,
        tabWidth: 4,
        bias: "nearest",
      }),
    ).toEqual({ line: 0, column: 4, offset: 2 });
  });

  test("nearest hit testing chooses closest boundary inside wide graphemes", () => {
    const buffer = text("a😀b");
    expect(
      hitTestTextPosition({
        text: buffer,
        x: 14,
        y: 0,
        lineHeight: 20,
        charWidth: 10,
        paddingLeft: 0,
        paddingTop: 0,
        tabWidth: 4,
        bias: "nearest",
      }),
    ).toEqual({ line: 0, column: 1, offset: 1 });
    expect(
      hitTestTextPosition({
        text: buffer,
        x: 26,
        y: 0,
        lineHeight: 20,
        charWidth: 10,
        paddingLeft: 0,
        paddingTop: 0,
        tabWidth: 4,
        bias: "nearest",
      }),
    ).toEqual({ line: 0, column: 3, offset: 3 });
  });

  test("hit testing supports before/after bias and clamps lines", () => {
    const buffer = text("a\nb\tc");
    expect(
      hitTestTextPosition({
        text: buffer,
        x: -100,
        y: -100,
        lineHeight: 20,
        charWidth: 10,
        paddingLeft: 8,
        paddingTop: 5,
        tabWidth: 4,
        bias: "before",
      }),
    ).toEqual({ line: 0, column: 0, offset: 0 });
    expect(
      hitTestTextPosition({
        text: buffer,
        x: 33,
        y: 25,
        lineHeight: 20,
        charWidth: 10,
        paddingLeft: 8,
        paddingTop: 5,
        tabWidth: 4,
        bias: "after",
      }),
    ).toEqual({ line: 1, column: 4, offset: 4 });
  });

  test("out-of-viewport anchors are ignored", () => {
    const snapshot = layoutViewport({
      text: text("a\nbb\nccc"),
      version: 1,
      startLine: 1,
      lineCount: 1,
      anchors: [
        { kind: "line", id: "l", line: 0, payload: null },
        { kind: "range", id: "r", from: 0, to: 1, payload: null },
        { kind: "overlay", id: "o", at: 0, payload: null },
        { kind: "block", id: "b", afterLine: 0, heightRows: 1, payload: null },
      ],
    });
    expect(snapshot.placements).toEqual([]);
    expect(snapshot.rows).toHaveLength(1);
  });
});
