/**
 * Bounded UTF-8 window reads for file previews. The picker never wants a whole
 * multi-MB file across IPC — it wants the head (or, later, a window around a
 * grep hit). This reads exactly `maxBytes` from a byte offset, trims the cut
 * UTF-8 char at the window edge, and reports whether the window is binary so
 * the renderer can skip it without transferring garbage.
 *
 * The alignment helpers are pure (operate on bytes) so they unit-test without
 * touching the filesystem.
 */

import * as fsp from "node:fs/promises";

export type TextRange = {
  readonly text: string;
  readonly offset: number;
  readonly bytesRead: number;
  readonly size: number;
  readonly eof: boolean;
  readonly binary: boolean;
};

/** A UTF-8 continuation byte (0b10xxxxxx) — never starts a char. */
const isContinuation = (byte: number): boolean => (byte & 0xc0) === 0x80;

/** Byte count a UTF-8 lead byte introduces (1 for ASCII). */
const sequenceLength = (lead: number): number =>
  lead < 0x80 ? 1 : lead < 0xe0 ? 2 : lead < 0xf0 ? 3 : 4;

/** First index that begins a whole char (skips a sequence split by `offset`). */
const skipLeadingContinuation = (bytes: Uint8Array): number => {
  let i = 0;
  while (i < bytes.length && isContinuation(bytes[i]!)) i++;
  return i;
};

/** End index dropping a trailing char the window cut mid-sequence. */
const trimTrailingPartial = (bytes: Uint8Array, end: number): number => {
  let i = end - 1;
  let seen = 0;
  while (i >= 0 && isContinuation(bytes[i]!) && seen < 3) {
    i--;
    seen++;
  }
  if (i < 0) return end; // malformed run; let the decoder cope
  return end - i >= sequenceLength(bytes[i]!) ? end : i;
};

/**
 * Sub-range of `bytes` that decodes as whole UTF-8 chars. `atStart`/`atEnd`
 * mark window edges that coincide with file edges, where no char can be cut.
 */
export const alignWindow = (
  bytes: Uint8Array,
  atStart: boolean,
  atEnd: boolean,
): { readonly start: number; readonly end: number } => {
  const start = atStart ? 0 : skipLeadingContinuation(bytes);
  const end = atEnd ? bytes.length : trimTrailingPartial(bytes, bytes.length);
  return { start: Math.min(start, end), end };
};

/** A NUL byte in the leading bytes marks the window as non-text. */
export const isBinaryHead = (bytes: Uint8Array): boolean => {
  const limit = Math.min(bytes.length, 1024);
  for (let i = 0; i < limit; i++) if (bytes[i] === 0) return true;
  return false;
};

const clamp = (value: number, lo: number, hi: number): number => Math.max(lo, Math.min(value, hi));

export async function readTextRange(
  filePath: string,
  offset: number,
  maxBytes: number,
): Promise<TextRange> {
  const handle = await fsp.open(filePath, "r");
  try {
    const { size } = await handle.stat();
    const start = clamp(Math.trunc(offset) || 0, 0, size);
    const want = Math.max(0, Math.min(Math.trunc(maxBytes), size - start));
    const buf = Buffer.alloc(want);
    let read = 0;
    while (read < want) {
      const { bytesRead } = await handle.read(buf, read, want - read, start + read);
      if (bytesRead === 0) break;
      read += bytesRead;
    }
    const slice = buf.subarray(0, read);
    const eof = start + read >= size;
    if (isBinaryHead(slice)) {
      return { text: "", offset: start, bytesRead: read, size, eof, binary: true };
    }
    const { start: s, end: e } = alignWindow(slice, start === 0, eof);
    return {
      text: Buffer.from(slice.subarray(s, e)).toString("utf8"),
      offset: start + s,
      bytesRead: e - s,
      size,
      eof,
      binary: false,
    };
  } finally {
    await handle.close();
  }
}
