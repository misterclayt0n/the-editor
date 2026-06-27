/**
 * The file-picker preview panel: a read-only, syntax-highlighted window onto
 * the selected file. It owns no editor truth — it reads a bounded window
 * through FileSystemPort.readTextRange and highlights it with the shared
 * detached highlighter. Selection churn is debounced and each effect cancels
 * its own in-flight load, so arrowing through results never thrashes.
 */

import { Effect } from "effect";
import { useEffect, useRef, useState } from "react";
import type { CodeSpan } from "../editor/highlightSpans";
import type { AppRuntime } from "../runtime";
import {
  loadFilePreview,
  type PreviewReader,
  type PreviewState,
  type PreviewTarget,
} from "./filePreview";
import { highlightPreview } from "./previewHighlight";

const PREVIEW_DEBOUNCE_MS = 40;

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
    const reader: PreviewReader = (path, options) => Effect.runPromise(read(path, options));
    setState({ status: "loading" });
    const timer = window.setTimeout(() => {
      void loadFilePreview(target, { read: reader, highlight: highlightPreview }).then((next) => {
        if (!cancelled) setState(next);
      });
    }, PREVIEW_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // `target` is a memoized, stable object from the picker (FileFinder).
  }, [target, runtime]);

  return state;
}

function PreviewCode({
  lines,
  focusLine,
}: {
  readonly lines: readonly (readonly CodeSpan[])[];
  readonly focusLine: number | undefined;
}) {
  const focusRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    focusRef.current?.scrollIntoView({ block: "center" });
  }, [focusLine, lines]);
  return (
    <div className="file-preview__code">
      {lines.map((spans, i) => (
        <div
          key={i}
          ref={i === focusLine ? focusRef : undefined}
          className={
            i === focusLine ? "file-preview__line file-preview__line--focus" : "file-preview__line"
          }
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
      ))}
    </div>
  );
}

function PreviewBody({ state }: { readonly state: PreviewState }) {
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
      return <PreviewCode lines={state.lines} focusLine={state.focusLine} />;
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
      <PreviewBody state={state} />
    </div>
  );
}
