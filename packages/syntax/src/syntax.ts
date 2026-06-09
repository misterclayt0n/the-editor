import Parser from "tree-sitter";
import type { TextBuffer, Transaction } from "@the-editor/engine";

export interface SyntaxText {
  readonly length: number;
  readonly sliceString: (from: number, to: number) => string;
}
export const syntaxTextFromString = (text: string): SyntaxText => ({
  length: text.length,
  sliceString: (from, to) => text.slice(from, to),
});
export const syntaxTextFromTextBuffer = (buffer: TextBuffer): SyntaxText => ({
  length: buffer.length,
  sliceString: (from, to) => buffer.sliceString(from, to),
});
const textAll = (text: SyntaxText): string => text.sliceString(0, text.length);
const bytes = (s: string): number => Buffer.byteLength(s, "utf8");
export type Point = { readonly row: number; readonly column: number };
export type ByteRange = { readonly start: number; readonly end: number };
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
export class IndentQuery {
  constructor(readonly query: Parser.Query) {}
}
export class TagQuery {
  constructor(readonly query: Parser.Query) {}
}
export class RainbowQuery {
  constructor(
    readonly query: Parser.Query,
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
  constructor(readonly config: LanguageSyntaxConfig) {}
  grammar(registry: LanguageRegistry) {
    return registry.resources.language(grammarName(this.config));
  }
  private compile(registry: LanguageRegistry, kind: QueryKind) {
    const lang = this.grammar(registry);
    if (!lang) return undefined;
    const src = registry.queryText(this.config.name, kind);
    if (!src) return undefined;
    try {
      return new Parser.Query(lang as any, src);
    } catch (e) {
      throw new QueryCompileError(String(e));
    }
  }
  highlightQuery(registry: LanguageRegistry) {
    if (this.h) return this.h;
    const q = this.compile(registry, "highlights");
    if (!q) return undefined;
    {
      const names = captureNamesFromQuery(registry.queryText(this.config.name, "highlights"));
      this.h = new HighlightQuery(
        q,
        names,
        names.map((n: string) => registry.vocabulary.intern(n)),
      );
    }
    return this.h;
  }
  indentQuery(r: LanguageRegistry) {
    const q = this.compile(r, "indents");
    return q && new IndentQuery(q);
  }
  textObjectQuery(r: LanguageRegistry) {
    const q = this.compile(r, "textobjects");
    return q && new TextObjectQuery(q);
  }
  tagQuery(r: LanguageRegistry) {
    const q = this.compile(r, "tags");
    return q && new TagQuery(q);
  }
  rainbowQuery(r: LanguageRegistry) {
    const q = this.compile(r, "rainbows");
    return q && new RainbowQuery(q);
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
      if (r.byScope.has(c.scope)) throw new DuplicateScopeError(c.scope);
      r.byName.set(c.name, id);
      r.byScope.set(c.scope, id);
      for (const ft of c.fileTypes) {
        if (typeof ft === "string") {
          if (r.byExt.has(ft)) throw new DuplicateExtensionError(ft);
          r.byExt.set(ft, id);
        }
      }
      for (const s of c.shebangs ?? []) {
        if (r.byShebang.has(s)) throw new DuplicateShebangError(s);
        r.byShebang.set(s, id);
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
    const line = text.sliceString(0, Math.min(text.length, 256)).split(/\r?\n/, 1)[0] ?? "";
    for (const [s, id] of this.byShebang) if (line.includes(s)) return id;
    return undefined;
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
    return (
      (args.path && this.assignByFilename(args.path)) ||
      this.assignByShebang(args.text) ||
      this.assignByMatch(args.text)
    );
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
  treeForByteRange(_s: number, _e: number) {
    return this.tree;
  }
  descendantForByteRange(s: number, e: number) {
    return this.tree.rootNode.descendantForIndex(s, e);
  }
  namedDescendantForByteRange(s: number, e: number) {
    return this.tree.rootNode.namedDescendantForIndex(s, e);
  }
  walk() {
    return this.tree.walk();
  }
  scopesAt(_text: SyntaxText, utf16Offset: number) {
    const b = utf16OffsetToUtf8ByteOffset(_text, utf16Offset);
    const n = this.namedDescendantForByteRange(b, b);
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
    const def = args.registry.language(this.languageId);
    const h = def?.highlightQuery(args.registry);
    if (!h) return [];
    const caps = h.query
      .captures(this.tree.rootNode)
      .filter(
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
}
export function normalizeHighlightRuns(
  captures: readonly SyntaxHighlightRun[],
): readonly SyntaxHighlightRun[] {
  if (captures.length === 0) return [];
  const boundaries = [
    ...new Set(captures.flatMap((run) => [run.byteRange.start, run.byteRange.end])),
  ].sort((a, b) => a - b);
  const out: SyntaxHighlightRun[] = [];
  for (let i = 0; i < boundaries.length - 1; i++) {
    const start = boundaries[i]!;
    const end = boundaries[i + 1]!;
    if (start >= end) continue;
    const candidates = captures.filter(
      (run) => run.byteRange.start <= start && end <= run.byteRange.end,
    );
    if (candidates.length === 0) continue;
    candidates.sort(
      (a, b) => b.byteRange.start - a.byteRange.start || a.byteRange.end - b.byteRange.end,
    );
    const selected = candidates[0]!;
    const last = out[out.length - 1];
    if (last && last.kind === selected.kind && last.byteRange.end === start) {
      out[out.length - 1] = {
        byteRange: { start: last.byteRange.start, end },
        kind: selected.kind,
      };
    } else {
      out.push({ byteRange: { start, end }, kind: selected.kind });
    }
  }
  return out;
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
    const startIndex = utf16OffsetToUtf8ByteOffset(oldText, e.start);
    const oldEndIndex = utf16OffsetToUtf8ByteOffset(oldText, e.end);
    const startPosition = utf16OffsetToPoint(oldText, e.start);
    return {
      startIndex,
      oldEndIndex,
      newEndIndex: startIndex + bytes(e.text),
      startPosition,
      oldEndPosition: utf16OffsetToPoint(oldText, e.end),
      newEndPosition: pointExtent(startPosition, e.text),
    };
  });
}
export function syntaxTextEditsFromTransaction(
  transaction: Transaction,
): readonly SyntaxTextEdit[] {
  return transaction.changes
    .toLegacyReplacements()
    .map((r) => ({ start: r.from, end: r.to, text: r.text }));
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
