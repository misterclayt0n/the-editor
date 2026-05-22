import { configureMonacoEditorEngine } from "@the-editor/editor-engine-monaco";
import { Effect } from "effect";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App.js";
import "./style.css";

Effect.runSync(configureMonacoEditorEngine);

const root = document.getElementById("root");
if (!root) throw new Error("Missing #root element");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
