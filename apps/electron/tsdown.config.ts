import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    "main/main": "src/main/main.ts",
    "policy/worker": "src/policy/worker.ts",
  },
  format: "cjs",
  platform: "node",
  target: "node24",
  clean: true,
  dts: false,
  sourcemap: true,
  external: ["electron"],
});
