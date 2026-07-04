import { describe, expect, test } from "vitest";
import { ContributionId } from "@the-editor/kernel";
import { PanelId, registerSurfaceFallback, SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import { parseChord, parseKeys, resolveKey, type KeyBindingContribution } from "../src/index";

const Editor = SurfaceKind("editor");
const Tree = SurfaceKind("tree");

let n = 0;
const binding = (
  keys: string,
  command: string,
  scope = SurfaceScope.global,
  when?: KeyBindingContribution["when"],
): KeyBindingContribution => ({
  id: ContributionId(`b${n++}`),
  chords: parseKeys(keys),
  command,
  scope,
  ...(when !== undefined ? { when } : {}),
});

const editorFocus = { id: PanelId("p1"), kind: Editor };

describe("resolver", () => {
  test("matches a single chord", () => {
    const result = resolveKey({
      bindings: [binding("ctrl+s", "save")],
      focus: editorFocus,
      pending: [],
      chord: parseChord("ctrl+s"),
    });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "save" } });
  });

  test("unbound chords resolve to none", () => {
    const result = resolveKey({
      bindings: [binding("ctrl+s", "save")],
      focus: editorFocus,
      pending: [],
      chord: parseChord("x"),
    });
    expect(result).toEqual({ _tag: "none" });
  });

  test("multi-chord sequences go pending, then match", () => {
    const bindings = [binding("ctrl+w h", "focus.left")];
    const first = resolveKey({
      bindings,
      focus: editorFocus,
      pending: [],
      chord: parseChord("ctrl+w"),
    });
    expect(first._tag).toBe("pending");
    const second = resolveKey({
      bindings,
      focus: editorFocus,
      pending: first._tag === "pending" ? first.chords : [],
      chord: parseChord("h"),
    });
    expect(second).toMatchObject({ _tag: "match", binding: { command: "focus.left" } });
  });

  test("a pending prefix that dead-ends resolves to none and reports the cancelled sequence", () => {
    const bindings = [binding("ctrl+w h", "focus.left")];
    const result = resolveKey({
      bindings,
      focus: editorFocus,
      pending: parseKeys("ctrl+w"),
      chord: parseChord("z"),
    });
    expect(result).toEqual({ _tag: "none", cancelled: parseKeys("ctrl+w z") });
  });

  test("prefixes win over exact matches (leader keys are never consumed early)", () => {
    const bindings = [binding("ctrl+w", "kill"), binding("ctrl+w h", "focus.left")];
    const result = resolveKey({
      bindings,
      focus: editorFocus,
      pending: [],
      chord: parseChord("ctrl+w"),
    });
    expect(result._tag).toBe("pending");
  });

  test("scope filters candidates: kind bindings need matching focus", () => {
    const bindings = [binding("j", "tree.next", SurfaceScope.surface(Tree))];
    const inEditor = resolveKey({
      bindings,
      focus: editorFocus,
      pending: [],
      chord: parseChord("j"),
    });
    expect(inEditor).toEqual({ _tag: "none" });
    const inTree = resolveKey({
      bindings,
      focus: { id: PanelId("p2"), kind: Tree },
      pending: [],
      chord: parseChord("j"),
    });
    expect(inTree).toMatchObject({ _tag: "match", binding: { command: "tree.next" } });
  });

  test("specificity: panel beats kind beats global", () => {
    const bindings = [
      binding("x", "global.x"),
      binding("x", "editor.x", SurfaceScope.surface(Editor)),
      binding("x", "panel.x", SurfaceScope.panel(editorFocus.id)),
    ];
    const result = resolveKey({
      bindings,
      focus: editorFocus,
      pending: [],
      chord: parseChord("x"),
    });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "panel.x" } });
  });

  test("later registration shadows earlier within a layer (user config wins)", () => {
    const bindings = [binding("ctrl+p", "default.palette"), binding("ctrl+p", "my.palette")];
    const result = resolveKey({
      bindings,
      focus: editorFocus,
      pending: [],
      chord: parseChord("ctrl+p"),
    });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "my.palette" } });
  });

  test("when facts are filters, not specificity boosters", () => {
    const bindings = [
      binding("esc", "vim.normal", SurfaceScope.surface(Editor), { "input.mode": "insert" }),
      binding("esc", "later.ungated", SurfaceScope.surface(Editor)),
    ];
    const insert = resolveKey({
      bindings,
      focus: editorFocus,
      pending: [],
      chord: parseChord("esc"),
      facts: { "input.mode": "insert" },
    });
    expect(insert).toMatchObject({ _tag: "match", binding: { command: "later.ungated" } });
    const normal = resolveKey({
      bindings,
      focus: editorFocus,
      pending: [],
      chord: parseChord("esc"),
      facts: { "input.mode": "normal" },
    });
    expect(normal).toMatchObject({ _tag: "match", binding: { command: "later.ungated" } });
  });

  test("failed facts exclude a binding", () => {
    const result = resolveKey({
      bindings: [binding("i", "vim.insert", SurfaceScope.surface(Editor), { "input.mode": "normal" })],
      focus: editorFocus,
      pending: [],
      chord: parseChord("i"),
      facts: { "input.mode": "insert" },
    });
    expect(result).toEqual({ _tag: "none" });
  });

  test("nothing focused: only global bindings resolve", () => {
    const bindings = [
      binding("x", "editor.x", SurfaceScope.surface(Editor)),
      binding("x", "global.x"),
    ];
    const result = resolveKey({ bindings, focus: undefined, pending: [], chord: parseChord("x") });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "global.x" } });
  });
});

describe("surface fallback", () => {
  test("a child surface inherits bindings scoped to its parent", () => {
    const Parent = SurfaceKind("keymap/fallback/parent");
    const Child = SurfaceKind("keymap/fallback/child");
    registerSurfaceFallback(Child, Parent);
    const focus = { id: PanelId("p1"), kind: Child };
    const result = resolveKey({
      bindings: [binding("a", "parent.cmd", SurfaceScope.surface(Parent))],
      focus,
      pending: [],
      chord: parseChord("a"),
    });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "parent.cmd" } });
  });

  test("a child-specific binding can shadow the inherited one", () => {
    const Parent = SurfaceKind("keymap/fallback/parent2");
    const Child = SurfaceKind("keymap/fallback/child2");
    registerSurfaceFallback(Child, Parent);
    const focus = { id: PanelId("p1"), kind: Child };
    const result = resolveKey({
      bindings: [
        binding("a", "parent.cmd", SurfaceScope.surface(Parent)),
        binding("a", "child.cmd", SurfaceScope.surface(Child)),
      ],
      focus,
      pending: [],
      chord: parseChord("a"),
    });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "child.cmd" } });
  });
});
