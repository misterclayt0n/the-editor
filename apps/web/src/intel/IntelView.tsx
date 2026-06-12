/**
 * React UI for the intel controller: the completion list and the hover
 * card, portaled into renderer-positioned float elements (so they're glued
 * to text through scrolling, like the caret).
 */

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { highlightSnippet } from "../editor/snippetHighlight";
import type { IntelController } from "./controller";
import type { SignatureInfo } from "./types";

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
  const html = DOMPurify.sanitize(
    marked.parse(contents, { async: false, gfm: true }) as string,
  );
  return <div className="intel-hover__md" dangerouslySetInnerHTML={{ __html: html }} />;
}

/** Copies the card's raw markdown, Zed-style. */
function CopyButton({ contents }: { readonly contents: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      className="intel-hover__copy"
      title="Copy"
      onClick={() => {
        void navigator.clipboard.writeText(contents).then(() => {
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1200);
        });
      }}
    >
      {copied ? (
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
          <path d="M3 8.5 6.5 12 13 4.5" stroke="currentColor" strokeWidth="1.6" />
        </svg>
      ) : (
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
          <rect x="5" y="5" width="8" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.3" />
          <path d="M11 5V4a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v5a2 2 0 0 0 2 2h1" stroke="currentColor" strokeWidth="1.3" />
        </svg>
      )}
    </button>
  );
}

export function IntelView({ controller }: { readonly controller: IntelController }) {
  const state = useSyncExternalStore(
    useCallback((cb) => controller.subscribe(cb), [controller]),
    () => controller.getState(),
  );
  const listRef = useRef<HTMLDivElement>(null);

  // Keep the selected row in view.
  useEffect(() => {
    if (!state.completion.open) return;
    const selected = listRef.current?.querySelector<HTMLElement>(".intel-item--selected");
    selected?.scrollIntoView({ block: "nearest" });
  }, [state.completion.selected, state.completion.open]);

  const completionHost = controller.completionElement();
  const hoverHost = controller.hoverElement();
  const signatureHost = controller.signatureElement();
  const signatureResult = state.signature.result;
  const activeSignature = signatureResult?.signatures[state.signature.selected];
  const activeParameter =
    activeSignature?.activeParameter ?? signatureResult?.activeParameter ?? 0;
  const activeParameterDoc =
    activeSignature?.parameters?.[activeParameter]?.documentation;

  return (
    <>
      {completionHost !== null &&
        state.completion.open &&
        createPortal(
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
          </div>,
          completionHost,
        )}
      {signatureHost !== null &&
        state.signature.open &&
        signatureResult !== null &&
        activeSignature !== undefined &&
        createPortal(
          <div className="intel-signature">
            <div className="intel-signature__line">
              {signatureResult.signatures.length > 1 && (
                <span className="intel-signature__counter">
                  {state.signature.selected + 1}/{signatureResult.signatures.length}
                </span>
              )}
              <SignatureLabel signature={activeSignature} activeParameter={activeParameter} />
            </div>
            {activeParameterDoc !== undefined && (
              <div className="intel-signature__param-doc">
                <Markdown contents={activeParameterDoc} />
              </div>
            )}
            {activeSignature.documentation !== undefined && (
              <div className="intel-signature__doc">
                <Markdown contents={activeSignature.documentation} />
              </div>
            )}
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
            {state.hover.parts.map((part, index) => (
              <div
                key={index}
                className={`intel-hover${part.kind === "diagnostic" ? " intel-hover--diagnostic" : ""}`}
              >
                <CopyButton contents={part.contents} />
                <Markdown contents={part.contents} />
              </div>
            ))}
          </div>,
          hoverHost,
        )}
    </>
  );
}
