import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { RenderFrameDelta } from "@the-editor/contracts/runtime";
import "./style.css";

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [frame, setFrame] = useState<RenderFrameDelta | undefined>();
  const [status, setStatus] = useState("booting");

  useEffect(() => {
    return window.theEditor.onPolicyEvent((event) => {
      if (!isObject(event)) return;
      if (event.type === "policy.ready") setStatus("policy ready");
      if (event.type === "policy.error") setStatus(String(event.message ?? "policy error"));
      if (event.type === "default.activeSurfaceChanged" && isObject(event.frame)) {
        setFrame(event.frame as unknown as RenderFrameDelta);
        setStatus("surface rendered");
      }
    });
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !frame) return;
    paintFrame(canvas, frame);
  }, [frame]);

  async function createScratch() {
    setStatus("creating scratch");
    try {
      const result = await window.theEditor.policyRequest("default.createScratch", {
        text: "hello from electron",
        viewport: { firstRow: 0, rowCount: 24, firstCol: 0, wrapWidth: 100 },
      });
      if (isObject(result) && isObject(result.frame)) {
        setFrame(result.frame as unknown as RenderFrameDelta);
      }
      setStatus("surface rendered");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <main className="app">
      <header className="toolbar">
        <strong>The Editor</strong>
        <button onClick={() => void createScratch()}>Create scratch</button>
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

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

createRoot(document.getElementById("root")!).render(<App />);
