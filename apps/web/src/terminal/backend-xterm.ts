/**
 * Terminal engine: xterm.js. The fit addon lives in a separate package, so we
 * drive auto-fit with a ResizeObserver.
 */

import type { TerminalKeyHandlerResult } from "./keys";

import "@xterm/xterm/css/xterm.css";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { LigaturesAddon } from "@xterm/addon-ligatures";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";

export { Terminal, FitAddon, LigaturesAddon, SearchAddon, Unicode11Addon, WebLinksAddon, WebglAddon };

/** xterm needs no async setup. */
export const ensureReady = async (): Promise<void> => {};

export const unsupportedQueryResponses = (
  _data: string,
  _theme: Readonly<Record<string, string>>,
): string => "";

export function hideNativeInputCaret(_term: Terminal): void {}
export type TerminalAddonSet = {
  readonly fit: FitAddon;
  readonly ligatures: LigaturesAddon;
  readonly search: SearchAddon;
  readonly unicode11: Unicode11Addon;
  readonly webLinks: WebLinksAddon;
  readonly webgl: WebglAddon | null;
};

const openWebLink = (_event: MouseEvent, uri: string): void => {
  window.open(uri, "_blank", "noopener,noreferrer");
};

export function loadTerminalAddons(term: Terminal, fit: FitAddon): TerminalAddonSet {
  const ligatures = new LigaturesAddon();
  const search = new SearchAddon();
  const unicode11 = new Unicode11Addon();
  const webLinks = new WebLinksAddon(openWebLink);

  term.loadAddon(fit);
  term.loadAddon(unicode11);
  term.unicode.activeVersion = "11";
  term.loadAddon(search);
  term.loadAddon(webLinks);
  term.loadAddon(ligatures);

  let webgl: WebglAddon | null = null;
  try {
    const nextWebgl = new WebglAddon();
    nextWebgl.onContextLoss(() => {
      try {
        nextWebgl.dispose();
        term.refresh(0, term.rows - 1);
      } catch {
        // Context loss can race terminal disposal; xterm's DOM renderer remains active.
      }
    });
    term.loadAddon(nextWebgl);
    webgl = nextWebgl;
  } catch {
    // WebGL is opportunistic; xterm's DOM renderer remains the correct fallback.
  }

  return { fit, ligatures, search, unicode11, webLinks, webgl };
}


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
 * claims the key, skip xterm. xterm's custom handler returns false to skip
 * processing and true to process normally.
 */
export function attachGlobalKeyHandler(
  term: Terminal,
  claim: (event: KeyboardEvent) => TerminalKeyHandlerResult,
): void {
  term.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown") return true;
    const result = claim(event);
    if (result === "skip") {
      event.preventDefault();
      event.stopPropagation();
      return false;
    }
    if (result === "process-prevent-default") {
      event.preventDefault();
    }
    return true;
  });
}
