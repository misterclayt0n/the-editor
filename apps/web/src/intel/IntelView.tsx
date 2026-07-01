/**
 * React UI for the intel controller: the completion list and the hover
 * card, portaled into renderer-positioned float elements (so they're glued
 * to text through scrolling, like the caret).
 */

import { useCallback, useEffect, useRef, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { highlightSnippet } from "../editor/snippetHighlight";
import type { CodeSpan } from "../editor/highlightSpans";
import type { IntelController } from "./controller";
import type { SignatureInfo } from "@the-editor/intel";

/** The signature label split around the active parameter's span. */
function SignatureLabel({
  signature,
  activeParameter,
}: {
  readonly signature: SignatureInfo;
  readonly activeParameter: number;
}) {
  const label = signature.label;
  const parameter = signature.parameters?.[activeParameter];
  let range: readonly [number, number] | null = null;
  if (parameter !== undefined) {
    if (typeof parameter.label === "string") {
      const index = label.indexOf(parameter.label);
      if (index >= 0) range = [index, index + parameter.label.length];
    } else {
      range = parameter.label;
    }
  }
  if (range === null) return <span className="intel-signature__label">{label}</span>;
  return (
    <span className="intel-signature__label">
      {label.slice(0, range[0])}
      <span className="intel-signature__active">{label.slice(range[0], range[1])}</span>
      {label.slice(range[1])}
    </span>
  );
}

const escapeHtml = (text: string): string =>
  text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");

// Fenced blocks with a language tag render through the editor's own
// tree-sitter engine (one composable call); the rest is marked's default.
marked.use({
  renderer: {
    code({ text, lang }: { text: string; lang?: string }): string {
      const spans = lang !== undefined ? highlightSnippet(text, lang) : null;
      const body =
        spans === null
          ? escapeHtml(text)
          : spans
              .map((span) =>
                span.color !== undefined
                  ? `<span style="color:${span.color}">${escapeHtml(span.text)}</span>`
                  : escapeHtml(span.text),
              )
              .join("");
      return `<pre><code>${body}</code></pre>\n`;
    },
  },
});

/** Full markdown (GFM), sanitized; hover contents come from servers. */
function Markdown({ contents }: { readonly contents: string }) {
  const html = DOMPurify.sanitize(marked.parse(contents, { async: false, gfm: true }) as string);
  return <div className="intel-hover__md" dangerouslySetInnerHTML={{ __html: html }} />;
}

/** A code string rendered with the editor's syntax highlighting (the hover
 * signature and the completion docs detail share this look). */
function HighlightedCode({
  code,
  language,
  className,
}: {
  readonly code: string;
  readonly language: string | undefined;
  readonly className?: string;
}) {
  const spans = language !== undefined ? highlightSnippet(code, language) : null;
  return (
    <div className={className}>
      {spans !== null
        ? spans.map((span, index) => (
            <span key={index} style={span.color !== undefined ? { color: span.color } : undefined}>
              {span.text}
            </span>
          ))
        : code}
    </div>
  );
}

/**
 * A hover card split into a Pierre-style header (the signature) and a body.
 * LSP hover is conventionally a leading fenced code block (the symbol's
 * signature) followed by markdown docs — so we lift that fence out, syntax
 * highlight it as the header, and render the rest as the body.
 */
type HoverHead = {
  readonly sigSpans: CodeSpan[] | null;
  readonly sigText: string | null;
  readonly body: string;
};

const LEADING_FENCE = /^```([\w+#.-]*)[ \t]*\r?\n([\s\S]*?)\r?\n?```/;

const splitHover = (contents: string): HoverHead => {
  const trimmed = contents.trim();
  const match = LEADING_FENCE.exec(trimmed);
  if (match === null) {
    return { sigSpans: null, sigText: null, body: trimmed };
  }
  const lang = (match[1] ?? "").trim();
  const code = (match[2] ?? "").replace(/\s+$/, "");
  return {
    sigSpans: lang.length > 0 ? highlightSnippet(code, lang) : null,
    sigText: code,
    body: trimmed.slice(match[0].length).trim(),
  };
};

export function IntelView({ controller }: { readonly controller: IntelController }) {
  const state = useSyncExternalStore(
    useCallback((cb) => controller.subscribe(cb), [controller]),
    () => controller.getState(),
  );
  const listRef = useRef<HTMLDivElement>(null);

  // Keep the selected row in view. NEVER scrollIntoView here: it walks ALL
  // scrollable ancestors — including the overflow:hidden editor viewport,
  // which it scrolls NATIVELY while the owned-scroll model still thinks 0,
  // visually shifting the whole buffer. Scroll the list alone.
  useEffect(() => {
    if (!state.completion.open) return;
    const list = listRef.current;
    const selected = list?.querySelector<HTMLElement>(".intel-item--selected");
    if (list == null || selected == null) return;
    const viewTop = list.scrollTop;
    const viewBottom = viewTop + list.clientHeight;
    const itemTop = selected.offsetTop;
    const itemBottom = itemTop + selected.offsetHeight;
    if (itemTop < viewTop) list.scrollTop = itemTop;
    else if (itemBottom > viewBottom) list.scrollTop = itemBottom - list.clientHeight;
  }, [state.completion.selected, state.completion.open]);

  const completionHost = controller.completionElement();
  const hoverHost = controller.hoverElement();
  const signatureHost = controller.signatureElement();
  const signatureResult = state.signature.result;
  const activeSignature = signatureResult?.signatures[state.signature.selected];
  const activeParameter = activeSignature?.activeParameter ?? signatureResult?.activeParameter ?? 0;

  return (
    <>
      {completionHost !== null &&
        state.completion.open &&
        createPortal(
          <div className="intel-completion-wrap">
            <div ref={listRef} className="intel-completion" role="listbox">
              {state.completion.items.slice(0, 60).map((scored, index) => (
                <div
                  key={`${scored.providerOrder}:${scored.item.label}:${index}`}
                  role="option"
                  aria-selected={index === state.completion.selected}
                  className={`intel-item${index === state.completion.selected ? " intel-item--selected" : ""}${scored.item.deprecated ? " intel-item--deprecated" : ""}`}
                  onMouseDown={(event) => {
                    event.preventDefault(); // keep editor focus
                  }}
                  onClick={() => controller.acceptIndex(index)}
                >
                  <span className={`intel-item__kind intel-kind--${scored.item.kind ?? "text"}`}>
                    {(scored.item.kind ?? "text").slice(0, 2)}
                  </span>
                  <span className="intel-item__label">{scored.item.label}</span>
                  {scored.item.detail !== undefined && (
                    <span className="intel-item__detail">{scored.item.detail}</span>
                  )}
                </div>
              ))}
            </div>
            {state.completionDocs !== null && (
              <div className="intel-docs">
                {state.completionDocs.detail !== undefined && (
                  <HighlightedCode
                    code={state.completionDocs.detail}
                    language={state.completionDocs.language}
                    className="intel-docs__detail"
                  />
                )}
                {state.completionDocs.documentation !== undefined && (
                  <Markdown contents={state.completionDocs.documentation} />
                )}
              </div>
            )}
          </div>,
          completionHost,
        )}
      {signatureHost !== null &&
        state.signature.open &&
        signatureResult !== null &&
        activeSignature !== undefined &&
        createPortal(
          <div className="intel-signature">
            {/* Just the signature label — docs are deliberately omitted; the big
             * description block was noise and overlapped the completion popup. */}
            <div className="intel-signature__line">
              {signatureResult.signatures.length > 1 && (
                <span className="intel-signature__counter">
                  {state.signature.selected + 1}/{signatureResult.signatures.length}
                </span>
              )}
              <SignatureLabel signature={activeSignature} activeParameter={activeParameter} />
            </div>
          </div>,
          signatureHost,
        )}
      {hoverHost !== null &&
        state.hover.open &&
        state.hover.parts.length > 0 &&
        createPortal(
          <div
            className="intel-hover-stack"
            onMouseEnter={() => controller.setHoverCardHovered(true)}
            onMouseLeave={() => controller.setHoverCardHovered(false)}
          >
            {state.hover.parts.map((part, index) => {
              const head = splitHover(part.contents);
              return (
                <div
                  key={index}
                  className={`intel-hover${part.kind === "diagnostic" ? " intel-hover--diagnostic" : ""}`}
                >
                  {head.sigText !== null && (
                    <span className="intel-hover__sig">
                      {head.sigSpans !== null
                        ? head.sigSpans.map((span, spanIndex) => (
                            <span
                              key={spanIndex}
                              style={span.color !== undefined ? { color: span.color } : undefined}
                            >
                              {span.text}
                            </span>
                          ))
                        : head.sigText}
                    </span>
                  )}
                  {head.body.length > 0 && <Markdown contents={head.body} />}
                </div>
              );
            })}
          </div>,
          hoverHost,
        )}
    </>
  );
}
