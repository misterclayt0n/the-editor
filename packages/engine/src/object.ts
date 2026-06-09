import type { Syntax } from "@the-editor/syntax";
import { Selection, SelectionRange } from "./selection";
import type { TextBuffer } from "./text/buffer";

export type ObjectDirection = "forward" | "backward";
export type ObjectMotionMode = "move" | "extend";
type SyntaxNode = any;

function mapSelectionRanges(selection: Selection, f: (range: SelectionRange) => SelectionRange): Selection {
  return Selection.create(selection.ranges.map(f), { primaryIndex: selection.primaryIndex, normalize: false });
}
function flatMapSelectionRanges(selection: Selection, f: (range: SelectionRange, index: number) => readonly SelectionRange[]): Selection {
  const ranges: SelectionRange[] = [];
  let primaryIndex = 0;
  selection.ranges.forEach((range, index) => {
    if (index === selection.primaryIndex) primaryIndex = ranges.length;
    const next = f(range, index);
    ranges.push(...(next.length === 0 ? [range] : next));
  });
  return Selection.create(ranges, { primaryIndex, normalize: false });
}
function nodeStart(node: SyntaxNode): number { return node.startIndex; }
function nodeEnd(node: SyntaxNode): number { return node.endIndex; }
function nodeIsNamed(node: SyntaxNode): boolean { return typeof node.isNamed === "function" ? node.isNamed() : !!node.isNamed; }
function nodeNamedChildCount(node: SyntaxNode): number { return typeof node.namedChildCount === "function" ? node.namedChildCount() : node.namedChildCount ?? node.namedChildren?.length ?? nodeChildren(node).filter(nodeIsNamed).length; }
function nodeChildren(node: SyntaxNode): readonly SyntaxNode[] { return node.children ?? Array.from({ length: node.childCount ?? 0 }, (_, i) => node.child(i)).filter(Boolean); }
function nodeParent(node: SyntaxNode): SyntaxNode | undefined { return node.parent ?? undefined; }
function nextSibling(node: SyntaxNode): SyntaxNode | undefined { return typeof node.nextSibling === "function" ? node.nextSibling() : node.nextSibling; }
function prevSibling(node: SyntaxNode): SyntaxNode | undefined { return typeof node.previousSibling === "function" ? node.previousSibling() : typeof node.prevSibling === "function" ? node.prevSibling() : node.previousSibling ?? node.prevSibling; }
function nodeKey(node: SyntaxNode): string { const id = node.id; return typeof id === "number" || typeof id === "string" ? String(id) : `${node.startIndex}:${node.endIndex}:${node.type}:${node.startPosition?.row ?? 0}:${node.startPosition?.column ?? 0}`; }
function sameNode(a: SyntaxNode, b: SyntaxNode): boolean { return nodeKey(a) === nodeKey(b); }
function rangeFromNode(node: SyntaxNode, direction: ObjectDirection | SelectionRange["direction"]): SelectionRange { return SelectionRange.create(nodeStart(node), nodeEnd(node)).withDirection(direction); }
function nodeForRange(syntax: Syntax, _text: TextBuffer, range: SelectionRange): SyntaxNode | undefined {
  const from = range.from, to = range.to;
  const tree = (syntax as any).treeForByteRange?.(from, to);
  const treeRoot = typeof tree?.rootNode === "function" ? tree.rootNode() : tree?.rootNode;
  const root = treeRoot ?? syntax.rootNode();
  return (syntax as any).namedDescendantForByteRange?.(from, to) ?? root.namedDescendantForIndex?.(from, to) ?? (syntax as any).descendantForByteRange?.(from, to) ?? root.descendantForIndex?.(from, to);
}
function namedChildContaining(node: SyntaxNode, range: SelectionRange): SyntaxNode | undefined {
  return nodeChildren(node).find((child) => nodeIsNamed(child) && nodeStart(child) <= range.from && nodeEnd(child) >= range.to);
}
function nextNamedSibling(node: SyntaxNode, direction: ObjectDirection): SyntaxNode | undefined {
  let sib = direction === "forward" ? nextSibling(node) : prevSibling(node);
  while (sib) { if (nodeIsNamed(sib)) return sib; sib = direction === "forward" ? nextSibling(sib) : prevSibling(sib); }
  return undefined;
}
function nearestParentWithMultipleNamedChildren(node: SyntaxNode): SyntaxNode | undefined {
  let current = node;
  while (nodeParent(current)) { const parent = nodeParent(current)!; if (nodeNamedChildCount(parent) > 1) return parent; current = parent; }
  return undefined;
}
function namedChildRanges(node: SyntaxNode, direction: SelectionRange["direction"]): readonly SelectionRange[] {
  return nodeChildren(node).filter(nodeIsNamed).map((child) => rangeFromNode(child, direction));
}
function selectSibling(syntax: Syntax, text: TextBuffer, selection: Selection, direction: ObjectDirection): Selection {
  return mapSelectionRanges(selection, (range) => {
    let node = nodeForRange(syntax, text, range);
    if (!node) return range;
    while (node) { const sib = nextNamedSibling(node, direction); if (sib) return rangeFromNode(sib, direction); node = nodeParent(node); }
    return range;
  });
}
function findParentStart(node: SyntaxNode): SyntaxNode | undefined {
  const start = nodeStart(node);
  let current: SyntaxNode | undefined = node;
  while (current && (nodeStart(current) >= start || !nodeIsNamed(current))) current = nodeParent(current);
  return current;
}

export function expandSelection(args: { readonly syntax: Syntax; readonly text: TextBuffer; readonly selection: Selection }): Selection {
  return mapSelectionRanges(args.selection, (range) => {
    let node = nodeForRange(args.syntax, args.text, range);
    if (!node) return range;
    while (nodeStart(node) === range.from && nodeEnd(node) === range.to) {
      const parent = nodeParent(node); if (!parent) break; node = parent;
    }
    return rangeFromNode(node, range.direction);
  });
}
export function shrinkSelection(args: { readonly syntax: Syntax; readonly text: TextBuffer; readonly selection: Selection }): Selection {
  return mapSelectionRanges(args.selection, (range) => {
    const node = nodeForRange(args.syntax, args.text, range); if (!node) return range;
    const child = namedChildContaining(node, range); return child ? rangeFromNode(child, range.direction) : range;
  });
}
export function selectNextSibling(args: { readonly syntax: Syntax; readonly text: TextBuffer; readonly selection: Selection }): Selection { return selectSibling(args.syntax, args.text, args.selection, "forward"); }
export function selectPrevSibling(args: { readonly syntax: Syntax; readonly text: TextBuffer; readonly selection: Selection }): Selection { return selectSibling(args.syntax, args.text, args.selection, "backward"); }
export function selectAllSiblings(args: { readonly syntax: Syntax; readonly text: TextBuffer; readonly selection: Selection }): Selection {
  return flatMapSelectionRanges(args.selection, (range) => { const node = nodeForRange(args.syntax, args.text, range); const parent = node && nearestParentWithMultipleNamedChildren(node); return parent ? namedChildRanges(parent, range.direction) : [range]; });
}
export function selectAllChildren(args: { readonly syntax: Syntax; readonly text: TextBuffer; readonly selection: Selection }): Selection {
  return flatMapSelectionRanges(args.selection, (range) => { const node = nodeForRange(args.syntax, args.text, range); return node ? namedChildRanges(node, range.direction) : [range]; });
}
export function moveParentEdge(args: { readonly syntax: Syntax; readonly text: TextBuffer; readonly selection: Selection; readonly direction: ObjectDirection; readonly mode: ObjectMotionMode }): Selection {
  return mapSelectionRanges(args.selection, (range) => {
    const docLen = args.text.length;
    let node = (args.syntax as any).namedDescendantForByteRange?.(range.from, range.to) ?? args.syntax.rootNode().namedDescendantForIndex?.(range.from, range.to);
    if (!node) return range;
    let head: number;
    if (args.direction === "forward") head = nodeEnd(node);
    else { head = nodeStart(node); if (head === range.head) { node = findParentStart(node) ?? node; head = nodeStart(node); } }
    if (args.mode === "move") {
      if (head >= docLen) return SelectionRange.point(docLen);
      head = Math.min(head, docLen); const other = Math.min(head + 1, docLen);
      return range.direction === "forward" ? SelectionRange.create(head, other) : SelectionRange.create(other, head);
    }
    if (head >= range.anchor) head += 1;
    return SelectionRange.create(Math.min(range.anchor, docLen), Math.min(head, docLen));
  });
}
