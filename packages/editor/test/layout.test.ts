import { describe, expect, test } from "vitest";
import { TextBuffer } from "@the-editor/engine";
import { hitTestTextPosition, layoutContentHeight, layoutViewport } from "../src/index";

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
      {
        kind: "text",
        row: 0,
        line: 1,
        from: 2,
        to: 4,
        text: "bb",
        segments: [{ kind: "text", from: 0, to: 2, text: "bb" }],
        top: 0,
        left: 0,
        height: 1,
      },
      {
        kind: "text",
        row: 1,
        line: 2,
        from: 5,
        to: 8,
        text: "ccc",
        segments: [{ kind: "text", from: 0, to: 3, text: "ccc" }],
        top: 1,
        left: 0,
        height: 1,
      },
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
        rects: [{ top: 0, left: 1, width: 1, height: 1 }],
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
        rects: [{ top: 0, left: 1, width: 1, height: 1 }],
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
        rects: [{ top: 1, left: 0, width: 2, height: 1 }],
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
        rect: { top: 1, left: 1, width: 1, height: 1 },
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

  test("text styles split and merge row segments", () => {
    const snapshot = layoutViewport({
      text: text("abcdef"),
      version: 1,
      startLine: 0,
      lineCount: 1,
      visuals: {
        textStyles: [
          { from: 1, to: 4, className: "a", style: { color: "red" }, owner: "one" },
          { from: 2, to: 5, className: "b", style: { color: "blue" }, priority: 1, owner: "two" },
        ],
      },
    });
    expect(snapshot.rows[0]).toMatchObject({
      kind: "text",
      segments: [
        { kind: "text", from: 0, to: 1, text: "a" },
        {
          kind: "text",
          from: 1,
          to: 2,
          text: "b",
          className: "a",
          style: { color: "red" },
          owners: ["one"],
        },
        {
          kind: "text",
          from: 2,
          to: 4,
          text: "cd",
          className: "a b",
          style: { color: "blue" },
          owners: ["one", "two"],
        },
        {
          kind: "text",
          from: 4,
          to: 5,
          text: "e",
          className: "b",
          style: { color: "blue" },
          owners: ["two"],
        },
        { kind: "text", from: 5, to: 6, text: "f" },
      ],
    });
  });

  test("virtual text orders by affinity priority and keeps row text", () => {
    const snapshot = layoutViewport({
      text: text("abc"),
      version: 1,
      startLine: 0,
      lineCount: 1,
      visuals: {
        virtualText: [
          { at: 1, text: "A", affinity: "after", priority: 2 },
          { at: 1, text: "B", affinity: "before" },
          { at: 3, text: "E", affinity: "after" },
        ],
      },
    });
    expect(snapshot.rows[0]).toMatchObject({ text: "abc" });
    expect((snapshot.rows[0] as any).segments).toEqual([
      { kind: "text", from: 0, to: 1, text: "a" },
      { kind: "virtual-text", at: 1, text: "B", affinity: "before" },
      { kind: "text", from: 1, to: 3, text: "bc" },
      { kind: "virtual-text", at: 1, text: "A", affinity: "after" },
      { kind: "virtual-text", at: 3, text: "E", affinity: "after" },
    ]);
  });

  test("range decorations produce sorted rects using visual columns", () => {
    const snapshot = layoutViewport({
      text: text("a\t😀\nxx"),
      version: 1,
      startLine: 0,
      lineCount: 2,
      tabWidth: 4,
      lineHeight: 20,
      charWidth: 10,
      visuals: {
        rangeDecorations: [
          { from: 1, to: 4, layer: "outline", priority: 1 },
          { from: 0, to: 6, layer: "background" },
        ],
      },
    });
    expect(snapshot.decorations.map((d) => d.layer)).toEqual(["background", "outline"]);
    expect(snapshot.decorations[0]!.rects).toEqual([
      { top: 0, left: 0, width: 60, height: 20 },
      { top: 20, left: 0, width: 10, height: 20 },
    ]);
    expect(snapshot.decorations[1]!.rects).toEqual([{ top: 0, left: 10, width: 50, height: 20 }]);
  });

  test("block widgets create rows outputs and content height", () => {
    const buffer = text("a\nb");
    const widgets = [{ id: "w", afterLine: 0, heightRows: 2, owner: "test" }];
    const snapshot = layoutViewport({
      text: buffer,
      version: 1,
      startLine: 0,
      lineCount: 2,
      lineHeight: 10,
      visuals: { blockWidgets: widgets },
    });
    expect(snapshot.rows.map((r) => [r.kind, r.row])).toEqual([
      ["text", 0],
      ["block", 1],
      ["text", 3],
    ]);
    expect(snapshot.blockWidgets).toEqual([
      {
        id: "w",
        afterLine: 0,
        row: 1,
        rect: { top: 10, left: 0, width: 0, height: 20 },
        owner: "test",
      },
    ]);
    expect(layoutContentHeight({ text: buffer, lineHeight: 10, blockWidgets: widgets })).toBe(40);
    expect(
      layoutContentHeight({
        text: buffer,
        lineHeight: 10,
        blockWidgets: [{ id: "bad", afterLine: 0, heightRows: 0 }],
      }),
    ).toBe(20);
  });

  test("block widgets shift following visual geometry", () => {
    const snapshot = layoutViewport({
      text: text("a\nb\nc"),
      version: 1,
      startLine: 0,
      lineCount: 3,
      lineHeight: 10,
      charWidth: 5,
      visuals: {
        blockWidgets: [{ id: "w", afterLine: 0, heightRows: 2 }],
        rangeDecorations: [{ from: 2, to: 3, layer: "background" }],
      },
      anchors: [
        { kind: "overlay", id: "o", at: 2, payload: null },
        { kind: "range", id: "r", from: 2, to: 3, payload: null },
      ],
    });
    expect(snapshot.rows.find((row) => row.kind === "text" && row.line === 1)).toMatchObject({ row: 3, top: 30 });
    expect(snapshot.blockWidgets[0]).toMatchObject({ row: 1, rect: { top: 10, left: 0, width: 0, height: 20 } });
    expect(snapshot.placements.find((p) => p.kind === "overlay")).toMatchObject({ rect: { top: 30, left: 0, width: 1, height: 10 } });
    expect(snapshot.placements.find((p) => p.kind === "range")).toMatchObject({ rects: [{ top: 30, left: 0, width: 5, height: 10 }] });
    expect(snapshot.decorations[0]).toMatchObject({ rects: [{ top: 30, left: 0, width: 5, height: 10 }] });
    expect(layoutContentHeight({ text: text("a\nb\nc"), lineHeight: 10, blockWidgets: [{ id: "w", afterLine: 0, heightRows: 2 }] })).toBe(50);
  });

  test("existing block anchors use visual row geometry", () => {
    const snapshot = layoutViewport({
      text: text("a\nb"),
      version: 1,
      startLine: 0,
      lineCount: 2,
      lineHeight: 10,
      anchors: [
        { kind: "block", id: "a", afterLine: 0, heightRows: 1, payload: null },
        { kind: "overlay", id: "o", at: 2, payload: null },
      ],
    });
    expect(snapshot.placements.find((p) => p.kind === "block")).toMatchObject({ row: 1, rect: { top: 10, left: 0, width: 0, height: 10 } });
    expect(snapshot.placements.find((p) => p.kind === "overlay")).toMatchObject({ row: 2, rect: { top: 20, left: 0, width: 1, height: 10 } });
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
