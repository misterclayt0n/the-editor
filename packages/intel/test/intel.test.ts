import { Effect } from "effect";
import { describe, expect, it } from "vitest";
import {
  makeCompletionProviders,
  makeLanguageServers,
  type CompletionProvider,
  type LanguageServerDefinition,
} from "../src/index";

describe("intel registries", () => {
  it("registers and retracts completion providers by scope", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const registry = yield* makeCompletionProviders;
        const provider: CompletionProvider = {
          id: "test",
          provide: () => ({ items: [] }),
        };
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

    expect(result.during.map((provider) => provider.id)).toEqual(["test"]);
    expect(result.after).toEqual([]);
  });

  it("publishes language-server definitions through the same scoped shape", async () => {
    const definition: LanguageServerDefinition = {
      id: "toy",
      command: "toy-lsp",
      languages: [{ languageId: "toy", fileTypes: ["toy"] }],
    };

    const seen = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const registry = yield* makeLanguageServers;
          yield* registry.register(definition);
          return yield* registry.list;
        }),
      ),
    );

    expect(seen).toEqual([definition]);
  });
});
