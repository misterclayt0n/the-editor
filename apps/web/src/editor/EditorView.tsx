import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { hitTestTextPosition, layoutViewport, type EditorSnapshot } from "@the-editor/editor";
import { Selection } from "@the-editor/engine";
import type { Chord } from "@the-editor/keymaps";
import type { AppRuntime } from "../runtime";

const LINE_HEIGHT = 20;
const TAB_WIDTH = 4;
const PADDING_LEFT = 12;
const PADDING_TOP = 8;
const OVERSCAN_LINES = 4;

type EditorViewProps = {
  readonly runtime: AppRuntime;
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
  readonly onPending?: (chords: ReadonlyArray<Chord>) => void;
};

export function EditorView({ runtime, onSnapshot, onPending }: EditorViewProps) {
  const [snapshot, setSnapshot] = useState<EditorSnapshot>(() => runtime.snapshot());
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [charWidth, setCharWidth] = useState(8);
  const [focused, setFocused] = useState(false);

  const scrollerRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLSpanElement>(null);

  useLayoutEffect(() => {
    const measure = measureRef.current;
    if (measure) setCharWidth(measure.getBoundingClientRect().width / 100);
  }, []);

  useLayoutEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const observer = new ResizeObserver(() => setViewportHeight(scroller.clientHeight));
    observer.observe(scroller);
    setViewportHeight(scroller.clientHeight);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    onSnapshot?.(snapshot);
  }, [snapshot, onSnapshot]);

  const refresh = useCallback(() => {
    const next = runtime.snapshot();
    setSnapshot(next);

    const scroller = scrollerRef.current;
    if (!scroller) return;
    const caretLine = next.text.lineRangeAt(next.selection.primary.head).index;
    const caretTop = PADDING_TOP + caretLine * LINE_HEIGHT;
    if (caretTop < scroller.scrollTop) {
      scroller.scrollTop = caretTop;
    } else if (caretTop + LINE_HEIGHT > scroller.scrollTop + scroller.clientHeight) {
      scroller.scrollTop = caretTop + LINE_HEIGHT - scroller.clientHeight;
    }
  }, [runtime]);

  // The renderer is a notification consumer: commands mutate the document and
  // publish invalidation batches; each batch triggers one snapshot re-read.
  // Payloads are hints only, so dropped batches (sliding bus) are harmless —
  // every refresh reads the full current truth.
  useEffect(() => runtime.subscribe(() => refresh()), [runtime, refresh]);

  // The whole input path: chord -> keymap resolution -> command execution.
  // The switch statement this replaces lives on as keybindings in the
  // editor-defaults package.
  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      const outcome = runtime.handleKeyboardEvent(event.nativeEvent);
      onPending?.(runtime.pendingChords());
      if (outcome !== "passthrough") event.preventDefault();
    },
    [runtime, onPending],
  );

  const handleMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      const content = event.currentTarget;
      const rect = content.getBoundingClientRect();
      const current = runtime.snapshot();
      const hit = hitTestTextPosition({
        text: current.text,
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
        lineHeight: LINE_HEIGHT,
        charWidth,
        paddingLeft: PADDING_LEFT,
        paddingTop: PADDING_TOP,
        tabWidth: TAB_WIDTH,
        bias: "nearest",
      });
      runtime.setSelection(Selection.point(hit.offset));
    },
    [runtime, charWidth],
  );

  const totalLines = snapshot.text.lineCount;
  const startLine = Math.max(
    0,
    Math.floor((scrollTop - PADDING_TOP) / LINE_HEIGHT) - OVERSCAN_LINES,
  );
  const visibleLines = Math.ceil(viewportHeight / LINE_HEIGHT) + OVERSCAN_LINES * 2;

  const layout = useMemo(() => {
    const primary = snapshot.selection.primary;
    return layoutViewport({
      text: snapshot.text,
      version: snapshot.version,
      startLine,
      lineCount: visibleLines,
      tabWidth: TAB_WIDTH,
      anchors: [
        { kind: "overlay", id: "caret", at: primary.head, payload: null },
        ...snapshot.selection.ranges
          .filter((range) => !range.isCollapsed)
          .map((range, index) => ({
            kind: "range" as const,
            id: `selection-${index}`,
            from: range.from,
            to: range.to,
            payload: null,
          })),
      ],
    });
  }, [snapshot, startLine, visibleLines]);

  return (
    <div
      className="editor"
      tabIndex={0}
      onKeyDown={handleKeyDown}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
    >
      <span ref={measureRef} className="editor-measure" aria-hidden>
        {"0".repeat(100)}
      </span>
      <div
        ref={scrollerRef}
        className="editor-scroller"
        onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      >
        <div
          className="editor-content"
          style={{ height: PADDING_TOP * 2 + totalLines * LINE_HEIGHT }}
          onMouseDown={handleMouseDown}
        >
          {layout.placements.map((placement) => {
            if (placement.kind === "range") {
              const left = PADDING_LEFT + placement.columnStart * charWidth;
              const width = Math.max((placement.columnEnd - placement.columnStart) * charWidth, 4);
              return (
                <div
                  key={`${placement.id}-${placement.line}`}
                  className="editor-selection"
                  style={{
                    top: PADDING_TOP + placement.line * LINE_HEIGHT,
                    left,
                    width,
                    height: LINE_HEIGHT,
                  }}
                />
              );
            }
            if (placement.kind === "overlay" && placement.id === "caret") {
              return (
                <div
                  key={`caret-${placement.at}-${snapshot.version}`}
                  className={focused ? "editor-caret" : "editor-caret editor-caret-idle"}
                  style={{
                    top: PADDING_TOP + placement.line * LINE_HEIGHT,
                    left: PADDING_LEFT + placement.column * charWidth,
                    height: LINE_HEIGHT,
                  }}
                />
              );
            }
            return null;
          })}
          {layout.rows.map((row) =>
            row.kind === "text" ? (
              <div
                key={row.line}
                className="editor-row"
                style={{ top: PADDING_TOP + row.line * LINE_HEIGHT }}
              >
                {row.text}
              </div>
            ) : null,
          )}
        </div>
      </div>
    </div>
  );
}
