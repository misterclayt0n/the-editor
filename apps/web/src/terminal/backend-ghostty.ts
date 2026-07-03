/**
 * Terminal engine: ghostty-web. Ghostty's VT emulator compiled to WASM behind
 * an xterm.js-shaped API, plus the local shims the host needs:
 *
 * - `parser.registerOscHandler` (OSC 52/9/777) via a stateful stream scanner —
 *   the WASM build intentionally ignores clipboard/notification OSCs, so the
 *   host owns them at the PTY-data seam.
 * - a bell that ignores the BEL bytes that merely terminate OSC sequences
 *   (shell title updates must not ring the attention bell).
 * - runtime theme changes wired to the canvas renderer (upstream only warns).
 * - buffer-scan search with the active match shown as the terminal selection.
 *
 * Known limits vs the old xterm backend: no font ligatures (per-cell canvas
 * renderer) and ANSI palette colors of already-written cells keep the palette
 * the WASM terminal was created with until the terminal is recreated.
 */

import type { TerminalKeyHandlerResult } from "./keys";

import {
  EventEmitter,
  FitAddon,
  Ghostty,
  KeyEncoderOption,
  Terminal as GhosttyWebTerminal,
  type IDisposable,
  type IEvent,
  type ITerminalCore,
  type ITerminalOptions,
} from "ghostty-web";
import ghosttyWasmUrl from "ghostty-web/ghostty-vt.wasm?url";

export { FitAddon };

let ghosttyRuntime: Ghostty | null = null;

/** Load the Ghostty WASM module once; Terminal construction requires it. */
export const ensureReady = async (): Promise<void> => {
  ghosttyRuntime ??= await Ghostty.load(ghosttyWasmUrl);
};

const requireGhostty = (): Ghostty => {
  if (ghosttyRuntime === null) {
    throw new Error("Terminal engine is not ready — await ensureReady() before creating a Terminal.");
  }
  return ghosttyRuntime;
};

// ---------------------------------------------------------------------------
// PTY stream scanning: OSC dispatch + bell filtering
// ---------------------------------------------------------------------------

export type TerminalOscEvent = { readonly code: number; readonly data: string };

/** A CSI sequence split as everything-before-the-final-byte plus the final. */
export type TerminalCsiEvent = { readonly body: string; readonly final: string };

export type TerminalStreamEvents = {
  readonly bells: number;
  readonly oscs: readonly TerminalOscEvent[];
  readonly csis: readonly TerminalCsiEvent[];
};

const ESC = "\x1b";
const BEL = "\x07";
const MAX_OSC_PAYLOAD_LENGTH = 512 * 1024;
const MAX_CSI_BODY_LENGTH = 64;

/** CSI finals worth surfacing: device attributes, kitty keyboard, DECRQM, and
 * mode set/reset (alt-screen tracking). Everything else (SGR spam) is dropped
 * inside the scanner so hot output paths allocate nothing per sequence. */
const INTERESTING_CSI_FINALS = new Set(["c", "u", "p", "h", "l"]);

/**
 * Minimal VT sequence scanner over the PTY byte stream. It exists for what
 * the WASM emulator does not surface: dispatching OSC payloads to registered
 * handlers, surfacing the CSI queries the WASM leaves unanswered (DA1/DA2,
 * kitty keyboard, DECRQM), tracking mode sets for the kitty tracker, and
 * counting bells while ignoring the BEL bytes that terminate OSC sequences
 * (and the bytes inside DCS/APC/PM/SOS strings). State survives feed() calls,
 * so sequences may span PTY chunks.
 */
export class TerminalStreamScanner {
  private state: "text" | "esc" | "osc" | "osc-esc" | "csi" | "string" | "string-esc" = "text";
  private payload = "";
  private overflowed = false;

  feed(data: string): TerminalStreamEvents {
    let bells = 0;
    const oscs: TerminalOscEvent[] = [];
    const csis: TerminalCsiEvent[] = [];
    let index = 0;
    while (index < data.length) {
      const char = data[index]!;
      switch (this.state) {
        case "text": {
          // Fast path: jump straight to the next byte we care about.
          const escIndex = data.indexOf(ESC, index);
          const belIndex = data.indexOf(BEL, index);
          if (escIndex === -1 && belIndex === -1) return { bells, oscs, csis };
          if (belIndex !== -1 && (escIndex === -1 || belIndex < escIndex)) {
            bells += 1;
            index = belIndex + 1;
          } else {
            this.state = "esc";
            index = escIndex + 1;
          }
          continue;
        }
        case "esc":
          if (char === "]") {
            this.state = "osc";
            this.payload = "";
            this.overflowed = false;
          } else if (char === "[") {
            this.state = "csi";
            this.payload = "";
            this.overflowed = false;
          } else if (char === "P" || char === "X" || char === "^" || char === "_") {
            this.state = "string";
          } else if (char !== ESC) {
            this.state = "text";
          }
          break;
        case "csi": {
          const code = char.charCodeAt(0);
          if (code >= 0x40 && code <= 0x7e) {
            if (!this.overflowed && INTERESTING_CSI_FINALS.has(char)) {
              csis.push({ body: this.payload, final: char });
            }
            this.payload = "";
            this.overflowed = false;
            this.state = "text";
          } else if (code >= 0x20 && code <= 0x3f) {
            if (this.payload.length >= MAX_CSI_BODY_LENGTH) this.overflowed = true;
            else this.payload += char;
          } else {
            // Malformed sequence (C0 control or stray ESC): bail out.
            this.payload = "";
            this.overflowed = false;
            this.state = char === ESC ? "esc" : "text";
          }
          break;
        }
        case "osc":
          if (char === BEL) {
            this.finishOsc(oscs);
            this.state = "text";
          } else if (char === ESC) {
            this.state = "osc-esc";
          } else {
            this.appendPayload(char);
          }
          break;
        case "osc-esc":
          if (char === "\\") {
            this.finishOsc(oscs);
            this.state = "text";
          } else if (char === ESC) {
            this.appendPayload(ESC);
          } else {
            this.appendPayload(ESC);
            this.appendPayload(char);
            this.state = "osc";
          }
          break;
        case "string":
          if (char === ESC) this.state = "string-esc";
          break;
        case "string-esc":
          if (char === "\\") this.state = "text";
          else if (char !== ESC) this.state = "string";
          break;
      }
      index += 1;
    }
    return { bells, oscs, csis };
  }

  private appendPayload(char: string): void {
    if (this.payload.length >= MAX_OSC_PAYLOAD_LENGTH) {
      this.overflowed = true;
      return;
    }
    this.payload += char;
  }

  private finishOsc(out: TerminalOscEvent[]): void {
    const payload = this.payload;
    this.payload = "";
    if (this.overflowed) {
      this.overflowed = false;
      return;
    }
    const semi = payload.indexOf(";");
    const identifier = semi === -1 ? payload : payload.slice(0, semi);
    if (!/^\d+$/.test(identifier)) return;
    out.push({
      code: Number.parseInt(identifier, 10),
      data: semi === -1 ? "" : payload.slice(semi + 1),
    });
  }
}

// ---------------------------------------------------------------------------
// Kitty keyboard protocol tracking
// ---------------------------------------------------------------------------

const KITTY_FLAGS_MASK = 0b11111;
const MAX_KITTY_STACK_DEPTH = 32;

/**
 * The WASM terminal records kitty keyboard pushes/pops internally but exports
 * no way to read them, so the JS key encoder would never learn that a TUI
 * enabled the protocol. Mirror the per-screen flag stacks by watching the
 * same CSI sequences the emulator sees.
 */
export class KittyKeyboardTracker {
  private readonly main: number[] = [0];
  private readonly alt: number[] = [0];
  private onAlt = false;

  get flags(): number {
    const stack = this.onAlt ? this.alt : this.main;
    return stack[stack.length - 1] ?? 0;
  }

  /**
   * Feed one CSI event; returns true when the effective flags may have
   * changed. Handles `CSI > flags u` (push), `CSI < n u` (pop),
   * `CSI = flags ; mode u` (set/or/not), and alt-screen switches.
   */
  apply(csi: TerminalCsiEvent): boolean {
    if (csi.final === "u") {
      const stack = this.onAlt ? this.alt : this.main;
      if (csi.body.startsWith(">")) {
        const flags = (Number.parseInt(csi.body.slice(1), 10) || 0) & KITTY_FLAGS_MASK;
        if (stack.length < MAX_KITTY_STACK_DEPTH) stack.push(flags);
        else stack[stack.length - 1] = flags;
        return true;
      }
      if (csi.body.startsWith("<")) {
        const count = Number.parseInt(csi.body.slice(1), 10) || 1;
        for (let i = 0; i < count && stack.length > 1; i += 1) stack.pop();
        return true;
      }
      if (csi.body.startsWith("=")) {
        const [flagsPart, modePart] = csi.body.slice(1).split(";");
        const flags = (Number.parseInt(flagsPart ?? "0", 10) || 0) & KITTY_FLAGS_MASK;
        const mode = Number.parseInt(modePart ?? "1", 10) || 1;
        const top = stack.length - 1;
        if (mode === 2) stack[top] = (stack[top] ?? 0) | flags;
        else if (mode === 3) stack[top] = (stack[top] ?? 0) & ~flags;
        else stack[top] = flags;
        return true;
      }
      return false;
    }
    if ((csi.final === "h" || csi.final === "l") && csi.body.startsWith("?")) {
      const modes = csi.body.slice(1).split(";");
      if (modes.includes("1049") || modes.includes("1047") || modes.includes("47")) {
        this.onAlt = csi.final === "h";
        return true;
      }
    }
    return false;
  }
}

// ---------------------------------------------------------------------------
// CSI queries the WASM build does not answer
// ---------------------------------------------------------------------------

export type TerminalQueryContext = {
  /** DEC/ANSI private mode lookup (DECRQM). */
  readonly getMode: (mode: number, isAnsi: boolean) => boolean;
  /** Current effective kitty keyboard flags. */
  readonly kittyFlags: () => number;
};

/**
 * The v0.4.0 WASM answers only DSR 5/6; DA1/DA2, the kitty keyboard query,
 * and DECRQM all go silent, which stalls capability probes (crossterm — and
 * therefore nushell — waits its full timeout on `CSI ? u` + `CSI c`). Answer
 * them here. Returns null for sequences that are not queries.
 */
export function terminalQueryResponse(
  csi: TerminalCsiEvent,
  context: TerminalQueryContext,
): string | null {
  if (csi.final === "c") {
    // DA1 / DA2, matching the replies native Ghostty sends.
    if (csi.body === "" || csi.body === "0") return "\x1b[?62;22c";
    if (csi.body === ">" || csi.body === ">0") return "\x1b[>1;10;0c";
    return null;
  }
  if (csi.final === "u" && csi.body === "?") {
    return `\x1b[?${context.kittyFlags()}u`;
  }
  if (csi.final === "p" && csi.body.endsWith("$")) {
    // DECRQM. Report set/reset from live emulator state; 0 would mean "not
    // recognized", but getMode cannot distinguish unknown from reset.
    const dec = csi.body.startsWith("?");
    const digits = csi.body.slice(dec ? 1 : 0, -1);
    if (!/^\d+$/.test(digits)) return null;
    const mode = Number.parseInt(digits, 10);
    let status = 2;
    try {
      status = context.getMode(mode, !dec) ? 1 : 2;
    } catch {
      // Terminal not open yet; report reset.
    }
    return dec ? `\x1b[?${mode};${status}$y` : `\x1b[${mode};${status}$y`;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Terminal
// ---------------------------------------------------------------------------

type OscHandler = (data: string) => boolean;

type TerminalParser = {
  readonly registerOscHandler: (code: number, handler: OscHandler) => IDisposable;
};

type TerminalModes = {
  readonly bracketedPasteMode: boolean;
};

type OptionChangeHandler = (key: string, next: unknown, previous: unknown) => void;

export class Terminal extends GhosttyWebTerminal {
  /** xterm-compatible OSC handler registry, fed from the PTY data stream. */
  public readonly parser: TerminalParser;

  /** Bell that ignores OSC-terminating BEL bytes; shadows the upstream event. */
  public override readonly onBell: IEvent<void>;

  private readonly oscHandlers = new Map<number, OscHandler[]>();
  private readonly streamScanner = new TerminalStreamScanner();
  private readonly filteredBellEmitter = new EventEmitter<void>();
  private readonly streamDecoder = new TextDecoder();
  private readonly kittyTracker = new KittyKeyboardTracker();
  private lastKittyFlags = 0;

  constructor(options: ITerminalOptions = {}) {
    super({ ...options, ghostty: options.ghostty ?? requireGhostty() });
    this.onBell = this.filteredBellEmitter.event;
    this.parser = {
      registerOscHandler: (code, handler) => {
        const handlers = this.oscHandlers.get(code) ?? [];
        handlers.push(handler);
        this.oscHandlers.set(code, handlers);
        return {
          dispose: () => {
            const current = this.oscHandlers.get(code);
            if (current === undefined) return;
            const index = current.indexOf(handler);
            if (index !== -1) current.splice(index, 1);
          },
        };
      },
    };
    // Upstream warns and ignores runtime theme assignment; route it to the
    // canvas renderer instead. The options object is a Proxy that calls the
    // (private) handleOptionChange, so shadow it on the instance.
    const basePrototype = GhosttyWebTerminal.prototype as unknown as {
      handleOptionChange: OptionChangeHandler;
    };
    (this as unknown as { handleOptionChange: OptionChangeHandler }).handleOptionChange = (
      key,
      next,
      previous,
    ) => {
      if (key === "theme") {
        this.renderer?.setTheme(this.options.theme);
        this.refresh(0, this.rows - 1);
        return;
      }
      basePrototype.handleOptionChange.call(this, key, next, previous);
    };
  }

  override open(parent: HTMLElement): void {
    super.open(parent);
    // macOS "Option as Meta" (and standard Alt-as-ESC-prefix elsewhere): the
    // encoder option is not exposed publicly yet, so set it best-effort.
    try {
      this.encoder()?.setOption?.(KeyEncoderOption.ALT_ESC_PREFIX, true);
    } catch {
      // Alt keys fall back to the encoder's default behavior.
    }
  }

  private encoder():
    | { setOption?: (option: number, value: boolean) => void; setKittyFlags?: (flags: number) => void }
    | undefined {
    return (this as unknown as {
      inputHandler?: {
        encoder?: {
          setOption?: (option: number, value: boolean) => void;
          setKittyFlags?: (flags: number) => void;
        };
      };
    }).inputHandler?.encoder;
  }

  override write(data: string | Uint8Array, callback?: () => void): void {
    const text =
      typeof data === "string" ? data : this.streamDecoder.decode(data, { stream: true });
    const events = this.streamScanner.feed(text);
    for (const osc of events.oscs) {
      const handlers = this.oscHandlers.get(osc.code);
      if (handlers === undefined) continue;
      for (const handler of handlers) {
        if (handler(osc.data)) break;
      }
    }
    if (events.bells > 0) this.filteredBellEmitter.fire();
    let kittyDirty = false;
    for (const csi of events.csis) {
      kittyDirty = this.kittyTracker.apply(csi) || kittyDirty;
      const response = terminalQueryResponse(csi, {
        getMode: (mode, isAnsi) => this.getMode(mode, isAnsi),
        kittyFlags: () => this.kittyTracker.flags,
      });
      // Responses ride the user-input path so they reach the PTY through the
      // same onData wiring — and before the WASM's own replies (DSR) for
      // queries later in this chunk, preserving response order.
      if (response !== null) {
        try {
          this.input(response, true);
        } catch {
          // Not open yet; the querier will retry or time out as before.
        }
      }
    }
    if (kittyDirty && this.kittyTracker.flags !== this.lastKittyFlags) {
      this.lastKittyFlags = this.kittyTracker.flags;
      try {
        this.encoder()?.setKittyFlags?.(this.lastKittyFlags);
      } catch {
        // Encoder unavailable; keys keep legacy encoding.
      }
    }
    super.write(data, callback);
  }

  /** xterm compatibility: the host only reads bracketed-paste mode. */
  get modes(): TerminalModes {
    try {
      return { bracketedPasteMode: this.hasBracketedPaste() };
    } catch {
      return { bracketedPasteMode: false };
    }
  }

  /**
   * Re-measure font metrics and resize the canvas. Needed when a webfont
   * finishes loading after open(): the upstream option-change path skips
   * re-measuring when the font string is unchanged.
   */
  remeasureFont(): void {
    if (this.renderer === undefined || this.wasmTerm === undefined) return;
    try {
      this.renderer.setFontFamily(this.options.fontFamily);
      (GhosttyWebTerminal.prototype as unknown as { handleFontChange: () => void }).handleFontChange.call(
        this,
      );
    } catch {
      // Metrics keep the fallback font's sizing until the next font change.
    }
  }

  /** Force a full repaint; the canvas renderer has no per-row refresh. */
  refresh(_start: number, _end: number): void {
    if (this.renderer === undefined || this.wasmTerm === undefined) return;
    try {
      const scrollbarOpacity =
        (this as unknown as { scrollbarOpacity?: number }).scrollbarOpacity ?? 0;
      this.renderer.render(this.wasmTerm, true, this.viewportY, this, scrollbarOpacity);
    } catch {
      // Refresh can race terminal disposal.
    }
  }
}

// ---------------------------------------------------------------------------
// Color queries the WASM build does not answer
// ---------------------------------------------------------------------------

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

/**
 * The Ghostty WASM build's color-operation handler is a stub, so OSC 10/11/12
 * and OSC 4 queries would otherwise hang programs that wait for a reply.
 * Answer them from the resolved theme at the PTY-data seam.
 */
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

/**
 * ghostty-web marks its container contenteditable (so browser extensions
 * treat it as an input), which makes the browser draw a blinking caret at the
 * container's top-left once it is focused. Kill it; the emulator paints its
 * own cursor.
 */
export function hideNativeInputCaret(term: Terminal): void {
  if (term.element !== undefined) term.element.style.caretColor = "transparent";
  if (term.textarea !== undefined) term.textarea.style.caretColor = "transparent";
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

export type TerminalSearchMatch = {
  readonly row: number;
  readonly col: number;
  readonly length: number;
};

export type TerminalSearchOptions = {
  readonly incremental?: boolean;
  /** Accepted for xterm API compatibility; the canvas renderer has no
   * decoration layer, so the active match is shown as the selection. */
  readonly decorations?: unknown;
};

export const MAX_TERMINAL_SEARCH_MATCHES = 250;

/** Case-insensitive literal search over buffer rows; pure for testability. */
export function findBufferMatches(
  lineText: (row: number) => string,
  rowCount: number,
  query: string,
  maxMatches: number = MAX_TERMINAL_SEARCH_MATCHES,
): TerminalSearchMatch[] {
  const matches: TerminalSearchMatch[] = [];
  if (query.length === 0 || rowCount <= 0) return matches;
  const needle = query.toLowerCase();
  for (let row = 0; row < rowCount; row += 1) {
    const text = lineText(row).toLowerCase();
    let col = text.indexOf(needle);
    while (col !== -1) {
      matches.push({ row, col, length: query.length });
      if (matches.length >= maxMatches) return matches;
      col = text.indexOf(needle, col + 1);
    }
  }
  return matches;
}

type SelectionManagerInternals = {
  selectionStart: { col: number; absoluteRow: number } | null;
  selectionEnd: { col: number; absoluteRow: number } | null;
  requestRender?: () => void;
};

/**
 * Buffer-scan search with the xterm SearchAddon surface the host uses. The
 * active match is revealed by scrolling and shown as the terminal selection.
 */
export class SearchAddon {
  private terminal: Terminal | null = null;
  private activeMatch: TerminalSearchMatch | null = null;

  activate(terminal: ITerminalCore): void {
    this.terminal = terminal as Terminal;
  }

  dispose(): void {
    this.terminal = null;
    this.activeMatch = null;
  }

  clearDecorations(): void {
    this.activeMatch = null;
  }

  clearActiveDecoration(): void {
    this.activeMatch = null;
  }

  findNext(query: string, options: TerminalSearchOptions = {}): boolean {
    return this.find(query, options.incremental === true ? "incremental" : "next");
  }

  findPrevious(query: string, _options: TerminalSearchOptions = {}): boolean {
    return this.find(query, "previous");
  }

  private find(query: string, direction: "incremental" | "next" | "previous"): boolean {
    const term = this.terminal;
    if (term === null || query.length === 0) return false;
    const buffer = term.buffer.active;
    const matches = findBufferMatches(
      (row) => buffer.getLine(row)?.translateToString(true) ?? "",
      buffer.length,
      query,
    );
    if (matches.length === 0) {
      this.activeMatch = null;
      try {
        term.clearSelection();
      } catch {
        // The terminal may be mid-disposal.
      }
      return false;
    }
    const anchor = this.activeMatch;
    let next: TerminalSearchMatch | undefined;
    if (anchor === null) {
      const top = this.viewportTopRow(term);
      next =
        direction === "previous"
          ? matches.findLast((match) => match.row <= top)
          : matches.find((match) => match.row >= top);
    } else if (direction === "incremental") {
      next = matches.find(
        (match) => match.row > anchor.row || (match.row === anchor.row && match.col >= anchor.col),
      );
    } else if (direction === "next") {
      next = matches.find(
        (match) => match.row > anchor.row || (match.row === anchor.row && match.col > anchor.col),
      );
    } else {
      next = matches.findLast(
        (match) => match.row < anchor.row || (match.row === anchor.row && match.col < anchor.col),
      );
    }
    const match = next ?? (direction === "previous" ? matches[matches.length - 1]! : matches[0]!);
    this.reveal(term, match);
    return true;
  }

  private viewportTopRow(term: Terminal): number {
    try {
      return Math.max(0, term.getScrollbackLength() - Math.max(0, Math.floor(term.getViewportY())));
    } catch {
      return 0;
    }
  }

  private reveal(term: Terminal, match: TerminalSearchMatch): void {
    this.activeMatch = match;
    try {
      const scrollback = term.getScrollbackLength();
      const viewportY = Math.max(0, Math.floor(term.getViewportY()));
      const top = scrollback - viewportY;
      const bottom = top + term.rows - 1;
      if (match.row < top) term.scrollToLine(scrollback - match.row);
      else if (match.row > bottom) term.scrollToLine(scrollback - match.row + term.rows - 1);
      // ghostty-web's public select() mis-maps rows once scrollback exists, so
      // set the selection through the manager's absolute-row coordinates.
      const manager = (term as unknown as { selectionManager?: SelectionManagerInternals })
        .selectionManager;
      if (manager !== undefined) {
        manager.selectionStart = { col: match.col, absoluteRow: match.row };
        manager.selectionEnd = { col: match.col + match.length - 1, absoluteRow: match.row };
        manager.requestRender?.();
      }
      term.refresh(0, term.rows - 1);
    } catch {
      // Reveal is best-effort; the match index is still advanced.
    }
  }
}

// ---------------------------------------------------------------------------
// Addon wiring
// ---------------------------------------------------------------------------

export type TerminalAddonSet = {
  readonly fit: FitAddon;
  search: SearchAddon;
};

/**
 * Unicode handling, hyperlink/URL detection, and kitty keyboard state live in
 * the Ghostty emulator itself, so only fit and search remain as addons.
 */
export function loadTerminalAddons(term: Terminal, fit: FitAddon): TerminalAddonSet {
  const search = new SearchAddon();
  term.loadAddon(fit);
  term.loadAddon(search);
  return { fit, search };
}

/**
 * The canvas renderer has no WebGL context to lose; a full repaint after
 * re-attach is all recovery means here.
 */
export function recoverTerminalRenderer(term: Terminal, _addons: TerminalAddonSet): void {
  if (term.rows > 0) term.refresh(0, term.rows - 1);
}

type SearchClearableTerminal = Pick<Terminal, "clearSelection" | "refresh" | "rows">;
type SearchClearableAddons = {
  readonly search: Pick<SearchAddon, "clearActiveDecoration" | "clearDecorations">;
};

export function clearTerminalSearch(term: SearchClearableTerminal, addons: SearchClearableAddons): void {
  addons.search.clearActiveDecoration();
  addons.search.clearDecorations();
  try {
    term.clearSelection();
  } catch {
    // Clearing can race terminal disposal.
  }
  if (term.rows > 0) term.refresh(0, term.rows - 1);
}

type SearchResettableTerminal = SearchClearableTerminal & Pick<Terminal, "loadAddon">;
type SearchResettableAddons = { search: SearchAddon };

export function resetTerminalSearchSession(
  term: SearchResettableTerminal,
  addons: SearchResettableAddons,
  createSearchAddon: () => SearchAddon = () => new SearchAddon(),
): void {
  clearTerminalSearch(term, addons);
  try {
    addons.search.dispose();
  } catch {
    // The terminal may be mid-disposal; the new search session below becomes the source of truth.
  }
  addons.search = createSearchAddon();
  term.loadAddon(addons.search);
  clearTerminalSearch(term, addons);
}

// ---------------------------------------------------------------------------
// Fit + keyboard glue
// ---------------------------------------------------------------------------

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
 * Run `claim` (the app's global keymap) before the terminal processes a
 * keydown. ghostty-web's custom handler semantics are inverted from xterm:
 * returning true means "consumed, skip terminal processing".
 */
export function attachGlobalKeyHandler(
  term: Terminal,
  claim: (event: KeyboardEvent) => TerminalKeyHandlerResult,
): void {
  term.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown") return false;
    const result = claim(event);
    if (result === "skip") {
      event.preventDefault();
      event.stopPropagation();
      return true;
    }
    if (result === "process-prevent-default") {
      event.preventDefault();
    }
    return false;
  });
}
