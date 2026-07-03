/**
 * Terminal engine seam. Two engines implement the same host-facing surface:
 *
 * - `backend-xterm`: xterm.js + addons (WebGL renderer, ligatures, decorated
 *   search). The default.
 * - `backend-ghostty`: ghostty-web — Ghostty's VT emulator compiled to WASM
 *   (better grapheme handling, native kitty keyboard state, DSR/DA answers).
 *
 * The user picks via `~/.the-editor/config.ts`:
 *
 *   export default defineConfig({ terminal: { engine: "ghostty" } });
 *
 * Engines load lazily through dynamic import so the unused one never enters
 * the boot bundle; the choice is read when a terminal panel mounts.
 */

import type { TerminalKeyHandlerResult, TerminalOptions } from "./keys";

export type TerminalEngineKind = "xterm" | "ghostty";

export const DEFAULT_TERMINAL_ENGINE: TerminalEngineKind = "xterm";

export const isTerminalEngineKind = (value: unknown): value is TerminalEngineKind =>
  value === "xterm" || value === "ghostty";

type Disposable = { dispose(): void };

/** The terminal surface the host (mount.ts) programs against. Both engines
 * satisfy it structurally; the adapters below cast at the seam. */
export interface TerminalHandle {
  readonly cols: number;
  readonly rows: number;
  /** Mutable runtime options (theme/font); engines apply them live. */
  readonly options: {
    theme?: Readonly<Record<string, string>>;
    fontFamily?: string;
    fontSize?: number;
  };
  readonly modes: { readonly bracketedPasteMode: boolean };
  readonly parser: {
    registerOscHandler(code: number, handler: (data: string) => boolean): Disposable;
  };
  open(parent: HTMLElement): void;
  write(data: string): void;
  dispose(): void;
  focus(): void;
  clear(): void;
  refresh(start: number, end: number): void;
  select(column: number, row: number, length: number): void;
  clearSelection(): void;
  selectAll(): void;
  getSelection(): string;
  hasSelection(): boolean;
  onData(listener: (data: string) => void): Disposable;
  onResize(listener: (size: { cols: number; rows: number }) => void): Disposable;
  onTitleChange(listener: (title: string) => void): Disposable;
  onBell(listener: () => void): Disposable;
  onSelectionChange(listener: () => void): Disposable;
  /** ghostty-only: re-measure canvas metrics after a late webfont load. */
  remeasureFont?(): void;
}

export interface TerminalFitHandle {
  fit(): void;
}

export interface TerminalSearchHandle {
  findNext(query: string, options?: { incremental?: boolean; decorations?: unknown }): unknown;
  findPrevious(query: string, options?: { decorations?: unknown }): unknown;
  clearDecorations(): void;
  clearActiveDecoration(): void;
}

export interface TerminalAddonSetHandle {
  readonly search: TerminalSearchHandle;
}

export interface TerminalEngine {
  readonly kind: TerminalEngineKind;
  /**
   * Whether assigning `options.theme` recolors the live terminal. The WASM
   * emulator resolves cell colors against its creation-time palette, so the
   * ghostty engine needs the host to rebuild the terminal instead.
   */
  readonly liveThemeUpdates: boolean;
  createTerminal(options: TerminalOptions): TerminalHandle;
  createFitAddon(): TerminalFitHandle;
  loadTerminalAddons(term: TerminalHandle, fit: TerminalFitHandle): TerminalAddonSetHandle;
  recoverTerminalRenderer(term: TerminalHandle, addons: TerminalAddonSetHandle): void;
  attachAutoFit(fit: TerminalFitHandle, container: HTMLElement): () => void;
  attachGlobalKeyHandler(
    term: TerminalHandle,
    claim: (event: KeyboardEvent) => TerminalKeyHandlerResult,
  ): void;
  unsupportedQueryResponses(data: string, theme: Readonly<Record<string, string>>): string;
  hideNativeInputCaret(term: TerminalHandle): void;
  clearTerminalSearch(term: TerminalHandle, addons: TerminalAddonSetHandle): void;
  resetTerminalSearchSession(term: TerminalHandle, addons: TerminalAddonSetHandle): void;
}

/* eslint-disable @typescript-eslint/no-explicit-any */
// The adapters cast between the shared handle types and each engine's
// concrete classes exactly once, here. mount.ts stays engine-agnostic.

const xtermEngine = async (): Promise<TerminalEngine> => {
  const mod = await import("./backend-xterm");
  await mod.ensureReady();
  return {
    kind: "xterm",
    liveThemeUpdates: true,
    createTerminal: (options) => new mod.Terminal(options as any) as unknown as TerminalHandle,
    createFitAddon: () => new mod.FitAddon() as unknown as TerminalFitHandle,
    loadTerminalAddons: (term, fit) =>
      mod.loadTerminalAddons(term as any, fit as any) as unknown as TerminalAddonSetHandle,
    recoverTerminalRenderer: (term, addons) => mod.recoverTerminalRenderer(term as any, addons as any),
    attachAutoFit: (fit, container) => mod.attachAutoFit(fit as any, container),
    attachGlobalKeyHandler: (term, claim) => mod.attachGlobalKeyHandler(term as any, claim),
    unsupportedQueryResponses: mod.unsupportedQueryResponses,
    hideNativeInputCaret: (term) => mod.hideNativeInputCaret(term as any),
    clearTerminalSearch: (term, addons) => mod.clearTerminalSearch(term as any, addons as any),
    resetTerminalSearchSession: (term, addons) =>
      mod.resetTerminalSearchSession(term as any, addons as any),
  };
};

const ghosttyEngine = async (): Promise<TerminalEngine> => {
  const mod = await import("./backend-ghostty");
  await mod.ensureReady();
  return {
    kind: "ghostty",
    liveThemeUpdates: false,
    createTerminal: (options) => new mod.Terminal(options as any) as unknown as TerminalHandle,
    createFitAddon: () => new mod.FitAddon() as unknown as TerminalFitHandle,
    loadTerminalAddons: (term, fit) =>
      mod.loadTerminalAddons(term as any, fit as any) as unknown as TerminalAddonSetHandle,
    recoverTerminalRenderer: (term, addons) => mod.recoverTerminalRenderer(term as any, addons as any),
    attachAutoFit: (fit, container) => mod.attachAutoFit(fit as any, container),
    attachGlobalKeyHandler: (term, claim) => mod.attachGlobalKeyHandler(term as any, claim),
    unsupportedQueryResponses: mod.unsupportedQueryResponses,
    hideNativeInputCaret: (term) => mod.hideNativeInputCaret(term as any),
    clearTerminalSearch: (term, addons) => mod.clearTerminalSearch(term as any, addons as any),
    resetTerminalSearchSession: (term, addons) =>
      mod.resetTerminalSearchSession(term as any, addons as any),
  };
};

/* eslint-enable @typescript-eslint/no-explicit-any */

const enginePromises = new Map<TerminalEngineKind, Promise<TerminalEngine>>();

/** Load (and memoize) a terminal engine; safe to call per terminal mount. */
export function loadTerminalEngine(kind: TerminalEngineKind): Promise<TerminalEngine> {
  let promise = enginePromises.get(kind);
  if (promise === undefined) {
    promise = kind === "ghostty" ? ghosttyEngine() : xtermEngine();
    // A failed load (e.g. WASM fetch) should not poison later attempts.
    promise.catch(() => enginePromises.delete(kind));
    enginePromises.set(kind, promise);
  }
  return promise;
}
