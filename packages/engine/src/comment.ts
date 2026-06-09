import { Effect } from "effect";
import { TextBuffer } from "./text/buffer";
import { Replacement, Transaction, type TextError } from "./text/transaction";
import { Selection, SelectionRange } from "./selection";

const DEFAULT_LINE_TOKEN = "#";
const DEFAULT_BLOCK_TOKEN: BlockCommentToken = { start: "/*", end: "*/" };

export type Edit = { readonly transaction: Transaction; readonly selection: Selection };
export type BlockCommentToken = { readonly start: string; readonly end: string };
export type CommentOptions = {
  readonly lineToken?: string;
  readonly blockTokens?: readonly BlockCommentToken[];
};
export class CommentError extends Error {}
export class InvalidCommentTokenError extends CommentError {
  constructor(readonly token: string) {
    super(`invalid comment token ${token}`);
  }
}
export class CommentSelectionOutOfBoundsError extends CommentError {
  constructor(
    readonly to: number,
    readonly length: number,
  ) {
    super(`comment selection ${to} exceeds text length ${length}`);
  }
}

type CommentChange =
  | {
      readonly type: "commented";
      readonly range: SelectionRange;
      readonly startPos: number;
      readonly endPos: number;
      readonly startMargin: boolean;
      readonly endMargin: boolean;
      readonly startToken: string;
      readonly endToken: string;
    }
  | {
      readonly type: "uncommented";
      readonly range: SelectionRange;
      readonly startPos: number;
      readonly endPos: number;
      readonly startToken: string;
      readonly endToken: string;
    }
  | { readonly type: "whitespace"; readonly range: SelectionRange };
type PreparedBlockToken = BlockCommentToken & {
  readonly startLen: number;
  readonly endLen: number;
};

const fail = <A = never>(e: CommentError): Effect.Effect<A, CommentError> => Effect.fail(e);
const validateSelection = (buffer: TextBuffer, selection: Selection): CommentError | undefined =>
  selection.ranges.find((r) => r.to > buffer.length)
    ? new CommentSelectionOutOfBoundsError(
        Math.max(...selection.ranges.map((r) => r.to)),
        buffer.length,
      )
    : undefined;
const previousBoundaryBuffer = (buffer: TextBuffer, offset: number): number => {
  if (offset <= 0) return 0;
  const s = buffer.sliceString(Math.max(0, offset - 2), offset);
  const last = s.charCodeAt(s.length - 1);
  const before = s.length >= 2 ? s.charCodeAt(s.length - 2) : undefined;
  return last >= 0xdc00 &&
    last <= 0xdfff &&
    before !== undefined &&
    before >= 0xd800 &&
    before <= 0xdbff
    ? offset - 2
    : offset - 1;
};
const firstNonWhitespace = (s: string): number | undefined => {
  const m = /\S/u.exec(s);
  return m ? m.index : undefined;
};
const lastNonWhitespace = (s: string): number | undefined => {
  for (let i = s.length; i > 0; ) {
    i = previousBoundaryString(s, i);
    if (!/\s/u.test(s.slice(i, i + (s.codePointAt(i)! > 0xffff ? 2 : 1)))) return i;
  }
  return undefined;
};
const previousBoundaryString = (s: string, offset: number): number => {
  if (offset <= 0) return 0;
  const last = s.charCodeAt(offset - 1);
  const before = offset >= 2 ? s.charCodeAt(offset - 2) : undefined;
  return last >= 0xdc00 &&
    last <= 0xdfff &&
    before !== undefined &&
    before >= 0xd800 &&
    before <= 0xdbff
    ? offset - 2
    : offset - 1;
};

function selectedLines(buffer: TextBuffer, selection: Selection): number[] {
  const out: number[] = [];
  let minNext = 0;
  for (const range of selection.ranges) {
    const start = buffer.lineRangeAt(range.from).index;
    const endOffset = range.isCollapsed
      ? range.to
      : Math.max(range.from, previousBoundaryBuffer(buffer, range.to));
    const endLine = buffer.lineRangeAt(endOffset).index;
    const from = Math.min(Math.max(start, minNext), buffer.lineCount);
    const to = Math.min(endLine + 1, buffer.lineCount);
    for (let i = from; i < to; i++) out.push(i);
    minNext = to;
  }
  return out;
}
function analyzeLine(buffer: TextBuffer, token: string, lines: readonly number[]) {
  let commented = true,
    saw = false,
    minColumn = Number.MAX_SAFE_INTEGER,
    margin = 1;
  const changes: Array<{ line: number; col: number }> = [];
  for (const line of lines) {
    const text = buffer.lineText(line);
    const col = firstNonWhitespace(text);
    if (col === undefined) continue;
    saw = true;
    minColumn = Math.min(minColumn, col);
    const rest = text.slice(col);
    if (!rest.startsWith(token)) commented = false;
    else if (text.slice(col + token.length, col + token.length + 1) !== " ") margin = 0;
    changes.push({ line, col });
  }
  return saw
    ? { commented, changes, minColumn, margin }
    : { commented: false, changes: [], minColumn: 0, margin: 1 };
}

export function toggleLineComments(
  buffer: TextBuffer,
  selection: Selection,
  token = DEFAULT_LINE_TOKEN,
): Effect.Effect<Edit, TextError | CommentError> {
  if (token.length === 0) return fail(new InvalidCommentTokenError(token));
  const err = validateSelection(buffer, selection);
  if (err) return fail(err);
  const lines = selectedLines(buffer, selection);
  const a = analyzeLine(buffer, token, lines);
  const text = a.margin === 0 ? token : `${token} `;
  const reps = a.changes.map(({ line, col }) => {
    const start = buffer.lineRange(line).from + (a.commented ? col : a.minColumn);
    return a.commented
      ? Replacement.delete({ start, end: start + token.length + a.margin })
      : Replacement.insert(start, text);
  });
  return Transaction.fromReplacements(buffer.length, reps).pipe(
    Effect.map((transaction) => ({ transaction, selection: selection.map(transaction.desc()) })),
  );
}

function prepareBlockTokens(
  tokens?: readonly BlockCommentToken[],
): Effect.Effect<readonly PreparedBlockToken[], CommentError> {
  const source = tokens && tokens.length > 0 ? tokens : [DEFAULT_BLOCK_TOKEN];
  const out: PreparedBlockToken[] = [];
  for (const t of source) {
    if (t.start.length === 0) return fail(new InvalidCommentTokenError(t.start));
    if (t.end.length === 0) return fail(new InvalidCommentTokenError(t.end));
    out.push({ ...t, startLen: t.start.length, endLen: t.end.length });
  }
  out.sort((a, b) => b.startLen - a.startLen || b.endLen - a.endLen);
  return Effect.succeed(out);
}
function matchingBlockToken(
  tokens: readonly PreparedBlockToken[],
  slice: string,
  startPos: number,
  endPos: number,
) {
  const len = endPos + 1 - startPos;
  for (const t of tokens) {
    if (len < t.startLen + t.endLen) continue;
    const afterStart = startPos + t.startLen;
    const beforeEnd = endPos - t.endLen;
    if (
      slice.slice(startPos, afterStart) === t.start &&
      slice.slice(beforeEnd + 1, endPos + 1) === t.end
    )
      return {
        token: t,
        startMargin: slice.slice(afterStart, afterStart + 1) === " ",
        endMargin: afterStart !== beforeEnd && slice.slice(beforeEnd, beforeEnd + 1) === " ",
      };
  }
  return undefined;
}
function analyzeBlock(
  buffer: TextBuffer,
  selection: Selection,
  tokens: readonly PreparedBlockToken[],
) {
  let commented = true,
    onlyWhitespace = true;
  const changes: CommentChange[] = [];
  const def = tokens[0]!;
  for (const range of selection.ranges) {
    const slice = buffer.sliceString(range.from, range.to);
    const startPos = firstNonWhitespace(slice);
    const endPos = lastNonWhitespace(slice);
    if (startPos === undefined || endPos === undefined) {
      changes.push({ type: "whitespace", range });
      continue;
    }
    onlyWhitespace = false;
    const match = matchingBlockToken(tokens, slice, startPos, endPos);
    if (match)
      changes.push({
        type: "commented",
        range,
        startPos,
        endPos,
        startMargin: match.startMargin,
        endMargin: match.endMargin,
        startToken: match.token.start,
        endToken: match.token.end,
      });
    else {
      commented = false;
      changes.push({
        type: "uncommented",
        range,
        startPos,
        endPos,
        startToken: def.start,
        endToken: def.end,
      });
    }
  }
  return { commented: onlyWhitespace ? false : commented, changes };
}
function blockReplacements(changes: readonly CommentChange[], commented: boolean): Replacement[] {
  const reps: Replacement[] = [];
  for (const c of changes) {
    if (commented && c.type === "commented") {
      const from = c.range.from;
      reps.push(
        Replacement.delete({
          start: from + c.startPos,
          end: from + c.startPos + c.startToken.length + Number(c.startMargin),
        }),
      );
      reps.push(
        Replacement.delete({
          start: from + c.endPos - c.endToken.length - Number(c.endMargin) + 1,
          end: from + c.endPos + 1,
        }),
      );
    } else if (!commented && c.type === "uncommented") {
      const from = c.range.from;
      reps.push(Replacement.insert(from + c.startPos, `${c.startToken} `));
      reps.push(Replacement.insert(from + c.endPos + 1, ` ${c.endToken}`));
    }
  }
  return reps;
}
const shiftRange = (range: SelectionRange, offset: number) =>
  SelectionRange.create(range.from + offset, range.to + offset).withDirection(range.direction);
function blockSelectionAfterComment(
  selection: Selection,
  changes: readonly CommentChange[],
): Selection {
  const ranges: SelectionRange[] = [];
  let offset = 0;
  for (const c of changes) {
    if (c.type === "uncommented") {
      const prefixLen = c.startToken.length + 1;
      const start = c.range.from + offset + c.startPos + prefixLen;
      const end = c.range.from + offset + c.endPos + 1 + prefixLen;
      ranges.push(SelectionRange.create(start, end).withDirection(c.range.direction));
      offset += prefixLen + 1 + c.endToken.length;
    } else ranges.push(shiftRange(c.range, offset));
  }
  return Selection.create(ranges, { primaryIndex: selection.primaryIndex });
}

export function toggleBlockComments(
  buffer: TextBuffer,
  selection: Selection,
  tokens?: readonly BlockCommentToken[],
): Effect.Effect<Edit, TextError | CommentError> {
  const err = validateSelection(buffer, selection);
  if (err) return fail(err);
  return prepareBlockTokens(tokens).pipe(
    Effect.flatMap((prepared) => {
      const a = analyzeBlock(buffer, selection, prepared);
      const reps = blockReplacements(a.changes, a.commented);
      return Transaction.fromReplacements(buffer.length, reps).pipe(
        Effect.map((transaction) => ({
          transaction,
          selection: a.commented
            ? selection.map(transaction.desc())
            : blockSelectionAfterComment(selection, a.changes),
        })),
      );
    }),
  );
}
