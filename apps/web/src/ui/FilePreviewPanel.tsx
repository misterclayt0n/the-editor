/**
 * The file-picker preview panel: a read-only, syntax-highlighted window onto
 * the selected file. It owns no editor truth — it reads a bounded window
 * through FileSystemPort.readTextRange and highlights it with the shared
 * detached highlighter. Selection churn is debounced and each effect cancels
 * its own in-flight load, so arrowing through results never thrashes.
 */

import { Effect } from "effect";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { CodeSpan } from "../editor/highlightSpans";
import type { AppRuntime } from "../runtime";
import {
  previewLineWindow,
  readPreviewWindow,
  shapePreview,
  type PreviewReader,
  type PreviewState,
  type PreviewTarget,
} from "./filePreview";
import { highlightPreview } from "./previewHighlight";

const PREVIEW_DEBOUNCE_MS = 40;
const DEFAULT_PREVIEW_ROW_HEIGHT = 18;

const basename = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

function useFilePreview(target: PreviewTarget | null, runtime: AppRuntime): PreviewState {
  const [state, setState] = useState<PreviewState>({ status: "empty" });

  useEffect(() => {
    if (target === null) {
      setState({ status: "empty" });
      return;
    }
    const read = runtime.fileSystem()?.readTextRange;
    if (read === undefined) {
      setState({ status: "error", message: "Preview is unavailable in this host." });
      return;
    }
    let cancelled = false;
    let cancelHighlight = () => {};
    const reader: PreviewReader = (path, options) => Effect.runPromise(read(path, options));
    setState({ status: "loading" });
    const timer = window.setTimeout(() => {
      void readPreviewWindow(target, reader).then((window_) => {
        if (cancelled) return;
        if (window_.status !== "ready") {
          setState(window_);
          return;
        }
        // Render plain text immediately — no synchronous tree-sitter parse on
        // the frame. Then upgrade to syntax colors when the main thread is idle,
        // so highlighting a large file never blocks typing in the picker.
        setState(shapePreview(window_));
        const upgrade = () => {
          if (!cancelled) setState(shapePreview(window_, highlightPreview));
        };
        if (typeof window.requestIdleCallback === "function") {
          const handle = window.requestIdleCallback(upgrade, { timeout: 500 });
          cancelHighlight = () => window.cancelIdleCallback(handle);
        } else {
          const handle = window.setTimeout(upgrade, 16);
          cancelHighlight = () => window.clearTimeout(handle);
        }
      });
    }, PREVIEW_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      cancelHighlight();
    };
    // `target` is a memoized, stable object from the picker (FileFinder).
  }, [target, runtime]);

  return state;
}

function PreviewCode({
  lines,
  focusLine,
  targetKey,
}: {
  readonly lines: readonly (readonly CodeSpan[])[];
  readonly focusLine: number | undefined;
  readonly targetKey: string | undefined;
}) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [rowHeight, setRowHeight] = useState(DEFAULT_PREVIEW_ROW_HEIGHT);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (el === null) return;
    const measure = () => {
      setViewportHeight(el.clientHeight);
      const next = Number.parseFloat(window.getComputedStyle(el).lineHeight);
      setRowHeight(Number.isFinite(next) && next > 0 ? next : DEFAULT_PREVIEW_ROW_HEIGHT);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (el === null) return;
    if (focusLine === undefined) {
      el.scrollTop = 0;
      setScrollTop(0);
      return;
    }
    const viewport = el.clientHeight;
    const centeredTop = focusLine * rowHeight - (viewport - rowHeight) / 2;
    const next = Math.max(0, centeredTop);
    el.scrollTop = next;
    setScrollTop(el.scrollTop);
  }, [targetKey, focusLine, rowHeight]);

  const lineWindow = useMemo(
    () =>
      previewLineWindow({
        lineCount: lines.length,
        scrollTop,
        viewportHeight,
        rowHeight,
      }),
    [lines.length, scrollTop, viewportHeight, rowHeight],
  );
  const visibleLines = lines.slice(lineWindow.start, lineWindow.end);

  return (
    <div
      ref={scrollRef}
      className="file-preview__code"
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div style={{ height: lineWindow.totalHeight, position: "relative" }}>
        <div
          style={{
            left: 0,
            minWidth: "100%",
            position: "absolute",
            top: 0,
            transform: `translateY(${lineWindow.offsetTop}px)`,
            willChange: "transform",
          }}
        >
          {visibleLines.map((spans, offset) => {
            const i = lineWindow.start + offset;
            return (
              <div
                key={i}
                className={
                  i === focusLine
                    ? "file-preview__line file-preview__line--focus"
                    : "file-preview__line"
                }
                style={{ height: rowHeight }}
              >
                <span className="file-preview__ln">{i + 1}</span>
                <span className="file-preview__line-text">
                  {spans.length === 0
                    ? "\u00A0"
                    : spans.map((span, j) => (
                        <span
                          key={j}
                          style={span.color !== undefined ? { color: span.color } : undefined}
                        >
                          {span.text}
                        </span>
                      ))}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function PreviewBody({
  state,
  targetKey,
}: {
  readonly state: PreviewState;
  readonly targetKey: string | undefined;
}) {
  switch (state.status) {
    case "empty":
    case "loading":
      return null;
    case "skipped":
      return (
        <div className="file-preview__status">
          {state.reason === "binary" ? "Binary file" : "File too large to preview"}
        </div>
      );
    case "error":
      return <div className="file-preview__status">{state.message}</div>;
    case "ready":
      return <PreviewCode lines={state.lines} focusLine={state.focusLine} targetKey={targetKey} />;
  }
}

export function FilePreview({
  target,
  runtime,
}: {
  readonly target: PreviewTarget | null;
  readonly runtime: AppRuntime;
}) {
  const state = useFilePreview(target, runtime);
  const truncated = state.status === "ready" && state.truncated;
  return (
    <div className="file-preview">
      <div className="file-preview__header">
        {target !== null && (
          <>
            <span className="file-preview__title">{basename(target.path)}</span>
            {truncated && <span className="file-preview__badge">truncated</span>}
          </>
        )}
      </div>
      <PreviewBody state={state} targetKey={target?.path} />
    </div>
  );
}
