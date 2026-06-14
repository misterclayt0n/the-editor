/**
 * Terminal engine: ghostty-web (WASM VT). One of two interchangeable backends
 * behind `./backend` — both expose the same xterm.js-shaped surface so the
 * panel renderer doesn't care which is active.
 */

import { init, Terminal, FitAddon } from "ghostty-web";

export { Terminal, FitAddon };

/** Load the shared VT WASM once before constructing any Terminal. */
export const ensureReady = (): Promise<void> => init();

/** Keep the terminal fitted to its container; returns a disposer. */
export function attachAutoFit(fit: FitAddon, _container: HTMLElement): () => void {
  fit.observeResize();
  return () => {};
}

/**
 * Run `claim` (the app's global keymap) before the terminal eats a keydown; if
 * it claims the key, swallow it. ghostty's custom handler: return true to
 * CONSUME (don't send to the PTY).
 */
export function attachGlobalKeyHandler(
  term: Terminal,
  claim: (event: KeyboardEvent) => boolean,
): void {
  term.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown") return false;
    if (claim(event)) {
      event.stopPropagation();
      return true;
    }
    return false;
  });
}
