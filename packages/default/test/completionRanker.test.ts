import { describe, expect, it } from "vitest";
import type { CompletionItem } from "@the-editor/editor";
import { rankCompletionEntries } from "../src/completionRanker.js";

function entry(label: string, options: Partial<CompletionItem> = {}) {
  return {
    sourceId: "default.lsp",
    sourcePriority: 100,
    matchScore: 0,
    item: {
      id: label,
      label,
      filterText: options.filterText ?? label,
      sortText: options.sortText ?? label,
      detail: options.detail,
      labelDescription: options.labelDescription,
      payload: options.payload,
    } satisfies CompletionItem,
    view: {
      id: label,
      label,
      labelHighlights: [],
      detailHighlights: [],
    },
  };
}

describe("completionRanker", () => {
  it("prefers exact match over lower sortText prefix match", () => {
    const ranked = rankCompletionEntries("into", [
      entry("into_searcher", { sortText: "0000" }),
      entry("into", { sortText: "9999" }),
    ]);
    expect(ranked.map((candidate) => candidate.item.label)).toEqual(["into", "into_searcher"]);
  });

  it("lets prefix ties fall through to lsp sortText", () => {
    const ranked = rankCompletionEntries("pip", [
      entry("piped()", { filterText: "piped", sortText: "0001" }),
      entry("pipe()", { filterText: "pipe", sortText: "0002" }),
    ]);
    expect(ranked[0]?.item.label).toBe("piped()");
  });

  it("prefers lsp preselect when other ranking ties", () => {
    const ranked = rankCompletionEntries("map", [
      entry("map", { payload: { preselect: false } }),
      entry("map", { payload: { preselect: true }, sortText: "map" }),
    ]);
    expect(ranked[0]?.item.payload).toEqual({ preselect: true });
  });
});
