import { describe, expect, test } from "vitest";
import { clearTerminalSearch, resetTerminalSearchSession, unsupportedQueryResponses, type SearchAddon } from "./backend-xterm";

const theme = {
  background: "#101010",
  foreground: "#d4d4d4",
  cursor: "#69b1ff",
  black: "#171717",
  red: "#ff2e3f",
  green: "#0dbe4e",
  yellow: "#ffca00",
  blue: "#009fff",
  magenta: "#e130ac",
  cyan: "#08c0ef",
  white: "#bcbcbc",
  brightBlack: "#171717",
  brightRed: "#ff2e3f",
  brightGreen: "#86c427",
  brightYellow: "#ffca00",
  brightBlue: "#009fff",
  brightMagenta: "#e130ac",
  brightCyan: "#08c0ef",
  brightWhite: "#bcbcbc",
  brightForeground: "#d4d4d4",
  dimForeground: "#636363",
  dimBlack: "#101010",
  dimRed: "#a31d28",
  dimGreen: "#087a32",
  dimYellow: "#a38300",
  dimBlue: "#0066a3",
  dimMagenta: "#8f1f6e",
  dimCyan: "#057a99",
  dimWhite: "#636363",
};

describe("unsupportedQueryResponses", () => {
  test("answers OSC foreground, background, cursor, and indexed color queries", () => {
    const response = unsupportedQueryResponses("\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\\x1b]4;2;?;259;?\x1b\\", theme);

    expect(response).toBe(
      "\x1b]10;rgb:d4d4/d4d4/d4d4\x1b\\" +
        "\x1b]11;rgb:1010/1010/1010\x1b\\" +
        "\x1b]12;rgb:6969/b1b1/ffff\x1b\\" +
        "\x1b]4;2;rgb:0d0d/bebe/4e4e\x1b\\" +
        "\x1b]4;259;rgb:1010/1010/1010\x1b\\",
    );
  });
});

describe("clearTerminalSearch", () => {
  test("clears search decorations, active selection, and repaints visible rows", () => {
    const calls: string[] = [];
    const staleDecoration = {
      remove: () => calls.push("removeDecoration"),
    } as unknown as Element;
    const term = {
      rows: 3,
      element: {
        querySelectorAll: (selector: string) => {
          calls.push(`query:${selector}`);
          return [staleDecoration];
        },
      } as unknown as HTMLElement,
      select: (col: number, row: number, length: number) => calls.push(`select:${col}-${row}-${length}`),
      clearSelection: () => calls.push("clearSelection"),
      refresh: (start: number, end: number) => calls.push(`refresh:${start}-${end}`),
    };
    const addons = {
      search: {
        clearActiveDecoration: () => calls.push("clearActiveDecoration"),
        clearDecorations: () => calls.push("clearDecorations"),
      },
    };

    clearTerminalSearch(term, addons);

    expect(calls).toEqual([
      "clearActiveDecoration",
      "clearDecorations",
      "select:0-0-0",
      "clearSelection",
      "query:.xterm-find-result-decoration, .xterm-find-active-result-decoration",
      "removeDecoration",
      "refresh:0-2",
    ]);
  });

  test("resets the search addon when closing a search session", () => {
    const calls: string[] = [];
    const nextSearch = {
      clearActiveDecoration: () => calls.push("next.clearActiveDecoration"),
      clearDecorations: () => calls.push("next.clearDecorations"),
      dispose: () => calls.push("next.dispose"),
    } as unknown as SearchAddon;
    const addons = {
      search: {
        clearActiveDecoration: () => calls.push("current.clearActiveDecoration"),
        clearDecorations: () => calls.push("current.clearDecorations"),
        dispose: () => calls.push("current.dispose"),
      } as unknown as SearchAddon,
    };
    const term = {
      rows: 4,
      element: undefined,
      select: (col: number, row: number, length: number) => calls.push(`select:${col}-${row}-${length}`),
      clearSelection: () => calls.push("clearSelection"),
      refresh: (start: number, end: number) => calls.push(`refresh:${start}-${end}`),
      loadAddon: (addon: SearchAddon) => calls.push(addon === nextSearch ? "loadNextSearch" : "loadUnknown"),
    };

    resetTerminalSearchSession(term, addons, () => nextSearch);

    expect(addons.search).toBe(nextSearch);
    expect(calls).toEqual([
      "current.clearActiveDecoration",
      "current.clearDecorations",
      "select:0-0-0",
      "clearSelection",
      "refresh:0-3",
      "current.dispose",
      "loadNextSearch",
      "next.clearActiveDecoration",
      "next.clearDecorations",
      "select:0-0-0",
      "clearSelection",
      "refresh:0-3",
    ]);
  });
});
