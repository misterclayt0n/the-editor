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
  clearTerminalSearch,
  resetTerminalSearchSession,
  type TerminalAddonSet,
} from "./backend";
import * as React from "react";
import { createRoot, type Root } from "react-dom/client";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "@the-editor/workbench";
import {
  terminalSearchDecorations,
  terminalTheme,
  type TerminalSearchDecorations,
} from "../theme/dom";
import { createTerminalOptions, macEditingSequence, terminalKeyHandlerResult } from "./keys";
import { TerminalContextMenu, type TerminalContextMenuActions } from "./TerminalContextMenu";
import { handleOsc52ClipboardRequest } from "./osc52";
import { parseOsc9, parseOsc777 } from "./notifications";
import {
  pasteTerminalClipboard,
  writeTerminalPasteText,
  type TerminalPasteTextOptions,
} from "./paste";
import { detectTerminalShortcutPlatform, terminalShortcutAction } from "./shortcuts";

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
  event.key.toLowerCase() === "f" &&
  (event.metaKey || event.ctrlKey) &&
  !event.shiftKey &&
  !event.altKey;

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
  let clearFrame: number | null = null;
  let clearTimeout: number | null = null;
  let clearGeneration = 0;

  const input = document.createElement("input");
  input.className = "terminal-search-input";
  input.type = "text";
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

  const clearSearch = (): void => {
    clearTerminalSearch(term, addons);
  };

  const clearSearchForQuery = (query: string): void => {
    if (query.length > 0) {
      try {
        addons.search.findNext(query, { incremental: true });
      } catch {
        // If the addon is mid-reset, the reset below becomes the source of truth.
      }
    }
    clearSearch();
  };

  const cancelPendingClear = (): void => {
    if (clearFrame !== null) cancelAnimationFrame(clearFrame);
    clearFrame = null;
    if (clearTimeout !== null) window.clearTimeout(clearTimeout);
    clearTimeout = null;
  };

  const resetSearchAfterPaint = (generation: number): void => {
    cancelPendingClear();
    resetTerminalSearchSession(term, addons);
    clearSearch();
    clearFrame = requestAnimationFrame(() => {
      clearFrame = null;
      if (!root.hidden || generation !== clearGeneration) return;
      clearSearch();
      clearTimeout = window.setTimeout(() => {
        clearTimeout = null;
        if (root.hidden && generation === clearGeneration) clearSearch();
      }, 0);
    });
  };

  const findNext = (incremental = false): void => {
    const query = input.value;
    if (query.length === 0) {
      clearSearch();
      return;
    }
    addons.search.findNext(query, options(incremental));
  };

  const findPrevious = (): void => {
    const query = input.value;
    if (query.length === 0) {
      clearSearch();
      return;
    }
    addons.search.findPrevious(query, options());
  };

  const refreshDecorations = (): void => {
    if (root.hidden || input.value.length === 0) return;
    findNext(true);
  };

  const open = (): void => {
    clearGeneration++;
    cancelPendingClear();
    root.hidden = false;
    const selection = term.getSelection();
    if (selection.length > 0) input.value = selection.split(/\r?\n/, 1)[0] ?? selection;
    input.focus();
    input.select();
    findNext(true);
  };

  const closeSearch = (): void => {
    const query = input.value;
    root.hidden = true;
    clearSearchForQuery(query);
    input.value = "";
    clearGeneration++;
    resetSearchAfterPaint(clearGeneration);
    term.focus();
  };

  const onInput = (): void => findNext(true);
  const stopPointerEvent = (event: Event): void => {
    event.stopPropagation();
  };
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
  root.addEventListener("mousedown", stopPointerEvent);
  root.addEventListener("click", stopPointerEvent);
  previous.addEventListener("click", findPrevious);
  next.addEventListener("click", () => findNext());
  close.addEventListener("click", closeSearch);

  return {
    element: root,
    open,
    close: closeSearch,
    toggle: () => (root.hidden ? open() : closeSearch()),
    findNext: () => findNext(),
    findPrevious,
    refreshDecorations,
    isOpen: () => !root.hidden,
    dispose: () => {
      cancelPendingClear();
      input.removeEventListener("input", onInput);
      input.removeEventListener("keydown", onKeyDown);
      root.removeEventListener("mousedown", stopPointerEvent);
      root.removeEventListener("click", stopPointerEvent);
      clearSearch();
    },
  };
};

export const TERMINAL_TITLE = "Terminal";

const shellQuote = (path: string): string => `'${path.replaceAll("'", "'\\''")}'`;

export function createTerminalRenderer(runtime: AppRuntime): PanelRenderer {
  return {
    title: () => TERMINAL_TITLE,
    mount: (element, panel) => {
      let disposed = false;
      let term: Terminal | null = null;
      let termId: number | null = null;
      let addons: TerminalAddonSet | null = null;
      let fitAddon: FitAddon | null = null;
      let searchWidget: TerminalSearchWidget | null = null;
      let disposeTermUi: (() => void) | null = null;
      let refreshFrame: number | null = null;
      let body: HTMLElement | null = null;
      let started = false;
      let reactRoot: Root | null = null;
      const port = runtime.terminalPort();
      const clipboard = runtime.clipboardPort();
      const platform = detectTerminalShortcutPlatform();

      element.classList.add("terminal-host");

      if (port === undefined) {
        const note = document.createElement("div");
        note.className = "terminal-empty";
        note.textContent = "No terminal available (needs the desktop app).";
        element.replaceChildren(note);
        return { dispose: () => void (disposed = true) };
      }

      const bodyHasVisibleSize = (): boolean => {
        const currentBody = body;
        if (currentBody === null || !currentBody.isConnected) return false;
        const rect = currentBody.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0;
      };

      const eventTargetsSearchWidget = (target: EventTarget | null): boolean =>
        target instanceof Element && target.closest(".terminal-search") !== null;

      const writePtyInput = (data: string): void => {
        if (termId !== null) port.write(termId, data);
      };

      const canCopySelection = (): boolean => term?.hasSelection() === true;

      const copySelection = async (): Promise<boolean> => {
        const selection = term?.getSelection() ?? "";
        if (selection.length === 0 || clipboard === undefined) return false;
        await clipboard.writeText(selection);
        return true;
      };

      const pasteTextToTerminal = async (
        text: string,
        options: TerminalPasteTextOptions = {},
      ): Promise<boolean> => {
        const current = term;
        if (current === null || termId === null || text.length === 0) return false;
        const pasted = await writeTerminalPasteText(writePtyInput, text, {
          ...options,
          bracketedPasteMode: current.modes.bracketedPasteMode,
        });
        if (pasted) current.focus();
        return pasted;
      };

      const pasteFromClipboard = async (): Promise<void> => {
        const current = term;
        if (current === null || termId === null) return;
        const result = await pasteTerminalClipboard({
          clipboard,
          write: writePtyInput,
          bracketedPasteMode: current.modes.bracketedPasteMode,
        });
        if (result.status === "pasted") current.focus();
      };

      const clearTerminal = (): void => {
        const current = term;
        if (current === null) return;
        current.select(0, 0, 0);
        current.clearSelection();
        current.clear();
        refreshVisibleRows(current);
        current.focus();
      };

      const selectAllTerminal = (): void => {
        term?.selectAll();
        term?.focus();
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
        const terminalBody = body;
        if (terminalBody === null) throw new Error("Terminal body is not mounted.");
        disposeTermUi?.();
        terminalBody.replaceChildren();
        const next = new Terminal(
          createTerminalOptions(runtime.themeSnapshot(), runtime.fontSnapshot()),
        );
        const fit = new FitAddon();
        next.open(terminalBody);
        const nextAddons = loadTerminalAddons(next, fit);
        const nextSearchWidget = createTerminalSearchWidget(next, nextAddons, () =>
          terminalSearchDecorations(runtime.themeSnapshot()),
        );
        terminalBody.appendChild(nextSearchWidget.element);
        hideNativeInputCaret(next);
        if (bodyHasVisibleSize()) {
          try {
            fit.fit();
          } catch {
            // The first visible attach will fit again.
          }
        }
        const disposeFit = attachAutoFit(fit, terminalBody);

        let suppressNextPasteEvent = false;
        let pasteSuppressionTimer: number | null = null;
        let selectionClipboardTimer: number | null = null;

        const clearPasteSuppression = (): void => {
          suppressNextPasteEvent = false;
          if (pasteSuppressionTimer !== null) window.clearTimeout(pasteSuppressionTimer);
          pasteSuppressionTimer = null;
        };

        const suppressNativePasteOnce = (): void => {
          clearPasteSuppression();
          suppressNextPasteEvent = true;
          pasteSuppressionTimer = window.setTimeout(() => {
            pasteSuppressionTimer = null;
            suppressNextPasteEvent = false;
          }, 0);
        };

        const writeSelectionClipboardSoon = (): void => {
          if (clipboard?.writeSelectionText === undefined) return;
          const selection = next.getSelection();
          if (selection.length === 0) return;
          if (selectionClipboardTimer !== null) window.clearTimeout(selectionClipboardTimer);
          selectionClipboardTimer = window.setTimeout(() => {
            selectionClipboardTimer = null;
            void clipboard.writeSelectionText?.(selection);
          }, 80);
        };

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
          if (!eventTargetsSearchWidget(event.target)) {
            const action = terminalShortcutAction(event, platform);
            if (action !== null) {
              event.preventDefault();
              event.stopPropagation();
              if (action === "copy") {
                void copySelection();
              } else if (action === "paste") {
                suppressNativePasteOnce();
                void pasteFromClipboard();
              } else if (action === "clear") {
                clearTerminal();
              } else if (action === "select-all") {
                selectAllTerminal();
              }
              return;
            }
          }
          if (!runtime.handleGlobalKey(event)) return;
          event.preventDefault();
          event.stopPropagation();
        };
        terminalBody.addEventListener("keydown", captureGlobalKey, { capture: true });

        const onCopy = (event: ClipboardEvent): void => {
          if (eventTargetsSearchWidget(event.target)) return;
          const selection = next.getSelection();
          if (selection.length === 0) return;
          event.preventDefault();
          event.stopPropagation();
          event.clipboardData?.setData("text/plain", selection);
          void clipboard?.writeText(selection);
        };
        terminalBody.addEventListener("copy", onCopy, { capture: true });

        const onPaste = (event: ClipboardEvent): void => {
          if (eventTargetsSearchWidget(event.target)) return;
          if (suppressNextPasteEvent) {
            clearPasteSuppression();
            event.preventDefault();
            event.stopPropagation();
            return;
          }
          event.preventDefault();
          event.stopPropagation();
          const text = event.clipboardData?.getData("text/plain") ?? "";
          if (text.length > 0) void pasteTextToTerminal(text);
          else void pasteFromClipboard();
        };
        terminalBody.addEventListener("paste", onPaste, { capture: true });

        const onMiddleMouseDown = (event: MouseEvent): void => {
          if (event.button !== 1 || eventTargetsSearchWidget(event.target)) return;
          if (clipboard?.readSelectionText === undefined) return;
          event.preventDefault();
          event.stopPropagation();
          void clipboard.readSelectionText({ maxBytes: 16 * 1024 * 1024 }).then((text) => {
            if (text.length > 0) void pasteTextToTerminal(text);
          });
        };
        terminalBody.addEventListener("mousedown", onMiddleMouseDown, { capture: true });

        const onDragOver = (event: DragEvent): void => {
          if ((event.dataTransfer?.files.length ?? 0) === 0) return;
          event.preventDefault();
          event.dataTransfer!.dropEffect = "copy";
        };
        const onDrop = (event: DragEvent): void => {
          const files = [...(event.dataTransfer?.files ?? [])];
          const paths = files.flatMap((file) => {
            const path = (file as File & { readonly path?: string }).path;
            return path === undefined || path.length === 0 ? [] : [shellQuote(path)];
          });
          if (paths.length === 0) return;
          event.preventDefault();
          event.stopPropagation();
          void pasteTextToTerminal(paths.join(" "));
        };
        terminalBody.addEventListener("dragover", onDragOver);
        terminalBody.addEventListener("drop", onDrop);

        const selectionDisposable = next.onSelectionChange(
          writeSelectionClipboardSoon,
        ) as TerminalDisposable;
        const osc52Disposable = next.parser.registerOscHandler(52, (data) =>
          handleOsc52ClipboardRequest(data, {
            allowClipboardWrite: true,
            writeClipboardText: (text) => clipboard?.writeText(text) ?? Promise.resolve(),
            writeSelectionText: (text) =>
              clipboard?.writeSelectionText?.(text) ?? Promise.resolve(),
          }),
        ) as TerminalDisposable;
        // Terminal notifications: bubble the desktop-notification escape
        // sequences (and the bell) up to the Attention fabric. These fire even
        // while the panel is hidden — xterm stays alive, detached/re-attached.
        let terminalTitle = TERMINAL_TITLE;
        const titleDisposable = next.onTitleChange((title) => {
          terminalTitle = title.length > 0 ? title : TERMINAL_TITLE;
        }) as TerminalDisposable;
        const osc9Disposable = next.parser.registerOscHandler(9, (data) => {
          const note = parseOsc9(data);
          if (note !== null) runtime.notifyTerminal({ ...note, panelId: panel.id });
          return note !== null;
        }) as TerminalDisposable;
        const osc777Disposable = next.parser.registerOscHandler(777, (data) => {
          const note = parseOsc777(data);
          if (note !== null) runtime.notifyTerminal({ ...note, panelId: panel.id });
          return note !== null;
        }) as TerminalDisposable;
        const bellDisposable = next.onBell(() =>
          runtime.notifyTerminal({
            body: terminalTitle !== TERMINAL_TITLE ? terminalTitle : "Terminal activity",
            fromBell: true,
            panelId: panel.id,
          }),
        ) as TerminalDisposable;

        attachGlobalKeyHandler(next, (event) => {
          if (
            !eventTargetsSearchWidget(event.target) &&
            terminalShortcutAction(event, platform) !== null
          )
            return "skip";
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
          if (pasteSuppressionTimer !== null) window.clearTimeout(pasteSuppressionTimer);
          if (selectionClipboardTimer !== null) window.clearTimeout(selectionClipboardTimer);
          osc52Disposable.dispose();
          titleDisposable.dispose();
          osc9Disposable.dispose();
          osc777Disposable.dispose();
          bellDisposable.dispose();
          selectionDisposable.dispose();
          dataDisposable.dispose();
          resizeDisposable.dispose();
          terminalBody.removeEventListener("keydown", captureGlobalKey, { capture: true });
          terminalBody.removeEventListener("copy", onCopy, { capture: true });
          terminalBody.removeEventListener("paste", onPaste, { capture: true });
          terminalBody.removeEventListener("mousedown", onMiddleMouseDown, { capture: true });
          terminalBody.removeEventListener("dragover", onDragOver);
          terminalBody.removeEventListener("drop", onDrop);
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

      const startTerminal = (): void => {
        if (started) return;
        started = true;
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
          // Let the runtime map this panel → PTY id, so closing the tab can
          // check for a running foreground process and confirm first.
          runtime.setTerminalProcessId(panel.id, id);
          syncPtySize(spawnedTerm);
          // PTY → screen, screen → PTY, and keep the PTY's window size in sync.
          port.onData(id, (data) => {
            if (disposed) return;
            const responses = unsupportedQueryResponses(
              data,
              terminalTheme(runtime.themeSnapshot()),
            );
            term?.write(data);
            if (responses.length > 0) port.write(id, responses);
          });
          port.onExit(id, (code) => {
            if (!disposed)
              term?.write(
                `\r\n\x1b[90m[process exited${code === null ? "" : ` (${code})`}]\x1b[0m\r\n`,
              );
          });
          refreshTerminalSurface({ focus: true });
        })();
      };

      const contextMenuActions: TerminalContextMenuActions = {
        copy: async () => {
          await copySelection();
        },
        paste: pasteFromClipboard,
        clear: clearTerminal,
        find: () => {
          searchWidget?.open();
        },
        selectAll: selectAllTerminal,
        focus: () => refreshTerminalSurface({ focus: true }),
      };
      reactRoot = createRoot(element);
      reactRoot.render(
        React.createElement(TerminalContextMenu, {
          actions: contextMenuActions,
          canCopy: canCopySelection,
          onBodyReady: (node: HTMLDivElement | null) => {
            body = node;
            if (node !== null) startTerminal();
          },
          platform,
        }),
      );

      return {
        dispose: () => {
          disposed = true;
          runtime.clearTerminalProcessId(panel.id);
          if (refreshFrame !== null) cancelAnimationFrame(refreshFrame);
          disposeTermUi?.();
          if (termId !== null) port.kill(termId);
          unsubscribeTheme();
          unsubscribeFont();
          reactRoot?.unmount();
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
