import { describe, expect, it } from "@effect/vitest";
import { Effect } from "effect";
import { definePackage, loadExtensionPackage, ExtensionLoadError } from "../src/index.js";

/**
 * A transpiler test double: strips nothing, since the fixtures below are
 * already valid JS-under-TS-syntax (no type annotations to erase). This
 * keeps the tests independent of esbuild-wasm's async init while still
 * exercising the exact same evaluate/validate path a real transpile feeds.
 */
const identityTranspiler = { transform: async (source: string) => source };

describe("loadExtensionPackage", () => {
  it.effect("evaluates a definePackage default export using virtual modules", () =>
    Effect.gen(function* () {
      const source = `
        const { definePackage } = require("@the-editor/extensions");
        module.exports.default = definePackage("acme.hello", () => {});
      `;
      const pkg = yield* loadExtensionPackage(source, "acme/hello.ts", {
        transpiler: identityTranspiler,
        virtualModules: { "@the-editor/extensions": { definePackage } },
      });
      expect(pkg.id).toEqual("acme.hello");
      expect(typeof pkg.activate).toEqual("function");
    }),
  );

  it.effect("fails with ExtensionLoadError for a disallowed import", () =>
    Effect.gen(function* () {
      const source = `require("node:fs");`;
      const result = yield* Effect.flip(
        loadExtensionPackage(source, "acme/bad-import.ts", {
          transpiler: identityTranspiler,
          virtualModules: {},
        }),
      );
      expect(result).toBeInstanceOf(ExtensionLoadError);
      expect(result.message).toContain("acme/bad-import.ts");
    }),
  );

  it.effect("fails with ExtensionLoadError when there is no default export", () =>
    Effect.gen(function* () {
      const source = `module.exports.default = { not: "a package" };`;
      const result = yield* Effect.flip(
        loadExtensionPackage(source, "acme/not-a-package.ts", {
          transpiler: identityTranspiler,
          virtualModules: {},
        }),
      );
      expect(result).toBeInstanceOf(ExtensionLoadError);
      expect(result.message).toContain("export default");
    }),
  );

  it.effect("fails with ExtensionLoadError on a syntax error", () =>
    Effect.gen(function* () {
      const source = `this is not valid javascript {{{`;
      const result = yield* Effect.flip(
        loadExtensionPackage(source, "acme/broken.ts", {
          transpiler: identityTranspiler,
          virtualModules: {},
        }),
      );
      expect(result).toBeInstanceOf(ExtensionLoadError);
    }),
  );

  it.effect("fails with ExtensionLoadError when the transpiler itself throws", () =>
    Effect.gen(function* () {
      const failingTranspiler = {
        transform: async () => {
          throw new Error("boom");
        },
      };
      const result = yield* Effect.flip(
        loadExtensionPackage("whatever", "acme/whatever.ts", {
          transpiler: failingTranspiler,
          virtualModules: {},
        }),
      );
      expect(result).toBeInstanceOf(ExtensionLoadError);
      expect(result.message).toContain("failed to compile");
    }),
  );
});

describe("createEsbuildTranspiler", () => {
  it("strips real TypeScript syntax down to runnable JavaScript", async () => {
    const { createEsbuildTranspiler } = await import("../src/loadModule.js");
    const transpiler = createEsbuildTranspiler();
    const code = await transpiler.transform(
      `
        interface Foo { x: number }
        const x: Foo = { x: 1 };
        export default function activate(pi: unknown): void {
          void pi;
          void x;
        }
      `,
      "acme/typed.ts",
    );
    expect(code).not.toContain("interface");
    expect(code).not.toContain(": Foo");
    expect(code).toContain("function");
  });

  // esbuild-wasm's initialize() may run at most once per realm. Shell.boot()
  // runs twice under React StrictMode (mount -> cleanup -> remount), so two
  // transpiler instances coexist in one realm; both MUST work. Before the
  // module-level init memoization, the second instance's own initialize()
  // threw "Cannot call initialize more than once", surfacing as the extension
  // loader's "failed to compile".
  it("lets multiple instances in one realm both transform (StrictMode/HMR)", async () => {
    const { createEsbuildTranspiler } = await import("../src/loadModule.js");
    const first = createEsbuildTranspiler();
    const second = createEsbuildTranspiler();
    const a = await first.transform("const x: number = 1; export default x;", "a.ts");
    const b = await second.transform("const y: number = 2; export default y;", "b.ts");
    expect(a).toContain("1");
    expect(b).toContain("2");
  });
});
