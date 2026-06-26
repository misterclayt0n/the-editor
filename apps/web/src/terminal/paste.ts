import type { ClipboardPort } from "../platform/clipboard";

export const TERMINAL_PASTE_DIRECT_MAX_BYTES = 64 * 1024;
export const TERMINAL_PASTE_CHUNK_MAX_BYTES = 16 * 1024;
export const TERMINAL_PASTE_MAX_BYTES = 16 * 1024 * 1024;

const ESCAPE_CODE_POINT = 0x1b;
const INERT_ESCAPE_CODE_POINT = 0x241b;
const INERT_ESCAPE = "\u241b";
const ESCAPE = "\u001b";
const LINE_BREAK_RE = /[\r\n]/;

export const BRACKETED_PASTE_START = `${ESCAPE}[200~`;
export const BRACKETED_PASTE_END = `${ESCAPE}[201~`;

export type TerminalPasteTextOptions = {
  readonly forceBracketedPaste?: boolean;
  readonly forceBracketedPasteForMultiline?: boolean;
};

export type TerminalClipboardPasteResult =
  | { readonly status: "pasted"; readonly kind: "text" | "image-path" }
  | {
      readonly status: "skipped";
      readonly reason: "empty" | "text-too-large" | "text-paste-failed" | "image-paste-failed";
    };

export type TerminalPasteChunkPlan = {
  readonly chunks: readonly string[];
  readonly bracketed: boolean;
  readonly byteLength: number;
};

export function sanitizeTerminalPasteText(text: string): string {
  return text.includes(ESCAPE) ? text.replaceAll(ESCAPE, INERT_ESCAPE) : text;
}

export function shouldBracketTerminalPaste(
  text: string,
  bracketedPasteMode: boolean,
  options: TerminalPasteTextOptions = {},
): boolean {
  if (options.forceBracketedPaste === true) return true;
  if (options.forceBracketedPasteForMultiline === true && LINE_BREAK_RE.test(text)) return true;
  return bracketedPasteMode;
}

export function wrapTerminalBracketedPasteText(text: string): string {
  return `${BRACKETED_PASTE_START}${sanitizeTerminalPasteText(text)}${BRACKETED_PASTE_END}`;
}

export function planTerminalPasteChunks(
  text: string,
  options: TerminalPasteTextOptions & {
    readonly bracketedPasteMode?: boolean;
    readonly maxChunkBytes?: number;
  } = {},
): TerminalPasteChunkPlan {
  const bracketed = shouldBracketTerminalPaste(text, options.bracketedPasteMode === true, options);
  const maxChunkBytes = Math.max(4, options.maxChunkBytes ?? TERMINAL_PASTE_CHUNK_MAX_BYTES);
  const chunks = [...iterateTerminalPasteChunks(text, maxChunkBytes, bracketed)];
  return {
    chunks,
    bracketed,
    byteLength: terminalPasteByteLength(text, bracketed),
  };
}

export async function writeTerminalPasteText(
  write: (data: string) => void,
  text: string,
  options: TerminalPasteTextOptions & {
    readonly bracketedPasteMode?: boolean;
    readonly maxChunkBytes?: number;
    readonly yieldAfterChunk?: () => Promise<void>;
  } = {},
): Promise<boolean> {
  if (utf8ByteLength(text) > TERMINAL_PASTE_MAX_BYTES) return false;
  const plan = planTerminalPasteChunks(text, options);
  if (plan.byteLength <= TERMINAL_PASTE_DIRECT_MAX_BYTES) {
    write(plan.chunks.join(""));
    return true;
  }

  const yieldAfterChunk = options.yieldAfterChunk ?? nextTask;
  for (const chunk of plan.chunks) {
    write(chunk);
    await yieldAfterChunk();
  }
  return true;
}

export async function pasteTerminalClipboard(options: {
  readonly clipboard: ClipboardPort | undefined;
  readonly write: (data: string) => void;
  readonly bracketedPasteMode: boolean;
  readonly forceBracketedMultilineTextPaste?: boolean;
}): Promise<TerminalClipboardPasteResult> {
  const clipboard = options.clipboard;
  if (clipboard === undefined) return { status: "skipped", reason: "empty" };

  let text = "";
  try {
    text = await clipboard.readText({ maxBytes: TERMINAL_PASTE_MAX_BYTES });
  } catch {
    return { status: "skipped", reason: "text-too-large" };
  }

  if (text.length > 0) {
    try {
      const pasted = await writeTerminalPasteText(options.write, text, {
        bracketedPasteMode: options.bracketedPasteMode,
        forceBracketedPasteForMultiline: options.forceBracketedMultilineTextPaste === true,
      });
      return pasted ? { status: "pasted", kind: "text" } : { status: "skipped", reason: "text-too-large" };
    } catch {
      return { status: "skipped", reason: "text-paste-failed" };
    }
  }

  if (clipboard.saveImageAsTempFile === undefined) return { status: "skipped", reason: "empty" };
  try {
    const imagePath = await clipboard.saveImageAsTempFile();
    if (imagePath === null || imagePath.length === 0) return { status: "skipped", reason: "empty" };
    const pasted = await writeTerminalPasteText(options.write, imagePath, {
      bracketedPasteMode: options.bracketedPasteMode,
      forceBracketedPaste: true,
    });
    return pasted ? { status: "pasted", kind: "image-path" } : { status: "skipped", reason: "text-too-large" };
  } catch {
    return { status: "skipped", reason: "image-paste-failed" };
  }
}

function* iterateTerminalPasteChunks(
  text: string,
  maxChunkBytes: number,
  bracketed: boolean,
): Generator<string> {
  if (bracketed) yield BRACKETED_PASTE_START;
  yield* iterateTextByUtf8Bytes(text, maxChunkBytes);
  if (bracketed) yield BRACKETED_PASTE_END;
}

function* iterateTextByUtf8Bytes(text: string, maxBytes: number): Generator<string> {
  let chunk = "";
  let chunkBytes = 0;
  for (let index = 0; index < text.length; index += 1) {
    const codePoint = text.codePointAt(index) ?? 0;
    const codeUnitLength = codePoint > 0xffff ? 2 : 1;
    const sanitizedEscape = codePoint === ESCAPE_CODE_POINT;
    const next = sanitizedEscape ? INERT_ESCAPE : text.slice(index, index + codeUnitLength);
    const nextBytes = utf8BytesForCodePoint(sanitizedEscape ? INERT_ESCAPE_CODE_POINT : codePoint);
    if (chunk.length > 0 && chunkBytes + nextBytes > maxBytes) {
      yield chunk;
      chunk = next;
      chunkBytes = nextBytes;
      if (codeUnitLength === 2) index += 1;
      continue;
    }
    chunk += next;
    chunkBytes += nextBytes;
    if (codeUnitLength === 2) index += 1;
  }
  if (chunk.length > 0) yield chunk;
}

function terminalPasteByteLength(text: string, bracketed: boolean): number {
  return utf8ByteLength(text) + (bracketed ? BRACKETED_PASTE_START.length + BRACKETED_PASTE_END.length : 0);
}

function utf8ByteLength(text: string): number {
  let total = 0;
  for (let index = 0; index < text.length; index += 1) {
    const codePoint = text.codePointAt(index) ?? 0;
    total += utf8BytesForCodePoint(codePoint === ESCAPE_CODE_POINT ? INERT_ESCAPE_CODE_POINT : codePoint);
    if (codePoint > 0xffff) index += 1;
  }
  return total;
}

function utf8BytesForCodePoint(codePoint: number): number {
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

function nextTask(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}
