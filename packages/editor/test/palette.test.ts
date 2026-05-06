import { describe, expect, it } from "vitest";
import { matchesCommandPaletteItem, scoreCommandPaletteItem } from "../src/palette.js";

describe("command palette scoring", () => {
  it("matches slash-separated specifiers", () => {
    const item = { title: ":reload default/ui", detail: "Reload ui target", keywords: ["reload", "default/ui"] };
    expect(matchesCommandPaletteItem("reload default/", item)).toBe(true);
    expect(scoreCommandPaletteItem("reload default/", item)).toBeGreaterThan(0);
  });

  it("matches fuzzy subsequences across separators", () => {
    const item = { title: ":reload default/ui", detail: "Reload ui target", keywords: ["reload", "default/ui"] };
    expect(matchesCommandPaletteItem("dfui", item)).toBe(true);
    expect(scoreCommandPaletteItem("dfui", item)).toBeGreaterThan(0);
  });
});
