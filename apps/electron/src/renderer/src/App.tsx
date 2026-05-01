import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { RenderFrameDelta } from "@the-editor/contracts/runtime";
import { configureTextSurface, loadEditorFont, paintTextSurface } from "./textSurface.js";
import "./style.css";

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [frame, setFrame] = useState<RenderFrameDelta | undefined>();
  const [mode, setMode] = useState("NORMAL");
  const [status, setStatus] = useState("booting");
  const [activePath, setActivePath] = useState<string | undefined>();
  const [softWrap, setSoftWrap] = useState(false);
  const [showAnnotations, setShowAnnotations] = useState(true);

  useEffect(() => {
    return window.theEditor.onPolicyEvent((event) => {
      if (!isObject(event)) return;
      if (event.type === "policy.ready") setStatus("ready");
      if (event.type === "policy.error") setStatus(String(event.message ?? "policy error"));
      if (event.type === "default.sessionChanged") {
        if (typeof event.mode === "string") setMode(event.mode.toUpperCase());
        setActivePath(typeof event.activePath === "string" ? event.activePath : undefined);
        if (typeof event.softWrap === "boolean") setSoftWrap(event.softWrap);
        if (typeof event.showAnnotations === "boolean") setShowAnnotations(event.showAnnotations);
        if (isObject(event.frame)) setFrame(event.frame as unknown as RenderFrameDelta);
      }
    });
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey) return;
      const input = toKeyInput(event);
      if (!input) return;
      event.preventDefault();
      void window.theEditor.policyRequest("input.keyDown", input).catch((error) => {
        setStatus(error instanceof Error ? error.message : String(error));
      });
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let disposed = false;
    let observer: ResizeObserver | undefined;

    const configure = () => {
      const config = configureTextSurface(canvas);
      void window.theEditor.policyRequest("surface.configure", config).catch((error) => {
        setStatus(error instanceof Error ? error.message : String(error));
      });
    };

    void loadEditorFont().finally(() => {
      if (disposed) return;
      configure();
      observer = new ResizeObserver(configure);
      observer.observe(canvas.parentElement ?? canvas);
    });

    return () => {
      disposed = true;
      observer?.disconnect();
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !frame) return;
    paintTextSurface(canvas, frame, { showAnnotations });
  }, [frame, showAnnotations]);

  async function openFile() {
    setStatus("opening file");
    try {
      const path = await window.theEditor.openFileDialog();
      if (!path) {
        setStatus("ready");
        return;
      }
      await window.theEditor.policyRequest("file.open", { path });
      setStatus("ready");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function toggleSoftWrap() {
    try {
      await window.theEditor.policyRequest("view.toggleSoftWrap");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function toggleAnnotations() {
    try {
      await window.theEditor.policyRequest("view.toggleAnnotations");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function resetScratch() {
    setStatus("resetting scratch");
    try {
      await window.theEditor.policyRequest("default.createScratch", {
        text: "",
        viewport: { firstRow: 0, rowCount: 24, firstCol: 0, wrapWidth: 100 },
      });
      setStatus("ready");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <main className="app">
      <header className="toolbar">
        <div className="titleGroup">
          <strong>The Editor</strong>
          <span className="fileName" title={activePath ?? "Scratch"}>{activePath ? basename(activePath) : "Scratch"}</span>
        </div>
        <div className="toolbarActions">
          <button onClick={() => void openFile()}>Open File</button>
          <button onClick={() => void resetScratch()}>Scratch</button>
          <button className={softWrap ? "toggle active" : "toggle"} onClick={() => void toggleSoftWrap()}>
            Wrap {softWrap ? "On" : "Off"}
          </button>
          <button className={showAnnotations ? "toggle active" : "toggle"} onClick={() => void toggleAnnotations()}>
            Annotations {showAnnotations ? "On" : "Off"}
          </button>
        </div>
        <div className="statusGroup">
          <span className="modeBadge">{mode}</span>
          <span className="statusText">{status}</span>
        </div>
      </header>
      <section className="surfaceShell">
        <canvas ref={canvasRef} />
      </section>
    </main>
  );
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

function toKeyInput(event: KeyboardEvent): { key: string; text?: string } | undefined {
  const key = canonicalKey(event);
  if (!key) return undefined;
  const text = printableText(event);
  return text === undefined ? { key } : { key, text };
}

function canonicalKey(event: KeyboardEvent): string | undefined {
  const key = event.key;
  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push("C");
  if (event.altKey) modifiers.push("A");
  if (event.metaKey) modifiers.push("M");

  let atom: string | undefined;
  if (key.length === 1) {
    atom = key === " " ? "space" : key;
  } else {
    atom = namedKey(key);
    if (!atom) return undefined;
    if (event.shiftKey) modifiers.push("S");
  }

  return modifiers.length ? `${modifiers.join("-")}-${atom}` : atom;
}

function printableText(event: KeyboardEvent): string | undefined {
  if (event.ctrlKey || event.altKey || event.metaKey) return undefined;
  if (event.key === " ") return " ";
  return event.key.length === 1 ? event.key : undefined;
}

function namedKey(key: string): string | undefined {
  switch (key) {
    case "Escape":
      return "esc";
    case "Enter":
      return "ret";
    case "Backspace":
      return "backspace";
    case "Delete":
      return "del";
    case "ArrowLeft":
      return "left";
    case "ArrowRight":
      return "right";
    case "ArrowUp":
      return "up";
    case "ArrowDown":
      return "down";
    case "PageUp":
      return "pageup";
    case "PageDown":
      return "pagedown";
    case "Home":
      return "home";
    case "End":
      return "end";
    case "Tab":
      return "tab";
    default:
      return undefined;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

createRoot(document.getElementById("root")!).render(<App />);
