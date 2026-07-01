import { Context } from "effect";

/**
 * Public host operations.
 *
 * These are small, synchronous intent surfaces for commands and extension code.
 * The host owns the privileged mechanics (native dialogs, app/session lifecycle,
 * pane creation, prompts, LSP controller state); packages own command identity,
 * keybindings, and when to invoke the operation. Methods intentionally return
 * `void`: keyboard command execution is synchronous, and long-running host work
 * continues behind the operation just like the former `Host*Actions` callbacks.
 */

export class FileOperations extends Context.Tag("@the-editor/editor/FileOperations")<
  FileOperations,
  {
    readonly save: () => void;
    readonly open: () => void;
    readonly find: () => void;
    readonly findInProject: () => void;
  }
>() {}

export class WorkspaceOperations extends Context.Tag("@the-editor/editor/WorkspaceOperations")<
  WorkspaceOperations,
  {
    readonly openFolder: () => void;
    readonly openSwitcher: () => void;
    readonly closeWorkspace: () => void;
  }
>() {}

export class ViewOperations extends Context.Tag("@the-editor/editor/ViewOperations")<
  ViewOperations,
  {
    readonly splitRight: () => void;
    readonly splitDown: () => void;
    readonly closeTab: () => void;
    readonly nextTab: () => void;
    readonly previousTab: () => void;
    readonly toggleTabs: () => void;
  }
>() {}

export class PaletteOperations extends Context.Tag("@the-editor/editor/PaletteOperations")<
  PaletteOperations,
  { readonly open: () => void }
>() {}

export class ThemeOperations extends Context.Tag("@the-editor/editor/ThemeOperations")<
  ThemeOperations,
  { readonly openSelector: () => void }
>() {}

export class LspOperations extends Context.Tag("@the-editor/editor/LspOperations")<
  LspOperations,
  {
    readonly goToDefinition: () => void;
    readonly formatDocument: () => void;
    readonly renameSymbol: () => void;
    readonly toggleInlayHints: () => void;
    readonly toggleLsp: () => void;
  }
>() {}
