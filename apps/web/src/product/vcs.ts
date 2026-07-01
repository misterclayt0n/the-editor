/**
 * Version control as a PROVIDER registry: the manager consults providers in
 * registration order and the first whose `detect` answers true owns the
 * workspace. Git ships as a default provider built on the ProcessPort; a
 * jj/hg/fossil provider registers through the exact same interface from any
 * package, with scoped retraction like every registry in the editor.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { VcsProviders, type VcsProvider } from "@the-editor/vcs";

/** Host seam: the stock git provider is built host-side (it needs processes). */
export class HostVcsDefaults extends Context.Tag("the-editor/web/HostVcsDefaults")<
  HostVcsDefaults,
  { readonly providers: ReadonlyArray<VcsProvider> }
>() {}

export const vcsDefaults = definePackage("the-editor/vcs-defaults", () =>
  Effect.gen(function* () {
    const registry = yield* VcsProviders;
    const host = yield* HostVcsDefaults;
    for (const provider of host.providers) {
      yield* registry.register(provider);
    }
  }),
);
