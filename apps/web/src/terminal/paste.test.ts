import { describe, expect, test } from "vitest";
import type { ClipboardPort } from "../platform/clipboard";
import {
  BRACKETED_PASTE_END,
  BRACKETED_PASTE_START,
  pasteTerminalClipboard,
  planTerminalPasteChunks,
  sanitizeTerminalPasteText,
  wrapTerminalBracketedPasteText,
  writeTerminalPasteText,
} from "./paste";

describe("terminal paste encoding", () => {
  test("sanitizes escape bytes before writing paste payloads", () => {
    expect(sanitizeTerminalPasteText("a\u001bb")).toBe("a\u241bb");
    expect(wrapTerminalBracketedPasteText("a\u001bb")).toBe(`${BRACKETED_PASTE_START}a\u241bb${BRACKETED_PASTE_END}`);
  });

  test("plans bracketed paste chunks without splitting surrogate pairs", () => {
    const plan = planTerminalPasteChunks("a🙂b", {
      bracketedPasteMode: true,
      maxChunkBytes: 4,
    });

    expect(plan.bracketed).toBe(true);
    expect(plan.chunks).toEqual([BRACKETED_PASTE_START, "a", "🙂", "b", BRACKETED_PASTE_END]);
  });

  test("writes direct bracketed paste payloads in one PTY write when small", async () => {
    const writes: string[] = [];

    await writeTerminalPasteText((data) => writes.push(data), "hello", {
      bracketedPasteMode: true,
    });

    expect(writes).toEqual([`${BRACKETED_PASTE_START}hello${BRACKETED_PASTE_END}`]);
  });

  test("chunks large payloads with async yields", async () => {
    const writes: string[] = [];
    let yields = 0;

    await writeTerminalPasteText((data) => writes.push(data), "x".repeat(70 * 1024), {
      maxChunkBytes: 16 * 1024,
      yieldAfterChunk: async () => {
        yields += 1;
      },
    });

    expect(writes.length).toBeGreaterThan(1);
    expect(yields).toBe(writes.length);
  });
});

describe("pasteTerminalClipboard", () => {
  test("pastes clipboard text to the PTY", async () => {
    const writes: string[] = [];
    const clipboard: ClipboardPort = {
      readText: async () => "from clipboard",
      writeText: async () => {},
    };

    await expect(
      pasteTerminalClipboard({
        clipboard,
        write: (data) => writes.push(data),
        bracketedPasteMode: false,
      }),
    ).resolves.toEqual({ status: "pasted", kind: "text" });
    expect(writes).toEqual(["from clipboard"]);
  });

  test("falls back to image path paste when the clipboard has no text", async () => {
    const writes: string[] = [];
    const clipboard: ClipboardPort = {
      readText: async () => "",
      writeText: async () => {},
      saveImageAsTempFile: async () => "/tmp/the-editor-clipboard/clipboard.png",
    };

    await expect(
      pasteTerminalClipboard({
        clipboard,
        write: (data) => writes.push(data),
        bracketedPasteMode: false,
      }),
    ).resolves.toEqual({ status: "pasted", kind: "image-path" });
    expect(writes).toEqual([
      `${BRACKETED_PASTE_START}/tmp/the-editor-clipboard/clipboard.png${BRACKETED_PASTE_END}`,
    ]);
  });

  test("reports oversized clipboard text without writing to the PTY", async () => {
    const clipboard: ClipboardPort = {
      readText: async () => {
        throw new Error("too large");
      },
      writeText: async () => {},
    };

    await expect(
      pasteTerminalClipboard({
        clipboard,
        write: () => {
          throw new Error("should not write");
        },
        bracketedPasteMode: false,
      }),
    ).resolves.toEqual({ status: "skipped", reason: "text-too-large" });
  });
});
