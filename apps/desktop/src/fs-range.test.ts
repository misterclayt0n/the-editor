import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { alignWindow, isBinaryHead, readTextRange } from "./fs-range";

const enc = (s: string) => new TextEncoder().encode(s);

describe("alignWindow", () => {
  test("file edges keep every byte", () => {
    const bytes = enc("héllo"); // é is 2 bytes
    expect(alignWindow(bytes, true, true)).toEqual({ start: 0, end: bytes.length });
  });

  test("drops a multibyte char cut by the window end", () => {
    const full = enc("ab→"); // → is 3 bytes (E2 86 92)
    const cut = full.subarray(0, full.length - 1); // last byte missing
    const { start, end } = alignWindow(cut, true, false);
    expect(new TextDecoder().decode(cut.subarray(start, end))).toBe("ab");
  });

  test("keeps a complete trailing char when not at eof", () => {
    const bytes = enc("ab→");
    const { start, end } = alignWindow(bytes, true, false);
    expect(new TextDecoder().decode(bytes.subarray(start, end))).toBe("ab→");
  });

  test("skips leading continuation bytes when the offset splits a char", () => {
    const full = enc("→ok"); // → = E2 86 92
    const tail = full.subarray(1); // starts mid-char (86 92 6F 6B)
    const { start, end } = alignWindow(tail, false, true);
    expect(new TextDecoder().decode(tail.subarray(start, end))).toBe("ok");
  });
});

describe("isBinaryHead", () => {
  test("flags a NUL byte", () => {
    expect(isBinaryHead(new Uint8Array([0x68, 0x00, 0x69]))).toBe(true);
  });
  test("passes plain text", () => {
    expect(isBinaryHead(enc("hello world"))).toBe(false);
  });
});

describe("readTextRange", () => {
  let dir: string;
  beforeAll(async () => {
    dir = await mkdtemp(path.join(tmpdir(), "fsrange-"));
  });
  afterAll(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  const fixture = async (name: string, content: string | Uint8Array): Promise<string> => {
    const p = path.join(dir, name);
    await writeFile(p, content);
    return p;
  };

  test("reads the whole small file (eof)", async () => {
    const p = await fixture("small.txt", "line1\nline2\n");
    const r = await readTextRange(p, 0, 64 * 1024);
    expect(r.text).toBe("line1\nline2\n");
    expect(r.eof).toBe(true);
    expect(r.binary).toBe(false);
    expect(r.size).toBe(12);
  });

  test("bounds the read and reports more content (no eof)", async () => {
    const big = "x".repeat(10_000);
    const p = await fixture("big.txt", big);
    const r = await readTextRange(p, 0, 256);
    expect(r.text.length).toBe(256);
    expect(r.eof).toBe(false);
    expect(r.size).toBe(10_000);
  });

  test("does not split a multibyte char at the window boundary", async () => {
    // 200 '→' chars (3 bytes each); cap at 256 bytes lands mid-char.
    const p = await fixture("multi.txt", "→".repeat(200));
    const r = await readTextRange(p, 0, 256);
    expect(r.eof).toBe(false);
    expect(r.text.endsWith("→")).toBe(true); // no partial char / replacement
    expect(r.text).not.toContain("\uFFFD");
    expect(r.bytesRead % 3).toBe(0);
  });

  test("flags binary content without returning it", async () => {
    const p = await fixture("bin", new Uint8Array([0x7f, 0x45, 0x4c, 0x46, 0x00, 0x01]));
    const r = await readTextRange(p, 0, 1024);
    expect(r.binary).toBe(true);
    expect(r.text).toBe("");
  });

  test("reads a window from a byte offset", async () => {
    const p = await fixture("offset.txt", "0123456789");
    const r = await readTextRange(p, 5, 3);
    expect(r.text).toBe("567");
    expect(r.offset).toBe(5);
    expect(r.eof).toBe(false);
  });
});
