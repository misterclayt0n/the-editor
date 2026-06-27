import { makeRequestTopic, type RequestTopic } from "@the-editor/kernel";

/**
 * The app's convention for requests that render as a modal choice. The kernel
 * Attention fabric stays presentation-agnostic; this is where the *product*
 * decides "a request shaped like this draws as an alert dialog with buttons."
 * A request topic whose question is a `ChoiceRequest` is picked up by
 * `AttentionLayer` and rendered generically — the layer never switches on the
 * topic id, so any future confirm/permission prompt reuses the same surface.
 */
export type ChoiceTone = "default" | "destructive" | "outline";

export interface Choice<Answer extends string> {
  readonly id: Answer;
  readonly label: string;
  readonly tone?: ChoiceTone;
  /** The answer Escape / overlay-dismiss resolves to (exactly one choice). */
  readonly cancel?: boolean;
}

export interface ChoiceRequest<Answer extends string> {
  readonly title: string;
  readonly description?: string;
  readonly choices: ReadonlyArray<Choice<Answer>>;
}

/** A request whose answer is one of its choice ids — the generic modal shape. */
export type ChoiceTopic<Answer extends string> = RequestTopic<ChoiceRequest<Answer>, Answer>;

export type DiscardAnswer = "save" | "discard" | "cancel";

/** Closing a buffer with unsaved changes. */
export const ConfirmDiscardChanges: ChoiceTopic<DiscardAnswer> = makeRequestTopic<
  ChoiceRequest<DiscardAnswer>,
  DiscardAnswer
>("@the-editor/web/confirm-discard-changes");

export const confirmDiscardRequest = (name: string): ChoiceRequest<DiscardAnswer> => ({
  title: "Save changes before closing?",
  description: `"${name}" has unsaved changes. They will be lost if you don't save them.`,
  choices: [
    { id: "cancel", label: "Cancel", tone: "outline", cancel: true },
    { id: "discard", label: "Don't Save", tone: "destructive" },
    { id: "save", label: "Save", tone: "default" },
  ],
});
