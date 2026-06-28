import { describe, expect, test } from "vitest";
import { parseOsc9, parseOsc777 } from "./notifications.js";

describe("parseOsc9", () => {
  test("treats free text as a notification message", () => {
    expect(parseOsc9("Build finished")).toEqual({ body: "Build finished" });
    expect(parseOsc9("5 files changed")).toEqual({ body: "5 files changed" });
  });
  test("ignores ConEmu progress/control payloads (numeric first field)", () => {
    expect(parseOsc9("4;50")).toBeNull();
    expect(parseOsc9("1")).toBeNull();
  });
  test("ignores empty payloads", () => {
    expect(parseOsc9("   ")).toBeNull();
  });
});

describe("parseOsc777", () => {
  test("parses notify with title and body", () => {
    expect(parseOsc777("notify;Claude;needs your approval")).toEqual({
      title: "Claude",
      body: "needs your approval",
    });
  });
  test("body may itself contain semicolons", () => {
    expect(parseOsc777("notify;t;a;b;c")).toEqual({ title: "t", body: "a;b;c" });
  });
  test("title-only collapses to body", () => {
    expect(parseOsc777("notify;just a title")).toEqual({ body: "just a title" });
  });
  test("ignores non-notify subcommands", () => {
    expect(parseOsc777("something;else")).toBeNull();
  });
});
