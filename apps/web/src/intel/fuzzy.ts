/**
 * Fuzzy matching for completion filtering — Monaco's signals, compact:
 * subsequence match scored by exact-prefix runs, word-boundary and
 * camelCase hits, consecutive-match streaks, and gap penalties. Returns
 * null when the pattern is not a subsequence (item is filtered out).
 */

const isUpper = (code: number) => code >= 65 && code <= 90;
const isSeparator = (ch: string) => "_-./\\: ".includes(ch);

export function fuzzyScore(pattern: string, word: string): number | null {
  if (pattern.length === 0) return 0;
  if (pattern.length > word.length) return null;

  const patternLower = pattern.toLowerCase();
  const wordLower = word.toLowerCase();

  // Exact-prefix fast path dominates everything else.
  if (wordLower.startsWith(patternLower)) {
    let score = 100 + pattern.length * 8;
    if (word.startsWith(pattern)) score += 10; // case-exact
    score -= Math.min(20, word.length - pattern.length); // shorter words first
    return score;
  }

  // Subsequence walk with positional bonuses.
  let score = 0;
  let wordIndex = 0;
  let lastMatch = -2;
  for (let p = 0; p < patternLower.length; p++) {
    const target = patternLower[p]!;
    let found = -1;
    while (wordIndex < wordLower.length) {
      if (wordLower[wordIndex] === target) {
        found = wordIndex;
        break;
      }
      wordIndex++;
    }
    if (found === -1) return null;

    let bonus = 1;
    if (found === 0) {
      bonus += 8;
    } else if (isSeparator(word[found - 1]!)) {
      bonus += 7; // word boundary
    } else if (isUpper(word.charCodeAt(found)) && !isUpper(word.charCodeAt(found - 1))) {
      bonus += 7; // camelCase boundary
    }
    if (found === lastMatch + 1) bonus += 5; // consecutive streak
    if (word[found] === pattern[p]) bonus += 1; // case match
    if (found > lastMatch + 1) bonus -= Math.min(3, found - lastMatch - 1); // gap penalty

    score += bonus;
    lastMatch = found;
    wordIndex = found + 1;
  }
  score -= Math.min(15, word.length - pattern.length); // prefer tighter words
  return score;
}
