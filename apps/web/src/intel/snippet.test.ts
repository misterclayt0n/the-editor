import { describe, expect, test } from "vitest";
import { looksLikeSnippet, parseSnippet } from "./snippet";

describe("parseSnippet", () => {
  test("plain text passes through", () => {
    expect(parseSnippet("hello")).toEqual({ text: "hello", stops: [] });
  });

  test("function call with placeholders (clangd printf shape)", () => {
    const parsed = parseSnippet("printf(${1:const char *__format, ...})");
    expect(parsed.text).toBe("printf(const char *__format, ...)");
    expect(parsed.stops).toEqual([
      { index: 1, ranges: [{ from: 7, to: 32 }] },
    ]);
  });

  test("bare tab stops and final caret order", () => {
    const parsed = parseSnippet("for $2 in $1 {\n$0\n}");
    expect(parsed.text).toBe("for  in  {\n\n}");
    expect(parsed.stops.map((s) => s.index)).toEqual([1, 2, 0]);
    expect(parsed.stops[0]).toEqual({ index: 1, ranges: [{ from: 8, to: 8 }] });
    expect(parsed.stops[2]).toEqual({ index: 0, ranges: [{ from: 11, to: 11 }] });
  });

  test("choices take the first option", () => {
    const parsed = parseSnippet("align(${1|left,right,center|})");
    expect(parsed.text).toBe("align(left)");
    expect(parsed.stops).toEqual([{ index: 1, ranges: [{ from: 6, to: 10 }] }]);
  });

  test("nested placeholders flatten", () => {
    const parsed = parseSnippet("${1:outer ${2:inner}}!");
    expect(parsed.text).toBe("outer inner!");
    expect(parsed.stops).toEqual([
      { index: 1, ranges: [{ from: 0, to: 11 }] },
      { index: 2, ranges: [{ from: 6, to: 11 }] },
    ]);
  });

  test("escapes and variables", () => {
    expect(parseSnippet("cost: \\$5").text).toBe("cost: $5");
    expect(parseSnippet("\\\\path\\}").text).toBe("\\path}");
    expect(parseSnippet("file $TM_FILENAME end").text).toBe("file  end");
    expect(parseSnippet("v ${TM_SELECTED_TEXT:fallback} w").text).toBe("v fallback w");
  });

  test("repeated stop records every occurrence", () => {
    const parsed = parseSnippet("${1:x} = ${1:x};");
    expect(parsed.text).toBe("x = x;");
    expect(parsed.stops).toEqual([
      { index: 1, ranges: [{ from: 0, to: 1 }, { from: 4, to: 5 }] },
    ]);
  });

  test("looksLikeSnippet pre-check", () => {
    expect(looksLikeSnippet("plain")).toBe(false);
    expect(looksLikeSnippet("f($1)")).toBe(true);
    expect(looksLikeSnippet("f(${1:a})")).toBe(true);
  });
});
