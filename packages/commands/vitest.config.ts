import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  resolve: {
    alias: {
      "@the-editor/extensions": `${root}/packages/extensions/src/index.ts`,
      "@the-editor/kernel": `${root}/packages/kernel/src/index.ts`,
    },
  },
});
