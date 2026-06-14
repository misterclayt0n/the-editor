/**
 * Terminal panel content: an xterm.js-API terminal wired to a PTY over the
 * TerminalPort. The engine (ghostty-web or xterm.js) is chosen in `./backend`
 * — swap that one line; this renderer is engine-agnostic. The host supplies
 * the port (Electron node-pty, or a dev echo for tests).
 */

import { ensureReady, Terminal, FitAddon, attachAutoFit, attachGlobalKeyHandler } from "./backend";
import type { AppRuntime } from "../runtime";
import type { PanelRenderer } from "../workbench/panelRenderers";

/** Some engines (ghostty) load WASM once; gate construction on it, lazily. */
let readyPromise: Promise<void> | null = null;
const ensureEngine = (): Promise<void> => (readyPromise ??= ensureReady());

export const TERMINAL_TITLE = "Terminal";

const FONT_FAMILY = '"SF Mono", Menlo, Monaco, "Cascadia Code", monospace';

export function createTerminalRenderer(runtime: AppRuntime): PanelRenderer {
  return {
    title: () => TERMINAL_TITLE,
    mount: (element) => {
      let disposed = false;
      let term: Terminal | null = null;
      let termId: number | null = null;
      let disposeFit: (() => void) | null = null;
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

      // Palette colors → terminal theme (read the CSS variables we theme with).
      const css = getComputedStyle(document.documentElement);
      const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;

      void (async () => {
        await ensureEngine();
        if (disposed) return;
        term = new Terminal({
          fontSize: 12.5,
          fontFamily: FONT_FAMILY,
          cursorBlink: true,
          theme: {
            background: v("--bg-editor", "#1e1e21"),
            foreground: v("--fg", "#e2e3e5"),
            cursor: v("--caret", "#f5f5f5"),
          },
        });
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.open(body);
        fit.fit();
        disposeFit = attachAutoFit(fit, body);

        // The terminal swallows every keydown (it encodes them for the PTY), so
        // app GLOBAL shortcuts (⌘⇧P, toggle terminal) would be unreachable — a
        // keyboard trap. Give the global keymap first refusal; the backend
        // normalizes the (inverted) custom-key-handler polarity between engines.
        attachGlobalKeyHandler(term, (event) => runtime.handleGlobalKey(event));

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
          disposeFit?.();
          if (termId !== null) port.kill(termId);
          term?.dispose();
          element.replaceChildren();
        },
        focus: () => term?.focus(),
      };
    },
  };
}
