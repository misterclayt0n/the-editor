import type { CompletionItem } from "@the-editor/editor";

export interface CompletionRankView {
  labelHighlights: number[];
  detailHighlights: number[];
}

export interface CompletionRankEntry<TView extends CompletionRankView = CompletionRankView> {
  sourceId: string;
  sourcePriority: number;
  item: CompletionItem;
  view: TView;
  matchScore: number;
}

export function rankCompletionEntries<T extends CompletionRankEntry>(query: string, items: readonly T[]): T[] {
  const normalizedQuery = normalizeCompletionText(query);
  return items
    .map((entry) => {
      const score = scoreCompletionItem(normalizedQuery, entry.item);
      const labelHighlights = highlightCompletionText(query, entry.item.label);
      const detailHighlights = highlightCompletionText(query, entry.item.detail ?? entry.item.labelDescription ?? "");
      return {
        ...entry,
        matchScore: score,
        view: {
          ...entry.view,
          labelHighlights,
          detailHighlights,
        },
      } satisfies T;
    })
    .filter((entry) => !normalizedQuery || entry.matchScore > 0)
    .sort((left, right) => {
      const exactOrdering = Number(isExactCompletionMatch(normalizedQuery, right.item)) - Number(isExactCompletionMatch(normalizedQuery, left.item));
      if (exactOrdering !== 0) return exactOrdering;
      const lspOrdering = compareLspCompletionEntries(left, right);
      if (lspOrdering !== undefined) return lspOrdering;
      return right.matchScore - left.matchScore
        || right.sourcePriority - left.sourcePriority
        || compareNullable(left.item.sortText, right.item.sortText)
        || left.item.label.localeCompare(right.item.label);
    });
}

export function scoreCompletionItem(query: string, item: CompletionItem): number {
  if (!query) return 1;
  const haystacks = [item.filterText, item.label, item.detail, item.labelDescription]
    .filter((value): value is string => Boolean(value?.trim()))
    .map(normalizeCompletionText);
  let best = 0;
  for (const haystack of haystacks) {
    best = Math.max(best, directCompletionScore(query, haystack));
    best = Math.max(best, fuzzyCompletionScore(query, haystack));
  }
  return best;
}

export function directCompletionScore(query: string, haystack: string): number {
  if (!query || !haystack) return 0;
  if (haystack === query) return 1200 - haystack.length;
  if (haystack.startsWith(query)) return 950;
  const index = haystack.indexOf(query);
  if (index >= 0) return 750 - Math.min(index, 32);
  return 0;
}

export function fuzzyCompletionScore(query: string, haystack: string): number {
  if (!query || !haystack) return 0;
  let queryIndex = 0;
  let score = 0;
  let firstMatch = -1;
  let consecutive = 0;
  for (let index = 0; index < haystack.length && queryIndex < query.length; index += 1) {
    if (haystack[index] !== query[queryIndex]) {
      consecutive = 0;
      continue;
    }
    if (firstMatch < 0) firstMatch = index;
    score += 10;
    if (index === 0 || !/[a-z0-9]/.test(haystack[index - 1] ?? "")) score += 16;
    if (consecutive > 0) score += 12 + consecutive * 3;
    consecutive += 1;
    queryIndex += 1;
  }
  if (queryIndex !== query.length) return 0;
  score -= Math.max(0, haystack.length - query.length);
  if (firstMatch > 0) score -= Math.min(firstMatch, 40);
  return Math.max(score, 1);
}

export function highlightCompletionText(query: string, value: string): number[] {
  const normalizedQuery = normalizeCompletionText(query);
  if (!normalizedQuery || !value) return [];
  const normalizedValue = normalizeCompletionText(value);
  const directIndex = normalizedValue.indexOf(normalizedQuery);
  if (directIndex >= 0) {
    return Array.from({ length: normalizedQuery.length }, (_, offset) => directIndex + offset);
  }

  const positions: number[] = [];
  let queryIndex = 0;
  for (let index = 0; index < normalizedValue.length && queryIndex < normalizedQuery.length; index += 1) {
    if (normalizedValue[index] !== normalizedQuery[queryIndex]) continue;
    positions.push(index);
    queryIndex += 1;
  }
  return queryIndex === normalizedQuery.length ? positions : [];
}

export function normalizeCompletionText(value: string): string {
  return value.trim().toLowerCase();
}

function isExactCompletionMatch(query: string, item: CompletionItem): boolean {
  if (!query) return false;
  return [item.filterText, item.label]
    .filter((value): value is string => Boolean(value?.trim()))
    .map(normalizeCompletionText)
    .includes(query);
}

function compareLspCompletionEntries(left: CompletionRankEntry, right: CompletionRankEntry): number | undefined {
  const leftIsLsp = left.sourceId === "default.lsp";
  const rightIsLsp = right.sourceId === "default.lsp";
  if (!leftIsLsp && !rightIsLsp) return undefined;
  if (leftIsLsp !== rightIsLsp) return leftIsLsp ? -1 : 1;
  return Number(Boolean(right.item.payload && (right.item.payload as { preselect?: boolean }).preselect))
    - Number(Boolean(left.item.payload && (left.item.payload as { preselect?: boolean }).preselect))
    || compareNullable(left.item.sortText, right.item.sortText)
    || right.matchScore - left.matchScore
    || left.item.label.localeCompare(right.item.label);
}

function compareNullable(left: string | undefined, right: string | undefined): number {
  if (left && right) return left.localeCompare(right);
  if (left) return -1;
  if (right) return 1;
  return 0;
}
