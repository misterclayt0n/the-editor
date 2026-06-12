/**
 * LSP snippet syntax (the TextMate/VSCode grammar, the parts completion
 * items actually use): tab stops `$1`/`${1}`, placeholders `${1:text}`
 * (nested snippets allowed in the default), choices `${1|a,b|}` (first
 * choice wins), `$0` as the final caret, and `\$`/`\\`/`\}` escapes.
 * Variables (`$TM_FILENAME`, `${NAME:default}`) expand to their default or
 * to nothing — completion snippets virtually never rely on them.
 *
 * Parsing produces the expanded plain text plus tab-stop ranges RELATIVE to
 * it; the controller turns those into a live tab-stop session (Tab/Shift+Tab
 * jump, ranges mapped through subsequent edits).
 */

export type SnippetStop = {
  readonly index: number;
  /** Occurrences in the expanded text, relative offsets. */
  readonly ranges: ReadonlyArray<{ readonly from: number; readonly to: number }>;
};

export type ParsedSnippet = {
  readonly text: string;
  /**
   * Visiting order: ascending index with 0 (the final caret) moved last.
   * Absent $0 means the session ends at the end of the inserted text.
   */
  readonly stops: ReadonlyArray<SnippetStop>;
};

/** True if the text uses any snippet construct (cheap pre-check). */
export const looksLikeSnippet = (text: string): boolean => /\$(\d|\{)/.test(text);

export function parseSnippet(source: string): ParsedSnippet {
  let out = "";
  const ranges = new Map<number, Array<{ from: number; to: number }>>();

  const record = (index: number, from: number, to: number): void => {
    const list = ranges.get(index) ?? [];
    list.push({ from, to });
    ranges.set(index, list);
  };

  let i = 0;
  const parseRun = (stopAtBrace: boolean): void => {
    while (i < source.length) {
      const ch = source[i]!;
      if (ch === "\\" && i + 1 < source.length) {
        out += source[i + 1]!;
        i += 2;
        continue;
      }
      if (stopAtBrace && ch === "}") return;
      if (ch === "$") {
        i++;
        parseDollar();
        continue;
      }
      out += ch;
      i++;
    }
  };

  const parseDollar = (): void => {
    // $1
    const simple = /^\d+/.exec(source.slice(i));
    if (simple !== null) {
      i += simple[0].length;
      record(Number(simple[0]), out.length, out.length);
      return;
    }
    if (source[i] !== "{") {
      // $VARIABLE — no value source; expands to nothing.
      const variable = /^[A-Za-z_][A-Za-z0-9_]*/.exec(source.slice(i));
      if (variable !== null) i += variable[0].length;
      else out += "$";
      return;
    }
    i++; // consume "{"
    const body = /^(\d+|[A-Za-z_][A-Za-z0-9_]*)/.exec(source.slice(i));
    if (body === null) {
      out += "${";
      return;
    }
    i += body[0].length;
    const isStop = /^\d+$/.test(body[0]);
    const index = isStop ? Number(body[0]) : null;
    const start = out.length;
    if (source[i] === ":") {
      i++; // default value, may nest
      parseRun(true);
    } else if (source[i] === "|") {
      // choices: first one is the placeholder text
      i++;
      const end = source.indexOf("|}", i);
      const choices = (end === -1 ? source.slice(i) : source.slice(i, end)).split(",");
      out += choices[0] ?? "";
      i = end === -1 ? source.length : end + 1; // leave "}" for below
    }
    if (source[i] === "}") i++;
    if (index !== null) record(index, start, out.length);
  };

  parseRun(false);

  const stops = [...ranges.entries()]
    .sort(([a], [b]) => (a === 0 ? 1 : b === 0 ? -1 : a - b))
    .map(([index, list]) => ({ index, ranges: list }));
  return { text: out, stops };
}
