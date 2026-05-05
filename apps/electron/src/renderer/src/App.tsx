import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { RenderFrame } from "@the-editor/contracts/runtime";
import { configureTextSurface, loadEditorFont, paintTextSurface } from "./textSurface.js";
import "./style.css";

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [frame, setFrame] = useState<RenderFrame | undefined>();

  useEffect(() => {
    return window.theEditor.onPolicyEvent((event) => {
      if (!isObject(event)) return;
      if (event.type === "policy.error") console.error(String(event.message ?? "policy error"));
      if (event.type === "default.sessionChanged" && isObject(event.frame)) {
        setFrame(event.frame as unknown as RenderFrame);
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
        console.error(error);
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
        console.error(error);
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
    paintTextSurface(canvas, frame);
  }, [frame]);

  return (
    <main className="app">
      <canvas ref={canvasRef} />
    </main>
  );
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
