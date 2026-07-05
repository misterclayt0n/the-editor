import { describe, expect, test } from "vitest";
import { TextBuffer } from "@the-editor/engine";
import {
  hitTestTextPosition,
  hitTestTextPositionExact,
  layoutContentHeight,
  layoutViewport,
  rowVisualColumns,
  stringColumns,
} from "../src/index";

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
        row: 1,
        line: 1,
        from: 2,
        to: 4,
        text: "bb",
        segments: [{ kind: "text", from: 0, to: 2, text: "bb" }],
        top: 1,
        left: 0,
        height: 1,
      },
      {
        kind: "text",
        row: 2,
        line: 2,
        from: 5,
        to: 8,
        text: "ccc",
        segments: [{ kind: "text", from: 0, to: 3, text: "ccc" }],
        top: 2,
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
        row: 1,
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
        row: 1,
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
        row: 2,
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
    expect(snapshot.placements).toEqual([{ kind: "line", id: "l", row: 2, line: 2, payload: 1 }]);
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
        row: 2,
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

  test("line decorations resolve to visual rows", () => {
    const snapshot = layoutViewport({
      text: text("abcdef"),
      version: 1,
      startLine: 0,
      lineCount: 1,
      lineHeight: 20,
      visuals: {
        lineDecorations: [
          {
            at: 3,
            target: "content",
            scope: "visual-row",
            paint: "horizontal-border",
            role: "current-line",
          },
        ],
      },
    });
    expect(snapshot.lineDecorations).toEqual([
      {
        at: 3,
        line: 0,
        row: 0,
        top: 0,
        height: 20,
        target: "content",
        scope: "visual-row",
        paint: "horizontal-border",
        role: "current-line",
      },
    ]);
  });

  test("line decorations can span a wrapped logical line", () => {
    const snapshot = layoutViewport({
      text: text("abcdef"),
      version: 1,
      startLine: 0,
      lineCount: 1,
      lineHeight: 10,
      wrapColumns: 3,
      visuals: {
        lineDecorations: [
          { at: 4, target: "content", scope: "logical-line", paint: "background" },
          { at: 4, target: "content", scope: "visual-row", paint: "horizontal-border" },
        ],
      },
    });
    expect(snapshot.lineDecorations.map((d) => ({ row: d.row, top: d.top, height: d.height }))).toEqual([
      { row: 0, top: 0, height: 20 },
      { row: 1, top: 10, height: 10 },
    ]);
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

  test("a leading block widget (afterLine -1) renders before the first line", () => {
    const buffer = text("a\nb");
    const widgets = [{ id: "h", afterLine: -1, heightRows: 2, owner: "mb" }];
    const snapshot = layoutViewport({
      text: buffer,
      version: 1,
      startLine: 0,
      lineCount: 2,
      lineHeight: 10,
      visuals: { blockWidgets: widgets },
    });
    // The header block is row 0; the first text line is pushed below it.
    expect(snapshot.rows.map((r) => [r.kind, r.row, r.top])).toEqual([
      ["block", 0, 0],
      ["text", 2, 20],
      ["text", 3, 30],
    ]);
    expect(snapshot.blockWidgets).toEqual([
      {
        id: "h",
        afterLine: -1,
        row: 0,
        rect: { top: 0, left: 0, width: 0, height: 20 },
        owner: "mb",
      },
    ]);
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
    expect(snapshot.rows.find((row) => row.kind === "text" && row.line === 1)).toMatchObject({
      row: 3,
      top: 30,
    });
    expect(snapshot.blockWidgets[0]).toMatchObject({
      row: 1,
      rect: { top: 10, left: 0, width: 0, height: 20 },
    });
    expect(snapshot.placements.find((p) => p.kind === "overlay")).toMatchObject({
      rect: { top: 30, left: 0, width: 1, height: 10 },
    });
    expect(snapshot.placements.find((p) => p.kind === "range")).toMatchObject({
      rects: [{ top: 30, left: 0, width: 5, height: 10 }],
    });
    expect(snapshot.decorations[0]).toMatchObject({
      rects: [{ top: 30, left: 0, width: 5, height: 10 }],
    });
    expect(
      layoutContentHeight({
        text: text("a\nb\nc"),
        lineHeight: 10,
        blockWidgets: [{ id: "w", afterLine: 0, heightRows: 2 }],
      }),
    ).toBe(50);
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
    expect(snapshot.placements.find((p) => p.kind === "block")).toMatchObject({
      row: 1,
      rect: { top: 10, left: 0, width: 0, height: 10 },
    });
    expect(snapshot.placements.find((p) => p.kind === "overlay")).toMatchObject({
      row: 2,
      rect: { top: 20, left: 0, width: 1, height: 10 },
    });
  });

  test("mid-document windows produce absolute geometry", () => {
    const snapshot = layoutViewport({
      text: text("a\nb\nc\nd"),
      version: 1,
      startLine: 2,
      lineCount: 2,
      lineHeight: 10,
      paddingTop: 4,
      anchors: [{ kind: "overlay", id: "caret", at: 6, payload: null }],
    });
    expect(snapshot.rows.map((r) => [r.kind === "text" ? r.line : -1, r.row, r.top])).toEqual([
      [2, 2, 24],
      [3, 3, 34],
    ]);
    expect(snapshot.placements[0]).toMatchObject({
      kind: "overlay",
      line: 3,
      rect: expect.objectContaining({ top: 34 }),
    });
  });

  test("block rows above the window shift it down", () => {
    const widgets = [{ id: "w", afterLine: 0, heightRows: 3 }];
    const snapshot = layoutViewport({
      text: text("a\nb\nc\nd"),
      version: 1,
      startLine: 2,
      lineCount: 2,
      lineHeight: 10,
      visuals: { blockWidgets: widgets },
    });
    expect(snapshot.rows.map((r) => [r.kind, r.row, r.top])).toEqual([
      ["text", 5, 50],
      ["text", 6, 60],
    ]);
    // The widget itself is outside the window, so it produces no output.
    expect(snapshot.blockWidgets).toEqual([]);
    // Window tops line up with the full content height coordinate space.
    expect(
      layoutContentHeight({ text: text("a\nb\nc\nd"), lineHeight: 10, blockWidgets: widgets }),
    ).toBe(70);
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

describe("virtual text geometry", () => {
  const text = TextBuffer.fromString("let seed = 1\nnext line\n");
  // Inlay hint ": Int" anchored after "seed" (offset 8), affinity before.
  const virtualText = [{ at: 8, text: ": Int", affinity: "before" as const }];
  const m = {
    tabWidth: 4,
    lineHeight: 20,
    charWidth: 8,
    paddingTop: 0,
    paddingLeft: 0,
  };

  test("caret placement after the hint includes its columns", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 2,
      ...m,
      visuals: { virtualText },
      anchors: [
        { kind: "overlay", id: "before-hint", at: 4, payload: null },
        { kind: "overlay", id: "line-end", at: 12, payload: null },
      ],
    });
    const byId = new Map(layout.placements.map((p) => [p.id, p] as const));
    const beforeHint = byId.get("before-hint")!;
    const lineEnd = byId.get("line-end")!;
    // Before the anchor: pure text columns.
    expect((beforeHint as { rect: { left: number } }).rect.left).toBe(4 * 8);
    // At line end: text columns (12) + hint columns (5).
    expect((lineEnd as { rect: { left: number } }).rect.left).toBe((12 + 5) * 8);
  });

  test("decoration rects right of / across the hint shift by its columns", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      visuals: {
        virtualText,
        rangeDecorations: [
          // The hover-range highlight shape: background behind "= 1"
          // (offsets 9..12), which renders RIGHT of the hint at offset 8.
          { from: 9, to: 12, layer: "background" as const, className: "hl" },
          // A diagnostics-underline shape crossing the hint (4..12).
          { from: 4, to: 12, layer: "underline" as const },
        ],
      },
    });
    const byLayer = new Map(layout.decorations.map((d) => [d.layer, d] as const));
    const background = byLayer.get("background")!;
    // Starts at text col 9 shifted by the hint's 5 cols; width unchanged.
    expect(background.rects[0]!.left).toBe((9 + 5) * 8);
    expect(background.rects[0]!.width).toBe(3 * 8);
    const underline = byLayer.get("underline")!;
    // Crosses the hint: anchored left of it, widened by its columns.
    expect(underline.rects[0]!.left).toBe(4 * 8);
    expect(underline.rects[0]!.width).toBe((12 + 5 - 4) * 8);
  });

  test("rowVisualColumns counts text (tab-expanded) plus virtual segments", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      visuals: { virtualText },
    });
    const row = layout.rows[0]! as Extract<(typeof layout.rows)[number], { kind: "text" }>;
    expect(rowVisualColumns(row, 4)).toBe(12 + 5);
    const tabbed = layoutViewport({
      text: TextBuffer.fromString("\tab"),
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
    });
    const tabbedRow = tabbed.rows[0]! as Extract<(typeof tabbed.rows)[number], { kind: "text" }>;
    expect(rowVisualColumns(tabbedRow, 4)).toBe(6); // tab → 4 cols, then "ab"
  });

  test("selection rect spanning the hint widens by its columns", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      visuals: { virtualText },
      anchors: [{ kind: "range", id: "sel", from: 4, to: 12, payload: null }],
    });
    const range = layout.placements.find((p) => p.kind === "range") as {
      rects: ReadonlyArray<{ left: number; width: number }>;
    };
    expect(range.rects[0]!.left).toBe(4 * 8);
    expect(range.rects[0]!.width).toBe((12 + 5 - 4) * 8);
  });

  test("hit testing maps x across and inside the hint", () => {
    const base = {
      text,
      y: 1,
      lineHeight: 20,
      charWidth: 8,
      paddingLeft: 0,
      paddingTop: 0,
      tabWidth: 4,
      bias: "nearest" as const,
      virtualText,
    };
    // Left of the hint: untouched mapping.
    expect(hitTestTextPosition({ ...base, x: 4 * 8 }).offset).toBe(4);
    // Inside the hint's visual span: resolves to its anchor.
    expect(hitTestTextPosition({ ...base, x: (8 + 2) * 8 }).offset).toBe(8);
    // Right of the hint: hint columns subtracted (visual col 15 = offset 10).
    expect(hitTestTextPosition({ ...base, x: (10 + 5) * 8 }).offset).toBe(10);
    // Far right: clamps to line end, not beyond.
    expect(hitTestTextPosition({ ...base, x: 60 * 8 }).offset).toBe(12);
  });

  test("hit testing stays aligned after MULTIPLE hints on one line", () => {
    // The LSP shape that regressed: `f(_event, filePath) => {}` with a
    // parameter-name hint before `_event` and a type hint after it. The
    // pointer sits on `filePath` — AFTER both hints — and must resolve
    // there, not inside the second hint's window (the old walk mixed
    // coordinate spaces and returned the second hint's anchor).
    const multi = TextBuffer.fromString("f(_event, filePath) => {}\n");
    const hints = [
      { at: 2, text: "listener: ", affinity: "before" as const }, // 10 cols
      { at: 8, text: ": IpcMainInvokeEvent", affinity: "before" as const }, // 20 cols
    ];
    const base = {
      text: multi,
      y: 1,
      lineHeight: 20,
      charWidth: 8,
      paddingLeft: 0,
      paddingTop: 0,
      tabWidth: 4,
      bias: "nearest" as const,
      virtualText: hints,
    };
    // 'P' of filePath: text col 14, visual col 14 + 10 + 20 = 44.
    expect(hitTestTextPosition({ ...base, x: 44 * 8 }).offset).toBe(14);
    // Start of filePath: text col 10, visual 40.
    expect(hitTestTextPosition({ ...base, x: 40 * 8 }).offset).toBe(10);
    // Inside the SECOND hint's run (visual 18..38): its anchor.
    expect(hitTestTextPosition({ ...base, x: 20 * 8 }).offset).toBe(8);
    // Between the hints, on `_event`: text col 4, visual 14.
    expect(hitTestTextPosition({ ...base, x: 14 * 8 }).offset).toBe(4);
  });

  test("exact hit testing reports null off the text", () => {
    const base = {
      text,
      y: 1,
      lineHeight: 20,
      charWidth: 8,
      paddingLeft: 0,
      paddingTop: 0,
      tabWidth: 4,
      virtualText,
    };
    // On text: same as nearest.
    expect(hitTestTextPositionExact({ ...base, x: 4 * 8 })?.offset).toBe(4);
    // Last visual column of the line ("let seed = 1" is 12 cols + 5 hint).
    expect(hitTestTextPositionExact({ ...base, x: 17 * 8 })?.offset).toBe(12);
    // Past the line's last visual column: null, not a clamp.
    expect(hitTestTextPositionExact({ ...base, x: 18 * 8 })).toBeNull();
    expect(hitTestTextPositionExact({ ...base, x: 60 * 8 })).toBeNull();
    // Left of the text origin (gutter): null.
    expect(hitTestTextPositionExact({ ...base, x: -8 })).toBeNull();
    // Above the first line / below the last: null.
    expect(hitTestTextPositionExact({ ...base, y: -1, x: 8 })).toBeNull();
    expect(hitTestTextPositionExact({ ...base, y: 20 * 30, x: 8 })).toBeNull();
  });
});

describe("measured grapheme widths", () => {
  // An Iosevka-like oracle: the em dash renders TWO cells wide.
  const width = (grapheme: string) => (grapheme === "—" ? 2 : 1);
  // "a — `examples/`" — offsets: a=0 ' '=1 —=2 ' '=3 `=4 examples=5..13
  const text = TextBuffer.fromString("a — `examples/`\n");
  const m = { tabWidth: 4, lineHeight: 20, charWidth: 8, paddingTop: 0, paddingLeft: 0 };

  test("rects right of a wide glyph shift by its extra cells", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      graphemeWidth: width,
      anchors: [{ kind: "range", id: "word", from: 5, to: 13, payload: null }],
    });
    const range = layout.placements.find((p) => p.kind === "range") as {
      columnStart: number;
      rects: ReadonlyArray<{ left: number; width: number }>;
    };
    // 5 text chars before "examples", the dash counting 2 → visual col 6.
    expect(range.columnStart).toBe(6);
    expect(range.rects[0]!.left).toBe(6 * 8);
    expect(range.rects[0]!.width).toBe(8 * 8);
  });

  test("hit tests map x through the wide glyph", () => {
    const base = {
      text,
      y: 1,
      lineHeight: 20,
      charWidth: 8,
      paddingLeft: 0,
      paddingTop: 0,
      tabWidth: 4,
      bias: "nearest" as const,
    };
    // Clicking the visual start of "examples" (col 6).
    expect(hitTestTextPosition({ ...base, graphemeWidth: width, x: 6 * 8 }).offset).toBe(5);
    // Without the oracle the SAME x drifts one character right — the
    // double-click-selects-the-wrong-span bug this pins down.
    expect(hitTestTextPosition({ ...base, x: 6 * 8 }).offset).toBe(6);
  });

  test("stringColumns and rowVisualColumns count the wide glyph", () => {
    expect(stringColumns("a — b", 4, width)).toBe(6);
    expect(stringColumns("a — b", 4)).toBe(5); // engine default: 1 cell
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      graphemeWidth: width,
    });
    const row = layout.rows[0]! as Extract<(typeof layout.rows)[number], { kind: "text" }>;
    expect(rowVisualColumns(row, 4, width)).toBe(16); // 15 chars + 1 extra dash cell
  });
});

describe("soft wrap layout", () => {
  const m = { tabWidth: 4, lineHeight: 20, charWidth: 8, paddingTop: 0, paddingLeft: 0 };
  // "aaa bbb ccc" wraps at 6 into "aaa " | "bbb " | "ccc" (breaks 4, 8).
  const text = TextBuffer.fromString("aaa bbb ccc\nend\n");

  test("wrapped lines emit one row per segment; rows after them shift", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 2,
      ...m,
      wrapColumns: 6,
    });
    const rows = layout.rows.filter((r) => r.kind === "text");
    expect(rows.map((r) => [r.line, r.text, r.top])).toEqual([
      [0, "aaa ", 0],
      [0, "bbb ", 20],
      [0, "ccc", 40],
      [1, "end", 60],
    ]);
    expect(rows.map((r) => (r as { continuation?: boolean }).continuation === true)).toEqual([
      false,
      true,
      true,
      false,
    ]);
  });

  test("a selection across wrap rows produces one rect per segment", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      wrapColumns: 6,
      anchors: [{ kind: "range", id: "sel", from: 2, to: 9, payload: null }],
    });
    const range = layout.placements.find((p) => p.kind === "range") as {
      rects: ReadonlyArray<{ top: number; left: number; width: number }>;
    };
    // "a|a " tail of row 0, all of "bbb " on row 1, "c" head of row 2.
    expect(range.rects).toEqual([
      { top: 0, left: 2 * 8, width: 2 * 8, height: 20 },
      { top: 20, left: 0, width: 4 * 8, height: 20 },
      { top: 40, left: 0, width: 1 * 8, height: 20 },
    ]);
  });

  test("the caret at a break lands at the continuation row start", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      wrapColumns: 6,
      anchors: [
        { kind: "overlay", id: "at-break", at: 4, payload: null },
        { kind: "overlay", id: "mid-row2", at: 9, payload: null },
      ],
    });
    const byId = new Map(layout.placements.map((p) => [p.id, p] as const));
    const atBreak = byId.get("at-break") as { rect: { top: number; left: number } };
    expect(atBreak.rect).toMatchObject({ top: 20, left: 0 });
    const mid = byId.get("mid-row2") as { rect: { top: number; left: number } };
    expect(mid.rect).toMatchObject({ top: 40, left: 1 * 8 });
  });

  test("hit tests resolve x within the requested wrap row", () => {
    const base = {
      text,
      y: 1,
      lineHeight: 20,
      charWidth: 8,
      paddingLeft: 0,
      paddingTop: 0,
      tabWidth: 4,
      bias: "nearest" as const,
    };
    // Row 1 ("bbb "), x at col 2 → offset 6.
    expect(
      hitTestTextPosition({ ...base, x: 2 * 8, wrap: { columns: 6, row: 1 } }).offset,
    ).toBe(6);
    // Row 2 ("ccc"), x at col 1 → offset 9.
    expect(
      hitTestTextPosition({ ...base, x: 1 * 8, wrap: { columns: 6, row: 2 } }).offset,
    ).toBe(9);
    // Exact: past row 1's four columns is a miss, not a clamp.
    expect(hitTestTextPositionExact({ ...base, x: 10 * 8, wrap: { columns: 6, row: 1 } })).toBeNull();
    expect(
      hitTestTextPositionExact({ ...base, x: 3 * 8, wrap: { columns: 6, row: 1 } })?.offset,
    ).toBe(7);
  });
});

describe("soft wrap window origin and affinity", () => {
  const m = { tabWidth: 4, lineHeight: 20, charWidth: 8, paddingTop: 0, paddingLeft: 0 };
  const text = TextBuffer.fromString("aaa bbb ccc\nend\n");

  test("startRow anchors absolute tops to the wrapped rows above the window", () => {
    // Line 0 wraps to 3 rows; a window starting at line 1 must sit at row 3.
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 1,
      lineCount: 1,
      ...m,
      wrapColumns: 6,
      startRow: 3,
      anchors: [{ kind: "overlay", id: "caret", at: 13, payload: null }],
    });
    expect(layout.rows[0]!.top).toBe(3 * 20);
    const caret = layout.placements[0] as { rect: { top: number } };
    expect(caret.rect.top).toBe(3 * 20);
  });

  test("assoc 'before' pins a break-offset caret to the previous row's end", () => {
    const layout = layoutViewport({
      text,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      wrapColumns: 6,
      anchors: [
        { kind: "overlay", id: "pinned", at: 4, assoc: "before", payload: null },
        { kind: "overlay", id: "free", at: 4, payload: null },
      ],
    });
    const byId = new Map(layout.placements.map((p) => [p.id, p] as const));
    // Pinned: end of row 0 ("aaa " = 4 columns). Free: start of row 1.
    expect((byId.get("pinned") as { rect: { top: number; left: number } }).rect).toMatchObject({
      top: 0,
      left: 4 * 8,
    });
    expect((byId.get("free") as { rect: { top: number; left: number } }).rect).toMatchObject({
      top: 20,
      left: 0,
    });
  });

  test("continuation rows carry the hanging indent in DOM and rects", () => {
    // "  aaa bbb" at width 6: "  aaa " | "bbb" hanging at indent 2.
    const indented = TextBuffer.fromString("  aaa bbb\n");
    const layout = layoutViewport({
      text: indented,
      version: 0,
      startLine: 0,
      lineCount: 1,
      ...m,
      wrapColumns: 6,
      anchors: [{ kind: "range", id: "word", from: 6, to: 9, payload: null }],
    });
    const rows = layout.rows.filter((r) => r.kind === "text");
    expect((rows[1] as { indentCols?: number }).indentCols).toBe(2);
    const range = layout.placements.find((p) => p.kind === "range") as {
      rects: ReadonlyArray<{ top: number; left: number }>;
    };
    expect(range.rects[0]).toMatchObject({ top: 20, left: 2 * 8 });
  });
});
