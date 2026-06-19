/**
 * Terminal panel content: an xterm.js-API terminal wired to a PTY over the
 * TerminalPort. The engine (ghostty-web or xterm.js) is chosen in `./backend`
 * — swap that one line; this renderer is engine-agnostic. The host supplies
 * the port (Electron node-pty, or a dev echo for tests).
 */

import {
  ensureReady,
  Terminal,
  FitAddon,
  attachAutoFit,
  attachGlobalKeyHandler,
  unsupportedQueryResponses,
  hideNativeInputCaret,
} from "./backend";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "../workbench/panelRenderers";
import { terminalTheme } from "../theme/dom";
import { createTerminalOptions, macEditingSequence, terminalKeyHandlerResult } from "./keys";

/** Some engines (ghostty) load WASM once; gate construction on it, lazily. */
let readyPromise: Promise<void> | null = null;
const ensureEngine = (): Promise<void> => (readyPromise ??= ensureReady());
const MAX_REPLAY_CHARS = 2_000_000;

type TerminalDisposable = { dispose(): void };

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
        const next = new Terminal(createTerminalOptions(runtime.themeSnapshot()));
        const fit = new FitAddon();
        next.loadAddon(fit);
        next.open(body);
        hideNativeInputCaret(next);
        fit.fit();
        const disposeFit = attachAutoFit(fit, body);

        const captureGlobalKey = (event: KeyboardEvent) => {
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
          disposeFit();
          next.dispose();
        };
        return next;
      };

      const unsubscribeTheme = runtime.subscribeTheme(() => {
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
