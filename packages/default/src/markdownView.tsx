import { DiffsHighlighter, getSharedHighlighter, registerCustomCSSVariableTheme, type SupportedLanguages } from "@pierre/diffs";
import React, {
  Children,
  Suspense,
  isValidElement,
  memo,
  use,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import type { Components } from "react-markdown";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { resolveThemeStyleLoose, type ResolvedThemeDefinition } from "@the-editor/editor/themes";

export interface MarkdownViewProps {
  markdown: string;
  className?: string;
  theme?: ResolvedThemeDefinition;
}

class CodeHighlightErrorBoundary extends React.Component<
  { fallback: ReactNode; children: ReactNode },
  { hasError: boolean }
> {
  constructor(props: { fallback: ReactNode; children: ReactNode }) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  override render() {
    if (this.state.hasError) return this.props.fallback;
    return this.props.children;
  }
}

const CODE_FENCE_LANGUAGE_REGEX = /(?:^|\s)language-([^\s]+)/;
const MARKDOWN_THEME_NAME = "the-editor-markdown";
const highlightedCodeCache = new Map<string, string>();
const highlighterPromiseCache = new Map<string, Promise<DiffsHighlighter>>();

registerCustomCSSVariableTheme(MARKDOWN_THEME_NAME, {}, true);

function extractFenceLanguage(className: string | undefined): string {
  const raw = className?.match(CODE_FENCE_LANGUAGE_REGEX)?.[1] ?? "text";
  return raw === "gitignore" ? "ini" : raw;
}

function nodeToPlainText(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map((child) => nodeToPlainText(child)).join("");
  if (isValidElement<{ children?: ReactNode }>(node)) return nodeToPlainText(node.props.children);
  return "";
}

function extractCodeBlock(children: ReactNode): { className: string | undefined; code: string } | null {
  const childNodes = Children.toArray(children);
  if (childNodes.length !== 1) return null;
  const onlyChild = childNodes[0];
  if (!isValidElement<{ className?: string; children?: ReactNode }>(onlyChild) || onlyChild.type !== "code") return null;
  return {
    className: onlyChild.props.className,
    code: nodeToPlainText(onlyChild.props.children),
  };
}

function createHighlightCacheKey(code: string, language: string): string {
  return `${fnv1a32(code).toString(36)}:${code.length}:${language}`;
}

function getHighlighterPromise(language: string): Promise<DiffsHighlighter> {
  const cached = highlighterPromiseCache.get(language);
  if (cached) return cached;

  const promise = getSharedHighlighter({
    themes: [MARKDOWN_THEME_NAME],
    langs: [language as SupportedLanguages],
    preferredHighlighter: "shiki-js",
  }).catch((error) => {
    highlighterPromiseCache.delete(language);
    if (language === "text") throw error;
    return getHighlighterPromise("text");
  });

  highlighterPromiseCache.set(language, promise);
  return promise;
}

function MarkdownCodeBlock({ code, children }: { code: string; children: ReactNode }) {
  const [copied, setCopied] = useState(false);
  const copiedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleCopy = useCallback(() => {
    if (typeof navigator === "undefined" || navigator.clipboard == null) return;
    void navigator.clipboard.writeText(code).then(() => {
      if (copiedTimerRef.current != null) clearTimeout(copiedTimerRef.current);
      setCopied(true);
      copiedTimerRef.current = setTimeout(() => {
        setCopied(false);
        copiedTimerRef.current = null;
      }, 1200);
    }).catch(() => undefined);
  }, [code]);

  useEffect(() => () => {
    if (copiedTimerRef.current != null) clearTimeout(copiedTimerRef.current);
  }, []);

  return (
    <div className="editorMarkdownCodeblock">
      <button
        type="button"
        className="editorMarkdownCopyButton"
        onClick={handleCopy}
        title={copied ? "Copied" : "Copy code"}
        aria-label={copied ? "Copied" : "Copy code"}
      >
        {copied ? "✓" : "⧉"}
      </button>
      {children}
    </div>
  );
}

function HighlightedCodeBlock({
  className,
  code,
  theme,
}: {
  className: string | undefined;
  code: string;
  theme: ResolvedThemeDefinition | undefined;
}) {
  const language = extractFenceLanguage(className);
  const cacheKey = createHighlightCacheKey(code, language);
  const cachedHighlightedHtml = highlightedCodeCache.get(cacheKey);

  if (cachedHighlightedHtml != null) {
    return <div className="editorMarkdownShiki" dangerouslySetInnerHTML={{ __html: cachedHighlightedHtml }} />;
  }

  const highlighter = use(getHighlighterPromise(language));
  const highlightedHtml = useMemo(() => {
    try {
      return highlighter.codeToHtml(code, { lang: language, theme: MARKDOWN_THEME_NAME });
    } catch {
      return highlighter.codeToHtml(code, { lang: "text", theme: MARKDOWN_THEME_NAME });
    }
  }, [code, highlighter, language]);

  useEffect(() => {
    if (highlightedCodeCache.size > 500) highlightedCodeCache.clear();
    highlightedCodeCache.set(cacheKey, highlightedHtml);
  }, [cacheKey, highlightedHtml]);

  return (
    <div
      className="editorMarkdownShiki"
      style={markdownThemeVariables(theme)}
      dangerouslySetInnerHTML={{ __html: highlightedHtml }}
    />
  );
}

function createMarkdownComponents(theme: ResolvedThemeDefinition | undefined): Components {
  return {
    a({ node: _node, className, href, ...props }) {
      return (
        <a
          {...props}
          href={href}
          target="_blank"
          rel="noreferrer noopener"
          className={joinClassName(className, "editorMarkdownLink")}
        />
      );
    },
    pre({ node: _node, children, ...props }) {
      const codeBlock = extractCodeBlock(children);
      if (!codeBlock) return <pre {...props}>{children}</pre>;
      return (
        <MarkdownCodeBlock code={codeBlock.code}>
          <CodeHighlightErrorBoundary fallback={<pre {...props}>{children}</pre>}>
            <Suspense fallback={<pre {...props}>{children}</pre>}>
              <HighlightedCodeBlock className={codeBlock.className} code={codeBlock.code} theme={theme} />
            </Suspense>
          </CodeHighlightErrorBoundary>
        </MarkdownCodeBlock>
      );
    },
  };
}

function MarkdownViewComponent({ markdown, className, theme }: MarkdownViewProps) {
  const markdownComponents = useMemo(() => createMarkdownComponents(theme), [theme]);
  return (
    <div className={className} style={markdownThemeVariables(theme)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
        {markdown}
      </ReactMarkdown>
    </div>
  );
}

function joinClassName(...parts: Array<string | undefined>): string | undefined {
  const value = parts.filter(Boolean).join(" ").trim();
  return value || undefined;
}

function markdownThemeVariables(theme: ResolvedThemeDefinition | undefined): CSSProperties {
  const text = resolveThemeStyleLoose(theme ?? { styles: {} }, "ui.text")?.fg;
  const textMuted = resolveThemeStyleLoose(theme ?? { styles: {} }, "ui.text.inactive", "ui.text")?.fg;
  const textFocus = resolveThemeStyleLoose(theme ?? { styles: {} }, "ui.text.focus", "ui.text")?.fg;
  const background = resolveThemeStyleLoose(theme ?? { styles: {} }, "ui.background")?.bg;
  const popupBg = resolveThemeStyleLoose(theme ?? { styles: {} }, "ui.popup", "ui.background")?.bg;
  const keyword = resolveThemeStyleLoose(theme ?? { styles: {} }, "keyword", ["ui.text.focus", "ui.text"])?.fg;
  const string = resolveThemeStyleLoose(theme ?? { styles: {} }, "string", "ui.text")?.fg;
  const comment = resolveThemeStyleLoose(theme ?? { styles: {} }, "comment", "ui.text.inactive")?.fg;
  const type = resolveThemeStyleLoose(theme ?? { styles: {} }, "type", "ui.text")?.fg;
  const variable = resolveThemeStyleLoose(theme ?? { styles: {} }, "variable", "ui.text")?.fg;
  const warning = resolveThemeStyleLoose(theme ?? { styles: {} }, "warning", "ui.text")?.fg;
  const danger = resolveThemeStyleLoose(theme ?? { styles: {} }, "error", "ui.text")?.fg;
  const hint = resolveThemeStyleLoose(theme ?? { styles: {} }, "hint", "ui.text")?.fg;

  return {
    "--diffs-foreground": text,
    "--diffs-background": popupBg ?? background,
    "--diffs-token-comment": comment ?? textMuted,
    "--diffs-token-string": string ?? text,
    "--diffs-token-constant": type ?? variable ?? text,
    "--diffs-token-keyword": keyword ?? textFocus ?? text,
    "--diffs-token-parameter": variable ?? text,
    "--diffs-token-function": variable ?? textFocus ?? text,
    "--diffs-token-string-expression": string ?? text,
    "--diffs-token-punctuation": textMuted ?? text,
    "--diffs-token-link": textFocus ?? text,
    "--diffs-token-inserted": hint ?? text,
    "--diffs-token-deleted": danger ?? text,
    "--diffs-token-changed": warning ?? text,
    "--diffs-ansi-black": theme?.palette.black,
    "--diffs-ansi-red": theme?.palette.red,
    "--diffs-ansi-green": theme?.palette.green,
    "--diffs-ansi-yellow": theme?.palette.yellow,
    "--diffs-ansi-blue": theme?.palette.blue,
    "--diffs-ansi-magenta": theme?.palette.magenta,
    "--diffs-ansi-cyan": theme?.palette.cyan,
    "--diffs-ansi-white": theme?.palette.white,
    "--diffs-ansi-bright-black": theme?.palette.gray,
    "--diffs-ansi-bright-red": theme?.palette["light-red"],
    "--diffs-ansi-bright-green": theme?.palette["light-green"],
    "--diffs-ansi-bright-yellow": theme?.palette["light-yellow"],
    "--diffs-ansi-bright-blue": theme?.palette["light-blue"],
    "--diffs-ansi-bright-magenta": theme?.palette["light-magenta"],
    "--diffs-ansi-bright-cyan": theme?.palette["light-cyan"],
    "--diffs-ansi-bright-white": theme?.palette["light-gray"] ?? theme?.palette.white,
  } as CSSProperties;
}

function fnv1a32(input: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < input.length; index += 1) {
    hash ^= input.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

export const MarkdownView = memo(MarkdownViewComponent);
