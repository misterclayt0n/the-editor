import { defineConfig } from "vitest/config";

// vscode-jsonrpc/-languageserver-protocol expose "./browser" only under the
// browser condition; their browser runtime layer is plain TextEncoder/timers
// and runs fine under node. Inline them so vite (not node) resolves.
export default defineConfig({
  resolve: { conditions: ["browser", "import", "default"] },
  ssr: {
    resolve: {
      conditions: ["browser", "import", "default"],
      externalConditions: ["browser", "import", "default"],
    },
  },
  test: {
    server: {
      deps: { inline: [/vscode-jsonrpc/, /vscode-languageserver-protocol/, /vscode-languageserver-types/] },
    },
  },
});
