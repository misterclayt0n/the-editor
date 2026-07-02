// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { Fragment, h } from "../src/h.js";

describe("h (native DOM hyperscript)", () => {
  it("creates elements with class, attributes, and text children", () => {
    const el = h("div", { class: "panel", id: "x" }, "hello ", "world") as HTMLElement;
    expect(el.tagName).toBe("DIV");
    expect(el.className).toBe("panel");
    expect(el.id).toBe("x");
    expect(el.textContent).toBe("hello world");
  });

  it("supports className alias, style objects (incl. CSS vars), and style strings", () => {
    const styled = h("span", {
      className: "s",
      style: { display: "flex", "--gap": "4px", flexGrow: 1 },
    }) as HTMLElement;
    expect(styled.className).toBe("s");
    expect(styled.style.display).toBe("flex");
    expect(styled.style.getPropertyValue("--gap")).toBe("4px");
    const stringStyled = h("span", { style: "color: red" }) as HTMLElement;
    expect(stringStyled.style.color).toBe("red");
  });

  it("wires on* props as event listeners", () => {
    let clicks = 0;
    const button = h("button", { onclick: () => clicks++ }, "go") as HTMLElement;
    button.dispatchEvent(new Event("click"));
    expect(clicks).toBe(1);
  });

  it("flattens nested child arrays and skips null/undefined/booleans", () => {
    const el = h("ul", null, [h("li", null, "a"), null, [h("li", null, "b"), false]]) as HTMLElement;
    expect(el.querySelectorAll("li").length).toBe(2);
    expect(el.textContent).toBe("ab");
  });

  it("boolean attributes, dataset, and ref", () => {
    let got: HTMLElement | null = null;
    const el = h("input", {
      disabled: true,
      dataset: { kind: "search" },
      ref: (node) => (got = node),
    }) as HTMLElement;
    expect(el.hasAttribute("disabled")).toBe(true);
    expect(el.dataset.kind).toBe("search");
    expect(got).toBe(el);
  });

  it("calls function components with props + children", () => {
    const Row = (props: Record<string, unknown>) =>
      h("div", { class: `row-${props.tone as string}` }, props.children as never);
    const el = h(Row, { tone: "warm" }, "inside") as HTMLElement;
    expect(el.className).toBe("row-warm");
    expect(el.textContent).toBe("inside");
  });

  it("Fragment collects children into a DocumentFragment", () => {
    const frag = h(Fragment as never, null, h("i", null, "a"), h("i", null, "b"));
    const host = document.createElement("div");
    host.appendChild(frag);
    expect(host.querySelectorAll("i").length).toBe(2);
  });
});
