import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { RenderFrame } from "@the-editor/contracts/runtime";
import type { BufferDecoration, ThemeSnapshot } from "@the-editor/editor";
import { configureTextSurface, loadEditorFont, paintTextSurface } from "./textSurface.js";
import { RendererPackageRuntime, type WorkbenchSnapshot } from "./uiRuntime.js";
import "./style.css";

const uiRuntime = new RendererPackageRuntime();

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [frame, setFrame] = useState<RenderFrame | undefined>();
  const [decorations, setDecorations] = useState<BufferDecoration[]>([]);
  const [theme, setTheme] = useState<ThemeSnapshot | undefined>();
  const [syntaxLegend, setSyntaxLegend] = useState<Record<number, string>>({});
  const [selectionPickerActive, setSelectionPickerActive] = useState(false);
  const [, setUiRevision] = useState(0);

  useEffect(() => uiRuntime.subscribe(() => {
    setUiRevision((revision) => revision + 1);
  }), []);

  useEffect(() => {
    return window.theEditor.onPolicyEvent((event) => {
      if (!isObject(event)) return;
      if (event.type === "policy.error") console.error(String(event.message ?? "policy error"));
      if (event.type === "default.sessionChanged") {
        if (isObject(event.frame)) setFrame(event.frame as unknown as RenderFrame);
        setDecorations(Array.isArray(event.decorations) ? event.decorations as BufferDecoration[] : []);
        const snapshot: WorkbenchSnapshot = {
          roles: (isObject(event.roles) ? event.roles : {}) as WorkbenchSnapshot["roles"],
          packages: (Array.isArray(event.packages) ? event.packages : []) as WorkbenchSnapshot["packages"],
          diagnostics: (Array.isArray(event.diagnostics) ? event.diagnostics : []) as WorkbenchSnapshot["diagnostics"],
          interaction: isObject(event.interaction) ? event.interaction as unknown as WorkbenchSnapshot["interaction"] : undefined,
          tools: (Array.isArray(event.tools) ? event.tools : []) as WorkbenchSnapshot["tools"],
          decorations: (Array.isArray(event.decorations) ? event.decorations : []) as WorkbenchSnapshot["decorations"],
          focus: isObject(event.focus) ? event.focus as WorkbenchSnapshot["focus"] : undefined,
          theme: isObject(event.theme) ? event.theme as unknown as ThemeSnapshot : undefined,
          syntaxLegend: isObject(event.syntaxLegend) ? event.syntaxLegend as Record<number, string> : {},
        };
        setTheme(snapshot.theme);
        setSyntaxLegend(snapshot.syntaxLegend ?? {});
        setSelectionPickerActive(Boolean(event.selectionPickerActive));
        void uiRuntime.update(snapshot).then(() => {
          setUiRevision((revision) => revision + 1);
        }).catch((error) => {
          console.error(error);
        });
      }
    });
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (isEditableTarget(event.target)) return;
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
    paintTextSurface(canvas, frame, decorations, theme, syntaxLegend, selectionPickerActive);
  }, [frame, decorations, theme, syntaxLegend, selectionPickerActive]);

  const leftSidebar = uiRuntime.renderToolRoot("leftSidebar");
  const rightSidebar = uiRuntime.renderToolRoot("rightSidebar");
  const bottomPanel = uiRuntime.renderToolRoot("bottomPanel");
  const editorAdjacent = uiRuntime.renderToolRoot("editorAdjacent");
  const overlayModal = uiRuntime.renderToolRoot("overlayModal");
  const overlayFloating = uiRuntime.renderToolRoot("overlayFloating");

  return (
    <main className="workbench">
      <section className="workbenchMainRow">
        {leftSidebar ? <aside className="workbenchSidebar left">{leftSidebar}</aside> : null}
        <section className="workbenchCenterColumn">
          <div className="workbenchEditorArea">
            <canvas ref={canvasRef} />
            {editorAdjacent ? <div className="workbenchEditorAdjacent">{editorAdjacent}</div> : null}
          </div>
          {bottomPanel ? <section className="workbenchBottomPanel">{bottomPanel}</section> : null}
          <footer className="workbenchStatuslineRoot">{uiRuntime.renderRole("statusline")}</footer>
        </section>
        {rightSidebar ? <aside className="workbenchSidebar right">{rightSidebar}</aside> : null}
      </section>

      <div className="workbenchOverlayRoot">
        {overlayModal}
        {overlayFloating}
        {uiRuntime.renderRole("inputPrompt")}
        {uiRuntime.renderRole("commandPalette")}
      </div>
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

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLInputElement) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  return target.isContentEditable;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

createRoot(document.getElementById("root")!).render(<App />);
