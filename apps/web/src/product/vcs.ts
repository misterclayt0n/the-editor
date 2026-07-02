/**
 * Version control as a PROVIDER registry: the manager consults providers in
 * registration order and the first whose `detect` answers true owns the
 * workspace. Git ships as a default provider built on the public `Processes`
 * seam — no host tag, so this package is pure package code on allowlisted
 * seams (`@the-editor/vcs` + `@the-editor/process`); a jj/hg/fossil provider
 * registers through the exact same interface from any package or user
 * extension, with scoped retraction like every registry in the editor.
 *
 * On a host with no process capability (plain web) `Processes` is still in the
 * layer; the git provider registers, but its `detect` runs `processes.run`,
 * fails `ProcessUnavailableError`, and reports "not a repo" — so git is
 * present-but-inert rather than a load-time failure.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Processes } from "@the-editor/process";
import { VcsProviders } from "@the-editor/vcs";
import { createGitProvider } from "../vcs/git";

export const vcsDefaults = definePackage("the-editor/vcs-defaults", () =>
  Effect.gen(function* () {
    const registry = yield* VcsProviders;
    const processes = yield* Processes;
    yield* registry.register(createGitProvider(processes));
  }),
);
