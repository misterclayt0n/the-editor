import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { layoutViewport, type EditorSnapshot } from "@the-editor/editor";
import { Selection } from "@the-editor/engine";
import type { EditorSession } from "./session";
import type { CommandResult } from "./commands";
import {
  deleteBackward,
  deleteForward,
  insertNewline,
  insertText,
  moveHorizontal,
  moveLineEdge,
  moveVertical,
} from "./commands";

const LINE_HEIGHT = 20;
const TAB_WIDTH = 4;
const PADDING_LEFT = 12;
const PADDING_TOP = 8;
const OVERSCAN_LINES = 4;

type EditorViewProps = {
  readonly session: EditorSession;
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
};

function offsetForVisualColumn(lineText: string, targetColumn: number): number {
  let column = 0;
  let offset = 0;
  for (const ch of lineText) {
    const advance = ch === "\t" ? TAB_WIDTH - (column % TAB_WIDTH) : 1;
    if (column + advance / 2 > targetColumn) return offset;
    column += advance;
    offset += ch.length;
  }
  return lineText.length;
}

export function EditorView({ session, onSnapshot }: EditorViewProps) {
  const [snapshot, setSnapshot] = useState<EditorSnapshot>(() => session.snapshot());
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
    const next = session.snapshot();
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
  }, [session]);

  const dispatch = useCallback(
    (result: CommandResult | undefined) => {
      if (!result) return;
      if (result.transaction) session.apply(result.transaction, result.selection);
      else session.setSelection(result.selection);
      refresh();
    },
    [session, refresh],
  );

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const current = session.snapshot();

      switch (event.key) {
        case "Enter":
          dispatch(insertNewline(current));
          break;
        case "Backspace":
          dispatch(deleteBackward(current));
          break;
        case "Delete":
          dispatch(deleteForward(current));
          break;
        case "Tab":
          dispatch(insertText(current, "\t"));
          break;
        case "ArrowLeft":
          dispatch(moveHorizontal(current, -1, event.shiftKey));
          break;
        case "ArrowRight":
          dispatch(moveHorizontal(current, 1, event.shiftKey));
          break;
        case "ArrowUp":
          dispatch(moveVertical(current, -1, event.shiftKey));
          break;
        case "ArrowDown":
          dispatch(moveVertical(current, 1, event.shiftKey));
          break;
        case "Home":
          dispatch(moveLineEdge(current, "start", event.shiftKey));
          break;
        case "End":
          dispatch(moveLineEdge(current, "end", event.shiftKey));
          break;
        default:
          if (event.key.length === 1) dispatch(insertText(current, event.key));
          else return;
      }
      event.preventDefault();
    },
    [session, dispatch],
  );

  const handleMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      const content = event.currentTarget;
      const rect = content.getBoundingClientRect();
      const current = session.snapshot();
      const line = Math.max(
        0,
        Math.min(
          current.text.lineCount - 1,
          Math.floor((event.clientY - rect.top - PADDING_TOP) / LINE_HEIGHT),
        ),
      );
      const range = current.text.lineRange(line);
      const lineText = current.text.sliceString(range.from, range.to);
      const column = Math.max(0, (event.clientX - rect.left - PADDING_LEFT) / charWidth);
      const offset = range.from + offsetForVisualColumn(lineText, column);
      dispatch({ selection: Selection.point(offset) });
    },
    [session, dispatch, charWidth],
  );

  const totalLines = snapshot.text.lineCount;
  const startLine = Math.max(0, Math.floor((scrollTop - PADDING_TOP) / LINE_HEIGHT) - OVERSCAN_LINES);
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
    <div className="editor" tabIndex={0} onKeyDown={handleKeyDown} onFocus={() => setFocused(true)} onBlur={() => setFocused(false)}>
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
