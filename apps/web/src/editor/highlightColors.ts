import { resolveSyntaxStyle } from "../theme/dom";

export function highlightColor(kind: string): string | undefined {
  return resolveSyntaxStyle(kind)?.color;
}
