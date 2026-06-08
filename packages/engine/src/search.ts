import {
  codePointAt,
  codePointStringAt,
  nextCodePointBoundary,
  previousCodePointBoundary,
} from "./chars";
import { SelectionRange } from "./selection";
import { TextBuffer } from "./text/index";

export type SearchText = TextBuffer | string;
export type SearchDirection = "next" | "previous";
export type SearchStart = "inclusive" | "exclusive";
export type CharMatcher = string | ((char: string, offset: number) => boolean);
export type SearchMatch = { readonly from: number; readonly to: number };
export type SearchRegexOptions = {
  readonly smartCase?: boolean;
  readonly caseSensitive?: boolean;
  readonly literal?: boolean;
  readonly unicode?: boolean;
  readonly multiline?: boolean;
};
export type RegexSearchOptions = {
  readonly direction?: SearchDirection;
  readonly start?: SearchStart;
  readonly wrap?: boolean;
};

export class SearchError extends Error {}
export class InvalidSearchOffsetError extends SearchError {
  constructor(
    readonly offset: number,
    readonly length: number,
  ) {
    super(`invalid search offset ${offset} for length ${length}`);
  }
}
export class InvalidSearchCountError extends SearchError {
  constructor(readonly count: number) {
    super(`invalid search count ${count}`);
  }
}
export class InvalidSearchRangeError extends SearchError {
  constructor(
    readonly from: number,
    readonly to: number,
    readonly length: number,
  ) {
    super(`invalid search range ${from}..${to} for length ${length}`);
  }
}
export class EmptySearchQueryError extends SearchError {
  constructor() {
    super("empty search query");
  }
}
export class InvalidSearchMatcherError extends SearchError {
  constructor(readonly matcher: string) {
    super("char matcher string must contain exactly one code point");
  }
}

const snapshotString = (text: SearchText): string =>
  typeof text === "string" ? text : text.toString();
const assertOffset = (text: string, offset: number): void => {
  if (!Number.isInteger(offset) || offset < 0 || offset > text.length)
    throw new InvalidSearchOffsetError(offset, text.length);
};
const assertCount = (count: number): void => {
  if (!Number.isInteger(count) || count < 0) throw new InvalidSearchCountError(count);
};
const assertRange = (text: string, range: SearchMatch): void => {
  if (
    !Number.isInteger(range.from) ||
    !Number.isInteger(range.to) ||
    range.from < 0 ||
    range.to < range.from ||
    range.to > text.length
  )
    throw new InvalidSearchRangeError(range.from, range.to, text.length);
};

const normalizeMatcher = (matcher: CharMatcher): ((char: string, offset: number) => boolean) => {
  if (typeof matcher !== "string") return matcher;
  if (matcher.length === 0 || nextCodePointBoundary(matcher, 0) !== matcher.length)
    throw new InvalidSearchMatcherError(matcher);
  return (char) => char === matcher;
};

export function findNth(
  text: SearchText,
  matcher: CharMatcher,
  pos: number,
  n: number,
  direction: SearchDirection,
  start: SearchStart = "inclusive",
): number | undefined {
  return direction === "next"
    ? findNthNext(text, matcher, pos, n, start)
    : findNthPrevious(text, matcher, pos, n, start);
}

export function findNthNext(
  text: SearchText,
  matcher: CharMatcher,
  pos: number,
  n: number,
  start: SearchStart = "inclusive",
): number | undefined {
  const source = snapshotString(text);
  assertOffset(source, pos);
  assertCount(n);
  if (n === 0) return undefined;
  const matches = normalizeMatcher(matcher);
  let offset = start === "inclusive" ? pos : nextCodePointBoundary(source, pos);
  let seen = 0;
  while (offset < source.length) {
    const char = codePointStringAt(source, offset)!;
    if (matches(char, offset) && ++seen === n) return offset;
    offset = nextCodePointBoundary(source, offset);
  }
  return undefined;
}

export function findNthPrevious(
  text: SearchText,
  matcher: CharMatcher,
  pos: number,
  n: number,
  start: SearchStart = "inclusive",
): number | undefined {
  const source = snapshotString(text);
  assertOffset(source, pos);
  assertCount(n);
  if (n === 0) return undefined;
  const matches = normalizeMatcher(matcher);
  let offset =
    pos === source.length
      ? previousCodePointBoundary(source, pos)
      : start === "inclusive"
        ? pos
        : previousCodePointBoundary(source, pos);
  let seen = 0;
  while (offset >= 0 && offset < source.length) {
    const char = codePointStringAt(source, offset)!;
    if (matches(char, offset) && ++seen === n) return offset;
    if (offset === 0) break;
    offset = previousCodePointBoundary(source, offset);
  }
  return undefined;
}

export function escapeRegexLiteral(query: string): string {
  return query.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&");
}

export function buildSearchRegex(query: string, options?: SearchRegexOptions): RegExp {
  if (query.length === 0) throw new EmptySearchQueryError();
  const source = options?.literal ? escapeRegexLiteral(query) : query;
  let flags = "g";
  if (options?.unicode !== false) flags += "u";
  if (options?.multiline !== false) flags += "m";
  const hasUpper = query.toLocaleLowerCase() !== query;
  const ignoreCase =
    options?.caseSensitive === false ||
    (options?.caseSensitive !== true && options?.smartCase === true && !hasUpper);
  if (ignoreCase) flags += "i";
  return new RegExp(source, flags);
}

const cloneRegexWithGlobal = (regex: RegExp): RegExp => {
  let flags = regex.flags.includes("g") ? regex.flags : regex.flags + "g";
  return new RegExp(regex.source, flags);
};

export function* iterateRegexMatches(
  text: SearchText,
  regex: RegExp,
  range?: SearchMatch,
): IterableIterator<SearchMatch> {
  // Regex search currently materializes a snapshot string. This is acceptable for
  // explicit query operations and can be optimized later with chunked search/indexing.
  const source = snapshotString(text);
  const bounds = range ?? { from: 0, to: source.length };
  assertRange(source, bounds);
  const bounded = source.slice(bounds.from, bounds.to);
  const re = cloneRegexWithGlobal(regex);
  for (;;) {
    const match = re.exec(bounded);
    if (!match) break;
    const from = bounds.from + match.index;
    const to = from + match[0].length;
    if (from >= bounds.from && to <= bounds.to) yield { from, to };
    if (match[0].length === 0) {
      if (re.lastIndex >= bounded.length) break;
      re.lastIndex = nextCodePointBoundary(bounded, re.lastIndex);
    }
  }
}

export function findRegexMatch(
  text: SearchText,
  regex: RegExp,
  start: number,
  options?: RegexSearchOptions,
): SearchMatch | undefined {
  return (options?.direction ?? "next") === "next"
    ? findNextRegexMatch(text, regex, start, options)
    : findPreviousRegexMatch(text, regex, start, options);
}

export function findNextRegexMatch(
  text: SearchText,
  regex: RegExp,
  start: number,
  options?: Omit<RegexSearchOptions, "direction">,
): SearchMatch | undefined {
  const source = snapshotString(text);
  assertOffset(source, start);
  const mode = options?.start ?? "inclusive";
  for (const match of iterateRegexMatches(source, regex, { from: start, to: source.length })) {
    if (mode === "exclusive" && match.from === start) continue;
    return match;
  }
  if (options?.wrap) {
    for (const match of iterateRegexMatches(source, regex, { from: 0, to: start })) return match;
  }
  return undefined;
}

export function findPreviousRegexMatch(
  text: SearchText,
  regex: RegExp,
  start: number,
  options?: Omit<RegexSearchOptions, "direction">,
): SearchMatch | undefined {
  const source = snapshotString(text);
  assertOffset(source, start);
  const mode = options?.start ?? "inclusive";
  let found: SearchMatch | undefined;
  for (const match of iterateRegexMatches(source, regex, { from: 0, to: source.length })) {
    const ok = mode === "inclusive" ? match.from <= start : match.from < start;
    if (ok) found = match;
    else break;
  }
  if (found) return found;
  if (options?.wrap) {
    for (const match of iterateRegexMatches(source, regex, { from: start, to: source.length }))
      found = match;
  }
  return found;
}

export function searchMatchToSelectionRange(match: SearchMatch): SelectionRange {
  return SelectionRange.create(match.from, match.to);
}
