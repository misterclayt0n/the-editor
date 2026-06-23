/**
 * Terminal panel content: an xterm.js terminal wired to a PTY over the
 * TerminalPort. The host supplies the port (Electron node-pty, or a dev echo
 * for tests).
 */

import {
  ensureReady,
  Terminal,
  FitAddon,
  loadTerminalAddons,
  attachAutoFit,
  attachGlobalKeyHandler,
  unsupportedQueryResponses,
  hideNativeInputCaret,
  type TerminalAddonSet,
} from "./backend";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "../workbench/panelRenderers";
import { terminalTheme } from "../theme/dom";
import { createTerminalOptions, macEditingSequence, terminalKeyHandlerResult } from "./keys";

/** xterm setup is synchronous today; keep the gate so async addons can hook in later. */
let readyPromise: Promise<void> | null = null;
const ensureEngine = (): Promise<void> => (readyPromise ??= ensureReady());
const MAX_REPLAY_CHARS = 2_000_000;

type TerminalDisposable = { dispose(): void };

type TerminalSearchWidget = {
  readonly element: HTMLElement;
  readonly open: () => void;
  readonly close: () => void;
  readonly toggle: () => void;
  readonly isOpen: () => boolean;
  readonly dispose: () => void;
};

const isFindShortcut = (event: KeyboardEvent): boolean =>
  event.key.toLowerCase() === "f" && (event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey;

const isFindNextShortcut = (event: KeyboardEvent): boolean =>
  event.key.toLowerCase() === "g" && (event.metaKey || event.ctrlKey) && !event.altKey;

const createTerminalSearchWidget = (
  term: Terminal,
  addons: TerminalAddonSet,
): TerminalSearchWidget => {
  const root = document.createElement("div");
  root.className = "terminal-search";
  root.hidden = true;

  const input = document.createElement("input");
  input.className = "terminal-search-input";
  input.type = "search";
  input.spellcheck = false;
  input.autocomplete = "off";
  input.placeholder = "Find";

  const previous = document.createElement("button");
  previous.className = "terminal-search-button";
  previous.type = "button";
  previous.title = "Previous match";
  previous.textContent = "↑";

  const next = document.createElement("button");
  next.className = "terminal-search-button";
  next.type = "button";
  next.title = "Next match";
  next.textContent = "↓";

  const close = document.createElement("button");
  close.className = "terminal-search-button";
  close.type = "button";
  close.title = "Close";
  close.textContent = "×";

  root.append(input, previous, next, close);

  const options = (incremental = false) => ({
    incremental,
    decorations: {
      matchBackground: "#7c5f00",
      activeMatchBackground: "#b45309",
      matchOverviewRuler: "#facc15",
      activeMatchColorOverviewRuler: "#fb923c",
    },
  });

  const findNext = (incremental = false): void => {
    const query = input.value;
    if (query.length === 0) {
      addons.search.clearDecorations();
      return;
    }
    addons.search.findNext(query, options(incremental));
  };

  const findPrevious = (): void => {
    const query = input.value;
    if (query.length === 0) {
      addons.search.clearDecorations();
      return;
    }
    addons.search.findPrevious(query, options());
  };

  const open = (): void => {
    root.hidden = false;
    const selection = term.getSelection();
    if (selection.length > 0) input.value = selection.split(/\r?\n/, 1)[0] ?? selection;
    input.focus();
    input.select();
    findNext(true);
  };

  const closeSearch = (): void => {
    root.hidden = true;
    addons.search.clearDecorations();
    term.focus();
  };

  const onInput = (): void => findNext(true);
  const onKeyDown = (event: KeyboardEvent): void => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeSearch();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      if (event.shiftKey) findPrevious();
      else findNext();
    }
  };

  input.addEventListener("input", onInput);
  input.addEventListener("keydown", onKeyDown);
  previous.addEventListener("click", findPrevious);
  next.addEventListener("click", () => findNext());
  close.addEventListener("click", closeSearch);

  return {
    element: root,
    open,
    close: closeSearch,
    toggle: () => root.hidden ? open() : closeSearch(),
    isOpen: () => !root.hidden,
    dispose: () => {
      input.removeEventListener("input", onInput);
      input.removeEventListener("keydown", onKeyDown);
      addons.search.clearDecorations();
    },
  };
};

export const TERMINAL_TITLE = "Terminal";


export function createTerminalRenderer(runtime: AppRuntime): PanelRenderer {
  return {
    title: () => TERMINAL_TITLE,
    mount: (element) => {
      let disposed = false;
      let term: Terminal | null = null;
      let termId: number | null = null;
      let disposeTermUi: (() => void) | null = null;
      let outputReplay = "";
      const port = runtime.terminalPort();

      element.classList.add("terminal-host");
      const body = document.createElement("div");
      body.className = "terminal-body";
      element.replaceChildren(body);

      if (port === undefined) {
        const note = document.createElement("div");
        note.className = "terminal-empty";
        note.textContent = "No terminal available (needs the desktop app).";
        element.replaceChildren(note);
        return { dispose: () => void (disposed = true) };
      }

      const appendReplay = (data: string): void => {
        outputReplay += data;
        if (outputReplay.length > MAX_REPLAY_CHARS) {
          outputReplay = outputReplay.slice(outputReplay.length - MAX_REPLAY_CHARS);
        }
      };

      const openTerminal = (replay: boolean): Terminal => {
        const shouldRefocus = body.contains(document.activeElement);
        disposeTermUi?.();
        body.replaceChildren();
        const next = new Terminal(createTerminalOptions(runtime.themeSnapshot(), runtime.fontSnapshot()));
        const fit = new FitAddon();
        next.open(body);
        const addons = loadTerminalAddons(next, fit);
        const searchWidget = createTerminalSearchWidget(next, addons);
        body.appendChild(searchWidget.element);
        hideNativeInputCaret(next);
        fit.fit();
        const disposeFit = attachAutoFit(fit, body);

        const captureGlobalKey = (event: KeyboardEvent) => {
          if (isFindShortcut(event)) {
            event.preventDefault();
            event.stopPropagation();
            searchWidget.toggle();
            return;
          }
          if (isFindNextShortcut(event) && searchWidget.isOpen()) {
            event.preventDefault();
            event.stopPropagation();
            const input = searchWidget.element.querySelector("input");
            if (input instanceof HTMLInputElement && input.value.length > 0) {
              if (event.shiftKey) addons.search.findPrevious(input.value);
              else addons.search.findNext(input.value);
              term?.focus();
            }
            return;
          }
          if (!runtime.handleGlobalKey(event)) return;
          event.preventDefault();
          event.stopPropagation();
        };
        body.addEventListener("keydown", captureGlobalKey, { capture: true });

        attachGlobalKeyHandler(next, (event) => {
          if (runtime.handleGlobalKey(event)) return "skip";
          const sequence = macEditingSequence(event);
          if (sequence !== null && termId !== null) {
            port.write(termId, sequence);
            return "skip";
          }
          return terminalKeyHandlerResult(event);
        });

        if (replay && outputReplay.length > 0) next.write(outputReplay);

        const dataDisposable = termId === null
          ? null
          : (next.onData((data) => port.write(termId!, data)) as TerminalDisposable);
        const resizeDisposable = termId === null
          ? null
          : (next.onResize(({ cols, rows }) => port.resize(termId!, cols, rows)) as TerminalDisposable);

        term = next;
        if (shouldRefocus) next.focus();
        disposeTermUi = () => {
          dataDisposable?.dispose();
          resizeDisposable?.dispose();
          body.removeEventListener("keydown", captureGlobalKey, { capture: true });
          searchWidget.dispose();
          disposeFit();
          next.dispose();
        };
        return next;
      };

      const unsubscribeTheme = runtime.subscribeTheme(() => {
        if (term !== null) openTerminal(termId !== null);
      });
      const unsubscribeFont = runtime.subscribeFont(() => {
        if (term !== null) openTerminal(termId !== null);
      });
      void (async () => {
        await ensureEngine();
        if (disposed) return;
        const spawnedTerm = openTerminal(false);

        const id = await port.spawn({
          cols: spawnedTerm.cols,
          rows: spawnedTerm.rows,
          ...(runtime.workspaceRoot !== null ? { cwd: runtime.workspaceRoot } : {}),
        });
        if (disposed) {
          if (id !== null) port.kill(id);
          return;
        }
        if (id === null) {
          spawnedTerm.write("\r\n  \x1b[31mcould not start a shell\x1b[0m\r\n");
          return;
        }
        termId = id;
        // PTY → screen, screen → PTY, and keep the PTY's window size in sync.
        port.onData(id, (data) => {
          if (disposed) return;
          appendReplay(data);
          const responses = unsupportedQueryResponses(data, terminalTheme(runtime.themeSnapshot()));
          term?.write(data);
          if (responses.length > 0) port.write(id, responses);
        });
        port.onExit(id, (code) => {
          if (!disposed) term?.write(`\r\n\x1b[90m[process exited${code === null ? "" : ` (${code})`}]\x1b[0m\r\n`);
        });
        const activeTerm = openTerminal(true);
        activeTerm.focus();
      })();

      return {
        dispose: () => {
          disposed = true;
          disposeTermUi?.();
          if (termId !== null) port.kill(termId);
          unsubscribeTheme();
          unsubscribeFont();
          element.replaceChildren();
        },
        focus: () => {
          // xterm.js's hidden textarea can miss a programmatic focus() call if
          // the reverse portal just re-attached its host (terminal toggle on/off).
          // Retry once after the browser has settled the DOM.
          term?.focus();
          requestAnimationFrame(() => term?.focus());
        },
      };
    },
  };
}
