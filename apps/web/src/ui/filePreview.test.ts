import { describe, expect, test, vi } from "vitest";
import type { FileTextRange } from "@the-editor/editor";
import {
  loadFilePreview,
  PREVIEW_MAX_FILE_BYTES,
  PREVIEW_MAX_LINES,
  PREVIEW_WINDOW_BYTES,
  type PreviewHighlighter,
  type PreviewReader,
} from "./filePreview";

const range = (over: Partial<FileTextRange>): FileTextRange => ({
  text: "",
  offset: 0,
  bytesRead: 0,
  size: 0,
  eof: true,
  binary: false,
  ...over,
});

const reader =
  (r: FileTextRange): PreviewReader =>
  async () =>
    r;
const plain: PreviewHighlighter = () => null;

describe("loadFilePreview", () => {
  test("skips files over the absolute ceiling without reading", async () => {
    const read = vi.fn();
    const state = await loadFilePreview(
      { path: "/a.bin", size: PREVIEW_MAX_FILE_BYTES + 1 },
      { read, highlight: plain },
    );
    expect(state).toEqual({ status: "skipped", reason: "too-large" });
    expect(read).not.toHaveBeenCalled();
  });

  test("skips binary windows", async () => {
    const state = await loadFilePreview(
      { path: "/a.png", size: 10 },
      { read: reader(range({ binary: true })), highlight: plain },
    );
    expect(state).toEqual({ status: "skipped", reason: "binary" });
  });

  test("surfaces read errors", async () => {
    const read: PreviewReader = async () => {
      throw new Error("boom");
    };
    const state = await loadFilePreview({ path: "/a.ts", size: 10 }, { read, highlight: plain });
    expect(state).toEqual({ status: "error", message: "boom" });
  });

  test("plain text yields one span per non-empty line", async () => {
    const state = await loadFilePreview(
      { path: "/a.txt", size: 12 },
      { read: reader(range({ text: "ab\n\ncd", eof: true })), highlight: plain },
    );
    expect(state.status).toBe("ready");
    if (state.status !== "ready") return;
    expect(state.lines).toEqual([[{ text: "ab" }], [], [{ text: "cd" }]]);
    expect(state.truncated).toBe(false);
  });

  test("marks truncated when the window did not reach eof", async () => {
    const state = await loadFilePreview(
      { path: "/a.ts", size: 99999 },
      { read: reader(range({ text: "x", eof: false })), highlight: plain },
    );
    expect(state.status === "ready" && state.truncated).toBe(true);
  });

  test("caps rendered lines and reports truncation", async () => {
    const text = Array.from({ length: PREVIEW_MAX_LINES + 50 }, (_, i) => `L${i}`).join("\n");
    const state = await loadFilePreview(
      { path: "/a.ts", size: text.length },
      { read: reader(range({ text, eof: true })), highlight: plain },
    );
    expect(state.status).toBe("ready");
    if (state.status !== "ready") return;
    expect(state.lines.length).toBe(PREVIEW_MAX_LINES);
    expect(state.truncated).toBe(true);
  });

  test("applies the highlighter per line when language is known", async () => {
    const highlight: PreviewHighlighter = (code, ext) => {
      expect(ext).toBe("ts");
      return code.split("\n").map((line) => [{ text: line, color: "#f00" }]);
    };
    const state = await loadFilePreview(
      { path: "/dir/a.ts", size: 7 },
      { read: reader(range({ text: "const x", eof: true })), highlight },
    );
    expect(state.status === "ready" && state.lines[0]).toEqual([
      { text: "const x", color: "#f00" },
    ]);
  });

  test("does not highlight extensionless files", async () => {
    const highlight = vi.fn(() => null);
    await loadFilePreview(
      { path: "/dir/Makefile", size: 4 },
      { read: reader(range({ text: "all:", eof: true })), highlight },
    );
    expect(highlight).not.toHaveBeenCalled();
  });

  test("keeps focusLine when it lands inside the rendered window", async () => {
    const state = await loadFilePreview(
      { path: "/a.ts", size: 5, focusLine: 1 },
      { read: reader(range({ text: "a\nb\nc", eof: true })), highlight: plain },
    );
    expect(state.status === "ready" && state.focusLine).toBe(1);
  });

  test("drops focusLine when it falls outside the rendered window", async () => {
    const state = await loadFilePreview(
      { path: "/a.ts", size: 5, focusLine: 999 },
      { read: reader(range({ text: "a\nb\nc", eof: true })), highlight: plain },
    );
    expect(state.status === "ready" && state.focusLine).toBeUndefined();
  });

  test("reads exactly the bounded window from offset 0", async () => {
    const read = vi.fn(reader(range({ text: "hi", eof: true })));
    await loadFilePreview({ path: "/a.ts", size: 2 }, { read, highlight: plain });
    expect(read).toHaveBeenCalledWith("/a.ts", { offset: 0, maxBytes: PREVIEW_WINDOW_BYTES });
  });
});
