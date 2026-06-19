import * as path from "node:path";
import * as fs from "node:fs";
import { createRequire } from "node:module";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, type Plugin } from "vite";

const port = Number(process.env.PORT ?? 5183);

/**
 * ghostty-web loads its VT WASM through init()'s default paths (it can't be
 * handed a URL — the Ghostty class isn't exported). The reliable path is
 * `/ghostty-vt.wasm`, so serve it from node_modules in dev and copy it into the
 * build, rather than vendoring the binary into the repo.
 */
function ghosttyWasm(): Plugin {
  const require = createRequire(import.meta.url);
  const wasmPath = require.resolve("ghostty-web/ghostty-vt.wasm");
  return {
    name: "ghostty-wasm",
    configureServer(server) {
      server.middlewares.use("/ghostty-vt.wasm", (_req, res) => {
        res.setHeader("Content-Type", "application/wasm");
        fs.createReadStream(wasmPath).pipe(res);
      });
    },
    generateBundle() {
      this.emitFile({
        type: "asset",
        fileName: "ghostty-vt.wasm",
        source: fs.readFileSync(wasmPath),
      });
    },
  };
}


export default defineConfig({
  plugins: [react(), tailwindcss(), ghosttyWasm()],
  // Relative base so the production build works when Electron loads it from file://.
  base: "./",
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "src"),
      "@the-editor/fonts": path.resolve(import.meta.dirname, "../../packages/fonts/src/index.ts"),
    },
  },
  server: {
    port,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
