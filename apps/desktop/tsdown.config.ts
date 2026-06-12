import { defineConfig } from "tsdown";

const shared = {
  format: "cjs" as const,
  outDir: "dist-electron",
  sourcemap: true,
  outExtensions: () => ({ js: ".cjs" }),
};

export default defineConfig([
  {
    ...shared,
    entry: ["src/main.ts"],
    clean: true,
  },
  {
    ...shared,
    entry: ["src/preload.ts"],
  },
  {
    ...shared,
    entry: ["src/syntax-engine.ts"],
    // Native modules must be required at runtime, never bundled. The
    // workspace syntax package IS bundled (plain TS).
    external: ["tree-sitter", "@the-editor/grammar-loader"],
    noExternal: ["@the-editor/syntax"],
  },
]);
