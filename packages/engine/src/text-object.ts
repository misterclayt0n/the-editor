import type { CapturedNode, Syntax, TextObjectQuery } from "@the-editor/syntax";
import {
  categorizeChar,
  charIsWhitespace,
  codePointStringAt,
  nextCodePointBoundary,
  previousCodePointBoundary,
} from "./chars";
import { nextGraphemeBoundary } from "./grapheme";
import { Selection, SelectionRange } from "./selection";
import { findNthClosestSurroundPair, findNthSurroundPair, SurroundError } from "./surround";
import type { TextBuffer } from "./text/buffer";
import { syntaxTextFromTextBuffer } from "./syntax-adapter";

export type TextObjectKind = "around" | "inside";
export type TextObjectDirection = "forward" | "backward";
export class TextObjectError extends Error {}
export class InvalidTextObjectCharError extends TextObjectError {
  constructor(readonly value: string) {
    super(`text-object char must be exactly one Unicode code point: ${JSON.stringify(value)}`);
  }
}

function count(n?: number) {
  return Math.max(1, n ?? 1);
}
function withDirection(r: SelectionRange, d: "forward" | "backward") {
  return r.withDirection(d);
}
function mapSelectionRanges(
  selection: Selection,
  f: (range: SelectionRange) => SelectionRange,
): Selection {
  return Selection.create(selection.ranges.map(f), {
    primaryIndex: selection.primaryIndex,
    normalize: false,
  });
}
function rangeCursor(_text: TextBuffer, range: SelectionRange) {
  return range.head;
}
function isWs(ch: string) {
  const c = categorizeChar(ch);
  return c === "whitespace" || c === "eol";
}
function charAt(text: TextBuffer, pos: number) {
  return codePointStringAt(text.toString(), pos);
}
function categoryAt(text: TextBuffer, pos: number) {
  const ch = charAt(text, pos);
  return ch ? categorizeChar(ch) : undefined;
}

function findWordBoundary(
  text: TextBuffer,
  pos0: number,
  direction: TextObjectDirection,
  long: boolean,
): number {
  const s = text.toString();
  let pos = pos0;
  let prev =
    direction === "forward"
      ? pos === 0
        ? "whitespace"
        : categorizeChar(codePointStringAt(s, previousCodePointBoundary(s, pos))!)
      : pos === s.length
        ? "whitespace"
        : categorizeChar(codePointStringAt(s, pos)!);
  while (direction === "forward" ? pos < s.length : pos > 0) {
    const p = direction === "forward" ? pos : previousCodePointBoundary(s, pos);
    const ch = codePointStringAt(s, p);
    if (!ch) break;
    const cat = categorizeChar(ch);
    if (cat === "whitespace" || cat === "eol") return pos;
    if (!long && cat !== prev && pos !== 0 && pos !== s.length) return pos;
    pos = direction === "forward" ? nextCodePointBoundary(s, pos) : p;
    prev = cat;
  }
  return pos;
}
function currentWordEnd(text: TextBuffer, pos: number, long: boolean): number | undefined {
  const cat = categoryAt(text, pos);
  if (!cat || cat === "whitespace" || cat === "eol") return undefined;
  return findWordBoundary(text, nextCodePointBoundary(text.toString(), pos), "forward", long);
}
function nextWordStart(text: TextBuffer, pos0: number): number | undefined {
  const s = text.toString();
  let pos = pos0;
  while (pos < s.length) {
    const ch = codePointStringAt(s, pos)!;
    if (!isWs(ch)) return pos;
    pos = nextCodePointBoundary(s, pos);
  }
  return undefined;
}
function aroundWordRange(text: TextBuffer, range: SelectionRange, start: number, end: number) {
  const s = text.toString();
  let right = end;
  while (right < s.length) {
    const ch = codePointStringAt(s, right)!;
    if (!charIsWhitespace(ch) && categorizeChar(ch) !== "eol") break;
    right = nextCodePointBoundary(s, right);
  }
  if (right > end) return withDirection(SelectionRange.create(start, right), range.direction);
  let left = start;
  while (left > 0) {
    const p = previousCodePointBoundary(s, left);
    const ch = codePointStringAt(s, p)!;
    if (!charIsWhitespace(ch) && categorizeChar(ch) !== "eol") break;
    left = p;
  }
  return withDirection(SelectionRange.create(left, end), range.direction);
}
function wordRange(
  text: TextBuffer,
  range: SelectionRange,
  kind: TextObjectKind,
  n: number,
  long: boolean,
) {
  const pos = rangeCursor(text, range);
  const start = findWordBoundary(text, pos, "backward", long);
  let end = currentWordEnd(text, pos, long);
  if (end === undefined) return withDirection(SelectionRange.point(pos), range.direction);
  for (let i = 1; i < n; i++) {
    const ns = nextWordStart(text, end);
    if (ns === undefined) break;
    const ne = currentWordEnd(text, ns, long);
    if (ne === undefined) break;
    end = ne;
  }
  return kind === "inside"
    ? withDirection(SelectionRange.create(start, end), range.direction)
    : aroundWordRange(text, range, start, end);
}
export function wordTextObject(args: {
  readonly text: TextBuffer;
  readonly selection: Selection;
  readonly kind: TextObjectKind;
  readonly count?: number;
  readonly long?: boolean;
}): Selection {
  return mapSelectionRanges(args.selection, (r) =>
    wordRange(args.text, r, args.kind, count(args.count), args.long ?? false),
  );
}

function lineCount(text: TextBuffer) {
  return text.lineCount;
}
function lineStartOffset(text: TextBuffer, line: number) {
  return line <= 0 ? 0 : line >= text.lineCount ? text.length : text.lineRange(line).from;
}
function lineEndOffset(text: TextBuffer, line: number) {
  const l = text.lineRange(Math.min(line, text.lineCount - 1));
  return l.to;
}
function lineText(text: TextBuffer, line: number) {
  return line >= text.lineCount ? "" : text.lineText(line);
}
function lineIndexAtOffset(text: TextBuffer, offset: number) {
  return text.lineRangeAt(offset).index;
}
function isEmptyLine(text: TextBuffer, line: number) {
  return line >= lineCount(text) || lineText(text, line).length === 0;
}
function nextNonEmptyLine(text: TextBuffer, line: number): number | undefined {
  while (line < lineCount(text) && isEmptyLine(text, line)) line++;
  return line < lineCount(text) ? line : undefined;
}
function prevNonEmptyLine(text: TextBuffer, line: number): number | undefined {
  while (line > 0 && isEmptyLine(text, line)) line--;
  return !isEmptyLine(text, line) ? line : undefined;
}
function paragraphStart(text: TextBuffer, line: number) {
  while (line > 0 && !isEmptyLine(text, line - 1)) line--;
  return line;
}
function paragraphEndLines(text: TextBuffer, start: number, n: number) {
  let line = start,
    inside = start,
    around = start,
    remaining = n;
  while (remaining-- > 0) {
    while (line < lineCount(text) && !isEmptyLine(text, line)) line++;
    inside = line;
    while (line < lineCount(text) && isEmptyLine(text, line)) line++;
    around = line;
    if (line >= lineCount(text)) break;
  }
  return { inside, around };
}
function paragraphRange(text: TextBuffer, range: SelectionRange, kind: TextObjectKind, n: number) {
  const cursor = rangeCursor(text, range);
  let line = lineIndexAtOffset(text, cursor);
  if (isEmptyLine(text, line)) {
    const next = nextNonEmptyLine(text, line);
    const prev = prevNonEmptyLine(text, Math.max(0, line - 1));
    if (next !== undefined) line = next;
    else if (prev !== undefined) line = prev;
    else return withDirection(SelectionRange.point(cursor), range.direction);
  }
  const startLine = paragraphStart(text, line);
  const ends = paragraphEndLines(text, startLine, n);
  const endLine = kind === "inside" ? ends.inside : ends.around;
  return withDirection(
    SelectionRange.create(lineStartOffset(text, startLine), lineStartOffset(text, endLine)),
    range.direction,
  );
}
export function paragraphTextObject(args: {
  readonly text: TextBuffer;
  readonly selection: Selection;
  readonly kind: TextObjectKind;
  readonly count?: number;
}): Selection {
  return mapSelectionRanges(args.selection, (r) =>
    paragraphRange(args.text, r, args.kind, count(args.count)),
  );
}

function pairRange(
  syntax: Syntax | undefined,
  text: TextBuffer,
  range: SelectionRange,
  kind: TextObjectKind,
  ch: string | undefined,
  n: number,
) {
  try {
    const pair =
      ch === undefined
        ? findNthClosestSurroundPair(
            syntaxTextFromTextBuffer(text),
            range,
            n,
            syntax ? { syntax } : {},
          )
        : findNthSurroundPair(syntaxTextFromTextBuffer(text), ch, range, n);
    const r =
      kind === "inside"
        ? SelectionRange.create(nextGraphemeBoundary(text.toString(), pair.open), pair.close)
        : SelectionRange.create(pair.open, nextGraphemeBoundary(text.toString(), pair.close));
    return withDirection(r, range.direction);
  } catch (e) {
    if (e instanceof SurroundError) return range;
    throw e;
  }
}
export function pairTextObject(args: {
  readonly syntax?: Syntax;
  readonly text: TextBuffer;
  readonly selection: Selection;
  readonly kind: TextObjectKind;
  readonly ch: string;
  readonly count?: number;
}): Selection {
  if (Array.from(args.ch).length !== 1) throw new InvalidTextObjectCharError(args.ch);
  return mapSelectionRanges(args.selection, (r) =>
    pairRange(args.syntax, args.text, r, args.kind, args.ch, count(args.count)),
  );
}
export function closestPairTextObject(args: {
  readonly syntax?: Syntax;
  readonly text: TextBuffer;
  readonly selection: Selection;
  readonly kind: TextObjectKind;
  readonly count?: number;
}): Selection {
  return mapSelectionRanges(args.selection, (r) =>
    pairRange(args.syntax, args.text, r, args.kind, undefined, count(args.count)),
  );
}

function capturedStart(n: CapturedNode): number {
  return n.type === "single" ? n.startByte : (n.nodes[0]?.startByte ?? 0);
}
function capturedEnd(n: CapturedNode): number {
  return n.type === "single" ? n.endByte : (n.nodes[n.nodes.length - 1]?.endByte ?? 0);
}
function syntaxNamedRange(
  text: TextBuffer,
  range: SelectionRange,
  captureName: string,
  query: TextObjectQuery,
  root: any,
): SelectionRange | undefined {
  const cursor = range.head;
  const node = query
    .captureNodes(captureName, root, syntaxTextFromTextBuffer(text))
    .filter((n) => capturedStart(n) <= cursor && cursor < capturedEnd(n))
    .sort((a, b) => capturedEnd(a) - capturedStart(a) - (capturedEnd(b) - capturedStart(b)))[0];
  return node
    ? withDirection(SelectionRange.create(capturedStart(node), capturedEnd(node)), range.direction)
    : undefined;
}
export function syntaxNamedTextObject(args: {
  readonly syntax: Syntax;
  readonly query: TextObjectQuery;
  readonly text: TextBuffer;
  readonly selection: Selection;
  readonly kind: TextObjectKind;
  readonly objectName: string;
}): Selection {
  const capture = `${args.objectName}.${args.kind}`;
  return mapSelectionRanges(
    args.selection,
    (r) => syntaxNamedRange(args.text, r, capture, args.query, args.syntax.rootNode()) ?? r,
  );
}
function nextSyntaxNamedRange(
  text: TextBuffer,
  range: SelectionRange,
  dir: TextObjectDirection,
  query: TextObjectQuery,
  root: any,
  around: string,
  inside: string,
): SelectionRange | undefined {
  const cursor = range.head;
  const nodes = query.captureNodesAny([around, inside], root, syntaxTextFromTextBuffer(text));
  const node =
    dir === "forward"
      ? nodes
          .filter((n) => capturedStart(n) > cursor)
          .sort((a, b) => capturedStart(a) - capturedStart(b) || capturedEnd(b) - capturedEnd(a))[0]
      : nodes
          .filter((n) => capturedEnd(n) < cursor)
          .sort(
            (a, b) => capturedEnd(b) - capturedEnd(a) || capturedStart(b) - capturedStart(a),
          )[0];
  return node
    ? withDirection(SelectionRange.create(capturedStart(node), capturedEnd(node)), range.direction)
    : undefined;
}
export function moveSyntaxNamedTextObject(args: {
  readonly syntax: Syntax;
  readonly query: TextObjectQuery;
  readonly text: TextBuffer;
  readonly selection: Selection;
  readonly objectName: string;
  readonly direction: TextObjectDirection;
  readonly count?: number;
}): Selection {
  const around = `${args.objectName}.around`,
    inside = `${args.objectName}.inside`,
    n = count(args.count);
  return mapSelectionRanges(args.selection, (r) => {
    let cur = r;
    for (let i = 0; i < n; i++) {
      const next = nextSyntaxNamedRange(
        args.text,
        cur,
        args.direction,
        args.query,
        args.syntax.rootNode(),
        around,
        inside,
      );
      if (!next || (next.anchor === cur.anchor && next.head === cur.head)) break;
      cur = next;
    }
    return cur;
  });
}
