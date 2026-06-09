import type {
  IndentQuery,
  IndentQueryPredicates,
  IndentScope,
  Syntax,
  SyntaxText,
} from "@the-editor/syntax";

export type IndentStyle =
  | { readonly type: "tabs" }
  | { readonly type: "spaces"; readonly size: number };
export const MAX_INDENT = 16;
const INDENTS = "                ";
const clampIndent = (n: number) => Math.max(1, Math.min(MAX_INDENT, Math.trunc(n || 1)));
export const IndentStyle = {
  tabs: (): IndentStyle => ({ type: "tabs" }),
  spaces: (size: number): IndentStyle => ({ type: "spaces", size: clampIndent(size) }),
  fromString: (indent: string): IndentStyle =>
    indent.startsWith(" ")
      ? { type: "spaces", size: clampIndent(indent.length) }
      : { type: "tabs" },
  asString: (style: IndentStyle): string =>
    style.type === "tabs" ? "\t" : INDENTS.slice(0, clampIndent(style.size)),
  width: (style: IndentStyle, tabWidth: number): number =>
    style.type === "tabs" ? tabWidth : clampIndent(style.size),
};
export type IndentationHeuristic = "simple" | "treeSitter" | "hybrid";
export type IndentCaptureType =
  | { readonly type: "indent" }
  | { readonly type: "indentAlways" }
  | { readonly type: "outdent" }
  | { readonly type: "outdentAlways" }
  | { readonly type: "align"; readonly whitespace: string };

const tabAt = (col: number, tabWidth: number) => tabWidth - (col % tabWidth || 0);
export function whitespaceWidth(text: string, tabWidth: number): number {
  let col = 0;
  for (const ch of text) col += ch === "\t" ? tabAt(col, tabWidth) : 1;
  return col;
}
export function whitespacePrefixForWidth(text: string, target: number, tabWidth: number): number {
  let col = 0,
    end = 0;
  for (let i = 0; i < text.length; ) {
    const ch = text[i]!;
    const len = ch.charCodeAt(0) >= 0xd800 && ch.charCodeAt(0) <= 0xdbff ? 2 : 1;
    const next = col + (ch === "\t" ? tabAt(col, tabWidth) : 1);
    if (next > target) break;
    col = next;
    end = i + len;
    i += len;
  }
  return end;
}
export function indentLevelForLine(line: string, tabWidth: number, indentWidth: number): number {
  if (indentWidth === 0) return 0;
  let col = 0;
  for (const ch of line) {
    if (ch === "\t") col += tabAt(col, tabWidth);
    else if (ch === " ") col++;
    else break;
  }
  return Math.floor(col / indentWidth);
}
export function normalizeIndentation(args: {
  readonly prefix: string;
  readonly line: string;
  readonly indentStyle: IndentStyle;
  readonly tabWidth: number;
}): { readonly text: string; readonly consumed: number } {
  const off = whitespaceWidth(args.prefix, args.tabWidth);
  let width = 0,
    consumed = 0;
  for (let i = 0; i < args.line.length; i++) {
    const ch = args.line[i]!;
    if (ch === "\t") width += tabAt(off + width, args.tabWidth);
    else if (ch === " ") width++;
    else break;
    consumed++;
  }
  let text = "";
  if (args.indentStyle.type === "tabs") {
    text += "\t".repeat(Math.floor(width / args.tabWidth));
    width %= args.tabWidth;
  }
  text += " ".repeat(width);
  return { text, consumed };
}

function lineCount(text: SyntaxText): number {
  let n = 1;
  for (let p = 0; p < text.length; p += 4096)
    n += text.sliceString(p, Math.min(text.length, p + 4096)).match(/\n/g)?.length ?? 0;
  return n;
}
function lineStartOffset(text: SyntaxText, line: number): number {
  if (line <= 0) return 0;
  let row = 0;
  for (let i = 0; i < text.length; i++)
    if (text.sliceString(i, i + 1) === "\n" && ++row === line) return i + 1;
  return text.length;
}
function lineEndOffset(text: SyntaxText, line: number): number {
  const start = lineStartOffset(text, line);
  for (let i = start; i < text.length; i++) {
    const ch = text.sliceString(i, i + 1);
    if (ch === "\n") return i > start && text.sliceString(i - 1, i) === "\r" ? i - 1 : i;
  }
  return text.length;
}
function lineText(text: SyntaxText, line: number): string {
  return text.sliceString(lineStartOffset(text, line), lineEndOffset(text, line));
}
function lineIndexAtOffset(text: SyntaxText, offset: number): number {
  let row = 0;
  for (let i = 0; i < Math.min(offset, text.length); i++)
    if (text.sliceString(i, i + 1) === "\n") row++;
  return row;
}
function leadingWhitespace(line: string): string {
  return line.match(/^[\t ]*/)?.[0] ?? "";
}
function firstNonWhitespaceOffset(line: string): number | undefined {
  const m = /\S/u.exec(line);
  return m?.index;
}

export function autoDetectIndentStyle(text: SyntaxText): IndentStyle | undefined {
  const hist = Array(MAX_INDENT + 1).fill(0) as number[];
  let prevTabs = false,
    prevCount = 0;
  const lines = Math.min(1000, lineCount(text));
  for (let l = 0; l < lines; l++) {
    const line = lineText(text, l);
    if (line.length === 0 || /^\s*$/.test(line)) continue;
    const first = line[0];
    if (first !== "\t" && first !== " ") {
      prevTabs = false;
      prevCount = 0;
      continue;
    }
    const tabs = first === "\t";
    let count = 0,
      weird = false;
    for (const ch of line) {
      if ((tabs && ch === "\t") || (!tabs && ch === " ")) count++;
      else if (ch === "\t" || ch === " ") weird = true;
      else break;
      if (count > 256) {
        weird = true;
        break;
      }
    }
    if (!weird && (prevTabs === tabs || prevCount === 0) && prevCount < count) {
      const inc = count - prevCount;
      if (tabs) hist[0] = hist[0]! + 1;
      else if (inc <= MAX_INDENT) hist[inc] = hist[inc]! + 1;
    }
    prevTabs = tabs;
    prevCount = count;
  }
  hist[0] = hist[0]! * 2;
  if (hist[1]! > 1) hist[1] = Math.floor(hist[1]! / 2);
  let best = 0,
    second = 0,
    idx = 0;
  for (let i = 0; i < hist.length; i++) {
    const v = hist[i]!;
    if (v > best) {
      second = best;
      best = v;
      idx = i;
    } else if (v > second) second = v;
  }
  return best >= 1 && second / best < 0.66
    ? idx === 0
      ? IndentStyle.tabs()
      : IndentStyle.spaces(idx)
    : undefined;
}

function addIndentLevel(base: string, diff: number, style: IndentStyle, tabWidth: number): string {
  if (diff >= 0) return base + IndentStyle.asString(style).repeat(diff);
  const target = Math.max(
    0,
    whitespaceWidth(base, tabWidth) + diff * IndentStyle.width(style, tabWidth),
  );
  return base.slice(0, whitespacePrefixForWidth(base, target, tabWidth));
}
export class Indentation {
  readonly indent: number;
  readonly indentAlways: number;
  readonly outdent: number;
  readonly outdentAlways: number;
  readonly align?: string;
  constructor(
    args: {
      readonly indent?: number;
      readonly indentAlways?: number;
      readonly outdent?: number;
      readonly outdentAlways?: number;
      readonly align?: string;
    } = {},
  ) {
    this.indent = args.indent ?? 0;
    this.indentAlways = args.indentAlways ?? 0;
    this.outdent = args.outdent ?? 0;
    this.outdentAlways = args.outdentAlways ?? 0;
    if (args.align !== undefined) this.align = args.align;
  }
  static empty() {
    return new Indentation();
  }
  addLine(a: Indentation): Indentation {
    if (this.align !== undefined) return this;
    if (a.align !== undefined) return new Indentation({ align: a.align });
    return new Indentation({
      indent: this.indent + a.indent,
      indentAlways: this.indentAlways + a.indentAlways,
      outdent: this.outdent + a.outdent,
      outdentAlways: this.outdentAlways + a.outdentAlways,
    });
  }
  addCapture(c: IndentCaptureType): Indentation {
    if (c.type === "indent")
      return this.indentAlways ? this : new Indentation({ ...this, indent: 1 });
    if (c.type === "indentAlways")
      return new Indentation({ ...this, indent: 0, indentAlways: this.indentAlways + 1 });
    if (c.type === "outdent")
      return this.outdentAlways ? this : new Indentation({ ...this, outdent: 1 });
    if (c.type === "outdentAlways")
      return new Indentation({ ...this, outdent: 0, outdentAlways: this.outdentAlways + 1 });
    return this.align === undefined ? new Indentation({ ...this, align: c.whitespace }) : this;
  }
  netIndent() {
    return this.indent + this.indentAlways - this.outdent - this.outdentAlways;
  }
  relativeIndent(args: {
    readonly otherComputedIndent: Indentation;
    readonly otherLeadingWhitespace: string;
    readonly indentStyle: IndentStyle;
    readonly tabWidth: number;
  }): string | undefined {
    if (this.align !== args.otherComputedIndent.align) return undefined;
    return addIndentLevel(
      args.otherLeadingWhitespace,
      this.netIndent() - args.otherComputedIndent.netIndent(),
      args.indentStyle,
      args.tabWidth,
    );
  }
  toString(args: { readonly indentStyle: IndentStyle; readonly tabWidth: number }): string {
    const base =
      this.align === undefined ? "" : " ".repeat(whitespaceWidth(this.align, args.tabWidth));
    return addIndentLevel(base, this.netIndent(), args.indentStyle, args.tabWidth);
  }
}

type SyntaxNode = any;
type NodeKey = string;
type InternalIndentCapture = {
  readonly captureType: IndentCaptureType;
  readonly scope: IndentScope;
};
type ExtendCapture = "extend" | "preventOnce";
type IndentQueryResult = {
  readonly indentCaptures: Map<NodeKey, readonly InternalIndentCapture[]>;
  readonly extendCaptures: Map<NodeKey, readonly ExtendCapture[]>;
};
const TREE_SITTER_MATCH_LIMIT = 10_000;
function nodeKey(node: SyntaxNode): string {
  const id = node.id;
  if (typeof id === "number" || typeof id === "string") return String(id);
  return `${node.startIndex}:${node.endIndex}:${node.type}:${node.startPosition?.row ?? 0}:${node.startPosition?.column ?? 0}`;
}
const sameNode = (a: SyntaxNode, b: SyntaxNode) => nodeKey(a) === nodeKey(b);
function getNodeStartLine(text: SyntaxText, node: SyntaxNode, newLinePos?: number): number {
  const line = lineIndexAtOffset(text, node.startIndex);
  return newLinePos !== undefined && node.startIndex >= newLinePos ? line + 1 : line;
}
function getNodeEndLine(text: SyntaxText, node: SyntaxNode, newLinePos?: number): number {
  const line = lineIndexAtOffset(text, node.endIndex);
  return newLinePos !== undefined && node.endIndex > newLinePos ? line + 1 : line;
}
function isFirstInLine(args: { readonly node: SyntaxNode; readonly text: SyntaxText; readonly newLinePos?: number }): boolean {
  let start = lineStartOffset(args.text, lineIndexAtOffset(args.text, args.node.startIndex));
  if (args.newLinePos !== undefined && start < args.newLinePos && args.newLinePos <= args.node.startIndex) start = args.newLinePos;
  return /^\s*$/u.test(args.text.sliceString(start, args.node.startIndex));
}
function matchCapture(match: any, name?: string): SyntaxNode | undefined {
  return name ? match.captures?.find((c: any) => c.name === name)?.node : undefined;
}
function predicatesSatisfied(args: { readonly predicates?: IndentQueryPredicates; readonly match: any; readonly text: SyntaxText; readonly newLinePos?: number }): boolean {
  const pred = args.predicates;
  if (!pred) return true;
  for (const p of pred.notKindEq) if (matchCapture(args.match, p.capture)?.type === p.kind) return false;
  if (pred.sameLine) {
    const a = matchCapture(args.match, pred.sameLine.leftCapture), b = matchCapture(args.match, pred.sameLine.rightCapture);
    if (!a || !b || (getNodeStartLine(args.text, a, args.newLinePos) === getNodeStartLine(args.text, b, args.newLinePos)) === pred.sameLine.negated) return false;
  }
  if (pred.oneLine) {
    const a = matchCapture(args.match, pred.oneLine.capture);
    if (!a || (getNodeStartLine(args.text, a, args.newLinePos) === getNodeEndLine(args.text, a, args.newLinePos)) === pred.oneLine.negated) return false;
  }
  return true;
}
function defaultScope(type: IndentCaptureType): IndentScope {
  return type.type === "indent" || type.type === "indentAlways" ? "tail" : "all";
}
function pushMap<K, V>(map: Map<K, readonly V[]>, key: K, value: V) {
  map.set(key, [...(map.get(key) ?? []), value]);
}
function queryIndents(args: { readonly query: IndentQuery; readonly syntax: Syntax; readonly text: SyntaxText; readonly startIndex: number; readonly endIndex: number; readonly newLinePos?: number }): IndentQueryResult {
  const indentCaptures = new Map<NodeKey, readonly InternalIndentCapture[]>();
  const extendCaptures = new Map<NodeKey, readonly ExtendCapture[]>();
  const matches = (args.query.query as any).matches(args.syntax.rootNode(), { startIndex: args.startIndex, endIndex: args.endIndex, matchLimit: TREE_SITTER_MATCH_LIMIT }) as any[];
  for (const match of matches) {
    if (!predicatesSatisfied({ ...(args.query.predicates.has(match.pattern) ? { predicates: args.query.predicates.get(match.pattern)! } : {}), match, text: args.text, ...(args.newLinePos !== undefined ? { newLinePos: args.newLinePos } : {}) })) continue;
    const added: Array<[NodeKey, InternalIndentCapture]> = [];
    let anchor: SyntaxNode | undefined;
    for (const c of match.captures ?? []) {
      const name = c.name as string;
      const node = c.node as SyntaxNode;
      let captureType: IndentCaptureType | undefined;
      if (name === args.query.indentCapture) captureType = { type: "indent" };
      else if (name === args.query.indentAlwaysCapture) captureType = { type: "indentAlways" };
      else if (name === args.query.outdentCapture) captureType = { type: "outdent" };
      else if (name === args.query.outdentAlwaysCapture) captureType = { type: "outdentAlways" };
      else if (name === args.query.alignCapture) captureType = { type: "align", whitespace: "" };
      else if (name === args.query.anchorCapture) { if (!anchor) anchor = node; continue; }
      else if (name === args.query.extendCapture) { pushMap(extendCaptures, nodeKey(node), "extend"); continue; }
      else if (name === args.query.extendPreventOnceCapture) { pushMap(extendCaptures, nodeKey(node), "preventOnce"); continue; }
      else continue;
      added.push([nodeKey(node), { captureType, scope: args.query.properties.get(match.pattern) ?? defaultScope(captureType) }]);
    }
    for (const [key, def] of added) {
      let captureType = def.captureType;
      if (captureType.type === "align") {
        if (!anchor) continue;
        captureType = { type: "align", whitespace: args.text.sliceString(lineStartOffset(args.text, lineIndexAtOffset(args.text, anchor.startIndex)), anchor.startIndex) };
      }
      pushMap(indentCaptures, key, { captureType, scope: def.scope });
    }
  }
  return { indentCaptures, extendCaptures };
}
function extendNodes(args: { readonly node: SyntaxNode; readonly deepestPreceding: SyntaxNode; readonly extendCaptures: ReadonlyMap<NodeKey, readonly ExtendCapture[]>; readonly text: SyntaxText; readonly line: number; readonly tabWidth: number; readonly indentWidth: number }): SyntaxNode {
  let node = args.node, deepest = args.deepestPreceding, stopExtend = false;
  while (!sameNode(deepest, node)) {
    let extendNode = false, nodeCaptured = false;
    for (const capture of args.extendCaptures.get(nodeKey(deepest)) ?? []) {
      if (capture === "preventOnce") stopExtend = true;
      else {
        nodeCaptured = true;
        if (lineIndexAtOffset(args.text, deepest.endIndex) === args.line) extendNode = true;
        else if (indentLevelForLine(lineText(args.text, args.line), args.tabWidth, args.indentWidth) > indentLevelForLine(lineText(args.text, lineIndexAtOffset(args.text, deepest.startIndex)), args.tabWidth, args.indentWidth)) extendNode = true;
      }
    }
    if (nodeCaptured && stopExtend) stopExtend = false;
    else if (extendNode && !stopExtend) return deepest;
    deepest = deepest.parent;
    if (!deepest) return node;
  }
  return node;
}
function lastChild(node: SyntaxNode): SyntaxNode | undefined { return node.childCount > 0 ? node.child(node.childCount - 1) : undefined; }
function initIndentQuery(args: { readonly query: IndentQuery; readonly syntax: Syntax; readonly text: SyntaxText; readonly tabWidth: number; readonly indentWidth: number; readonly line: number; readonly pos: number; readonly newLinePos?: number }): { readonly node: SyntaxNode; readonly indentCaptures: Map<NodeKey, readonly InternalIndentCapture[]> } | undefined {
  let node = (args.syntax.rootNode() as any).descendantForIndex?.(args.pos, args.pos) ?? (args.syntax as any).descendantForByteRange(args.pos, args.pos);
  if (!node) return undefined;
  let deepestPreceding: SyntaxNode | undefined;
  for (const child of node.children ?? []) if (child.endIndex <= args.pos) deepestPreceding = child;
  if (deepestPreceding) while (lastChild(deepestPreceding)) deepestPreceding = lastChild(deepestPreceding);
  const startIndex = deepestPreceding ? Math.max(0, deepestPreceding.endIndex - 1) : args.pos;
  const queryResult = queryIndents({ query: args.query, syntax: args.syntax, text: args.text, startIndex, endIndex: args.pos + 1, ...(args.newLinePos !== undefined ? { newLinePos: args.newLinePos } : {}) });
  if (deepestPreceding) node = extendNodes({ node, deepestPreceding, extendCaptures: queryResult.extendCaptures, text: args.text, line: args.line, tabWidth: args.tabWidth, indentWidth: args.indentWidth });
  return { node, indentCaptures: queryResult.indentCaptures };
}

export function treeSitterIndentForPos(args: {
  readonly query: IndentQuery;
  readonly syntax: Syntax;
  readonly text: SyntaxText;
  readonly tabWidth: number;
  readonly indentWidth: number;
  readonly line: number;
  readonly pos: number;
  readonly newLine: boolean;
}): Indentation | undefined {
  const newLinePos = args.newLine ? args.pos : undefined;
  const init = initIndentQuery({ query: args.query, syntax: args.syntax, text: args.text, tabWidth: args.tabWidth, indentWidth: args.indentWidth, line: args.line, pos: args.pos, ...(newLinePos !== undefined ? { newLinePos } : {}) });
  if (!init) return undefined;
  let node = init.node;
  const indentCaptures = new Map(init.indentCaptures);
  let result = Indentation.empty(), indentForLine = Indentation.empty(), indentForLineBelow = Indentation.empty();
  while (true) {
    const first = isFirstInLine({ node, text: args.text, ...(newLinePos !== undefined ? { newLinePos } : {}) });
    const defs = indentCaptures.get(nodeKey(node));
    if (defs) for (const def of defs) {
      if (def.scope === "all") {
        if (first) indentForLine = indentForLine.addCapture(def.captureType);
        else indentForLineBelow = indentForLineBelow.addCapture(def.captureType);
      } else indentForLineBelow = indentForLineBelow.addCapture(def.captureType);
    }
    const parent = node.parent;
    if (parent) {
      const nodeLine = getNodeStartLine(args.text, node, newLinePos), parentLine = getNodeStartLine(args.text, parent, newLinePos);
      if (nodeLine !== parentLine) {
        if (nodeLine < args.line + (args.newLine ? 1 : 0)) result = result.addLine(indentForLineBelow);
        if (nodeLine === parentLine + 1) indentForLineBelow = indentForLine;
        else { result = result.addLine(indentForLine); indentForLineBelow = Indentation.empty(); }
        indentForLine = Indentation.empty();
      }
      node = parent;
    } else {
      const nodeStartLine = lineIndexAtOffset(args.text, node.startIndex);
      if (nodeStartLine < args.line || (args.newLine && nodeStartLine === args.line && node.startIndex < args.pos)) result = result.addLine(indentForLineBelow);
      result = result.addLine(indentForLine);
      break;
    }
  }
  return result;
}

export function indentForNewline(args: {
  readonly query?: IndentQuery;
  readonly syntax?: Syntax;
  readonly heuristic: IndentationHeuristic;
  readonly indentStyle: IndentStyle;
  readonly tabWidth: number;
  readonly text: SyntaxText;
  readonly lineBefore: number;
  readonly lineBeforeEndPos: number;
  readonly currentLine: number;
}): string {
  const indentWidth = IndentStyle.width(args.indentStyle, args.tabWidth);
  if (args.heuristic !== "simple" && args.query && args.syntax) {
    const indent = treeSitterIndentForPos({
      query: args.query,
      syntax: args.syntax,
      text: args.text,
      tabWidth: args.tabWidth,
      indentWidth,
      line: args.lineBefore,
      pos: args.lineBeforeEndPos,
      newLine: true,
    });
    if (indent) {
      if (args.heuristic === "hybrid") {
        let attempts = 0;
        for (let l = args.lineBefore; l >= 0; l--) {
          const lt = lineText(args.text, l);
          const first = firstNonWhitespaceOffset(lt);
          if (first === undefined) continue;
          attempts++;
          const base = treeSitterIndentForPos({
            query: args.query,
            syntax: args.syntax,
            text: args.text,
            tabWidth: args.tabWidth,
            indentWidth,
            line: l,
            pos: lineStartOffset(args.text, l) + first,
            newLine: false,
          });
          const rel =
            base &&
            indent.relativeIndent({
              otherComputedIndent: base,
              otherLeadingWhitespace: leadingWhitespace(lt),
              indentStyle: args.indentStyle,
              tabWidth: args.tabWidth,
            });
          if (rel !== undefined) return rel;
          if (attempts === 4) break;
        }
      }
      return indent.toString({ indentStyle: args.indentStyle, tabWidth: args.tabWidth });
    }
  }
  return IndentStyle.asString(args.indentStyle).repeat(
    indentLevelForLine(lineText(args.text, args.currentLine), args.tabWidth, indentWidth),
  );
}
