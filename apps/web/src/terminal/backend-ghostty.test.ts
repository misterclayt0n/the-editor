import { describe, expect, test } from "vitest";
import {
  clearTerminalSearch,
  findBufferMatches,
  KittyKeyboardTracker,
  resetTerminalSearchSession,
  terminalQueryResponse,
  TerminalStreamScanner,
  unsupportedQueryResponses,
  type SearchAddon,
} from "./backend-ghostty";

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

describe("TerminalStreamScanner", () => {
  test("dispatches OSC payloads terminated by BEL or ST", () => {
    const scanner = new TerminalStreamScanner();
    const events = scanner.feed("\x1b]9;build done\x07plain\x1b]777;notify;Title;Body\x1b\\");

    expect(events.oscs).toEqual([
      { code: 9, data: "build done" },
      { code: 777, data: "notify;Title;Body" },
    ]);
  });

  test("reassembles OSC sequences split across PTY chunks", () => {
    const scanner = new TerminalStreamScanner();
    const first = scanner.feed("\x1b]52;c;aGVs");
    const second = scanner.feed("bG8=\x07");

    expect(first.oscs).toEqual([]);
    expect(second.oscs).toEqual([{ code: 52, data: "c;aGVsbG8=" }]);
  });

  test("counts bare bells but not the BEL that terminates an OSC", () => {
    const scanner = new TerminalStreamScanner();
    const events = scanner.feed("ding\x07\x1b]0;my title\x07more\x07");

    expect(events.bells).toBe(2);
    expect(events.oscs).toEqual([{ code: 0, data: "my title" }]);
  });

  test("ignores bells inside DCS strings", () => {
    const scanner = new TerminalStreamScanner();
    const events = scanner.feed("\x1bPq\x07payload\x1b\\after\x07");

    expect(events.bells).toBe(1);
  });

  test("keeps scanning after a split ST terminator", () => {
    const scanner = new TerminalStreamScanner();
    scanner.feed("\x1b]9;hello\x1b");
    const events = scanner.feed("\\\x07");

    expect(events.oscs).toEqual([{ code: 9, data: "hello" }]);
    expect(events.bells).toBe(1);
  });

  test("surfaces query CSIs and drops SGR noise", () => {
    const scanner = new TerminalStreamScanner();
    const events = scanner.feed("\x1b[38;5;10mtext\x1b[?u\x1b[c\x1b[?2026$p\x1b[?25l");

    expect(events.csis).toEqual([
      { body: "?", final: "u" },
      { body: "", final: "c" },
      { body: "?2026$", final: "p" },
      { body: "?25", final: "l" },
    ]);
  });

  test("reassembles a CSI split across chunks", () => {
    const scanner = new TerminalStreamScanner();
    const first = scanner.feed("\x1b[?20");
    const second = scanner.feed("04$p");

    expect(first.csis).toEqual([]);
    expect(second.csis).toEqual([{ body: "?2004$", final: "p" }]);
  });
});

describe("KittyKeyboardTracker", () => {
  test("tracks push, set, and pop on the main screen", () => {
    const tracker = new KittyKeyboardTracker();
    expect(tracker.flags).toBe(0);

    tracker.apply({ body: ">5", final: "u" });
    expect(tracker.flags).toBe(5);

    tracker.apply({ body: "=2;2", final: "u" });
    expect(tracker.flags).toBe(7);

    tracker.apply({ body: "=1;3", final: "u" });
    expect(tracker.flags).toBe(6);

    tracker.apply({ body: "<1", final: "u" });
    expect(tracker.flags).toBe(0);
  });

  test("keeps separate stacks per screen", () => {
    const tracker = new KittyKeyboardTracker();
    tracker.apply({ body: ">1", final: "u" });
    tracker.apply({ body: "?1049", final: "h" });
    expect(tracker.flags).toBe(0);

    tracker.apply({ body: ">15", final: "u" });
    expect(tracker.flags).toBe(15);

    tracker.apply({ body: "?1049", final: "l" });
    expect(tracker.flags).toBe(1);
  });

  test("never pops the base entry", () => {
    const tracker = new KittyKeyboardTracker();
    tracker.apply({ body: "<8", final: "u" });
    expect(tracker.flags).toBe(0);
  });
});

describe("terminalQueryResponse", () => {
  const context = {
    getMode: (mode: number) => mode === 2004,
    kittyFlags: () => 5,
  };

  test("answers DA1, DA2, and the kitty keyboard query", () => {
    expect(terminalQueryResponse({ body: "", final: "c" }, context)).toBe("\x1b[?62;22c");
    expect(terminalQueryResponse({ body: "0", final: "c" }, context)).toBe("\x1b[?62;22c");
    expect(terminalQueryResponse({ body: ">", final: "c" }, context)).toBe("\x1b[>1;10;0c");
    expect(terminalQueryResponse({ body: "?", final: "u" }, context)).toBe("\x1b[?5u");
  });

  test("answers DECRQM from emulator mode state", () => {
    expect(terminalQueryResponse({ body: "?2004$", final: "p" }, context)).toBe("\x1b[?2004;1$y");
    expect(terminalQueryResponse({ body: "?2026$", final: "p" }, context)).toBe("\x1b[?2026;2$y");
  });

  test("ignores non-query sequences", () => {
    expect(terminalQueryResponse({ body: "?25", final: "l" }, context)).toBeNull();
    expect(terminalQueryResponse({ body: ">1", final: "u" }, context)).toBeNull();
    expect(terminalQueryResponse({ body: "?abc$", final: "p" }, context)).toBeNull();
  });
});

describe("findBufferMatches", () => {
  const lines = ["Hello world", "no match here?", "hello HELLO", ""];
  const lineText = (row: number) => lines[row] ?? "";

  test("finds case-insensitive literal matches in row/column order", () => {
    expect(findBufferMatches(lineText, lines.length, "hello")).toEqual([
      { row: 0, col: 0, length: 5 },
      { row: 2, col: 0, length: 5 },
      { row: 2, col: 6, length: 5 },
    ]);
  });

  test("returns nothing for an empty query", () => {
    expect(findBufferMatches(lineText, lines.length, "")).toEqual([]);
  });

  test("caps the number of matches", () => {
    const matches = findBufferMatches(() => "aaaa", 100, "a", 10);
    expect(matches).toHaveLength(10);
  });
});

describe("clearTerminalSearch", () => {
  test("clears the search state, the selection, and repaints", () => {
    const calls: string[] = [];
    const term = {
      rows: 3,
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

    expect(calls).toEqual(["clearActiveDecoration", "clearDecorations", "clearSelection", "refresh:0-2"]);
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
      clearSelection: () => calls.push("clearSelection"),
      refresh: (start: number, end: number) => calls.push(`refresh:${start}-${end}`),
      loadAddon: (addon: unknown) => calls.push(addon === nextSearch ? "loadNextSearch" : "loadUnknown"),
    };

    resetTerminalSearchSession(term, addons, () => nextSearch);

    expect(addons.search).toBe(nextSearch);
    expect(calls).toEqual([
      "current.clearActiveDecoration",
      "current.clearDecorations",
      "clearSelection",
      "refresh:0-3",
      "current.dispose",
      "loadNextSearch",
      "next.clearActiveDecoration",
      "next.clearDecorations",
      "clearSelection",
      "refresh:0-3",
    ]);
  });
});
