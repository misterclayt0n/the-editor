import { makeRequestTopic, type ChoiceRequest, type ChoiceTopic } from "@the-editor/extensions";

/**
 * The app's convention for requests that render as a modal choice. The kernel
 * Attention fabric stays presentation-agnostic; this is where the *product*
 * decides "a request shaped like this draws as an alert dialog with buttons."
 * A request topic whose question is a `ChoiceRequest` is picked up by
 * `AttentionLayer` and rendered generically — the layer never switches on the
 * topic id, so any future confirm/permission prompt reuses the same surface.
 */
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

export type TrustAnswer = "trust" | "skip";

/** A workspace has a `.the-editor/extensions` folder the user hasn't
 *  decided about yet (mirrors pi's project_trust: global extensions are
 *  implicitly trusted, project-local ones are not). */
export const ConfirmTrustProjectExtensions: ChoiceTopic<TrustAnswer> = makeRequestTopic<
  ChoiceRequest<TrustAnswer>,
  TrustAnswer
>("@the-editor/web/confirm-trust-project-extensions");

export const confirmTrustExtensionsRequest = (root: string): ChoiceRequest<TrustAnswer> => ({
  title: "Trust this workspace's extensions?",
  description: `"${root}" has a .the-editor/extensions folder. Extensions run with full local code execution — only trust folders you wrote or control.`,
  choices: [
    { id: "skip", label: "Don't Load", tone: "outline", cancel: true },
    { id: "trust", label: "Trust & Load", tone: "default" },
  ],
});

export type KillAnswer = "kill" | "cancel";

/** Closing a terminal that still has a foreground process running. */
export const ConfirmKillProcess: ChoiceTopic<KillAnswer> = makeRequestTopic<
  ChoiceRequest<KillAnswer>,
  KillAnswer
>("@the-editor/web/confirm-kill-process");

export const confirmKillProcessRequest = (processName: string): ChoiceRequest<KillAnswer> => {
  const base = processName.split("/").pop() ?? processName;
  return {
    title: "A process is still running",
    description: `“${base}” is running in this terminal. Closing it will end the process.`,
    choices: [
      { id: "cancel", label: "Cancel", tone: "outline", cancel: true },
      { id: "kill", label: "Close anyway", tone: "destructive" },
    ],
  };
};
