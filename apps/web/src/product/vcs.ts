/**
 * Version control as a PROVIDER registry: the manager consults providers in
 * registration order and the first whose `detect` answers true owns the
 * workspace. Git ships as a default provider built on the ProcessPort; a
 * jj/hg/fossil provider registers through the exact same interface from any
 * package, with scoped retraction like every registry in the editor.
 */

import { Context, Effect, Ref, Scope } from "effect";
import { definePackage } from "@the-editor/extensions";

/** Matches @pierre/trees git-status vocabulary so badges map 1:1. */
export type VcsFileStatus =
  | "added"
  | "deleted"
  | "ignored"
  | "modified"
  | "renamed"
  | "untracked";

export type VcsStatusEntry = {
  /** Workspace-root-relative path. */
  readonly path: string;
  readonly status: VcsFileStatus;
};

export type VcsInfo = { readonly branch: string };

export interface VcsProvider {
  readonly id: string;
  detect(root: string): Promise<boolean>;
  /** Working-tree status for the whole repo. */
  status(root: string): Promise<ReadonlyArray<VcsStatusEntry>>;
  /** File content at the comparison baseline (HEAD); null when absent. */
  headContent(root: string, relPath: string): Promise<string | null>;
  /** Tracked + untracked-but-not-ignored files; null = not supported. */
  listFiles(root: string): Promise<ReadonlyArray<string> | null>;
  info(root: string): Promise<VcsInfo | null>;
}

export class VcsProviders extends Context.Tag("the-editor/web/VcsProviders")<
  VcsProviders,
  {
    readonly register: (provider: VcsProvider) => Effect.Effect<void, never, Scope.Scope>;
    readonly list: Effect.Effect<ReadonlyArray<VcsProvider>>;
  }
>() {}

export const makeVcsProviders = Effect.gen(function* () {
  const providers = yield* Ref.make<ReadonlyArray<VcsProvider>>([]);
  return {
    register: (provider: VcsProvider) =>
      Effect.acquireRelease(
        Ref.update(providers, (list) => [...list, provider]),
        () => Ref.update(providers, (list) => list.filter((entry) => entry !== provider)),
      ).pipe(Effect.asVoid),
    list: Ref.get(providers),
  } satisfies VcsProviders.Service;
});

export namespace VcsProviders {
  export type Service = Context.Tag.Service<VcsProviders>;
}

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
