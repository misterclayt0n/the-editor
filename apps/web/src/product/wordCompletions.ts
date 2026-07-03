/**
 * Word-based completions from the buffer itself (Monaco's
 * wordBasedSuggestions): every word in the current document is a candidate.
 * Deliberately a SEPARATE package from LSP — the proof that completion is
 * provider-shaped: disable this package and buffer words disappear;
 * replace it with your own corpus and the controller never knows.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { CompletionProviders, type CompletionItem } from "@the-editor/intel";

const WORD_REGEX = /[\p{L}_$][\p{L}\p{N}_$]*/gu;
/** Word scanning is capped: beyond this the document stops contributing. */
const MAX_SCAN_CHARS = 400_000;
const MAX_WORDS = 10_000;

export const wordCompletions = definePackage("the-editor/word-completions", () =>
  Effect.gen(function* () {
    const completions = yield* CompletionProviders;
    yield* completions.register({
      id: "words",
      // Only surfaces when no richer provider answered (VSCode semantics):
      // a bare word must never shadow the real LSP symbol.
      fallback: true,
      // The corpus is every word in the buffer regardless of the typed
      // prefix — deleting never invalidates it; refiltering is enough.
      prefixIndependent: true,
      provide: (context) => {
        if (context.text.length > MAX_SCAN_CHARS) return null;
        const content = context.text.sliceString(0, context.text.length);
        const currentWordStart = context.offset - context.prefix.length;
        const seen = new Set<string>();
        WORD_REGEX.lastIndex = 0;
        let match: RegExpExecArray | null;
        while ((match = WORD_REGEX.exec(content)) !== null && seen.size < MAX_WORDS) {
          // The word being typed is not a suggestion for itself.
          if (match.index === currentWordStart) continue;
          seen.add(match[0]!);
        }
        seen.delete(context.prefix);
        const items: CompletionItem[] = [...seen].map((word) => ({
          label: word,
          kind: "text",
          insertText: word,
          // Rank below same-scored provider items via sortText suffix.
          sortText: `~${word}`,
        }));
        return { items };
      },
    });
  }),
);
