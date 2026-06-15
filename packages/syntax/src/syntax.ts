import Parser from "tree-sitter";
export interface SyntaxText {
  readonly length: number;
  readonly sliceString: (from: number, to: number) => string;
}
export const syntaxTextFromString = (text: string): SyntaxText => ({
  length: text.length,
  sliceString: (from, to) => text.slice(from, to),
});
const textAll = (text: SyntaxText): string => text.sliceString(0, text.length);
const bytes = (s: string): number => Buffer.byteLength(s, "utf8");
export type Point = { readonly row: number; readonly column: number };
export type ByteRange = { readonly start: number; readonly end: number };
// Native node-tree-sitter callback parsing uses JavaScript string indices for
// input callbacks and SyntaxNode startIndex/endIndex. Public editor offsets are
// also UTF-16 string indices, so syntax indices are identity-mapped to UTF-16.
export type SyntaxIndex = number;
export type SyntaxRange = { readonly start: SyntaxIndex; readonly end: SyntaxIndex };
export function utf16OffsetToSyntaxIndex(text: SyntaxText, offset: number): SyntaxIndex {
  assertOffset(text, offset);
  return offset;
}
export function syntaxIndexToUtf16Offset(text: SyntaxText, index: SyntaxIndex): number {
  assertOffset(text, index);
  return index;
}
export function utf16OffsetToUtf8ByteOffset(text: SyntaxText, offset: number): number {
  assertOffset(text, offset);
  return bytes(text.sliceString(0, offset));
}
export function utf8ByteOffsetToUtf16Offset(text: SyntaxText, byteOffset: number): number {
  if (!Number.isInteger(byteOffset) || byteOffset < 0)
    throw new InvalidSyntaxOffsetError(byteOffset, text.length);
  let b = 0;
  for (let i = 0; i < text.length; ) {
    const ch = text.sliceString(i, i + 2);
    const cp = ch.codePointAt(0)!;
    const len = cp > 0xffff ? 2 : 1;
    const part = text.sliceString(i, i + len);
    const nb = bytes(part);
    if (b + nb > byteOffset) return i;
    b += nb;
    i += len;
  }
  if (byteOffset > b) throw new InvalidSyntaxOffsetError(byteOffset, b);
  return text.length;
}
export function utf16OffsetToPoint(text: SyntaxText, offset: number): Point {
  assertOffset(text, offset);
  let row = 0,
    column = 0;
  for (let i = 0; i < offset; ) {
    const ch = text.sliceString(i, i + 1);
    if (ch === "\n") {
      row++;
      column = 0;
      i++;
    } else {
      const cp = text.sliceString(i, i + 2).codePointAt(0)!;
      const len = cp > 0xffff ? 2 : 1;
      column += bytes(text.sliceString(i, i + len));
      i += len;
    }
  }
  return { row, column };
}
export function pointToUtf16Offset(text: SyntaxText, point: Point): number {
  let row = 0;
  let column = 0;
  for (let i = 0; i < text.length; ) {
    if (row === point.row && column >= point.column) return i;
    const ch = text.sliceString(i, i + 1);
    if (ch === "\n") {
      if (row === point.row) return i;
      row++;
      column = 0;
      i++;
    } else {
      const cp = text.sliceString(i, i + 2).codePointAt(0)!;
      const len = cp > 0xffff ? 2 : 1;
      const width = bytes(text.sliceString(i, i + len));
      if (row === point.row && column < point.column && point.column < column + width) return i;
      column += width;
      i += len;
    }
  }
  return text.length;
}
const assertOffset = (text: SyntaxText, offset: number) => {
  if (!Number.isInteger(offset) || offset < 0 || offset > text.length)
    throw new InvalidSyntaxOffsetError(offset, text.length);
};

export type SyntaxConfiguration = { readonly language: readonly LanguageSyntaxConfig[] };
export type LanguageSyntaxConfig = {
  readonly name: string;
  readonly scope: string;
  readonly fileTypes: readonly FileType[];
  readonly shebangs?: readonly string[];
  readonly commentTokens?: readonly string[];
  readonly blockCommentTokens?: readonly BlockCommentToken[];
  readonly textWidth?: number;
  readonly softWrap?: SoftWrap;
  readonly grammar?: string;
  readonly injectionRegex?: RegExp | string;
  readonly indent?: IndentationConfiguration;
  readonly autoPairs?: unknown;
  readonly rulers?: readonly number[];
  readonly rainbowBrackets?: boolean;
};
export type FileType = string | { readonly glob: string };
export type BlockCommentToken = { readonly start: string; readonly end: string };
export type IndentationConfiguration = { readonly tabWidth: number; readonly unit: string };
export type SoftWrap = "none" | "editor-width" | "text-width";
export const grammarName = (config: LanguageSyntaxConfig): string => config.grammar ?? config.name;
export type QueryKind =
  | "highlights"
  | "injections"
  | "locals"
  | "indents"
  | "textobjects"
  | "tags"
  | "rainbows";
export const queryKindFilename = (kind: QueryKind): string => `${kind}.scm`;
export type LanguageId = number & { readonly LanguageId: unique symbol };
export const LanguageId = {
  of(index: number): LanguageId {
    if (!Number.isInteger(index) || index < 0)
      throw new SyntaxConfigError(`invalid language id ${index}`);
    return index as LanguageId;
  },
  get: (id: LanguageId) => id as number,
};
export type HighlightKindId = number & { readonly HighlightKindId: unique symbol };
export const HighlightKindId = {
  of(index: number): HighlightKindId {
    if (!Number.isInteger(index) || index < 0)
      throw new SyntaxConfigError(`invalid highlight id ${index}`);
    return index as HighlightKindId;
  },
  get: (id: HighlightKindId) => id as number,
};
// tree-sitter@0.21 typings do not expose Parser.Language even though the
// runtime API consumes native Language objects. Keep the resource boundary loose
// and cast only when calling native parser/query constructors.
export type NativeTreeSitterLanguage = unknown;
export interface SyntaxResources {
  readonly language: (grammarName: string) => NativeTreeSitterLanguage | undefined;
  readonly query: (languageId: string, kind: QueryKind) => string | undefined;
}
export const NullSyntaxResources: SyntaxResources = {
  language: () => undefined,
  query: () => undefined,
};
export class SyntaxErrorBase extends Error {}
export class DuplicateLanguageIdError extends SyntaxErrorBase {}
export class DuplicateScopeError extends SyntaxErrorBase {}
export class DuplicateExtensionError extends SyntaxErrorBase {}
export class DuplicateShebangError extends SyntaxErrorBase {}
export class MissingGrammarError extends SyntaxErrorBase {}
export class SyntaxConfigError extends SyntaxErrorBase {}
export class QueryCompileError extends SyntaxErrorBase {}
export class BackendInitError extends SyntaxErrorBase {}
export class ParseTimeoutError extends SyntaxErrorBase {}
export class ParseFailedError extends SyntaxErrorBase {}
export class InvalidSyntaxOffsetError extends SyntaxErrorBase {
  constructor(
    readonly offset: number,
    readonly length: number,
  ) {
    super(`invalid syntax offset ${offset}`);
  }
}
export class InvalidSyntaxEditError extends SyntaxErrorBase {}
export type LanguageAssignmentSource =
  | "userOverride"
  | "name"
  | "scope"
  | "filename"
  | "shebang"
  | "glob"
  | "injectionRegex";
export type LanguageAssignment = {
  readonly languageId: LanguageId;
  readonly source: LanguageAssignmentSource;
};

export class HighlightVocabulary {
  private list: string[] = [];
  private map = new Map<string, HighlightKindId>();
  intern(name: string) {
    let id = this.map.get(name);
    if (id !== undefined) return id;
    id = HighlightKindId.of(this.list.length);
    this.list.push(name);
    this.map.set(name, id);
    return id;
  }
  name(id: HighlightKindId) {
    return this.list[HighlightKindId.get(id)];
  }
  names() {
    return this.list;
  }
}
export class HighlightQuery {
  constructor(
    readonly query: Parser.Query,
    readonly captureNames: readonly string[],
    readonly captureKinds: readonly (HighlightKindId | undefined)[],
  ) {}
}
export type IndentScope = "all" | "tail";
export type IndentQueryPredicates = {
  readonly notKindEq: readonly { readonly capture: string; readonly kind: string }[];
  readonly sameLine?: {
    readonly leftCapture: string;
    readonly rightCapture: string;
    readonly negated: boolean;
  };
  readonly oneLine?: { readonly capture: string; readonly negated: boolean };
};
export class IndentQuery {
  constructor(
    readonly query: Parser.Query,
    readonly properties: ReadonlyMap<number, IndentScope>,
    readonly predicates: ReadonlyMap<number, IndentQueryPredicates>,
    readonly indentCapture?: string,
    readonly indentAlwaysCapture?: string,
    readonly outdentCapture?: string,
    readonly outdentAlwaysCapture?: string,
    readonly alignCapture?: string,
    readonly anchorCapture?: string,
    readonly extendCapture?: string,
    readonly extendPreventOnceCapture?: string,
  ) {}
}
export type TagStripPredicate = { readonly capture: string; readonly pattern: string };
export type TagSelectAdjacentPredicate = {
  readonly leftCapture: string;
  readonly rightCapture: string;
};
export class TagQuery {
  constructor(
    readonly query: Parser.Query,
    readonly stripPredicates: ReadonlyMap<number, readonly TagStripPredicate[]>,
    readonly selectAdjacentPredicates: ReadonlyMap<number, readonly TagSelectAdjacentPredicate[]>,
    readonly localPatterns: ReadonlySet<number>,
  ) {}
}
export class RainbowQuery {
  constructor(
    readonly query: Parser.Query,
    readonly includeChildrenPatterns: ReadonlySet<number>,
    readonly scopeCapture?: string,
    readonly bracketCapture?: string,
  ) {}
}
export type CapturedNodeRange = {
  readonly startByte: number;
  readonly endByte: number;
  readonly kind: string;
};
export type CapturedNode =
  | {
      readonly type: "single";
      readonly startByte: number;
      readonly endByte: number;
      readonly kind: string;
    }
  | { readonly type: "grouped"; readonly nodes: readonly CapturedNodeRange[] };
export class TextObjectQuery {
  constructor(readonly query: Parser.Query) {}
  captureNodes(captureName: string, node: Parser.SyntaxNode, text: SyntaxText) {
    return this.captureNodesAny([captureName], node, text);
  }
  captureNodesAny(
    captureNames: readonly string[],
    node: Parser.SyntaxNode,
    _text: SyntaxText,
  ): readonly CapturedNode[] {
    const names = new Set(captureNames);
    return this.query
      .captures(node)
      .filter((c) => names.has(c.name))
      .map((c) => ({
        type: "single",
        startByte: c.node.startIndex,
        endByte: c.node.endIndex,
        kind: c.name,
      }));
  }
}

const captureNamesFromQuery = (query: string): readonly string[] => [
  ...new Set([...query.matchAll(/@([A-Za-z0-9_.-]+)/g)].map((m) => m[1]!)),
];
const captureName = (names: readonly string[], name: string): string | undefined =>
  names.includes(name) ? name : undefined;
type QueryAtom = { readonly type: "capture" | "string" | "symbol"; readonly value: string };
type PredicateForm = {
  readonly name: string;
  readonly args: readonly QueryAtom[];
  readonly start: number;
  readonly end: number;
};
type TopLevelForm = {
  readonly start: number;
  readonly end: number;
  readonly predicates: readonly PredicateForm[];
};
const readStringAtom = (source: string, i: number): { value: string; end: number } => {
  let out = "";
  for (let p = i + 1; p < source.length; p++) {
    const ch = source[p]!;
    if (ch === "\\") {
      out += source[p + 1] ?? "";
      p++;
    } else if (ch === '"') return { value: out, end: p + 1 };
    else out += ch;
  }
  throw new QueryCompileError("unterminated query string");
};
const parsePredicate = (source: string, start: number, end: number): PredicateForm | undefined => {
  let i = start + 1;
  while (/\s/.test(source[i] ?? "")) i++;
  if (source[i] !== "#") return undefined;
  const nameStart = ++i;
  while (i < end && !/[\s()]/.test(source[i]!)) i++;
  const name = source.slice(nameStart, i);
  const args: QueryAtom[] = [];
  while (i < end - 1) {
    while (/\s/.test(source[i] ?? "")) i++;
    if (i >= end - 1) break;
    const ch = source[i]!;
    if (ch === '"') {
      const s = readStringAtom(source, i);
      args.push({ type: "string", value: s.value });
      i = s.end;
    } else {
      const a = i;
      while (i < end - 1 && !/[\s()]/.test(source[i]!)) i++;
      const value = source.slice(a, i);
      args.push({
        type: value.startsWith("@") ? "capture" : "symbol",
        value: value.startsWith("@") ? value.slice(1) : value,
      });
    }
  }
  return { name, args, start, end };
};
const predicateSpansIn = (
  source: string,
  start: number,
  end: number,
): readonly { start: number; end: number }[] => {
  const spans: { start: number; end: number }[] = [];
  const stack: number[] = [];
  let inString = false,
    escape = false,
    comment = false;
  for (let i = start; i < end; i++) {
    const ch = source[i]!;
    if (comment) {
      if (ch === "\n") comment = false;
      continue;
    }
    if (inString) {
      if (escape) escape = false;
      else if (ch === "\\") escape = true;
      else if (ch === '"') inString = false;
      continue;
    }
    if (ch === ";") {
      comment = true;
      continue;
    }
    if (ch === '"') {
      inString = true;
      continue;
    }
    if (ch === "(") stack.push(i);
    else if (ch === ")") {
      const s = stack.pop();
      if (s === undefined) throw new QueryCompileError("unbalanced query parentheses");
      if (
        source
          .slice(s + 1, i)
          .trimStart()
          .startsWith("#")
      )
        spans.push({ start: s, end: i + 1 });
    }
  }
  if (stack.length) throw new QueryCompileError("unbalanced query parentheses");
  return spans;
};
const matchingDelimiter = (source: string, start: number): number => {
  const open = source[start]!;
  const close = open === "(" ? ")" : "]";
  let depth = 0,
    inString = false,
    escape = false,
    comment = false;
  for (let i = start; i < source.length; i++) {
    const ch = source[i]!;
    if (comment) {
      if (ch === "\n") comment = false;
      continue;
    }
    if (inString) {
      if (escape) escape = false;
      else if (ch === "\\") escape = true;
      else if (ch === '"') inString = false;
      continue;
    }
    if (ch === ";") {
      comment = true;
      continue;
    }
    if (ch === '"') {
      inString = true;
      continue;
    }
    if (ch === open) depth++;
    else if (ch === close && --depth === 0) return i + 1;
  }
  throw new QueryCompileError("unbalanced query delimiters");
};
type QueryItem = {
  readonly type: "structural" | "predicate" | "capture" | "symbol";
  readonly start: number;
  readonly end: number;
};
const topLevelItems = (source: string): readonly QueryItem[] => {
  const items: QueryItem[] = [];
  for (let i = 0; i < source.length; ) {
    const ch = source[i]!;
    if (/\s/.test(ch)) {
      i++;
      continue;
    }
    if (ch === ";") {
      while (i < source.length && source[i] !== "\n") i++;
      continue;
    }
    if (ch === "(" || ch === "[") {
      const end = matchingDelimiter(source, i);
      const isPredicate =
        ch === "(" &&
        source
          .slice(i + 1, end - 1)
          .trimStart()
          .startsWith("#");
      items.push({ type: isPredicate ? "predicate" : "structural", start: i, end });
      i = end;
      continue;
    }
    const start = i;
    while (
      i < source.length &&
      !/\s/.test(source[i]!) &&
      source[i] !== ";" &&
      source[i] !== "(" &&
      source[i] !== "["
    )
      i++;
    items.push({ type: source[start] === "@" ? "capture" : "symbol", start, end: i });
  }
  return items;
};
const queryForms = (source: string): readonly TopLevelForm[] => {
  const forms: TopLevelForm[] = [];
  let current:
    | { start: number; end: number; spans: { start: number; end: number }[]; sawPredicate: boolean }
    | undefined;
  const finish = () => {
    if (!current) return;
    forms.push({
      start: current.start,
      end: current.end,
      predicates: current.spans
        .map((s) => parsePredicate(source, s.start, s.end))
        .filter((p): p is PredicateForm => !!p),
    });
    current = undefined;
  };
  for (const item of topLevelItems(source)) {
    if (item.type === "structural") {
      if (current) finish();
      current = {
        start: item.start,
        end: item.end,
        spans: [...predicateSpansIn(source, item.start, item.end)],
        sawPredicate: false,
      };
    } else if (item.type === "capture" || item.type === "symbol") {
      if (current) current.end = item.end;
    } else if (item.type === "predicate") {
      if (!current) continue;
      current.end = item.end;
      current.sawPredicate = true;
      current.spans.push({ start: item.start, end: item.end });
    }
  }
  finish();
  return forms;
};
const stripSpans = (source: string, spans: readonly { start: number; end: number }[]): string => {
  let out = source;
  for (const s of [...spans].sort((a, b) => b.start - a.start))
    out = `${out.slice(0, s.start)}${" ".repeat(s.end - s.start)}${out.slice(s.end)}`;
  return out;
};
const capArg = (p: PredicateForm, i: number) => {
  const a = p.args[i];
  if (a?.type !== "capture")
    throw new QueryCompileError(`predicate #${p.name} expects capture argument ${i + 1}`);
  return a.value;
};
const strArg = (p: PredicateForm, i: number) => {
  const a = p.args[i];
  if (a?.type !== "string")
    throw new QueryCompileError(`predicate #${p.name} expects string argument ${i + 1}`);
  return a.value;
};
const checkArity = (p: PredicateForm, n: number) => {
  if (p.args.length !== n)
    throw new QueryCompileError(`predicate #${p.name} expects ${n} arguments`);
};
const compileQuery = (lang: unknown, source: string) => {
  try {
    return new Parser.Query(lang as any, source);
  } catch (e) {
    throw new QueryCompileError(String(e));
  }
};
export function readQueryWithInherits(
  languageId: string,
  read: (languageId: string) => string,
): string {
  const seen = new Set<string>();
  const visit = (id: string): string => {
    if (seen.has(id)) return "";
    seen.add(id);
    const q = read(id) ?? "";
    const m = q.match(/^\s*;\s*inherits:\s*(.+)$/m);
    const inherited = m
      ? m[1]!
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean)
          .map(visit)
          .join("\n")
      : "";
    return `${inherited}\n${q}`;
  };
  return visit(languageId).trim();
}

export class LanguageDefinition {
  private h?: HighlightQuery;
  private indent?: IndentQuery;
  private textObjects?: TextObjectQuery;
  private tags?: TagQuery;
  private rainbows?: RainbowQuery;
  constructor(readonly config: LanguageSyntaxConfig) {}
  grammar(registry: LanguageRegistry) {
    return registry.resources.language(grammarName(this.config));
  }
  private querySource(registry: LanguageRegistry, kind: QueryKind) {
    return registry.queryText(this.config.name, kind);
  }
  private compileRawQuery(registry: LanguageRegistry, kind: QueryKind) {
    const lang = this.grammar(registry);
    if (!lang) return undefined;
    const src = this.querySource(registry, kind);
    if (!src) return undefined;
    return compileQuery(lang, src);
  }
  highlightQuery(registry: LanguageRegistry) {
    if (this.h) return this.h;
    const q = this.compileRawQuery(registry, "highlights");
    if (!q) return undefined;
    const names = captureNamesFromQuery(this.querySource(registry, "highlights"));
    this.h = new HighlightQuery(
      q,
      names,
      names.map((n: string) => registry.vocabulary.intern(n)),
    );
    return this.h;
  }
  indentQuery(registry: LanguageRegistry) {
    if (this.indent) return this.indent;
    const lang = this.grammar(registry);
    const src = this.querySource(registry, "indents");
    if (!lang || !src) return undefined;
    const forms = queryForms(src);
    const properties = new Map<number, IndentScope>();
    const predicates = new Map<number, IndentQueryPredicates>();
    const strip: { start: number; end: number }[] = [];
    for (let i = 0; i < forms.length; i++)
      for (const p of forms[i]!.predicates) {
        const entry = () => predicates.get(i) ?? { notKindEq: [] };
        if (p.name === "set!" && p.args[0]?.type === "symbol" && p.args[0].value === "scope") {
          checkArity(p, 2);
          const scope = strArg(p, 1);
          if (scope !== "all" && scope !== "tail")
            throw new QueryCompileError(`unknown scope (#set! scope \"${scope}\")`);
          properties.set(i, scope);
          continue;
        }
        if (p.name === "not-kind-eq?") {
          checkArity(p, 2);
          const e = entry();
          predicates.set(i, {
            ...e,
            notKindEq: [...e.notKindEq, { capture: capArg(p, 0), kind: strArg(p, 1) }],
          });
          strip.push(p);
        } else if (p.name === "same-line?" || p.name === "not-same-line?") {
          checkArity(p, 2);
          const e = entry();
          predicates.set(i, {
            ...e,
            sameLine: {
              leftCapture: capArg(p, 0),
              rightCapture: capArg(p, 1),
              negated: p.name === "not-same-line?",
            },
          });
          strip.push(p);
        } else if (p.name === "one-line?" || p.name === "not-one-line?") {
          checkArity(p, 1);
          const e = entry();
          predicates.set(i, {
            ...e,
            oneLine: { capture: capArg(p, 0), negated: p.name === "not-one-line?" },
          });
          strip.push(p);
        } else if (
          ![
            "set!",
            "eq?",
            "not-eq?",
            "any-eq?",
            "any-not-eq?",
            "match?",
            "not-match?",
            "any-match?",
            "any-not-match?",
            "any-of?",
            "not-any-of?",
            "is?",
            "is-not?",
          ].includes(p.name)
        )
          throw new QueryCompileError(`unknown indent predicate #${p.name}`);
      }
    const q = compileQuery(lang, stripSpans(src, strip));
    const names = captureNamesFromQuery(src);
    this.indent = new IndentQuery(
      q,
      properties,
      predicates,
      captureName(names, "indent"),
      captureName(names, "indent.always"),
      captureName(names, "outdent"),
      captureName(names, "outdent.always"),
      captureName(names, "align"),
      captureName(names, "anchor"),
      captureName(names, "extend"),
      captureName(names, "extend.prevent-once"),
    );
    return this.indent;
  }
  textObjectQuery(r: LanguageRegistry) {
    if (this.textObjects) return this.textObjects;
    const q = this.compileRawQuery(r, "textobjects");
    if (!q) return undefined;
    this.textObjects = new TextObjectQuery(q);
    return this.textObjects;
  }
  tagQuery(registry: LanguageRegistry) {
    if (this.tags) return this.tags;
    const lang = this.grammar(registry);
    const src = this.querySource(registry, "tags");
    if (!lang || !src) return undefined;
    const stripPredicates = new Map<number, TagStripPredicate[]>();
    const selectAdjacentPredicates = new Map<number, TagSelectAdjacentPredicate[]>();
    const localPatterns = new Set<number>();
    const strip: { start: number; end: number }[] = [];
    const forms = queryForms(src);
    for (let i = 0; i < forms.length; i++)
      for (const p of forms[i]!.predicates) {
        if (p.name === "strip!") {
          checkArity(p, 2);
          stripPredicates.set(i, [
            ...(stripPredicates.get(i) ?? []),
            { capture: capArg(p, 0), pattern: strArg(p, 1) },
          ]);
          strip.push(p);
        } else if (p.name === "select-adjacent!") {
          checkArity(p, 2);
          selectAdjacentPredicates.set(i, [
            ...(selectAdjacentPredicates.get(i) ?? []),
            { leftCapture: capArg(p, 0), rightCapture: capArg(p, 1) },
          ]);
          strip.push(p);
        } else if (
          p.name === "is?" &&
          p.args.length === 1 &&
          p.args[0]?.type === "symbol" &&
          p.args[0].value === "local"
        )
          localPatterns.add(i);
        // Keep native JS-supported predicates in tag queries for compatibility;
        // custom tag-only predicates above are stripped and normalized.
        else if (
          ![
            "set!",
            "eq?",
            "not-eq?",
            "any-eq?",
            "any-not-eq?",
            "match?",
            "not-match?",
            "any-match?",
            "any-not-match?",
            "any-of?",
            "not-any-of?",
            "is?",
            "is-not?",
          ].includes(p.name)
        )
          throw new QueryCompileError(`unknown tag predicate #${p.name}`);
      }
    this.tags = new TagQuery(
      compileQuery(lang, stripSpans(src, strip)),
      stripPredicates,
      selectAdjacentPredicates,
      localPatterns,
    );
    return this.tags;
  }
  rainbowQuery(registry: LanguageRegistry) {
    if (this.rainbows) return this.rainbows;
    const lang = this.grammar(registry);
    const src = this.querySource(registry, "rainbows");
    if (!lang || !src) return undefined;
    const include = new Set<number>();
    const forms = queryForms(src);
    for (let i = 0; i < forms.length; i++)
      for (const p of forms[i]!.predicates) {
        if (
          p.name === "set!" &&
          p.args[0]?.type === "symbol" &&
          p.args[0].value === "rainbow.include-children"
        ) {
          if (p.args.length !== 1)
            throw new QueryCompileError(
              "property 'rainbow.include-children' does not take an argument",
            );
          include.add(i);
        } else if (
          p.name === "set!" &&
          p.args[0]?.type === "symbol" &&
          p.args[0].value.startsWith("rainbow.")
        )
          throw new QueryCompileError(`unknown rainbow property ${p.args[0].value}`);
        else if (
          ![
            "set!",
            "eq?",
            "not-eq?",
            "any-eq?",
            "any-not-eq?",
            "match?",
            "not-match?",
            "any-match?",
            "any-not-match?",
            "any-of?",
            "not-any-of?",
            "is?",
            "is-not?",
          ].includes(p.name)
        )
          throw new QueryCompileError(`unknown rainbow predicate #${p.name}`);
      }
    const q = compileQuery(lang, src);
    const names = captureNamesFromQuery(src);
    this.rainbows = new RainbowQuery(
      q,
      include,
      captureName(names, "rainbow.scope"),
      captureName(names, "rainbow.bracket"),
    );
    return this.rainbows;
  }
}

export class LanguageRegistry {
  readonly vocabulary = new HighlightVocabulary();
  private defs: LanguageDefinition[] = [];
  private byName = new Map<string, LanguageId>();
  private byScope = new Map<string, LanguageId>();
  private byExt = new Map<string, LanguageId>();
  private byShebang = new Map<string, LanguageId>();
  private constructor(readonly resources: SyntaxResources) {}
  static create(config: SyntaxConfiguration, resources: SyntaxResources) {
    const r = new LanguageRegistry(resources);
    config.language.forEach((c, i) => {
      const id = LanguageId.of(i);
      if (r.byName.has(c.name)) throw new DuplicateLanguageIdError(c.name);
      r.byName.set(c.name, id);
      // Scopes and shebangs legitimately collide in real configurations
      // (helix: json/jsonc share "source.json"); registration order is
      // priority, first claim wins for lookup.
      if (!r.byScope.has(c.scope)) r.byScope.set(c.scope, id);
      for (const ft of c.fileTypes) {
        if (typeof ft === "string") {
          if (r.byExt.has(ft)) throw new DuplicateExtensionError(ft);
          r.byExt.set(ft, id);
        }
      }
      for (const s of c.shebangs ?? []) {
        if (!r.byShebang.has(s)) r.byShebang.set(s, id);
      }
      r.defs.push(new LanguageDefinition(c));
    });
    return r;
  }
  get size() {
    return this.defs.length;
  }
  get isEmpty() {
    return this.defs.length === 0;
  }
  *languages() {
    for (let i = 0; i < this.defs.length; i++) yield [LanguageId.of(i), this.defs[i]!] as const;
  }
  language(id: LanguageId) {
    return this.defs[LanguageId.get(id)];
  }
  languageForName(n: string) {
    return this.byName.get(n);
  }
  languageForScope(s: string) {
    return this.byScope.get(s);
  }
  languageForExtension(e: string) {
    return this.byExt.get(e);
  }
  languageForFilename(path: string) {
    const base = path.split(/[\\/]/).pop() ?? path;
    for (const [i, d] of this.defs.entries())
      for (const ft of d.config.fileTypes)
        if (typeof ft !== "string" && simpleGlob(base, ft.glob)) return LanguageId.of(i);
    const ext = base.includes(".") ? base.split(".").pop()! : base;
    return this.languageForExtension(ext);
  }
  languageForShebang(text: SyntaxText) {
    // Only a real "#!" line counts, and only its interpreter name —
    // substring scans would let any first line claim a language.
    const line = text.sliceString(0, Math.min(text.length, 256)).split(/\r?\n/, 1)[0] ?? "";
    if (!line.startsWith("#!")) return undefined;
    const tokens = line.slice(2).trim().split(/\s+/);
    let interpreter = tokens[0]?.split("/").pop() ?? "";
    if (interpreter === "env") interpreter = tokens[1]?.split("/").pop() ?? "";
    return interpreter.length > 0 ? this.byShebang.get(interpreter) : undefined;
  }
  languageForMatch(text: SyntaxText) {
    const prefix = text.sliceString(0, Math.min(text.length, 4096));
    for (const [i, d] of this.defs.entries()) {
      const re = d.config.injectionRegex;
      if (re && new RegExp(re).test(prefix)) return LanguageId.of(i);
    }
    return undefined;
  }
  assignByName(name: string) {
    const id = this.languageForName(name);
    return id !== undefined ? { languageId: id, source: "name" as const } : undefined;
  }
  assignByScope(scope: string) {
    const id = this.languageForScope(scope);
    return id !== undefined ? { languageId: id, source: "scope" as const } : undefined;
  }
  assignByFilename(path: string) {
    const id = this.languageForFilename(path);
    return id !== undefined ? { languageId: id, source: "filename" as const } : undefined;
  }
  assignByShebang(text: SyntaxText) {
    const id = this.languageForShebang(text);
    return id !== undefined ? { languageId: id, source: "shebang" as const } : undefined;
  }
  assignByMatch(text: SyntaxText) {
    const id = this.languageForMatch(text);
    return id !== undefined ? { languageId: id, source: "injectionRegex" as const } : undefined;
  }
  detectLanguageForDocument(args: { path?: string; text: SyntaxText }) {
    // Filename, then shebang — never injection regexes: those match
    // injection-site LANGUAGE IDENTIFIERS (```rust fences), not document
    // content. Matching them against prose claims plain text for whatever
    // language's pattern happens to occur in the first paragraph
    // (elixir's "(elixir|ex)" matches the word "text").
    return (args.path && this.assignByFilename(args.path)) || this.assignByShebang(args.text);
  }
  grammar(id: LanguageId) {
    return this.language(id)?.grammar(this);
  }
  queryText(languageId: string, kind: QueryKind) {
    return this.resources.query(languageId, kind) ?? "";
  }
  highlightName(id: HighlightKindId) {
    return this.vocabulary.name(id);
  }
  highlightNames() {
    return this.vocabulary.names();
  }
  createSyntax(text: SyntaxText, id: LanguageId) {
    return Syntax.parse({ text, languageId: id, registry: this });
  }
  activateSyntax(text: SyntaxText, a: LanguageAssignment): DocumentSyntaxState {
    try {
      const s = this.createSyntax(text, a.languageId);
      return s
        ? { type: "active", state: SyntaxState.create(a.languageId, s) }
        : { type: "unavailable", reason: { type: "missingGrammar" } };
    } catch {
      return { type: "unavailable", reason: { type: "backendInit" } };
    }
  }
  deferredDocumentSyntax(a: LanguageAssignment): DocumentSyntax {
    return { type: "assigned", assignment: a, state: { type: "deferred" } };
  }
  activateDocumentSyntax(text: SyntaxText, a: LanguageAssignment): DocumentSyntax {
    return { type: "assigned", assignment: a, state: this.activateSyntax(text, a) };
  }
}
const simpleGlob = (name: string, glob: string) =>
  glob.startsWith("*.") ? name.endsWith(glob.slice(1)) : name === glob;

export type SyntaxUnavailableReason =
  | { readonly type: "missingGrammar" }
  | { readonly type: "invalidSyntaxConfig" }
  | { readonly type: "invalidQuery"; readonly kind: QueryKind }
  | { readonly type: "backendInit" };
export type DocumentSyntaxState =
  | { readonly type: "deferred" }
  | { readonly type: "unavailable"; readonly reason: SyntaxUnavailableReason }
  | { readonly type: "active"; readonly state: SyntaxState };
export type DocumentSyntax =
  | { readonly type: "unassigned" }
  | {
      readonly type: "assigned";
      readonly assignment: LanguageAssignment;
      readonly state: DocumentSyntaxState;
    };
export const documentSyntaxAssignment = (s: DocumentSyntax) =>
  s.type === "assigned" ? s.assignment : undefined;
export const documentSyntaxState = (s: DocumentSyntax) =>
  s.type === "assigned" ? s.state : undefined;

export type SyntaxHighlightRun = { readonly byteRange: ByteRange; readonly kind: HighlightKindId };
export class Syntax {
  private injections = true;
  private constructor(
    readonly languageId: LanguageId,
    readonly tree: Parser.Tree,
  ) {}
  static parse(args: {
    text: SyntaxText;
    languageId: LanguageId;
    registry: LanguageRegistry;
    oldTree?: Parser.Tree;
    timeoutMs?: number;
  }) {
    const lang = args.registry.grammar(args.languageId);
    if (!lang) throw new MissingGrammarError();
    const parser = new Parser();
    parser.setLanguage(lang as any);
    // Zed-style parse budget: callers may bound the SYNCHRONOUS cost
    // (incremental reparses after small edits almost always fit) and fall
    // back to an unbounded parse elsewhere on timeout (ParseFailedError).
    if (args.timeoutMs !== undefined) {
      (parser as any).setTimeoutMicros(Math.max(1, Math.round(args.timeoutMs * 1000)));
    }
    // The native Node binding invokes input callbacks with JavaScript string
    // offsets. Keep the callback chunked and avoid materializing whole TextBuffer
    // input; public conversion helpers still expose UTF-8 byte coordinates where
    // syntax APIs need byte-oriented ranges.
    const input = (index: number) =>
      args.text.sliceString(index, Math.min(args.text.length, index + 4096));
    const tree = parser.parse(input, args.oldTree);
    if (!tree) throw new ParseFailedError();
    return new Syntax(args.languageId, tree);
  }
  reparse(args: { text: SyntaxText; registry: LanguageRegistry; timeoutMs?: number }) {
    return Syntax.parse({
      text: args.text,
      registry: args.registry,
      languageId: this.languageId,
      oldTree: this.tree,
      ...(args.timeoutMs !== undefined ? { timeoutMs: args.timeoutMs } : {}),
    });
  }
  tryReparse(args: { text: SyntaxText; registry: LanguageRegistry; timeoutMs?: number }) {
    try {
      return this.reparse(args);
    } catch {
      return undefined;
    }
  }
  interpolate(edits: readonly SyntaxInputEdit[]) {
    const t = this.tree;
    for (const e of edits) t.edit(e as any);
    return new Syntax(this.languageId, t);
  }
  rootNode() {
    return this.tree.rootNode;
  }
  treeForSyntaxRange(_s: SyntaxIndex, _e: SyntaxIndex) {
    return this.tree;
  }
  descendantForSyntaxRange(s: SyntaxIndex, e: SyntaxIndex) {
    return this.tree.rootNode.descendantForIndex(s, e);
  }
  namedDescendantForSyntaxRange(s: SyntaxIndex, e: SyntaxIndex) {
    return this.tree.rootNode.namedDescendantForIndex(s, e);
  }
  /** @deprecated Node tree-sitter indices are SyntaxIndex/UTF-16 offsets for callback input. */
  treeForByteRange(s: number, e: number) {
    return this.treeForSyntaxRange(s, e);
  }
  /** @deprecated Use descendantForSyntaxRange. */
  descendantForByteRange(s: number, e: number) {
    return this.descendantForSyntaxRange(s, e);
  }
  /** @deprecated Use namedDescendantForSyntaxRange. */
  namedDescendantForByteRange(s: number, e: number) {
    return this.namedDescendantForSyntaxRange(s, e);
  }
  walk() {
    return this.tree.walk();
  }
  scopesAt(_text: SyntaxText, utf16Offset: number) {
    const index = utf16OffsetToSyntaxIndex(_text, utf16Offset);
    const n = this.namedDescendantForSyntaxRange(index, index);
    const out: string[] = [];
    let cur = n;
    while (cur) {
      out.unshift(cur.type);
      cur = cur.parent!;
    }
    return out;
  }
  get injectionsEnabled() {
    return this.injections;
  }
  set injectionsEnabled(v: boolean) {
    this.injections = v;
  }
  highlightRunsInByteRange(args: {
    text: SyntaxText;
    registry: LanguageRegistry;
    byteRange: ByteRange;
  }) {
    return highlightRunsForTree({
      tree: this.tree,
      registry: args.registry,
      languageId: this.languageId,
      byteRange: args.byteRange,
    });
  }
}

/**
 * Windowed highlight runs for a raw tree — hosts that manage their own
 * parse loop (persistent parser, resume slicing) use this directly.
 * Range-restricting the cursor makes the query cost O(window), not O(file).
 */
export function highlightRunsForTree(args: {
  tree: Parser.Tree;
  registry: LanguageRegistry;
  languageId: LanguageId;
  byteRange: ByteRange;
}): readonly SyntaxHighlightRun[] {
  const def = args.registry.language(args.languageId);
  const h = def?.highlightQuery(args.registry);
  if (!h) return [];
  // node-tree-sitter 0.21.1 quirk: the binding forwards startIndex RAW
  // (TS bytes = 2x UTF-16 index) but shifts endIndex <<1 itself — so
  // pre-double only the start (src/query.cc:189,192).
  const caps = (
    h.query.captures(args.tree.rootNode, {
      startIndex: args.byteRange.start * 2,
      endIndex: args.byteRange.end,
    } as any) as Parser.QueryCapture[]
  ).filter(
    (c) => c.node.endIndex > args.byteRange.start && c.node.startIndex < args.byteRange.end,
  );
  return normalizeRuns(
    caps
      .map((c) => ({
        byteRange: {
          start: Math.max(c.node.startIndex, args.byteRange.start),
          end: Math.min(c.node.endIndex, args.byteRange.end),
        },
        kind: h.captureKinds[h.captureNames.indexOf(c.name)],
      }))
      .filter((r): r is SyntaxHighlightRun => !!r.kind && r.byteRange.start < r.byteRange.end),
  );
}
/**
 * Flatten overlapping captures into disjoint runs: in every segment the
 * INNERMOST capture wins (latest start; ties to the earliest end), and
 * adjacent same-kind segments merge. Sweep line, O(n log n) — the previous
 * per-boundary candidate scan was O(boundaries x captures) and cost ~20ms
 * on dense windows, ON THE KEYSTROKE PATH.
 */
export function normalizeHighlightRuns(
  captures: readonly SyntaxHighlightRun[],
): readonly SyntaxHighlightRun[] {
  if (captures.length === 0) return [];
  // [position, isStart, captureIndex]; ends sort before starts at a
  // position, so a run ending at p never covers [p, next).
  const events: Array<[number, 0 | 1, number]> = [];
  for (let i = 0; i < captures.length; i++) {
    const range = captures[i]!.byteRange;
    if (range.start >= range.end) continue;
    events.push([range.start, 1, i], [range.end, 0, i]);
  }
  events.sort((a, b) => a[0] - b[0] || a[1] - b[1]);

  const active = new Set<number>();
  const out: Array<{ start: number; end: number; kind: SyntaxHighlightRun["kind"] }> = [];
  let prev = -1;
  let i = 0;
  while (i < events.length) {
    const pos = events[i]![0];
    if (prev !== -1 && pos > prev && active.size > 0) {
      let winner: SyntaxHighlightRun | null = null;
      let winnerIndex = -1;
      for (const index of active) {
        const run = captures[index]!;
        if (
          winner === null ||
          run.byteRange.start > winner.byteRange.start ||
          (run.byteRange.start === winner.byteRange.start &&
            (run.byteRange.end < winner.byteRange.end ||
              (run.byteRange.end === winner.byteRange.end && index > winnerIndex)))
        ) {
          winner = run;
          winnerIndex = index;
        }
      }
      const last = out[out.length - 1];
      if (last !== undefined && last.kind === winner!.kind && last.end === prev) {
        last.end = pos;
      } else {
        out.push({ start: prev, end: pos, kind: winner!.kind });
      }
    }
    while (i < events.length && events[i]![0] === pos) {
      const [, isStart, index] = events[i]!;
      if (isStart === 1) active.add(index);
      else active.delete(index);
      i++;
    }
    prev = pos;
  }
  return out.map((run) => ({ byteRange: { start: run.start, end: run.end }, kind: run.kind }));
}
const normalizeRuns = normalizeHighlightRuns;

export class InvalidatedByteRanges {
  private ranges: ByteRange[] = [];
  get isEmpty() {
    return this.ranges.length === 0;
  }
  get length() {
    return this.ranges.length;
  }
  values() {
    return this.ranges;
  }
  clear() {
    this.ranges = [];
  }
  intersects(r: ByteRange) {
    return this.ranges.some((x) => x.start < r.end && r.start < x.end);
  }
  insert(r: ByteRange) {
    this.ranges.push(r);
    this.ranges.sort((a, b) => a.start - b.start);
    const out: ByteRange[] = [];
    for (const x of this.ranges) {
      const l = out[out.length - 1];
      if (l && x.start <= l.end)
        out[out.length - 1] = { start: l.start, end: Math.max(l.end, x.end) };
      else out.push(x);
    }
    this.ranges = out;
  }
}
export type SyntaxInvalidation = {
  readonly tree: InvalidatedByteRanges;
  readonly baseline: InvalidatedByteRanges;
  readonly enrichment: InvalidatedByteRanges;
};
export type SyntaxGeneration = {
  readonly edit: number;
  readonly tree: number;
  readonly baseline: number;
  readonly enrichment: number;
};
export type SyntaxSnapshot = {
  readonly languageId: LanguageId;
  readonly syntax: Syntax;
  readonly source: SyntaxText;
  readonly editGeneration: number;
};
export class SyntaxState {
  private gen: SyntaxGeneration = { edit: 0, tree: 0, baseline: 0, enrichment: 0 };
  private inv: SyntaxInvalidation = {
    tree: new InvalidatedByteRanges(),
    baseline: new InvalidatedByteRanges(),
    enrichment: new InvalidatedByteRanges(),
  };
  private pending = false;
  private parsed = 0;
  private constructor(
    private lang: LanguageId,
    private syn: Syntax,
  ) {}
  static create(id: LanguageId, s: Syntax) {
    return new SyntaxState(id, s);
  }
  get languageId() {
    return this.lang;
  }
  interpolate(edits: readonly SyntaxInputEdit[]) {
    if (edits.length === 0) return;
    this.syn = this.syn.interpolate(edits);
    this.gen = { ...this.gen, edit: this.gen.edit + 1 };
    this.pending = true;
    for (const e of edits) {
      const range = { start: e.startIndex, end: Math.max(e.oldEndIndex, e.newEndIndex) };
      if (range.start >= range.end) continue;
      this.inv.tree.insert(range);
      this.inv.baseline.insert(range);
      this.inv.enrichment.insert(range);
    }
  }
  snapshot(source: SyntaxText): SyntaxSnapshot {
    return { languageId: this.lang, syntax: this.syn, source, editGeneration: this.gen.edit };
  }
  applyParsed(s: SyntaxSnapshot) {
    if (s.languageId !== this.lang || s.editGeneration !== this.gen.edit) return false;
    this.syn = s.syntax;
    this.gen = {
      edit: this.gen.edit,
      tree: s.editGeneration,
      baseline: s.editGeneration,
      enrichment: s.editGeneration,
    };
    this.inv.tree.clear();
    this.inv.baseline.clear();
    this.inv.enrichment.clear();
    this.pending = false;
    this.parsed = this.gen.edit;
    return true;
  }
  get generation() {
    return this.gen;
  }
  get invalidation() {
    return this.inv;
  }
  get needsReparse() {
    return this.pending || !this.inv.tree.isEmpty;
  }
  get editGeneration() {
    return this.gen.edit;
  }
  get treeGeneration() {
    return this.gen.tree;
  }
  get baselineGeneration() {
    return this.gen.baseline;
  }
  get enrichmentGeneration() {
    return this.gen.enrichment;
  }
  get interpolatedVersion() {
    return this.gen.edit;
  }
  get parsedVersion() {
    return this.parsed;
  }
  get isParsePending() {
    return this.pending;
  }
  setParsePending(p: boolean) {
    this.pending = p;
  }
  get isAccurate() {
    return !this.pending && this.parsed === this.gen.edit;
  }
  syntax() {
    return this.syn;
  }
  replaceSyntax(s: Syntax) {
    this.syn = s;
  }
}

export type SyntaxTextEdit = {
  readonly start: number;
  readonly end: number;
  readonly text: string;
};
export type SyntaxInputEdit = {
  readonly startIndex: number;
  readonly oldEndIndex: number;
  readonly newEndIndex: number;
  readonly startPosition: Point;
  readonly oldEndPosition: Point;
  readonly newEndPosition: Point;
};
const pointExtent = (start: Point, insert: string): Point => {
  let row = start.row,
    col = start.column;
  for (const ch of insert) {
    if (ch === "\n") {
      row++;
      col = 0;
    } else col += bytes(ch);
  }
  return { row, column: col };
};
export function generateSyntaxEdits(
  oldText: SyntaxText,
  edits: readonly SyntaxTextEdit[],
): readonly SyntaxInputEdit[] {
  let last = 0;
  return edits.map((e) => {
    if (e.start < last || e.end < e.start || e.end > oldText.length)
      throw new InvalidSyntaxEditError();
    last = e.end;
    const startIndex = utf16OffsetToSyntaxIndex(oldText, e.start);
    const oldEndIndex = utf16OffsetToSyntaxIndex(oldText, e.end);
    const startPosition = utf16OffsetToPoint(oldText, e.start);
    return {
      startIndex,
      oldEndIndex,
      newEndIndex: startIndex + e.text.length,
      startPosition,
      oldEndPosition: utf16OffsetToPoint(oldText, e.end),
      newEndPosition: pointExtent(startPosition, e.text),
    };
  });
}
export function prettyPrintTree(node: Parser.SyntaxNode, indent = ""): string {
  return `${indent}${node.type} [${node.startIndex}, ${node.endIndex}]\n${node.children.map((c) => prettyPrintTree(c, indent + "  ")).join("")}`;
}
export function childForByteRange(node: Parser.SyntaxNode, range: ByteRange) {
  return node.children.find((c) => c.startIndex <= range.start && c.endIndex >= range.end);
}
export class RuntimeSyntax {
  private constructor(readonly registry: LanguageRegistry) {}
  static disabled() {
    return new RuntimeSyntax(LanguageRegistry.create({ language: [] }, NullSyntaxResources));
  }
  static create(c: SyntaxConfiguration, r: SyntaxResources) {
    return new RuntimeSyntax(LanguageRegistry.create(c, r));
  }
  detectLanguageForPath(args: { path?: string; text: SyntaxText }) {
    return this.registry.detectLanguageForDocument(args);
  }
  activateDocument(args: { text: SyntaxText; path?: string }) {
    const a = this.detectLanguageForPath(args);
    return a ? this.registry.activateDocumentSyntax(args.text, a) : { type: "unassigned" as const };
  }
  advanceDocumentSyntax(args: { syntax: DocumentSyntax; text: SyntaxText; budgetMs?: number }) {
    return this.reparseDocument(args);
  }
  reparseDocument(args: { syntax: DocumentSyntax; text: SyntaxText }) {
    if (args.syntax.type !== "assigned" || args.syntax.state.type !== "active") return false;
    const state = args.syntax.state.state;
    const snapshot = state.snapshot(args.text);
    const next = snapshot.syntax.tryReparse({ text: args.text, registry: this.registry });
    if (!next) return false;
    return state.applyParsed({ ...snapshot, syntax: next });
  }
}
