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

/** Some engines (ghostty) load WASM once; gate construction on it, lazily. */
let readyPromise: Promise<void> | null = null;
const ensureEngine = (): Promise<void> => (readyPromise ??= ensureReady());

export const TERMINAL_TITLE = "Terminal";

const FONT_FAMILY = '"SF Mono", Menlo, Monaco, "Cascadia Code", monospace';

/**
 * macOS "natural text editing": Cmd/Option line- and word-motions that every
 * native terminal (Terminal.app, iTerm) maps to readline control bytes. xterm
 * sends NOTHING for Meta-modified keys, so without this the terminal feels dead
 * to ⌘⌫ etc. Returns the bytes to write to the PTY, or null to leave the key.
 * (zsh/bash default to emacs readline, where these control codes apply.)
 */
function macEditingSequence(event: KeyboardEvent): string | null {
  if (event.ctrlKey || event.shiftKey) return null;
  if (event.metaKey && !event.altKey) {
    switch (event.key) {
      case "Backspace":
        return "\x15"; // Ctrl+U — delete to start of line
      case "ArrowLeft":
        return "\x01"; // Ctrl+A — start of line
      case "ArrowRight":
        return "\x05"; // Ctrl+E — end of line
      default:
        return null;
    }
  }
  if (event.altKey && !event.metaKey) {
    switch (event.key) {
      case "Backspace":
        return "\x17"; // Ctrl+W — delete word backward
      case "ArrowLeft":
        return "\x1bb"; // Esc b — word backward
      case "ArrowRight":
        return "\x1bf"; // Esc f — word forward
      case "Delete":
        return "\x1bd"; // Esc d — delete word forward
      default:
        return null;
    }
  }
  return null;
}

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


      const unsubscribeTheme = runtime.subscribeTheme(() => {
        if (term !== null) term.options.theme = terminalTheme(runtime.themeSnapshot());
      });
      void (async () => {
        await ensureEngine();
        if (disposed) return;
        term = new Terminal({
          fontSize: 12.5,
          fontFamily: FONT_FAMILY,
          cursorBlink: true,
          theme: terminalTheme(runtime.themeSnapshot()),
        });
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.open(body);
        fit.fit();
        disposeFit = attachAutoFit(fit, body);

        // The terminal swallows every keydown (it encodes them for the PTY), so
        // routing happens in this claim, in order: (1) app GLOBAL shortcuts
        // (⌘⇧P, toggle terminal) — else the terminal is a keyboard trap;
        // (2) macOS natural-editing chords (⌘⌫, ⌥←, …) which xterm would drop —
        // we send the readline bytes to the PTY ourselves; (3) everything else
        // goes to the shell. The backend normalizes the inverted custom-key
        // polarity between ghostty and xterm.
        attachGlobalKeyHandler(term, (event) => {
          if (runtime.handleGlobalKey(event)) return true;
          const sequence = macEditingSequence(event);
          if (sequence !== null && termId !== null) {
            port.write(termId, sequence);
            return true;
          }
          return false;
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
