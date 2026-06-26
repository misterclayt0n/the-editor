import { describe, expect, test, vi } from "vitest";
import { handleOsc52ClipboardRequest, parseOsc52 } from "./osc52";

const base64 = (text: string): string => {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
};

describe("parseOsc52", () => {
  test("decodes clipboard writes", () => {
    expect(parseOsc52(`c;${base64("hello")}`)).toEqual({
      kind: "write",
      selections: "c",
      text: "hello",
    });
  });

  test("preserves UTF-8 text and combined selections", () => {
    expect(parseOsc52(`pc;${base64("café 日本語")}`)).toEqual({
      kind: "write",
      selections: "pc",
      text: "café 日本語",
    });
  });

  test("ignores clipboard queries instead of leaking clipboard data", () => {
    expect(parseOsc52("c;?")).toEqual({ kind: "query" });
  });

  test("rejects malformed payloads", () => {
    expect(parseOsc52(base64("missing semicolon"))).toMatchObject({ kind: "invalid" });
    expect(parseOsc52(`;${base64("empty selection")}`)).toMatchObject({ kind: "invalid" });
    expect(parseOsc52("x;AAAA")).toMatchObject({ kind: "invalid" });
    expect(parseOsc52("c;not base64!")).toMatchObject({ kind: "invalid" });
  });
});

describe("handleOsc52ClipboardRequest", () => {
  test("writes valid clipboard payloads when enabled", () => {
    const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);

    expect(
      handleOsc52ClipboardRequest(`c;${base64("from remote")}`, {
        allowClipboardWrite: true,
        writeClipboardText,
      }),
    ).toBe(true);

    expect(writeClipboardText).toHaveBeenCalledWith("from remote");
  });

  test("writes primary selection payloads when available", () => {
    const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);
    const writeSelectionText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);

    handleOsc52ClipboardRequest(`p;${base64("primary")}`, {
      allowClipboardWrite: true,
      writeClipboardText,
      writeSelectionText,
    });

    expect(writeClipboardText).not.toHaveBeenCalled();
    expect(writeSelectionText).toHaveBeenCalledWith("primary");
  });

  test("surfaces blocked writes without answering queries", () => {
    const onBlockedWrite = vi.fn();
    const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);

    handleOsc52ClipboardRequest(`c;${base64("blocked")}`, {
      allowClipboardWrite: false,
      writeClipboardText,
      onBlockedWrite,
    });
    handleOsc52ClipboardRequest("c;?", {
      allowClipboardWrite: false,
      writeClipboardText,
      onBlockedWrite,
    });

    expect(writeClipboardText).not.toHaveBeenCalled();
    expect(onBlockedWrite).toHaveBeenCalledTimes(1);
  });
});
