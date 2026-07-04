import { describe, expect, it } from "@effect/vitest";
import { Context, Effect, Layer, Scope } from "effect";
import { definePackage, toPackageDescriptor } from "@the-editor/extensions";
import { makePackageRuntime, PackageId } from "@the-editor/kernel";
import { InputProfiles, ViewInput, makeInputProfiles } from "../src/index";

const setup = Effect.gen(function* () {
  const runtime = yield* makePackageRuntime;
  const input = yield* makeInputProfiles(runtime.contributionIndex);
  return { runtime, input, layer: Layer.succeed(InputProfiles, input) };
});

describe("InputProfiles", () => {
  it.scoped("views use default until the requested profile is registered, then switch", () =>
    Effect.gen(function* () {
      const { runtime, input, layer } = yield* setup;
      yield* input.setActiveProfile("modal");
      const viewScope = yield* Scope.make();
      const view = yield* input.attachView({ id: "view-1", context: Context.empty(), scope: viewScope });
      const viewInput = Context.get(view.context as Context.Context<ViewInput>, ViewInput);
      expect((yield* viewInput.context).profile).toBe("default");

      const pkg = definePackage("modal-profile", () =>
        Effect.flatMap(InputProfiles, (profiles) =>
          profiles.register({
            id: "modal",
            title: "Modal",
            initialContext: () => ({ profile: "modal", mode: "normal", textFallback: "none" }),
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);
      expect(yield* viewInput.context).toMatchObject({ profile: "modal", mode: "normal" });
    }),
  );

  it.scoped("profile retraction resets live views and clears continuations", () =>
    Effect.gen(function* () {
      const { runtime, input, layer } = yield* setup;
      const pkg = definePackage("modal-profile", () =>
        Effect.flatMap(InputProfiles, (profiles) =>
          profiles.register({
            id: "modal",
            title: "Modal",
            initialContext: () => ({ profile: "modal", mode: "normal", textFallback: "none" }),
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);
      yield* input.setActiveProfile("modal");

      const viewScope = yield* Scope.make();
      const view = yield* input.attachView({ id: "view-1", context: Context.empty(), scope: viewScope });
      const viewInput = Context.get(view.context as Context.Context<ViewInput>, ViewInput);
      yield* viewInput.pushContinuation({
        owner: "modal",
        label: "await key",
        onKey: () => ({ _tag: "handled" }),
      });
      expect((yield* viewInput.peekContinuation)?.label).toBe("await key");

      yield* runtime.disposePackage(PackageId("modal-profile"));
      expect(yield* viewInput.context).toMatchObject({ profile: "default", mode: "insert" });
      expect(yield* viewInput.peekContinuation).toBeNull();
      expect(yield* input.activeProfile).toBe("default");
    }),
  );
});
