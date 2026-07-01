import { useCallback, useEffect, useSyncExternalStore } from "react";
import type { Pending } from "@the-editor/kernel";
import type { ChoiceRequest } from "@the-editor/extensions";
import type { AppRuntime } from "../runtime";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/ui/alert-dialog";

/**
 * The app's single responder for the Attention fabric. It registers itself for
 * its lifetime (so `ask` knows a human can answer), mirrors the open requests,
 * and renders the first one shaped as a `ChoiceRequest` as an alert dialog.
 *
 * It is deliberately generic: it never switches on the topic id. Any request
 * whose question carries a title + unique choices with exactly one cancel
 * choice renders here, so a future permission prompt or "overwrite?" confirm
 * reuses this exact surface for free.
 */
function isChoiceRequest(question: unknown): question is ChoiceRequest<string> {
  if (typeof question !== "object" || question === null) return false;
  const candidate = question as { title?: unknown; description?: unknown; choices?: unknown };
  if (typeof candidate.title !== "string") return false;
  if (candidate.description !== undefined && typeof candidate.description !== "string")
    return false;
  if (!Array.isArray(candidate.choices) || candidate.choices.length === 0) return false;
  let cancelCount = 0;
  const ids = new Set<string>();
  for (const choice of candidate.choices) {
    if (typeof choice !== "object" || choice === null) return false;
    const c = choice as { id?: unknown; label?: unknown; tone?: unknown; cancel?: unknown };
    if (
      typeof c.id !== "string" ||
      ids.has(c.id) ||
      typeof c.label !== "string" ||
      (c.tone !== undefined &&
        c.tone !== "default" &&
        c.tone !== "destructive" &&
        c.tone !== "outline") ||
      (c.cancel !== undefined && typeof c.cancel !== "boolean")
    ) {
      return false;
    }
    ids.add(c.id);
    if (c.cancel === true) cancelCount++;
  }
  return cancelCount === 1;
}

export function AttentionLayer({ runtime }: { readonly runtime: AppRuntime }) {
  useEffect(() => runtime.acquireAttentionResponder(), [runtime]);

  const requests = useSyncExternalStore(
    useCallback((cb: () => void) => runtime.subscribeAttention(cb), [runtime]),
    useCallback(() => runtime.attentionSnapshot(), [runtime]),
  );

  useEffect(() => {
    for (const request of requests) {
      if (!isChoiceRequest(request.question)) runtime.declineRequest(request);
    }
  }, [runtime, requests]);

  // First open request that is a renderable choice. Others queue behind it.
  const pending = requests.find((request) => isChoiceRequest(request.question)) as
    | Pending<ChoiceRequest<string>, string>
    | undefined;
  const request = pending?.question;

  const answer = useCallback(
    (choice: string) => {
      if (pending !== undefined) runtime.respondToRequest(pending as Pending, choice);
    },
    [runtime, pending],
  );

  const cancelId = request?.choices.find((choice) => choice.cancel)?.id;
  // Cancel sits left (shadcn layout); the rest follow in declaration order.
  const cancel = request?.choices.find((choice) => choice.cancel);
  const actions = request?.choices.filter((choice) => !choice.cancel) ?? [];

  return (
    <AlertDialog
      open={pending !== undefined}
      onOpenChange={(open) => {
        // Escape / dismissal resolves to the designated cancel choice. `respond`
        // is idempotent, so a trailing close after an explicit click is a no-op.
        if (!open && cancelId !== undefined) answer(cancelId);
      }}
    >
      {request !== undefined && (
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{request.title}</AlertDialogTitle>
            {request.description !== undefined && (
              <AlertDialogDescription>{request.description}</AlertDialogDescription>
            )}
          </AlertDialogHeader>
          <AlertDialogFooter>
            {cancel !== undefined && (
              <AlertDialogCancel onClick={() => answer(cancel.id)}>
                {cancel.label}
              </AlertDialogCancel>
            )}
            {actions.map((choice) => (
              <AlertDialogAction
                key={choice.id}
                tone={choice.tone === "destructive" ? "destructive" : "default"}
                onClick={() => answer(choice.id)}
              >
                {choice.label}
              </AlertDialogAction>
            ))}
          </AlertDialogFooter>
        </AlertDialogContent>
      )}
    </AlertDialog>
  );
}
