/**
 * The single consumer of the Attention fabric's `notices` half. It owns the
 * one policy the fabric deliberately doesn't: where a notice goes.
 *
 *   editor focused   → Sonner toast (in-app)
 *   editor unfocused → OS notification (like Ghostty), click to come back
 *
 * A bare terminal bell is suppressed while focused (it's noise when you're
 * looking) and becomes an OS notification when you're away — the catch-all that
 * fires even for agents that don't emit a structured notification sequence.
 */

import { useEffect } from "react";
import { toast } from "sonner";
import type { Notice } from "@the-editor/kernel";
import {
  TerminalNotification,
  type TerminalNotificationPayload,
} from "../terminal/notifications";
import type { AppRuntime } from "../runtime";
import { Toaster } from "./sonner";

const isTerminalNotice = (notice: Notice): notice is Notice<TerminalNotificationPayload> =>
  notice.topicId === TerminalNotification.id;

const genericText = (notice: Notice): string => {
  const payload = notice.payload;
  if (typeof payload === "object" && payload !== null && "message" in payload) {
    return String((payload as { message: unknown }).message);
  }
  return notice.topicId;
};

const showOsNotification = (title: string, body: string, onActivate: () => void): void => {
  if (typeof Notification === "undefined" || Notification.permission !== "granted") return;
  try {
    const notification = new Notification(title, { body });
    notification.onclick = () => {
      window.focus();
      onActivate();
      notification.close();
    };
  } catch {
    // Some platforms throw if notifications are unavailable; ignore.
  }
};

export function NotificationCenter({ runtime }: { readonly runtime: AppRuntime }) {
  useEffect(() => {
    // Ask once; a no-op in Electron (auto-granted) or when already decided.
    if (typeof Notification !== "undefined" && Notification.permission === "default") {
      void Notification.requestPermission();
    }

    return runtime.subscribeNotices((notice) => {
      const terminal = isTerminalNotice(notice) ? notice.payload : null;
      const title = terminal?.title;
      const body = terminal?.body ?? genericText(notice);
      // Click target: jump back to the terminal that emitted it.
      const panelId = terminal?.panelId;
      const jump = panelId !== undefined ? () => runtime.focusTerminal(panelId) : undefined;

      if (document.hasFocus()) {
        if (terminal?.fromBell === true) return; // a focused bell is just noise
        const text = title !== undefined ? `${title}: ${body}` : body;
        const options =
          jump !== undefined ? { action: { label: "Show", onClick: jump } } : {};
        if (notice.level === "success") toast.success(text, options);
        else if (notice.level === "warning") toast.warning(text, options);
        else if (notice.level === "error") toast.error(text, options);
        else toast(text, options);
      } else {
        showOsNotification(title ?? "the-editor", body, jump ?? (() => window.focus()));
      }
    });
  }, [runtime]);

  return <Toaster runtime={runtime} />;
}
