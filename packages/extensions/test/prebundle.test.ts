// @vitest-environment happy-dom
import { describe, expect, it } from "@effect/vitest";
import { Effect } from "effect";
import * as esbuild from "esbuild-wasm";
import * as path from "node:path";
import { definePackage, loadExtensionPackage } from "../src/index.js";

/**
 * THE PREBUNDLING RECIPE, proven end to end (docs/EXTENSION-AUTHORING.md):
 * a React extension with hooks, DOM events, and a CSS import is bundled to
 * ONE file with `effect` + `@the-editor/*` as externals, then loaded through
 * the real extension pipeline (`loadExtensionPackage` + the closed virtual-
 * module surface) and driven in a DOM. If this passes, "bring any framework
 * by prebundling" is a fact, not a claim — React inlined, hooks working,
 * styles carried, host modules resolved through the loader's `require`.
 */
describe("prebundled framework extensions (React)", () => {
  it.effect(
    "bundles a React extension with externals and runs it through the loader",
    () =>
      Effect.gen(function* () {
        const result = yield* Effect.tryPromise({
          try: () =>
            esbuild.build({
              entryPoints: [path.join(import.meta.dirname, "fixtures/react-panel.tsx")],
              bundle: true,
              format: "cjs",
              platform: "browser",
              write: false,
              // The recipe, verbatim from the authoring guide:
              external: ["effect", "@the-editor/*", "test:*"],
              define: { "process.env.NODE_ENV": '"production"' },
              loader: { ".css": "text" },
            }),
          catch: (cause) => new Error(`bundle failed: ${String(cause)}`),
        });
        const bundled = result.outputFiles?.[0]?.text ?? "";
        expect(bundled.length).toBeGreaterThan(10_000); // React really inlined

        let reported: { host: HTMLElement; styles: string } | null = null;
        const pkg = yield* loadExtensionPackage(bundled, "acme/react-panel/index.ts", {
          // The bundle is already plain CJS JavaScript — identity "transpile",
          // same as the real loader running esbuild over JS (a no-op).
          transpiler: { transform: async (source) => source },
          virtualModules: {
            effect: { Effect },
            "@the-editor/extensions": { definePackage },
            "test:react-probe": {
              report: (payload: { host: HTMLElement; styles: string }) => (reported = payload),
            },
          },
        });

        expect(pkg.id).toBe("acme.react-proof");
        expect(reported).not.toBeNull();
        const { host, styles } = reported!;
        // The CSS import rode the bundle and reached the DOM.
        expect(styles).toContain("var(--bg-panel)");
        expect(host.querySelector("style")?.textContent).toContain(".rx-button");
        // React rendered…
        const button = host.querySelector("button.rx-button") as HTMLButtonElement;
        expect(button.textContent).toBe("count: 0");
        // …and HOOKS work: click twice, state updates through React's scheduler.
        button.click();
        button.click();
        yield* Effect.promise(() => new Promise((resolve) => setTimeout(resolve, 20)));
        expect(button.textContent).toBe("count: 2");
      }),
    15_000,
  );
});
