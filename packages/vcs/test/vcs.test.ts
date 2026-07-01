import { Effect } from "effect";
import { describe, expect, it } from "vitest";
import { makeVcsProviders, type VcsProvider } from "../src/index";

const provider: VcsProvider = {
  id: "memory",
  detect: async () => true,
  status: async () => [],
  headContent: async () => null,
  listFiles: async () => [],
  patch: async () => null,
  info: async () => ({ branch: "main" }),
};

describe("VcsProviders", () => {
  it("registers and retracts providers by scope", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const registry = yield* makeVcsProviders;
        const during = yield* Effect.scoped(
          Effect.gen(function* () {
            yield* registry.register(provider);
            return yield* registry.list;
          }),
        );
        const after = yield* registry.list;
        return { during, after };
      }),
    );

    expect(result.during).toEqual([provider]);
    expect(result.after).toEqual([]);
  });
});
