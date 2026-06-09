import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const port = Number(process.env.PORT ?? 5183);

export default defineConfig({
  plugins: [react()],
  // Relative base so the production build works when Electron loads it from file://.
  base: "./",
  server: {
    port,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
