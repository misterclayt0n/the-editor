/**
 * Terminal notifications: bubble the desktop-notification escape sequences a
 * program emits in a terminal up to the editor's Attention fabric, so a single
 * router can show them in-app (Sonner) when focused or as an OS notification
 * when not — the way Ghostty/kitty do it.
 *
 * The channels we catch (the same ones real terminals catch):
 *   - OSC 9  ; <message>            iTerm2-style notification
 *   - OSC 777 ; notify ; <title> ; <body>   urxvt/kitty-style
 *   - the bell (BEL)                generic, only meaningful when unfocused
 *
 * Parsing is pure so it can be unit-tested without a terminal.
 */

import { makeNoticeTopic } from "@the-editor/kernel";

export interface TerminalNotificationPayload {
  readonly title?: string;
  readonly body: string;
  /** A bare bell — no message. Surfaced only when the editor is unfocused. */
  readonly fromBell?: boolean;
  /** The terminal panel that emitted it, so a click can jump back to it. */
  readonly panelId?: string;
}

export const TerminalNotification = makeNoticeTopic<TerminalNotificationPayload>(
  "@the-editor/web/terminal-notification",
);

/**
 * OSC 9 payload (everything after `OSC 9 ;`). iTerm2 uses it for a free-text
 * notification; ConEmu overloaded it for progress (`OSC 9 ; 4 ; …`), so a
 * payload whose first field is a bare number is treated as control, not a
 * message.
 */
export function parseOsc9(data: string): TerminalNotificationPayload | null {
  const firstSemi = data.indexOf(";");
  const firstField = firstSemi === -1 ? data : data.slice(0, firstSemi);
  if (/^\d+$/.test(firstField.trim())) return null;
  const body = data.trim();
  return body.length === 0 ? null : { body };
}

/** OSC 777 payload (`notify ; <title> ; <body>`). */
export function parseOsc777(data: string): TerminalNotificationPayload | null {
  const parts = data.split(";");
  if (parts[0] !== "notify") return null;
  const title = (parts[1] ?? "").trim();
  const body = parts.slice(2).join(";").trim();
  if (title.length === 0 && body.length === 0) return null;
  if (body.length === 0) return { body: title };
  return title.length > 0 ? { title, body } : { body };
}
