/**
 * Extension trust: "does this workspace's `.the-editor/extensions` get to
 * run?" A self-contained load/ask/persist unit with zero dependency on
 * PackageRuntime or reconcile — it only needs a filesystem to persist to and
 * Attention to ask through, so it's testable without spinning up Commands or
 * the kernel's package runtime at all.
 *
 * Global `~/.the-editor/extensions` is implicitly trusted (your own machine);
 * this module exists only for project-local roots, mirroring pi's
 * project_trust: ask once per root, remember the answer for the session and
 * (best-effort) to disk.
 */

import { Effect, Either } from "effect";
import { Attention, type AttentionUnhandledError } from "@the-editor/kernel";
import { FileSystemPort } from "@the-editor/editor";
import { ConfirmTrustProjectExtensions, confirmTrustExtensionsRequest } from "./attention";

export interface ExtensionTrust {
  /** Resolves the trust decision for `root`, asking (and persisting) at most
   *  once per root per process. */
  readonly ensureTrust: (root: string) => Effect.Effect<boolean, AttentionUnhandledError>;
}

const trimTrailingSlash = (path: string): string => path.replace(/\/+$/, "") || "/";

export function makeExtensionTrust(
  fileSystem: FileSystemPort.Service,
  attention: Attention.Service,
  homeDir: string | undefined,
): ExtensionTrust {
  const trustedRoots = new Set<string>();
  const deniedRoots = new Set<string>();
  let trustFileLoaded = false;

  const trustFilePath =
    homeDir !== undefined ? `${trimTrailingSlash(homeDir)}/.the-editor/trust.json` : undefined;

  const loadTrustFile = (): Effect.Effect<void> =>
    Effect.gen(function* () {
      if (trustFileLoaded || trustFilePath === undefined) return;
      trustFileLoaded = true;
      const read = yield* fileSystem.readFile(trustFilePath).pipe(Effect.either);
      if (Either.isLeft(read)) return;
      try {
        const parsed = JSON.parse(read.right) as Record<string, "trust" | "skip">;
        for (const [root, decision] of Object.entries(parsed)) {
          if (decision === "trust") trustedRoots.add(root);
          else deniedRoots.add(root);
        }
      } catch {
        // Corrupt trust file: treat as empty rather than failing extension loading.
      }
    });

  const persistTrust = (): Effect.Effect<void> =>
    Effect.gen(function* () {
      if (trustFilePath === undefined || homeDir === undefined) return;
      const payload: Record<string, "trust" | "skip"> = {};
      for (const root of trustedRoots) payload[root] = "trust";
      for (const root of deniedRoots) payload[root] = "skip";
      if (fileSystem.mkdir !== undefined) {
        yield* fileSystem.mkdir(`${trimTrailingSlash(homeDir)}/.the-editor`).pipe(Effect.ignore);
      }
      yield* fileSystem
        .writeFile(trustFilePath, JSON.stringify(payload, null, 2))
        .pipe(Effect.ignore);
    });

  const ensureTrust = (root: string): Effect.Effect<boolean, AttentionUnhandledError> =>
    Effect.gen(function* () {
      yield* loadTrustFile();
      if (trustedRoots.has(root)) return true;
      if (deniedRoots.has(root)) return false;
      const answer = yield* attention.ask(
        ConfirmTrustProjectExtensions,
        confirmTrustExtensionsRequest(root),
        { level: "warning", default: "skip" },
      );
      if (answer === "trust") trustedRoots.add(root);
      else deniedRoots.add(root);
      yield* persistTrust();
      return answer === "trust";
    });

  return { ensureTrust };
}
