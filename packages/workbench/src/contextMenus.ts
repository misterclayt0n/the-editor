/**
 * Context menus as a contribution registry rendered by the OS, not by us.
 *
 * A provider contributes entries for a named menu (`"terminal"`, `"editor"`,
 * `"file-tree"`, …); at popup time the host collects every provider's entries
 * for the request and asks the platform to draw the menu — the real OS context
 * menu on desktop (Electron `Menu.popup`), a minimal plain-DOM fallback on the
 * web. No component library owns the menu; the OS does.
 *
 * `ContextMenuProviders` is the extension-facing registry (register entries).
 * `ContextMenus` is the host operation that actually pops a menu — it reads the
 * registry, so a caller only supplies the request + screen position.
 */

import { Context, Effect, type Scope } from "effect";

export type ContextMenuItem = {
  readonly id: string;
  readonly label: string;
  readonly enabled?: boolean;
  /** Display-only shortcut hint (e.g. "⌘C"); the OS renders it right-aligned. */
  readonly accelerator?: string;
  readonly run: () => void;
};

export type ContextMenuSeparator = { readonly separator: true };

export type ContextMenuEntry = ContextMenuItem | ContextMenuSeparator;

export const isMenuSeparator = (entry: ContextMenuEntry): entry is ContextMenuSeparator =>
  (entry as ContextMenuSeparator).separator === true;

/** What a contextmenu event resolves to: a named menu + optional target data
 *  (the specific terminal instance's actions, a file path, …). */
export type ContextMenuRequest = {
  readonly menu: string;
  readonly data?: unknown;
};

export type ContextMenuProvider = {
  readonly id: string;
  /** Return entries for this request, or [] to sit out (wrong menu, no target). */
  readonly provide: (request: ContextMenuRequest) => readonly ContextMenuEntry[];
};

export class ContextMenuProviders extends Context.Tag("@the-editor/workbench/ContextMenuProviders")<
  ContextMenuProviders,
  ContextMenuProviders.Service
>() {}

export const makeContextMenuProviders = (): ContextMenuProviders.Service => {
  const providers = new Set<ContextMenuProvider>();
  return {
    register: (provider) =>
      Effect.acquireRelease(
        Effect.sync(() => void providers.add(provider)),
        () => Effect.sync(() => void providers.delete(provider)),
      ).pipe(Effect.asVoid),
    /** Every registered provider's entries for the request, groups joined by a
     *  separator (empty groups skipped). Registration order. */
    entriesFor: (request) => {
      const groups: ContextMenuEntry[][] = [];
      for (const provider of providers) {
        const entries = provider.provide(request);
        if (entries.length > 0) groups.push([...entries]);
      }
      const out: ContextMenuEntry[] = [];
      groups.forEach((group, index) => {
        if (index > 0) out.push({ separator: true });
        out.push(...group);
      });
      return out;
    },
  };
};

export namespace ContextMenuProviders {
  export interface Service {
    readonly register: (
      provider: ContextMenuProvider,
    ) => Effect.Effect<void, never, Scope.Scope>;
    readonly entriesFor: (request: ContextMenuRequest) => readonly ContextMenuEntry[];
  }
}

/** Screen coordinates for the popup (a contextmenu event's clientX/clientY). */
export type MenuPosition = { readonly x: number; readonly y: number };

/**
 * Host operation: pop a context menu. Collects entries from
 * `ContextMenuProviders` for the request and hands them to the platform menu.
 * Host-implemented (it reaches the OS bridge); a package/extension calls it
 * from a `contextmenu` handler.
 */
export class ContextMenus extends Context.Tag("@the-editor/workbench/ContextMenus")<
  ContextMenus,
  {
    readonly open: (request: ContextMenuRequest, position: MenuPosition) => void;
  }
>() {}

export namespace ContextMenus {
  export type Service = Context.Tag.Service<ContextMenus>;
}
