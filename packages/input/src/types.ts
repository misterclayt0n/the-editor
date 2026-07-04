/**
 * Input is the focused-view interaction substrate. It intentionally does NOT
 * register keybindings: Keymaps remains the single keys→command registry. Input
 * only publishes the facts Keymaps filters on and owns semantic pending state
 * that is not a static chord sequence (`f<char>`, `r<char>`, operator prompts).
 */

import type { Context, Effect, Scope, Stream } from "effect";
import type { Chord, KeyBindingFacts, KeyboardEventLike } from "@the-editor/keymaps";
import { ContributionId } from "@the-editor/kernel";
import type { OwnedContribution } from "@the-editor/kernel";

export type InputProfileId = ContributionId;
export const InputProfileId = ContributionId;
export const DEFAULT_INPUT_PROFILE = InputProfileId("default");

export type InputFactValue = string | number | boolean;
export type InputFacts = Readonly<Record<string, InputFactValue | undefined>>;

export type TextFallbackPolicy = "insert" | "none";

export interface InputContextSnapshot {
  readonly profile: string;
  readonly mode?: string;
  readonly transient?: string;
  readonly facets?: Readonly<Record<string, InputFactValue>>;
  /** Whether an unbound printable key becomes `editor.insertText`. */
  readonly textFallback: TextFallbackPolicy;
}

export interface InputWhenOptions {
  readonly profile?: string;
  readonly mode?: string;
  readonly transient?: string;
  readonly acceptsText?: boolean;
  readonly facets?: Readonly<Record<string, InputFactValue>>;
}

export const inputWhen = (options: InputWhenOptions): KeyBindingFacts => ({
  ...(options.profile !== undefined ? { "input.profile": options.profile } : {}),
  ...(options.mode !== undefined ? { "input.mode": options.mode } : {}),
  ...(options.transient !== undefined ? { "input.transient": options.transient } : {}),
  ...(options.acceptsText !== undefined ? { "input.acceptsText": options.acceptsText } : {}),
  ...Object.fromEntries(
    Object.entries(options.facets ?? {}).map(([key, value]) => [`input.${key}`, value]),
  ),
});

export const factsOfInputContext = (context: InputContextSnapshot): InputFacts => ({
  "input.profile": context.profile,
  ...(context.mode !== undefined ? { "input.mode": context.mode } : {}),
  ...(context.transient !== undefined ? { "input.transient": context.transient } : {}),
  "input.acceptsText": context.textFallback === "insert",
  ...Object.fromEntries(
    Object.entries(context.facets ?? {}).map(([key, value]) => [`input.${key}`, value]),
  ),
});

export type InputContinuationResult =
  | { readonly _tag: "handled" }
  | { readonly _tag: "fallthrough" }
  | { readonly _tag: "cancel" }
  | { readonly _tag: "execute"; readonly command: string; readonly args?: unknown };

export interface InputContinuation {
  readonly owner: string;
  readonly label: string;
  readonly onKey: (chord: Chord, context: InputContextSnapshot) => InputContinuationResult;
  readonly onCancel?: () => void;
}

export type InputInterceptResult = { readonly _tag: "handled" } | { readonly _tag: "passthrough" };

export interface InputKeyEvent {
  readonly chord: Chord;
  readonly raw: KeyboardEventLike;
  readonly context: InputContextSnapshot;
}

export interface InputInterceptor {
  readonly owner: string;
  readonly label?: string;
  readonly onKey: (event: InputKeyEvent) => InputInterceptResult;
}

export interface InputStatus {
  readonly context: InputContextSnapshot;
  readonly facts: InputFacts;
  readonly pendingContinuationLabel?: string;
}

export interface ViewInputService {
  readonly context: Effect.Effect<InputContextSnapshot>;
  readonly changes: Stream.Stream<InputContextSnapshot>;
  readonly facts: Effect.Effect<InputFacts>;
  readonly set: (next: InputContextSnapshot) => Effect.Effect<void>;
  readonly update: (
    f: (previous: InputContextSnapshot) => InputContextSnapshot,
  ) => Effect.Effect<void>;
  readonly pushContinuation: (continuation: InputContinuation) => Effect.Effect<void>;
  readonly clearContinuations: (owner?: string) => Effect.Effect<void>;
  readonly peekContinuation: Effect.Effect<InputContinuation | null>;
  readonly handleContinuation: (
    chord: Chord,
  ) => Effect.Effect<InputContinuationResult | null>;
  /** Scoped, newest-wins pre-keymap interception. */
  readonly registerPreKeymap: (
    interceptor: InputInterceptor,
  ) => Effect.Effect<void, never, Scope.Scope>;
  readonly dispatchPreKeymap: (event: InputKeyEvent) => Effect.Effect<InputInterceptResult>;
  readonly status: Effect.Effect<InputStatus>;
}

export interface InputViewTarget {
  readonly viewId: string;
  readonly context: Context.Context<any>;
  readonly input: ViewInputService;
}

export interface InputProfileDefinition {
  readonly id: string;
  readonly title: string;
  readonly initialContext: () => InputContextSnapshot;
  readonly attachView?: (target: InputViewTarget) => Effect.Effect<void, never, Scope.Scope>;
}

export interface InputProfileContribution extends Omit<InputProfileDefinition, "id"> {
  readonly id: InputProfileId;
}

export type OwnedInputProfile = OwnedContribution<InputProfileContribution>;
