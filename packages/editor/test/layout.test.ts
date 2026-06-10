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
      { kind: "text", row: 0, line: 1, from: 2, to: 4, text: "bb" },
      { kind: "text", row: 1, line: 2, from: 5, to: 8, text: "ccc" },
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
      { kind: "block", id: "b", row: 2, afterLine: 1, heightRows: 2, payload: "b" },
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
      { kind: "overlay", id: "o", row: 1, line: 2, at: 6, column: 1, payload: "o" },
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
