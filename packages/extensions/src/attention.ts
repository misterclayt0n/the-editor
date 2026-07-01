import type { RequestTopic } from "@the-editor/kernel";

/**
 * The public request vocabulary the default app responder knows how to render.
 *
 * Attention itself is presentation-agnostic: topics carry typed questions and
 * answers, but the fabric never decides whether a request is a modal, picker,
 * toast action, or something else. The default distribution currently exposes
 * one human-answerable request shape: a titled choice prompt whose answer is
 * one of its choice ids.
 */
export type ChoiceTone = "default" | "destructive" | "outline";

export interface Choice<Answer extends string> {
  /** Stable answer id. Ids should be unique within a ChoiceRequest. */
  readonly id: Answer;
  readonly label: string;
  readonly tone?: ChoiceTone;
  /** The answer Escape / overlay-dismiss resolves to (exactly one choice). */
  readonly cancel?: boolean;
}

export interface ChoiceRequest<Answer extends string> {
  readonly title: string;
  readonly description?: string;
  /**
   * Must include exactly one `cancel: true` choice so dismissal always has an
   * answer. The default AttentionLayer declines malformed dynamic requests.
   */
  readonly choices: ReadonlyArray<Choice<Answer>>;
}

/** A request whose answer is one of its choice ids — the generic modal shape. */
export type ChoiceTopic<Answer extends string> = RequestTopic<ChoiceRequest<Answer>, Answer>;
