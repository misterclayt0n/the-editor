import { describe, expect, it } from "@effect/vitest";
import { Effect, Exit } from "effect";
import { defineConfig, loadConfig } from "../src/index.js";

const identityTranspiler = { transform: async (source: string) => source };
const modules = { "@the-editor/extensions": { defineConfig } };

describe("loadConfig", () => {
  it.effect("evaluates a defineConfig default export with a disable list", () =>
    Effect.gen(function* () {
      const source = `
        const { defineConfig } = require("@the-editor/extensions");
        module.exports.default = defineConfig({ disable: ["the-editor/lsp-intel"] });
      `;
      const config = yield* loadConfig(source, "~/.the-editor/config.ts", {
        transpiler: identityTranspiler,
        virtualModules: modules,
      });
      expect(config.disable).toEqual(["the-editor/lsp-intel"]);
    }),
  );

  it.effect("accepts an empty config (no disable)", () =>
    Effect.gen(function* () {
      const source = `
        const { defineConfig } = require("@the-editor/extensions");
        module.exports.default = defineConfig({});
      `;
      const config = yield* loadConfig(source, "config.ts", {
        transpiler: identityTranspiler,
        virtualModules: modules,
      });
      expect(config.disable).toBeUndefined();
    }),
  );

  it.effect("rejects a default export that isn't a config object", () =>
    Effect.gen(function* () {
      const source = `module.exports.default = { disable: "not-an-array" };`;
      const exit = yield* Effect.exit(
        loadConfig(source, "bad.ts", { transpiler: identityTranspiler, virtualModules: modules }),
      );
      expect(Exit.isFailure(exit)).toBe(true);
    }),
  );
});
