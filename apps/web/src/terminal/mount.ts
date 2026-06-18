/**
 * Terminal panel content: an xterm.js-API terminal wired to a PTY over the
 * TerminalPort. The engine (ghostty-web or xterm.js) is chosen in `./backend`
 * — swap that one line; this renderer is engine-agnostic. The host supplies
 * the port (Electron node-pty, or a dev echo for tests).
 */

import { ensureReady, Terminal, FitAddon, attachAutoFit, attachGlobalKeyHandler } from "./backend";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "../workbench/panelRenderers";
import { terminalTheme } from "../theme/dom";
import { createTerminalOptions, macEditingSequence, terminalKeyHandlerResult } from "./keys";

/** Some engines (ghostty) load WASM once; gate construction on it, lazily. */
let readyPromise: Promise<void> | null = null;
const ensureEngine = (): Promise<void> => (readyPromise ??= ensureReady());
export const TERMINAL_TITLE = "Terminal";


export function createTerminalRenderer(runtime: AppRuntime): PanelRenderer {
  return {
    title: () => TERMINAL_TITLE,
    mount: (element) => {
      let disposed = false;
      let term: Terminal | null = null;
      let termId: number | null = null;
      let disposeFit: (() => void) | null = null;
      let disposeGlobalKeyCapture: (() => void) | null = null;
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


      const unsubscribeTheme = runtime.subscribeTheme(() => {
        if (term !== null) term.options.theme = terminalTheme(runtime.themeSnapshot());
      });
      void (async () => {
        await ensureEngine();
        if (disposed) return;
        term = new Terminal(createTerminalOptions(runtime.themeSnapshot()));
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.open(body);
        fit.fit();
        disposeFit = attachAutoFit(fit, body);

        // Some browser/Electron accelerators (notably ⌘W) must be cancelled at
        // DOM capture time; xterm's custom key handler can be too late even
        // though ordinary globals like ⌘⇧P still route through it.
        const captureGlobalKey = (event: KeyboardEvent) => {
          if (!runtime.handleGlobalKey(event)) return;
          event.preventDefault();
          event.stopPropagation();
        };
        body.addEventListener("keydown", captureGlobalKey, { capture: true });
        disposeGlobalKeyCapture = () => body.removeEventListener("keydown", captureGlobalKey, { capture: true });

        // The terminal swallows every keydown before it reaches the PTY, so the
        // browser gate runs first: (1) reserve app-global shortcuts so the
        // terminal is never a keyboard trap; (2) emit a tiny set of native macOS
        // readline chords ourselves; (3) let xterm encode everything else,
        // including kitty keyboard protocol sequences for apps that opt into
        // richer modified-key reporting; (4) keep Shift+Tab inside the terminal
        // instead of letting browser focus move.
        attachGlobalKeyHandler(term, (event) => {
          if (runtime.handleGlobalKey(event)) return "skip";
          const sequence = macEditingSequence(event);
          if (sequence !== null && termId !== null) {
            port.write(termId, sequence);
            return "skip";
          }
          return terminalKeyHandlerResult(event);
        });

        const id = await port.spawn({
          cols: term.cols,
          rows: term.rows,
          ...(runtime.workspaceRoot !== null ? { cwd: runtime.workspaceRoot } : {}),
        });
        if (disposed) {
          if (id !== null) port.kill(id);
          return;
        }
        if (id === null) {
          term.write("\r\n  \x1b[31mcould not start a shell\x1b[0m\r\n");
          return;
        }
        termId = id;
        // PTY → screen, screen → PTY, and keep the PTY's window size in sync.
        port.onData(id, (data) => { if (!disposed) term?.write(data); });
        port.onExit(id, (code) => {
          if (!disposed) term?.write(`\r\n\x1b[90m[process exited${code === null ? "" : ` (${code})`}]\x1b[0m\r\n`);
        });
        term.onData((data) => port.write(id, data));
        term.onResize(({ cols, rows }) => port.resize(id, cols, rows));
        term.focus();
      })();

      return {
        dispose: () => {
          disposed = true;
          disposeGlobalKeyCapture?.();
          disposeFit?.();
          if (termId !== null) port.kill(termId);
          term?.dispose();
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
