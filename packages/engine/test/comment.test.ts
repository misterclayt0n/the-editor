import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import { InvalidCommentTokenError, Selection, SelectionRange, TextBuffer, toggleBlockComments, toggleLineComments, type Edit } from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);
const apply = (buffer: TextBuffer, edit: Edit) => edit.transaction.apply(buffer).toString();

describe("comment", () => {
  test("line comments comment selected lines at minimum indentation column", () => {
    const buffer = TextBuffer.fromString("line1\n  line2\n    line3");
    const selection = Selection.single(0, buffer.length - 1);
    const edit = run(toggleLineComments(buffer, selection, "//"));
    expect(apply(buffer, edit)).toBe("// line1\n//   line2\n//     line3");
    expect(edit.selection).toEqual(selection.map(edit.transaction.desc()));
  });

  test("line comments uncomment using zero or one margin consistently", () => {
    const buffer = TextBuffer.fromString("// line1\n  //line2\n    // line3");
    const selection = Selection.single(0, buffer.length - 1);
    const edit = run(toggleLineComments(buffer, selection, "//"));
    expect(apply(buffer, edit)).toBe(" line1\n  line2\n     line3");
    expect(edit.selection).toEqual(selection.map(edit.transaction.desc()));
  });

  test("line comments skip blank lines and default token is hash", () => {
    const buffer = TextBuffer.fromString("first\n\n  third");
    const edit = run(toggleLineComments(buffer, Selection.single(0, buffer.length)));
    expect(apply(buffer, edit)).toBe("# first\n\n#   third");
    const commented = TextBuffer.fromString("// first\n\n    // second\n\n  // third");
    const uncomment = run(toggleLineComments(commented, Selection.single(0, commented.length), "//"));
    expect(apply(commented, uncomment)).toBe("first\n\n    second\n\n  third");
  });

  test("block comments wrap original content and keep selection inside delimiters", () => {
    const buffer = TextBuffer.fromString("1\n2\n3");
    const selection = Selection.single(0, buffer.length);
    const edit = run(toggleBlockComments(buffer, selection, [{ start: "/*", end: "*/" }]));
    expect(apply(buffer, edit)).toBe("/* 1\n2\n3 */");
    expect(edit.selection.ranges).toEqual([SelectionRange.create(3, 8)]);
  });

  test("block comments unwrap existing comments and map selection back", () => {
    const buffer = TextBuffer.fromString("/* 1\n2\n3 */");
    const selection = Selection.single(0, buffer.length);
    const edit = run(toggleBlockComments(buffer, selection));
    expect(apply(buffer, edit)).toBe("1\n2\n3");
    expect(edit.selection).toEqual(selection.map(edit.transaction.desc()));
  });

  test("block comments leave whitespace-only selection unchanged", () => {
    const buffer = TextBuffer.fromString("   ");
    const selection = Selection.single(0, buffer.length);
    const edit = run(toggleBlockComments(buffer, selection));
    expect(apply(buffer, edit)).toBe("   ");
    expect(edit.selection).toEqual(selection);
  });

  test("block token sort prefers longest token", () => {
    const buffer = TextBuffer.fromString("/** x **/");
    const edit = run(toggleBlockComments(buffer, Selection.single(0, buffer.length), [
      { start: "/*", end: "*/" },
      { start: "/**", end: "**/" },
    ]));
    expect(apply(buffer, edit)).toBe("x");
  });

  test("multiple selections and invalid tokens", () => {
    const buffer = TextBuffer.fromString("a\nb");
    const selection = Selection.create([SelectionRange.point(0), SelectionRange.point(2)], { normalize: false });
    const edit = run(toggleLineComments(buffer, selection, "//"));
    expect(apply(buffer, edit)).toBe("// a\n// b");
    expect(() => run(toggleLineComments(buffer, selection, ""))).toThrow(/invalid comment token/);
    expect(() => run(toggleBlockComments(buffer, selection, [{ start: "", end: "*/" }]))).toThrow(/invalid comment token/);
    expect(() => run(toggleBlockComments(buffer, selection, [{ start: "/*", end: "" }]))).toThrow(/invalid comment token/);
  });
});
