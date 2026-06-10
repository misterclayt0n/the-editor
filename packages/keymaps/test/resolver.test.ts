import { describe, expect, test } from "vitest";
import { ContributionId } from "@the-editor/kernel";
import { PanelId, SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import { parseChord, parseKeys, resolveKey, type KeyBindingContribution } from "../src/index";

const Editor = SurfaceKind("editor");
const Tree = SurfaceKind("tree");

let n = 0;
const binding = (
  keys: string,
  command: string,
  scope = SurfaceScope.global,
): KeyBindingContribution => ({
  id: ContributionId(`b${n++}`),
  chords: parseKeys(keys),
  command,
  scope,
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
    const first = resolveKey({ bindings, focus: editorFocus, pending: [], chord: parseChord("ctrl+w") });
    expect(first._tag).toBe("pending");
    const second = resolveKey({
      bindings,
      focus: editorFocus,
      pending: first._tag === "pending" ? first.chords : [],
      chord: parseChord("h"),
    });
    expect(second).toMatchObject({ _tag: "match", binding: { command: "focus.left" } });
  });

  test("a pending prefix that dead-ends resolves to none", () => {
    const bindings = [binding("ctrl+w h", "focus.left")];
    const result = resolveKey({
      bindings,
      focus: editorFocus,
      pending: parseKeys("ctrl+w"),
      chord: parseChord("z"),
    });
    expect(result).toEqual({ _tag: "none" });
  });

  test("prefixes win over exact matches (leader keys are never consumed early)", () => {
    const bindings = [binding("ctrl+w", "kill"), binding("ctrl+w h", "focus.left")];
    const result = resolveKey({ bindings, focus: editorFocus, pending: [], chord: parseChord("ctrl+w") });
    expect(result._tag).toBe("pending");
  });

  test("scope filters candidates: kind bindings need matching focus", () => {
    const bindings = [binding("j", "tree.next", SurfaceScope.surface(Tree))];
    const inEditor = resolveKey({ bindings, focus: editorFocus, pending: [], chord: parseChord("j") });
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
    const result = resolveKey({ bindings, focus: editorFocus, pending: [], chord: parseChord("x") });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "panel.x" } });
  });

  test("later registration shadows earlier within a layer (user config wins)", () => {
    const bindings = [binding("ctrl+p", "default.palette"), binding("ctrl+p", "my.palette")];
    const result = resolveKey({ bindings, focus: editorFocus, pending: [], chord: parseChord("ctrl+p") });
    expect(result).toMatchObject({ _tag: "match", binding: { command: "my.palette" } });
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
