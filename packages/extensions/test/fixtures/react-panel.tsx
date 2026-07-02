/**
 * PROOF FIXTURE for the prebundling recipe (see docs/EXTENSION-AUTHORING.md):
 * a real React extension — hooks, events, CSS — authored as a normal multi-dep
 * project and bundled to one file with `effect` + `@the-editor/*` as externals.
 * The test bundles this with esbuild and loads the OUTPUT through
 * `loadExtensionPackage`, exactly like a user dropping the bundle into
 * `~/.the-editor/extensions/`.
 */

import { useState } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
// A CSS import, the way a shadcn/tailwind project would carry styles. Bundled
// as text (`--loader:.css=text`) and injected by the extension itself.
import styles from "./react-panel.css";

declare const require: (id: string) => { report: (payload: unknown) => void };
const probe = require("test:react-probe");

function Counter() {
  const [count, setCount] = useState(0);
  return (
    <div className="rx-panel">
      <button className="rx-button" onClick={() => setCount((n) => n + 1)}>
        count: {count}
      </button>
    </div>
  );
}

// Mount at module scope so the test can drive it without a full workbench:
// a real extension would do this inside a PanelRenderer's `mount(element)`.
const host = document.createElement("div");
const style = document.createElement("style");
style.textContent = styles;
host.append(style);
const rootEl = document.createElement("div");
host.append(rootEl);
const root = createRoot(rootEl);
flushSync(() => {
  root.render(<Counter />);
});
probe.report({ host, styles });

export default definePackage("acme.react-proof", () => Effect.void);
