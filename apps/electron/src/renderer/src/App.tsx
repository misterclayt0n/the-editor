import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { RenderFrameDelta } from "@the-editor/contracts/runtime";
import "./style.css";

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [frame, setFrame] = useState<RenderFrameDelta | undefined>();
  const [mode, setMode] = useState("NORMAL");
  const [status, setStatus] = useState("booting");

  useEffect(() => {
    return window.theEditor.onPolicyEvent((event) => {
      if (!isObject(event)) return;
      if (event.type === "policy.ready") setStatus("ready");
      if (event.type === "policy.error") setStatus(String(event.message ?? "policy error"));
      if (event.type === "default.sessionChanged") {
        if (typeof event.mode === "string") setMode(event.mode.toUpperCase());
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
    if (!canvas || !frame) return;
    paintFrame(canvas, frame);
  }, [frame]);

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
        <strong>The Editor</strong>
        <span>{mode}</span>
        <button onClick={() => void resetScratch()}>Reset scratch</button>
        <span>{status}</span>
      </header>
      <section className="surfaceShell">
        <canvas ref={canvasRef} width={1000} height={500} />
      </section>
    </main>
  );
}

function paintFrame(canvas: HTMLCanvasElement, frame: RenderFrameDelta) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.fillStyle = "#111111";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.font = "14px ui-monospace, SFMono-Regular, Menlo, monospace";
  ctx.textBaseline = "top";
  ctx.fillStyle = "#d4d4d4";

  for (const row of frame.changedRows) {
    ctx.fillText(row.text, 8, row.placement.yPx + 8);
  }

  const cursors = frame.layers.cursors?.cursors ?? [];
  ctx.fillStyle = "#eeeeee";
  for (const cursor of cursors) {
    ctx.fillRect(8 + cursor.xPx, 8 + cursor.yPx, Math.max(2, cursor.widthPx), cursor.heightPx);
  }
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
