import MarkdownIt from "markdown-it";

const markdown = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
  breaks: false,
});

const allowedSchemes = /^(https?:|mailto:|file:|#|\/)/i;
markdown.validateLink = (url: string) => allowedSchemes.test(url.trim());

type LinkOpenRule = NonNullable<typeof markdown.renderer.rules.link_open>;
const defaultLinkOpen: LinkOpenRule = markdown.renderer.rules.link_open
  ?? ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
markdown.renderer.rules.link_open = (tokens, idx, options, env, self) => {
  const token = tokens[idx];
  if (!token) return defaultLinkOpen(tokens, idx, options, env, self);
  token.attrSet("target", "_blank");
  token.attrSet("rel", "noreferrer noopener");
  token.attrJoin("class", "editorMarkdownLink");
  return defaultLinkOpen(tokens, idx, options, env, self);
};

export function renderMarkdownToHtml(source: string): string {
  return markdown.render(source ?? "");
}
