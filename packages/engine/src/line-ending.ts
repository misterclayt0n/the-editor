/**
 * Line-ending utilities.
 *
 * The engine's preferred/document line-ending policy is intentionally small:
 * LF and CRLF. Bare CR can appear in input and is handled during scanning and
 * normalization, but it is not a preferred document line ending.
 *
 * This module does not implement Unicode soft line breaking or wrapping.
 */

export type LineEnding = "lf" | "crlf";

export type LineEndingScanKind = "unknown" | "lf" | "crlf" | "mixed" | "invalid";

export type LineEndingScan = {
  readonly eolCount: number;
  readonly firstLineLength: number;
  readonly lastLineLength: number;
  readonly kind: LineEndingScanKind;
  readonly hasBareCarriageReturn: boolean;
};

export const LF = "\n";
export const CRLF = "\r\n";

const isWindows = typeof process !== "undefined" && process.platform === "win32";
export const nativeLineEnding: LineEnding = isWindows ? "crlf" : "lf";

export function lineEndingString(ending: LineEnding): "\n" | "\r\n" {
  return ending === "lf" ? LF : CRLF;
}

export function lineEndingLength(ending: LineEnding): 1 | 2 {
  return ending === "lf" ? 1 : 2;
}

export function lineEndingFromString(text: string): LineEnding | undefined {
  if (text === LF) return "lf";
  if (text === CRLF) return "crlf";
  return undefined;
}

export function isLineEndingString(text: string): boolean {
  return lineEndingFromString(text) !== undefined;
}

export function getLineEndingOfString(line: string): LineEnding | undefined {
  if (line.endsWith(CRLF)) return "crlf";
  if (line.endsWith(LF)) return "lf";
  return undefined;
}

export function lineEndWithoutLineEnding(line: string): number {
  const ending = getLineEndingOfString(line);
  return ending === undefined ? line.length : line.length - lineEndingLength(ending);
}

export function stripLineEnding(line: string): string {
  return line.slice(0, lineEndWithoutLineEnding(line));
}

export function scanLineEndings(text: string): LineEndingScan {
  let eolCount = 0;
  let firstLineLength = text.length;
  let lastLineStart = 0;
  let sawLf = false;
  let sawCrlf = false;
  let hasBareCarriageReturn = false;

  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    let eolLength = 0;
    if (code === 13) {
      if (i + 1 < text.length && text.charCodeAt(i + 1) === 10) {
        sawCrlf = true;
        eolLength = 2;
      } else {
        hasBareCarriageReturn = true;
        eolLength = 1;
      }
    } else if (code === 10) {
      sawLf = true;
      eolLength = 1;
    }

    if (eolLength > 0) {
      if (eolCount === 0) firstLineLength = i;
      eolCount++;
      lastLineStart = i + eolLength;
      if (eolLength === 2) i++;
    }
  }

  const kind: LineEndingScanKind = hasBareCarriageReturn
    ? "invalid"
    : sawLf && sawCrlf
      ? "mixed"
      : sawLf
        ? "lf"
        : sawCrlf
          ? "crlf"
          : "unknown";

  return {
    eolCount,
    firstLineLength,
    lastLineLength: text.length - lastLineStart,
    kind,
    hasBareCarriageReturn,
  };
}

export function detectLineEnding(text: string): LineEnding | undefined {
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    if (code === 13) {
      if (i + 1 < text.length && text.charCodeAt(i + 1) === 10) return "crlf";
    } else if (code === 10) {
      return "lf";
    }
  }
  return undefined;
}

export function normalizeLineEndings(text: string, ending: LineEnding): string {
  return text.replace(/\r\n|\r|\n/g, lineEndingString(ending));
}
