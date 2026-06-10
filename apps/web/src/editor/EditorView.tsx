import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  DocumentInvalidated,
  SelectionInvalidated,
  hasInvalidation,
  hitTestTextPosition,
  layoutContentHeight,
  layoutViewport,
  type EditorNotification,
  type EditorSnapshot,
  type LayoutBlockWidget,
  type LayoutDecoration,
  type LayoutPlacement,
  type LayoutRow,
} from "@the-editor/editor";
import { Selection, type TextBuffer } from "@the-editor/engine";
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

type TextRenderState = {
  readonly text: TextBuffer;
  readonly version: number;
};

type SelectionRenderState = {
  readonly selection: Selection;
};

const TextLayer = memo(function TextLayer({ rows }: { readonly rows: readonly LayoutRow[] }) {
  return rows.map((row) =>
    row.kind === "text" ? (
      <div
        key={row.line}
        className="editor-row"
        style={{ top: row.top, left: row.left, height: row.height }}
      >
        {row.segments.length === 0
          ? row.text
          : row.segments.map((segment, index) =>
              segment.kind === "text" ? (
                <span key={index} className={segment.className} style={segment.style}>
                  {segment.text}
                </span>
              ) : (
                <span
                  key={index}
                  className={segment.className ?? "editor-virtual-text"}
                  style={segment.style}
                >
                  {segment.text}
                </span>
              ),
            )}
      </div>
    ) : null,
  );
});

const DecorationLayer = memo(function DecorationLayer({
  decorations,
  layers,
}: {
  readonly decorations: readonly LayoutDecoration[];
  readonly layers: readonly string[];
}) {
  return decorations
    .filter((decoration) => layers.includes(decoration.layer))
    .flatMap((decoration, decorationIndex) =>
      decoration.rects.map((rect, rectIndex) => (
        <div
          key={`${decoration.layer}-${decorationIndex}-${rectIndex}`}
          className={[
            "editor-decoration",
            `editor-decoration-${decoration.layer}`,
            decoration.className,
          ]
            .filter(Boolean)
            .join(" ")}
          style={{ ...rect, ...decoration.style }}
        />
      )),
    );
});

const BlockWidgetLayer = memo(function BlockWidgetLayer({
  widgets,
}: {
  readonly widgets: readonly LayoutBlockWidget[];
}) {
  return widgets.map((widget) => (
    <div
      key={widget.id}
      className="editor-block-widget"
      data-widget-id={widget.id}
      style={widget.rect}
    />
  ));
});

const SelectionLayer = memo(function SelectionLayer({
  placements,
}: {
  readonly placements: readonly LayoutPlacement<null>[];
}) {
  return placements.map((placement) => {
    if (placement.kind !== "range") return null;
    return placement.rects.map((rect, rectIndex) => (
      <div
        key={`${placement.id}-${placement.line}-${rectIndex}`}
        className="editor-selection"
        style={{
          top: rect.top,
          left: rect.left,
          width: Math.max(rect.width, 4),
          height: rect.height,
        }}
      />
    ));
  });
});

const CaretLayer = memo(function CaretLayer({
  placements,
  focused,
  version,
}: {
  readonly placements: readonly LayoutPlacement<null>[];
  readonly focused: boolean;
  readonly version: number;
}) {
  return placements.map((placement) => {
    if (placement.kind !== "overlay" || placement.id !== "caret") return null;
    return (
      <div
        key={`caret-${placement.at}-${version}`}
        className={focused ? "editor-caret" : "editor-caret editor-caret-idle"}
        style={{
          top: placement.rect.top,
          left: placement.rect.left,
          height: placement.rect.height,
        }}
      />
    );
  });
});

export function EditorView({ runtime, onSnapshot, onPending }: EditorViewProps) {
  const initial = useMemo(() => runtime.snapshot(), [runtime]);
  const [textState, setTextState] = useState<TextRenderState>(() => ({
    text: initial.text,
    version: initial.version,
  }));
  const [selectionState, setSelectionState] = useState<SelectionRenderState>(() => ({
    selection: initial.selection,
  }));
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
    onSnapshot?.(initial);
  }, [initial, onSnapshot]);

  const revealCaret = useCallback(
    (next: EditorSnapshot) => {
      const scroller = scrollerRef.current;
      if (!scroller) return;
      const caretLayout = layoutViewport({
        text: next.text,
        version: next.version,
        startLine: 0,
        lineCount: next.text.lineCount,
        lineHeight: LINE_HEIGHT,
        charWidth,
        paddingTop: PADDING_TOP,
        paddingLeft: PADDING_LEFT,
        tabWidth: TAB_WIDTH,
        anchors: [{ kind: "overlay", id: "caret", at: next.selection.primary.head, payload: null }],
      });
      const caretTop =
        caretLayout.placements.find((placement) => placement.kind === "overlay")?.rect.top ?? 0;
      if (caretTop < scroller.scrollTop) {
        scroller.scrollTop = caretTop;
      } else if (caretTop + LINE_HEIGHT > scroller.scrollTop + scroller.clientHeight) {
        scroller.scrollTop = caretTop + LINE_HEIGHT - scroller.clientHeight;
      }
    },
    [charWidth],
  );

  const refresh = useCallback(
    (notification: EditorNotification) => {
      const documentChanged = hasInvalidation(notification, DocumentInvalidated);
      const selectionChanged = hasInvalidation(notification, SelectionInvalidated);
      if (!documentChanged && !selectionChanged) return;

      const next = runtime.snapshot();
      if (documentChanged) setTextState({ text: next.text, version: next.version });
      if (documentChanged || selectionChanged) setSelectionState({ selection: next.selection });
      revealCaret(next);
      onSnapshot?.(next);
    },
    [runtime, revealCaret, onSnapshot],
  );

  // The renderer is a notification consumer: document invalidations update text
  // layout state; selection-only invalidations update only overlay/caret state.
  useEffect(() => runtime.subscribe((notification) => refresh(notification)), [runtime, refresh]);

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

  const startLine = Math.max(
    0,
    hitTestTextPosition({
      text: textState.text,
      x: 0,
      y: scrollTop,
      lineHeight: LINE_HEIGHT,
      charWidth,
      paddingLeft: PADDING_LEFT,
      paddingTop: PADDING_TOP,
      tabWidth: TAB_WIDTH,
      bias: "before",
    }).line - OVERSCAN_LINES,
  );
  const visibleLines = Math.ceil(viewportHeight / LINE_HEIGHT) + OVERSCAN_LINES * 2;

  const textLayout = useMemo(
    () =>
      layoutViewport({
        text: textState.text,
        version: textState.version,
        startLine,
        lineCount: visibleLines,
        tabWidth: TAB_WIDTH,
        lineHeight: LINE_HEIGHT,
        charWidth,
        paddingTop: PADDING_TOP,
        paddingLeft: PADDING_LEFT,
        anchors: [],
      }),
    [textState.text, textState.version, startLine, visibleLines, charWidth],
  );

  const overlayLayout = useMemo(() => {
    const primary = selectionState.selection.primary;
    return layoutViewport({
      text: textState.text,
      version: textState.version,
      startLine,
      lineCount: visibleLines,
      tabWidth: TAB_WIDTH,
      lineHeight: LINE_HEIGHT,
      charWidth,
      paddingTop: PADDING_TOP,
      paddingLeft: PADDING_LEFT,
      anchors: [
        { kind: "overlay", id: "caret", at: primary.head, payload: null },
        ...selectionState.selection.ranges
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
  }, [
    textState.text,
    textState.version,
    selectionState.selection,
    startLine,
    visibleLines,
    charWidth,
  ]);

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
          style={{
            height: layoutContentHeight({
              text: textState.text,
              lineHeight: LINE_HEIGHT,
              paddingTop: PADDING_TOP,
            }),
          }}
          onMouseDown={handleMouseDown}
        >
          <DecorationLayer decorations={textLayout.decorations} layers={["background"]} />
          <SelectionLayer placements={overlayLayout.placements} />
          <BlockWidgetLayer widgets={textLayout.blockWidgets} />
          <TextLayer rows={textLayout.rows} />
          <DecorationLayer decorations={textLayout.decorations} layers={["underline", "outline"]} />
          <CaretLayer
            placements={overlayLayout.placements}
            focused={focused}
            version={textState.version}
          />
        </div>
      </div>
    </div>
  );
}
