import type { TerminalKeyHandlerResult } from "./keys";
/**
 * Terminal engine: xterm.js. Interchangeable with the ghostty backend behind
 * `./backend`. No WASM (ensureReady is a no-op) and the fit addon lives in a
 * separate package, so we drive auto-fit with a ResizeObserver.
 */

import "@xterm/xterm/css/xterm.css";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";

export { Terminal, FitAddon };

/** xterm needs no async setup. */
export const ensureReady = async (): Promise<void> => {};

/** Keep the terminal fitted to its container; returns a disposer. */
export function attachAutoFit(fit: FitAddon, container: HTMLElement): () => void {
  const observer = new ResizeObserver(() => {
    try {
      fit.fit();
    } catch {
      // Fitting a detached/zero-size terminal is harmless to skip.
    }
  });
  observer.observe(container);
  return () => observer.disconnect();
}

/**
 * Run `claim` (the app's global keymap) before xterm processes a keydown; if it
 * claims the key, skip xterm. xterm's custom handler has INVERTED polarity vs
 * ghostty: return FALSE to skip (don't process), TRUE to process normally.
 */
export function attachGlobalKeyHandler(
  term: Terminal,
  claim: (event: KeyboardEvent) => TerminalKeyHandlerResult,
): void {
  term.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown") return true;
    const result = claim(event);
    if (result === "skip") {
      event.stopPropagation();
      return false;
    }
    if (result === "process-prevent-default") {
      event.preventDefault();
    }
    return true;
  });
}
