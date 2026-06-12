/**
 * Language-intelligence extension points: completion and hover PROVIDERS.
 *
 * The model is Monaco's, reduced to its essence: providers produce items
 * for a position; the CONTROLLER owns triggering, client-side fuzzy
 * filtering as the user types, and re-querying (trigger characters,
 * incomplete lists). LSP is one provider among equals — a user package
 * registers a snippet source or a custom oracle through the exact same
 * registry, with the same scoped retraction as every registry in the
 * editor.
 */

import { Context, Effect, Ref, Scope } from "effect";
import type { TextBuffer } from "@the-editor/engine";

export type CompletionTrigger =
  | { readonly kind: "invoked" } // explicit (ctrl+space)
  | { readonly kind: "typing" } // word characters as you type
  | { readonly kind: "triggerCharacter"; readonly character: string }
  | { readonly kind: "incomplete" }; // re-query of an incomplete list

export type CompletionEdit = {
  readonly from: number;
  readonly to: number;
  readonly text: string;
};

/** Lazily-resolved item details, fetched when the item is accepted. */
export type CompletionResolution = {
  /** Extra edits applied alongside the insert (auto-imports). */
  readonly additionalEdits?: ReadonlyArray<CompletionEdit>;
  readonly documentation?: string;
  readonly detail?: string;
};

export type CompletionItem = {
  readonly label: string;
  /** Category name, used for the badge (e.g. "function", "text", "snippet"). */
  readonly kind?: string;
  readonly detail?: string;
  readonly documentation?: string;
  /**
   * Text to insert; when `isSnippet`, LSP snippet syntax — placeholders
   * become a tab-stop session on accept (Tab/Shift+Tab between stops).
   */
  readonly insertText: string;
  readonly isSnippet?: boolean;
  /** Replace range; defaults to the session's word prefix range. */
  readonly range?: { readonly from: number; readonly to: number };
  /** Edits known up front that ride along with the accept (auto-imports). */
  readonly additionalEdits?: ReadonlyArray<CompletionEdit>;
  /**
   * Called on accept for providers with lazy details (LSP
   * completionItem/resolve). Offsets in the result are CURRENT-document.
   * The controller calls this proactively for the SELECTED item while the
   * popup is open — by accept time the result is usually already in hand.
   */
  readonly resolve?: () => Promise<CompletionResolution | null>;
  /**
   * Runs after the insert and any resolved edits are applied (LSP: the
   * item's command via workspace/executeCommand — some servers deliver
   * auto-imports this way).
   */
  readonly postAccept?: () => Promise<void>;
  readonly filterText?: string;
  readonly sortText?: string;
  readonly deprecated?: boolean;
};

export type CompletionContext = {
  readonly docId: string;
  readonly path: string | null;
  readonly text: TextBuffer;
  readonly offset: number;
  /** The word prefix being completed (anchor..offset). */
  readonly prefix: string;
  readonly trigger: CompletionTrigger;
};

export type CompletionResult = {
  readonly items: readonly CompletionItem[];
  /** True: the list narrows server-side; the controller re-queries as you type. */
  readonly isIncomplete?: boolean;
};

export interface CompletionProvider {
  readonly id: string;
  /** Characters that open a session even mid-punctuation (".", ":", "/"). */
  readonly triggerCharacters?: (docId: string) => readonly string[];
  /**
   * Fallback providers (buffer words) only surface when NO regular provider
   * returned items — VSCode's word-suggestion semantics. Without this, a
   * plain-text word shadows the identically-labeled LSP item and accepting
   * it skips imports/snippets.
   */
  readonly fallback?: boolean;
  provide(context: CompletionContext): Promise<CompletionResult | null> | CompletionResult | null;
}

export type HoverContext = {
  readonly docId: string;
  readonly path: string | null;
  readonly text: TextBuffer;
  readonly offset: number;
};

export type HoverResult = {
  /** Markdown-ish text (code fences and inline code are rendered). */
  readonly contents: string;
  readonly range?: { readonly from: number; readonly to: number };
  /** Visual treatment: diagnostics render as their own tinted card. */
  readonly kind?: "info" | "diagnostic";
};

export interface HoverProvider {
  readonly id: string;
  provide(context: HoverContext): Promise<HoverResult | null> | HoverResult | null;
}

const makeProviderRegistry = <P>() =>
  Effect.gen(function* () {
    const providers = yield* Ref.make<ReadonlyArray<P>>([]);
    return {
      register: (provider: P): Effect.Effect<void, never, Scope.Scope> =>
        Effect.acquireRelease(
          Ref.update(providers, (list) => [...list, provider]),
          () => Ref.update(providers, (list) => list.filter((entry) => entry !== provider)),
        ).pipe(Effect.asVoid),
      list: Ref.get(providers),
    };
  });

export class CompletionProviders extends Context.Tag("the-editor/web/CompletionProviders")<
  CompletionProviders,
  {
    readonly register: (
      provider: CompletionProvider,
    ) => Effect.Effect<void, never, Scope.Scope>;
    readonly list: Effect.Effect<ReadonlyArray<CompletionProvider>>;
  }
>() {}

export class HoverProviders extends Context.Tag("the-editor/web/HoverProviders")<
  HoverProviders,
  {
    readonly register: (provider: HoverProvider) => Effect.Effect<void, never, Scope.Scope>;
    readonly list: Effect.Effect<ReadonlyArray<HoverProvider>>;
  }
>() {}

export const makeCompletionProviders = makeProviderRegistry<CompletionProvider>();
export const makeHoverProviders = makeProviderRegistry<HoverProvider>();

export namespace CompletionProviders {
  export type Service = Context.Tag.Service<CompletionProviders>;
}
export namespace HoverProviders {
  export type Service = Context.Tag.Service<HoverProviders>;
}
