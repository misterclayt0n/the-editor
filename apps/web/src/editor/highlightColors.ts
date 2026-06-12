/**
 * Capture name → color, helix-style: "keyword.control.return" falls back
 * through "keyword.control" to "keyword". Placeholder palette until the
 * theme system owns these.
 */

const palette: Readonly<Record<string, string>> = {
  attribute: "#dcdcaa",
  comment: "#6a9955",
  constant: "#4fc1ff",
  "constant.builtin": "#569cd6",
  "constant.character.escape": "#d7ba7d",
  "constant.numeric": "#b5cea8",
  constructor: "#4ec9b0",
  function: "#dcdcaa",
  keyword: "#569cd6",
  "keyword.control": "#c586c0",
  "keyword.control.import": "#c586c0",
  label: "#c8c8c8",
  namespace: "#4ec9b0",
  operator: "#d4d4d4",
  punctuation: "#808080",
  special: "#d7ba7d",
  string: "#ce9178",
  "string.special": "#d7ba7d",
  tag: "#569cd6",
  type: "#4ec9b0",
  variable: "#9cdcfe",
  "variable.builtin": "#569cd6",
  "variable.other.member": "#9cdcfe",
  "variable.parameter": "#9cdcfe",
};

const resolved = new Map<string, string | undefined>();

export function highlightColor(kind: string): string | undefined {
  if (resolved.has(kind)) return resolved.get(kind);
  let probe = kind;
  let color: string | undefined;
  for (;;) {
    color = palette[probe];
    if (color !== undefined) break;
    const dot = probe.lastIndexOf(".");
    if (dot === -1) break;
    probe = probe.slice(0, dot);
  }
  resolved.set(kind, color);
  return color;
}
