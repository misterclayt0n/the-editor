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
    entry: ["src/main/index.ts"],
    clean: true,
    noExternal: (id) => id.startsWith("@the-editor/"),
  },
  {
    ...shared,
    entry: ["src/preload/index.ts"],
  },
]);
