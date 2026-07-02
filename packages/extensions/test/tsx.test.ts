// @vitest-environment happy-dom
import { describe, expect, it } from "@effect/vitest";
import { Effect } from "effect";
import {
  createEsbuildTranspiler,
  definePackage,
  Fragment,
  h,
  loadExtensionPackage,
} from "../src/index.js";

/**
 * The full `.tsx` authoring path, end to end with the REAL transpiler:
 * JSX compiles to the native `h` builder (jsxFactory), evaluates against the
 * virtual-module surface, and produces actual DOM — no React anywhere.
 */
describe("tsx extensions (JSX → native DOM)", () => {
  it.effect("compiles and evaluates a .tsx extension whose JSX builds real DOM", () =>
    Effect.gen(function* () {
      const source = `
        import { definePackage, h, Fragment } from "@the-editor/extensions";
        import { report } from "test:probe";

        const Panel = (props: { title: string; children?: unknown }) => (
          <div class="acme-panel" style={{ display: "flex", gap: "6px" }}>
            <strong>{props.title}</strong>
            {props.children}
          </div>
        );

        report(
          (
            <Panel title="hi">
              <>
                <button onclick={() => {}}>run</button>
                <span data-x="1">tail</span>
              </>
            </Panel>
          ) as HTMLElement,
        );

        export default definePackage("acme.tsx-demo", () => {
          return undefined as never;
        });
      `;

      let reported: HTMLElement | null = null;
      const pkg = yield* loadExtensionPackage(source, "acme/panel.tsx", {
        transpiler: createEsbuildTranspiler(),
        virtualModules: {
          "@the-editor/extensions": { definePackage, h, Fragment },
          "test:probe": { report: (el: HTMLElement) => (reported = el) },
        },
      });

      expect(pkg.id).toBe("acme.tsx-demo");
      expect(reported).not.toBeNull();
      const el = reported! as HTMLElement;
      expect(el.className).toBe("acme-panel");
      expect(el.style.display).toBe("flex");
      expect(el.querySelector("strong")?.textContent).toBe("hi");
      expect(el.querySelector("button")?.textContent).toBe("run");
      expect(el.querySelector("span")?.dataset.x).toBe("1");
    }),
  );
});
