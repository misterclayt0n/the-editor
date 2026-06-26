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

const OSC_QUERY = /\x1b\]([\s\S]*?)(?:\x07|\x1b\\)/g;

const colorResponse = (color: string | undefined): string | null => {
  if (color === undefined || color[0] !== "#" || color.length !== 7) return null;
  const red = color.slice(1, 3);
  const green = color.slice(3, 5);
  const blue = color.slice(5, 7);
  return `rgb:${red}${red}/${green}${green}/${blue}${blue}`;
};

const ANSI_COLOR_KEYS = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

const TERMINAL_COLOR_QUERY_KEYS: Readonly<Record<number, string>> = {
  256: "foreground",
  257: "background",
  258: "cursor",
  259: "dimBlack",
  260: "dimRed",
  261: "dimGreen",
  262: "dimYellow",
  263: "dimBlue",
  264: "dimMagenta",
  265: "dimCyan",
  266: "dimWhite",
  267: "brightForeground",
  268: "dimForeground",
};

const indexedColor = (index: number, theme: Readonly<Record<string, string>>): string | undefined => {
  const key = ANSI_COLOR_KEYS[index] ?? TERMINAL_COLOR_QUERY_KEYS[index];
  return key === undefined ? undefined : theme[key];
};

const appendOscColorResponse = (
  out: string[],
  code: string,
  color: string | undefined,
): void => {
  const response = colorResponse(color);
  if (response !== null) out.push(`\x1b]${code};${response}\x1b\\`);
};

export const unsupportedQueryResponses = (
  data: string,
  theme: Readonly<Record<string, string>>,
): string => {
  const responses: string[] = [];
  for (const match of data.matchAll(OSC_QUERY)) {
    const payload = match[1] ?? "";
    const parts = payload.split(";");
    if (parts[0] === "10" && parts[1] === "?") appendOscColorResponse(responses, "10", theme.foreground);
    else if (parts[0] === "11" && parts[1] === "?") appendOscColorResponse(responses, "11", theme.background);
    else if (parts[0] === "12" && parts[1] === "?") appendOscColorResponse(responses, "12", theme.cursor);
    else if (parts[0] === "4") {
      for (let index = 1; index + 1 < parts.length; index += 2) {
        if (parts[index + 1] !== "?") continue;
        const colorIndex = Number.parseInt(parts[index]!, 10);
        if (!Number.isFinite(colorIndex)) continue;
        appendOscColorResponse(responses, `4;${colorIndex}`, indexedColor(colorIndex, theme));
      }
    }
  }
  return responses.join("");
};

export function hideNativeInputCaret(_term: Terminal): void {}
export type TerminalAddonSet = {
  readonly fit: FitAddon;
  readonly ligatures: LigaturesAddon;
  search: SearchAddon;
  readonly unicode11: Unicode11Addon;
  readonly webLinks: WebLinksAddon;
  webgl: WebglAddon | null;
};

const openWebLink = (_event: MouseEvent, uri: string): void => {
  window.open(uri, "_blank", "noopener,noreferrer");
};

const refreshVisibleRows = (term: Terminal): void => {
  if (term.rows <= 0) return;
  term.refresh(0, term.rows - 1);
};

const loadWebglAddon = (term: Terminal): WebglAddon | null => {
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => {
      try {
        webgl.dispose();
        refreshVisibleRows(term);
      } catch {
        // Context loss can race terminal disposal; xterm's DOM renderer remains active.
      }
    });
    term.loadAddon(webgl);
    return webgl;
  } catch {
    // WebGL is opportunistic; xterm's DOM renderer remains the correct fallback.
    return null;
  }
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

  const webgl = loadWebglAddon(term);

  return { fit, ligatures, search, unicode11, webLinks, webgl };
}

export function recoverTerminalRenderer(term: Terminal, addons: TerminalAddonSet): void {
  if (addons.webgl !== null) {
    try {
      addons.webgl.dispose();
    } catch {
      // Disposal can race context loss; either way we fall back to DOM and try a fresh WebGL addon.
    }
    addons.webgl = null;
  }

  addons.webgl = loadWebglAddon(term);
  try {
    refreshVisibleRows(term);
  } catch {
    // Refresh can race terminal disposal.
  }
}

type SearchClearableTerminal = Pick<Terminal, "clearSelection" | "element" | "refresh" | "rows">;
type SearchClearableAddons = { readonly search: Pick<SearchAddon, "clearDecorations"> };

export function clearTerminalSearch(term: SearchClearableTerminal, addons: SearchClearableAddons): void {
  addons.search.clearDecorations();
  term.clearSelection();
  term.element
    ?.querySelectorAll(".xterm-find-result-decoration, .xterm-find-active-result-decoration")
    .forEach((element) => element.remove());
  if (term.rows > 0) term.refresh(0, term.rows - 1);
}

type SearchResettableTerminal = SearchClearableTerminal & Pick<Terminal, "loadAddon">;
type SearchResettableAddons = { search: SearchAddon };

export function resetTerminalSearchSession(
  term: SearchResettableTerminal,
  addons: SearchResettableAddons,
  createSearchAddon: () => SearchAddon = () => new SearchAddon(),
): void {
  try {
    addons.search.dispose();
  } catch {
    // The terminal may be mid-disposal; the new search session below becomes the source of truth.
  }
  addons.search = createSearchAddon();
  term.loadAddon(addons.search);
  clearTerminalSearch(term, addons);
}

const hasVisibleSize = (container: HTMLElement): boolean => {
  if (!container.isConnected) return false;
  const rect = container.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
};

/** Keep the terminal fitted to its container; returns a disposer. */
export function attachAutoFit(fit: FitAddon, container: HTMLElement): () => void {
  const observer = new ResizeObserver(() => {
    if (!hasVisibleSize(container)) return;
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
