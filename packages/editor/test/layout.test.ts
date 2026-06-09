import { describe, expect, test } from "vitest";
import { TextBuffer } from "@the-editor/engine";
import { layoutViewport } from "../src/index";

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
