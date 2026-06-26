import { describe, expect, test } from "vitest";
import { unsupportedQueryResponses } from "./backend-xterm";

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
