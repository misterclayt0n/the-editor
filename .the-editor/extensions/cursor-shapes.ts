import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import { ViewCursor, type CursorStyle } from "@the-editor/editor";

const EditorSurface = SurfaceKind("the-editor/editor");
const editorScope = SurfaceScope.surface(EditorSurface);

const STYLE_ID = "the-editor-cursor-shapes-extension";

function installStyles(): HTMLStyleElement {
  const existing = document.getElementById(STYLE_ID);
  if (existing instanceof HTMLStyleElement) existing.remove();

  const style = document.createElement("style");
  style.id = STYLE_ID;
  style.textContent = `
    .cursor-shapes-smooth-block {
      transition:
        left 90ms cubic-bezier(.2, .8, .2, 1),
        top 90ms cubic-bezier(.2, .8, .2, 1),
        width 90ms cubic-bezier(.2, .8, .2, 1),
        height 90ms cubic-bezier(.2, .8, .2, 1),
        opacity 90ms ease-out;
      will-change: left, top, width, height, opacity;
    }

    .cursor-shapes-smooth-block .cursor-shapes-core {
      width: 100%;
      height: 100%;
      border-radius: 4px;
      background: color-mix(in srgb, var(--caret) 84%, var(--bg-editor));
      box-shadow:
        0 0 0 1px color-mix(in srgb, var(--caret) 42%, transparent),
        0 0 18px color-mix(in srgb, var(--caret) 28%, transparent);
      animation: cursor-shapes-breathe 920ms ease-in-out infinite alternate;
    }

    .cursor-shapes-smooth-block .cursor-shapes-glint {
      position: absolute;
      inset: 1px auto auto 1px;
      width: 45%;
      height: 35%;
      border-radius: 999px;
      background: color-mix(in srgb, white 42%, transparent);
      filter: blur(.5px);
      opacity: .5;
      pointer-events: none;
    }

    .cursor-shapes-pulse {
      inset: -10px;
      border-radius: 999px;
      border: 1px solid color-mix(in srgb, var(--caret) 70%, transparent);
      animation: cursor-shapes-pulse 240ms ease-out forwards;
    }

    @keyframes cursor-shapes-breathe {
      from { transform: scale(.94); opacity: .82; }
      to { transform: scale(1); opacity: 1; }
    }

    @keyframes cursor-shapes-pulse {
      from { opacity: .7; transform: scale(.45); }
      to { opacity: 0; transform: scale(1.85); }
    }
  `;
  document.head.append(style);
  return style;
}

const setCursor = (style: CursorStyle) =>
  Effect.gen(function* () {
    const cursor = yield* ViewCursor;
    cursor.setStyle(style);
    cursor.spawnEffect({ className: "cursor-shapes-pulse", ttlMs: 260 });
  });

export default definePackage("local.cursor-shapes", () =>
  Effect.gen(function* () {
    const style = yield* Effect.acquireRelease(
      Effect.sync(installStyles),
      (el) => Effect.sync(() => el.remove()),
    );
    void style;

    const commands = yield* Commands;

    yield* commands.register("cursor.shape.bar", {
      title: "Cursor Shape: Bar",
      scope: editorScope,
      palette: { group: "Cursor", aliases: ["cursor bar", "caret bar"] },
      run: () => setCursor({ shape: "bar", blink: true }),
    });

    yield* commands.register("cursor.shape.block", {
      title: "Cursor Shape: Block",
      scope: editorScope,
      palette: { group: "Cursor", aliases: ["cursor block", "caret block"] },
      run: () => setCursor({ shape: "block", blink: false }),
    });

    yield* commands.register("cursor.shape.underline", {
      title: "Cursor Shape: Underline",
      scope: editorScope,
      palette: { group: "Cursor", aliases: ["cursor underline", "caret underline"] },
      run: () => setCursor({ shape: "underline", blink: true, widthPx: 3 }),
    });

    yield* commands.register("cursor.shape.hidden", {
      title: "Cursor Shape: Hidden",
      scope: editorScope,
      palette: { group: "Cursor", aliases: ["cursor hide", "caret hide"] },
      run: () => setCursor({ shape: "hidden", blink: false }),
    });

    yield* commands.register("cursor.shape.smoothBlock", {
      title: "Cursor Shape: Smooth Animated Block",
      scope: editorScope,
      palette: {
        group: "Cursor",
        aliases: ["cursor smooth block", "cursor animated block", "caret smooth block"],
      },
      run: () =>
        setCursor({
          shape: "block",
          blink: false,
          className: "cursor-shapes-smooth-block",
          widthColumns: 1,
          render: (element: HTMLElement) => {
            element.replaceChildren();
            const core = document.createElement("div");
            core.className = "cursor-shapes-core";
            const glint = document.createElement("div");
            glint.className = "cursor-shapes-glint";
            core.append(glint);
            element.append(core);
          },
        }),
    });
  }),
);
