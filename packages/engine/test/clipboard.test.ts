import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  ClipboardCommandFailedError,
  ClipboardError,
  ClipboardIoError,
  ClipboardMissingStdoutError,
  ClipboardPlatformError,
  ClipboardReadingNotSupportedError,
  ClipboardStdinWriteFailedError,
  ClipboardUtf8Error,
  NoClipboard,
} from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);

describe("clipboard boundary", () => {
  test("NoClipboard has name none", () => {
    expect(new NoClipboard().name).toBe("none");
  });

  test("NoClipboard setContents succeeds for both clipboard types", () => {
    const clipboard = new NoClipboard();
    expect(run(clipboard.setContents("x", "clipboard"))).toBeUndefined();
    expect(run(clipboard.setContents("x", "selection"))).toBeUndefined();
  });

  test("NoClipboard getContents fails with reading unsupported", () => {
    const error = run(Effect.flip(new NoClipboard().getContents("clipboard")));
    expect(error).toBeInstanceOf(ClipboardReadingNotSupportedError);
  });

  test("error classes are ClipboardError instances", () => {
    expect(new ClipboardIoError(new Error("x"))).toBeInstanceOf(ClipboardError);
    expect(new ClipboardUtf8Error(new Error("x"))).toBeInstanceOf(ClipboardError);
    expect(new ClipboardCommandFailedError()).toBeInstanceOf(ClipboardError);
    expect(new ClipboardStdinWriteFailedError()).toBeInstanceOf(ClipboardError);
    expect(new ClipboardMissingStdoutError()).toBeInstanceOf(ClipboardError);
    expect(new ClipboardReadingNotSupportedError()).toBeInstanceOf(ClipboardError);
    expect(new ClipboardPlatformError("x")).toBeInstanceOf(ClipboardError);
  });
});
