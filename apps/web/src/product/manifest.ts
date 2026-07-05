/**
 * The default package set, as DATA. This is the manifest the boot sequence
 * activates and user config edits — the concrete form of "the-editor is just a
 * default *set* of packages." Each entry is a first-party package plus a
 * `required` mark: `required` packages are the kernel/editor foundation a user
 * may not drop (editing would break); everything else is `swappable` and a
 * user's config can `disable` it (then ship a replacement as an ordinary
 * extension). Order here is activation order.
 *
 * Not in this list (and therefore never optional): the workspace, the picker
 * host, the attention layer, and the user-extensions loader itself — those are
 * boot infrastructure, not product packages.
 */

/* eslint-disable @typescript-eslint/no-explicit-any -- heterogeneous package
 * requirements, all satisfied by the single `services` layer at activation. */
import type { ExtensionPackage } from "@the-editor/extensions";
import { editorDefaults } from "./editorDefaults";
import { lineNumbers } from "./lineNumbers";
import { filesPackage } from "./files";
import { palettePackage } from "./palette";
import { themeCommands } from "./themes";
import { workspacesPackage } from "./workspaces";
import { viewPackage } from "./view";
import { diffViewPackage } from "./diffView";
import { lspDefaults } from "./lsp";
import { lspActions } from "./lspActions";
import { lspIntel } from "./lspIntel";
import { wordCompletions } from "./wordCompletions";
import { intelActions } from "./intelActions";
import { vcsDefaults } from "./vcs";
import { fileTreePackage } from "./fileTree";
import { terminalPackage } from "./terminal";
import { documentHighlights } from "./documentHighlights";
import { selectionMatches } from "./selectionMatches";
import { vscodeKeybindings } from "./vscodeKeybindings";
import { helixLite } from "./helixLite";

export type DefaultPackageEntry = {
  readonly pkg: ExtensionPackage<any>;
  /** Foundation the user cannot disable (a `disable` naming it is ignored). */
  readonly required: boolean;
};

const required = (pkg: ExtensionPackage<any>): DefaultPackageEntry => ({ pkg, required: true });
const swappable = (pkg: ExtensionPackage<any>): DefaultPackageEntry => ({ pkg, required: false });

/** The ordered default set. `editorDefaults` is the sole required entry — it
 *  wires the core editor surface + typing commands, so dropping it breaks
 *  editing itself. Everything else is a replaceable product decision. */
export function defaultPackages(): readonly DefaultPackageEntry[] {
  return [
    required(editorDefaults),
    swappable(lineNumbers),
    swappable(filesPackage),
    swappable(palettePackage),
    swappable(themeCommands),
    swappable(workspacesPackage),
    swappable(viewPackage),
    swappable(diffViewPackage),
    swappable(lspDefaults),
    swappable(lspActions),
    swappable(lspIntel),
    swappable(wordCompletions),
    swappable(intelActions),
    swappable(vcsDefaults),
    swappable(fileTreePackage),
    swappable(terminalPackage),
    swappable(documentHighlights),
    swappable(selectionMatches),
    swappable(vscodeKeybindings),
    swappable(helixLite),
  ];
}
