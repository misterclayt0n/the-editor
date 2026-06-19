/**
 * Terminal engine: ghostty-web (WASM VT). One of two interchangeable backends
 * behind `./backend` — both expose the same xterm.js-shaped surface so the
 * panel renderer doesn't care which is active.
 */

import { init, Terminal, FitAddon } from "ghostty-web";
import type { TerminalKeyHandlerResult } from "./keys";
import { ghosttyUnsupportedQueryResponses } from "./queryResponses";

export { Terminal, FitAddon };

/** Load the shared VT WASM once before constructing any Terminal. */
export const ensureReady = (): Promise<void> => init();

/** Keep the terminal fitted to its container; returns a disposer. */
export function attachAutoFit(fit: FitAddon, _container: HTMLElement): () => void {
  fit.observeResize();
  const onWindowResize = () => fit.fit();
  window.addEventListener("resize", onWindowResize);
  return () => window.removeEventListener("resize", onWindowResize);
}

export const unsupportedQueryResponses = ghosttyUnsupportedQueryResponses;

export function hideNativeInputCaret(term: Terminal): void {
  const textarea = term.textarea;
  if (textarea === undefined) return;
  textarea.style.caretColor = "transparent";
  textarea.style.color = "transparent";
  textarea.style.background = "transparent";
  textarea.style.left = "-10000px";
  textarea.style.top = "-10000px";
}

/**
 * Run `claim` (the app's global keymap) before the terminal eats a keydown; if
 * it claims the key, swallow it. ghostty's custom handler: return true to
 * CONSUME (don't send to the PTY).
 */
export function attachGlobalKeyHandler(
  term: Terminal,
  claim: (event: KeyboardEvent) => TerminalKeyHandlerResult,
): void {
  term.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown") return false;
    const result = claim(event);
    if (result === "skip") {
      event.preventDefault();
      event.stopPropagation();
      return true;
    }
    if (result === "process-prevent-default") {
      event.preventDefault();
    }
    return false;
  });
}
