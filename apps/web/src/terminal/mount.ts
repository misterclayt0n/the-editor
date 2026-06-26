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
  recoverTerminalRenderer,
  attachAutoFit,
  attachGlobalKeyHandler,
  unsupportedQueryResponses,
  hideNativeInputCaret,
  type TerminalAddonSet,
} from "./backend";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "../workbench/panelRenderers";
import { terminalSearchDecorations, terminalTheme, type TerminalSearchDecorations } from "../theme/dom";
import { createTerminalOptions, macEditingSequence, terminalKeyHandlerResult } from "./keys";

/** xterm setup is synchronous today; keep the gate so async addons can hook in later. */
let readyPromise: Promise<void> | null = null;
const ensureEngine = (): Promise<void> => (readyPromise ??= ensureReady());

type TerminalDisposable = { dispose(): void };

type TerminalSearchWidget = {
  readonly element: HTMLElement;
  readonly open: () => void;
  readonly close: () => void;
  readonly toggle: () => void;
  readonly findNext: () => void;
  readonly findPrevious: () => void;
  readonly refreshDecorations: () => void;
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
  decorations: () => TerminalSearchDecorations,
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
    decorations: decorations(),
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

  const refreshDecorations = (): void => {
    if (root.hidden || input.value.length === 0) return;
    findNext(true);
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
    findNext: () => findNext(),
    findPrevious,
    refreshDecorations,
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
      let addons: TerminalAddonSet | null = null;
      let fitAddon: FitAddon | null = null;
      let searchWidget: TerminalSearchWidget | null = null;
      let disposeTermUi: (() => void) | null = null;
      let refreshFrame: number | null = null;
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

      const bodyHasVisibleSize = (): boolean => {
        if (!body.isConnected) return false;
        const rect = body.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0;
      };

      const syncPtySize = (current: Terminal): void => {
        if (termId === null || current.cols <= 0 || current.rows <= 0) return;
        port.resize(termId, current.cols, current.rows);
      };

      const refreshVisibleRows = (current: Terminal): void => {
        if (current.rows <= 0) return;
        current.refresh(0, current.rows - 1);
      };

      const refreshTerminalSurface = (
        options: { readonly focus?: boolean; readonly recoverRenderer?: boolean } = {},
      ): void => {
        const current = term;
        if (current === null) return;
        const shouldFocus = options.focus === true;
        const run = (recoverRenderer: boolean): void => {
          if (disposed || term !== current || !bodyHasVisibleSize()) return;
          if (recoverRenderer && addons !== null) recoverTerminalRenderer(current, addons);
          try {
            fitAddon?.fit();
          } catch {
            // Detached or zero-sized terminals are refit on the next visible frame.
          }
          syncPtySize(current);
          try {
            refreshVisibleRows(current);
          } catch {
            // Refresh can race terminal disposal.
          }
          if (shouldFocus) current.focus();
        };

        run(false);
        if (refreshFrame !== null) cancelAnimationFrame(refreshFrame);
        refreshFrame = requestAnimationFrame(() => {
          refreshFrame = null;
          run(options.recoverRenderer === true);
        });
      };

      const applyTerminalOptions = (recoverRenderer: boolean): void => {
        const current = term;
        if (current === null) return;
        const options = createTerminalOptions(runtime.themeSnapshot(), runtime.fontSnapshot());
        if (options.theme !== undefined) current.options.theme = options.theme;
        if (options.fontFamily !== undefined) current.options.fontFamily = options.fontFamily;
        if (options.fontSize !== undefined) current.options.fontSize = options.fontSize;
        searchWidget?.refreshDecorations();
        refreshTerminalSurface({ recoverRenderer });
      };

      const openTerminal = (): Terminal => {
        disposeTermUi?.();
        body.replaceChildren();
        const next = new Terminal(createTerminalOptions(runtime.themeSnapshot(), runtime.fontSnapshot()));
        const fit = new FitAddon();
        next.open(body);
        const nextAddons = loadTerminalAddons(next, fit);
        const nextSearchWidget = createTerminalSearchWidget(next, nextAddons, () =>
          terminalSearchDecorations(runtime.themeSnapshot()),
        );
        body.appendChild(nextSearchWidget.element);
        hideNativeInputCaret(next);
        if (bodyHasVisibleSize()) {
          try {
            fit.fit();
          } catch {
            // The first visible attach will fit again.
          }
        }
        const disposeFit = attachAutoFit(fit, body);

        const captureGlobalKey = (event: KeyboardEvent) => {
          if (isFindShortcut(event)) {
            event.preventDefault();
            event.stopPropagation();
            nextSearchWidget.toggle();
            return;
          }
          if (isFindNextShortcut(event) && nextSearchWidget.isOpen()) {
            event.preventDefault();
            event.stopPropagation();
            const input = nextSearchWidget.element.querySelector("input");
            if (input instanceof HTMLInputElement && input.value.length > 0) {
              if (event.shiftKey) nextSearchWidget.findPrevious();
              else nextSearchWidget.findNext();
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

        const dataDisposable = next.onData((data) => {
          if (termId !== null) port.write(termId, data);
        }) as TerminalDisposable;
        const resizeDisposable = next.onResize(({ cols, rows }) => {
          if (termId !== null) port.resize(termId, cols, rows);
        }) as TerminalDisposable;

        term = next;
        addons = nextAddons;
        fitAddon = fit;
        searchWidget = nextSearchWidget;
        disposeTermUi = () => {
          dataDisposable.dispose();
          resizeDisposable.dispose();
          body.removeEventListener("keydown", captureGlobalKey, { capture: true });
          nextSearchWidget.dispose();
          disposeFit();
          next.dispose();
          if (term === next) term = null;
          if (addons === nextAddons) addons = null;
          if (fitAddon === fit) fitAddon = null;
          if (searchWidget === nextSearchWidget) searchWidget = null;
        };
        return next;
      };

      const unsubscribeTheme = runtime.subscribeTheme(() => {
        applyTerminalOptions(false);
      });
      const unsubscribeFont = runtime.subscribeFont(() => {
        applyTerminalOptions(true);
      });
      void (async () => {
        await ensureEngine();
        if (disposed) return;
        const spawnedTerm = openTerminal();

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
        syncPtySize(spawnedTerm);
        // PTY → screen, screen → PTY, and keep the PTY's window size in sync.
        port.onData(id, (data) => {
          if (disposed) return;
          const responses = unsupportedQueryResponses(data, terminalTheme(runtime.themeSnapshot()));
          term?.write(data);
          if (responses.length > 0) port.write(id, responses);
        });
        port.onExit(id, (code) => {
          if (!disposed) term?.write(`\r\n\x1b[90m[process exited${code === null ? "" : ` (${code})`}]\x1b[0m\r\n`);
        });
        refreshTerminalSurface({ focus: true });
      })();

      return {
        dispose: () => {
          disposed = true;
          if (refreshFrame !== null) cancelAnimationFrame(refreshFrame);
          disposeTermUi?.();
          if (termId !== null) port.kill(termId);
          unsubscribeTheme();
          unsubscribeFont();
          element.replaceChildren();
        },
        onAttached: () => {
          refreshTerminalSurface({ recoverRenderer: true });
        },
        focus: () => {
          // xterm.js's hidden textarea can miss a programmatic focus() call if
          // the reverse portal just re-attached its host (terminal toggle on/off).
          // Retry once after the browser has settled the DOM.
          refreshTerminalSurface({ focus: true });
        },
      };
    },
  };
}
