/**
 * Language-intelligence extension points.
 *
 * Providers produce language features for a document position; controllers own
 * triggering, filtering, display, and acceptance. LSP is just one provider
 * among equals, and language-server definitions are likewise ordinary scoped
 * registry entries.
 */

import { Context, Effect, Ref, type Scope } from "effect";
import type { RenderableText } from "@the-editor/engine";

export type CompletionTrigger =
  | { readonly kind: "invoked" }
  | { readonly kind: "typing" }
  | { readonly kind: "triggerCharacter"; readonly character: string }
  | { readonly kind: "incomplete" };

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
   * become a tab-stop session on accept.
   */
  readonly insertText: string;
  readonly isSnippet?: boolean;
  /** Replace range; defaults to the session's word prefix range. */
  readonly range?: { readonly from: number; readonly to: number };
  /** Edits known up front that ride along with the accept (auto-imports). */
  readonly additionalEdits?: ReadonlyArray<CompletionEdit>;
  /** Lazy details and extra edits for providers that resolve on selection. */
  readonly resolve?: () => Promise<CompletionResolution | null>;
  /** Runs after insert and resolved edits are applied. */
  readonly postAccept?: () => Promise<void>;
  readonly filterText?: string;
  readonly sortText?: string;
  readonly deprecated?: boolean;
};

export type CompletionContext = {
  readonly docId: string;
  readonly path: string | null;
  readonly text: RenderableText;
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
   * Fallback providers only surface when no regular provider returned items —
   * VSCode's word-suggestion semantics.
   */
  readonly fallback?: boolean;
  provide(context: CompletionContext): Promise<CompletionResult | null> | CompletionResult | null;
}

export type HoverContext = {
  readonly docId: string;
  readonly path: string | null;
  readonly text: RenderableText;
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

export type SignatureTrigger =
  | { readonly kind: "invoked" }
  | { readonly kind: "triggerCharacter"; readonly character: string }
  | { readonly kind: "contentChange" };

export type SignatureParameter = {
  /** Plain text, or [start, end) offsets into the signature label. */
  readonly label: string | readonly [number, number];
  readonly documentation?: string;
};

export type SignatureInfo = {
  readonly label: string;
  readonly documentation?: string;
  readonly parameters?: ReadonlyArray<SignatureParameter>;
  readonly activeParameter?: number;
};

export type SignatureResult = {
  readonly signatures: ReadonlyArray<SignatureInfo>;
  readonly activeSignature?: number;
  readonly activeParameter?: number;
};

export type SignatureContext = {
  readonly docId: string;
  readonly path: string | null;
  readonly text: RenderableText;
  readonly offset: number;
  readonly trigger: SignatureTrigger;
  /** True when a widget is already showing (the session is re-querying). */
  readonly isRetrigger: boolean;
  /** The currently displayed help, for servers that refine it. */
  readonly active?: SignatureResult;
};

export interface SignatureProvider {
  readonly id: string;
  /** Characters that open the widget ("(",  ","). */
  readonly triggerCharacters?: (docId: string) => readonly string[];
  /** Characters that re-query an open widget. */
  readonly retriggerCharacters?: (docId: string) => readonly string[];
  provide(context: SignatureContext): Promise<SignatureResult | null> | SignatureResult | null;
}

export type LanguageServerDefinition = {
  /** Stable id; one client per (id, workspace root). */
  readonly id: string;
  readonly command: string;
  readonly args?: readonly string[];
  /** LSP languageId by file extension (bare, no dot). */
  readonly languages: ReadonlyArray<{
    readonly languageId: string;
    readonly fileTypes: readonly string[];
    /** Position in the language's server list — feature routing order. */
    readonly priority?: number;
    /** Helix only-features: this server serves ONLY these for the language. */
    readonly only?: readonly string[];
    /** Helix except-features: everything BUT these. */
    readonly except?: readonly string[];
  }>;
  /** Files marking the workspace root, nearest-ancestor-wins. */
  readonly rootMarkers?: readonly string[];
  /** Passed verbatim in initialize. */
  readonly initializationOptions?: unknown;
  /** Workspace settings; answers workspace/configuration by section path. */
  readonly settings?: Record<string, unknown>;
};

type RegistryService<P> = {
  readonly register: (provider: P) => Effect.Effect<void, never, Scope.Scope>;
  readonly list: Effect.Effect<ReadonlyArray<P>>;
};

const makeProviderRegistry = <P>(): Effect.Effect<RegistryService<P>> =>
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

export class CompletionProviders extends Context.Tag("@the-editor/intel/CompletionProviders")<
  CompletionProviders,
  RegistryService<CompletionProvider>
>() {}

export class HoverProviders extends Context.Tag("@the-editor/intel/HoverProviders")<
  HoverProviders,
  RegistryService<HoverProvider>
>() {}

export class SignatureProviders extends Context.Tag("@the-editor/intel/SignatureProviders")<
  SignatureProviders,
  RegistryService<SignatureProvider>
>() {}

export class LanguageServers extends Context.Tag("@the-editor/intel/LanguageServers")<
  LanguageServers,
  RegistryService<LanguageServerDefinition>
>() {}

export const makeCompletionProviders = makeProviderRegistry<CompletionProvider>();
export const makeHoverProviders = makeProviderRegistry<HoverProvider>();
export const makeSignatureProviders = makeProviderRegistry<SignatureProvider>();
export const makeLanguageServers = makeProviderRegistry<LanguageServerDefinition>();

export namespace CompletionProviders {
  export type Service = Context.Tag.Service<CompletionProviders>;
}
export namespace HoverProviders {
  export type Service = Context.Tag.Service<HoverProviders>;
}
export namespace SignatureProviders {
  export type Service = Context.Tag.Service<SignatureProviders>;
}
export namespace LanguageServers {
  export type Service = Context.Tag.Service<LanguageServers>;
}
