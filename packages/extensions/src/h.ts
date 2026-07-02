/// <reference lib="dom" />
/**
 * `h`: a hyperscript builder for PLAIN DOM — the-editor's answer to "how do
 * extensions write UI without a framework". The mount contract everywhere in
 * the editor is an `HTMLElement` (panels, widgets, floats), so the native
 * component model is the DOM itself; `h` just makes it ergonomic, the same way
 * pi-tui's `Component`/`render()` is pi's own primitive rather than React.
 *
 * The extension transpiler compiles `.tsx` with `jsxFactory: "h"` /
 * `jsxFragment: "Fragment"`, so an extension can write JSX that produces real
 * DOM nodes with zero runtime dependency:
 *
 *   import { definePackage, h, Fragment } from "@the-editor/extensions";
 *   const view = (
 *     <div class="acme-panel" style={{ display: "flex", gap: "8px" }}>
 *       <button onclick={() => run()}>Run</button>
 *     </div>
 *   );
 *
 * Deliberately NOT a framework: no vdom, no diffing, no state. `h` returns the
 * node; you own it (mutate it, replace it, portal a real framework into it —
 * see the prebundling recipe in docs/EXTENSION-AUTHORING.md).
 */

export type HChild = Node | string | number | boolean | null | undefined | readonly HChild[];

export type HProps = Readonly<
  Record<string, unknown> & {
    readonly class?: string;
    readonly className?: string;
    readonly style?: string | Readonly<Record<string, string | number>>;
    /** Called with the created element — grab refs without a framework. */
    readonly ref?: (element: HTMLElement) => void;
  }
> | null;

/** A function component: any `(props & {children}) => Node`. */
export type HComponent = (props: Record<string, unknown>) => Node;

const appendChild = (parent: Node, child: HChild): void => {
  if (child === null || child === undefined || child === false || child === true) return;
  if (Array.isArray(child)) {
    for (const item of child) appendChild(parent, item);
    return;
  }
  parent.appendChild(
    child instanceof Node ? child : document.createTextNode(String(child as string | number)),
  );
};

const applyStyle = (
  element: HTMLElement,
  style: string | Readonly<Record<string, string | number>>,
): void => {
  if (typeof style === "string") {
    element.style.cssText = style;
    return;
  }
  for (const [key, value] of Object.entries(style)) {
    if (key.startsWith("--")) element.style.setProperty(key, String(value));
    else (element.style as unknown as Record<string, string>)[key] = String(value);
  }
};

export function h(
  tag: string | HComponent,
  props: HProps,
  ...children: readonly HChild[]
): HTMLElement | Node {
  if (typeof tag === "function") {
    // Function components get children through props, React-style.
    return tag({ ...(props ?? {}), children });
  }
  const element = document.createElement(tag);
  if (props !== null && props !== undefined) {
    for (const [key, value] of Object.entries(props)) {
      if (value === null || value === undefined || value === false) continue;
      if (key === "ref" && typeof value === "function") {
        (value as (el: HTMLElement) => void)(element);
      } else if (key === "class" || key === "className") {
        element.className = String(value);
      } else if (key === "style") {
        applyStyle(element, value as string | Record<string, string>);
      } else if (key.startsWith("on") && typeof value === "function") {
        // onclick / onClick / onInput… — lowercase past "on" is the event name.
        element.addEventListener(key.slice(2).toLowerCase(), value as EventListener);
      } else if (key === "dataset" && typeof value === "object") {
        for (const [dataKey, dataValue] of Object.entries(value as Record<string, unknown>)) {
          element.dataset[dataKey] = String(dataValue);
        }
      } else if (value === true) {
        element.setAttribute(key, "");
      } else {
        element.setAttribute(key, String(value));
      }
    }
  }
  for (const child of children) appendChild(element, child);
  return element;
}

/** `<>` — children collected into a DocumentFragment. */
export function Fragment(props: Record<string, unknown>): Node {
  const fragment = document.createDocumentFragment();
  appendChild(fragment, props.children as HChild);
  return fragment;
}
