import { Effect } from "effect";

export type ClipboardType = "clipboard" | "selection";

export class ClipboardError extends Error {}

export class ClipboardIoError extends ClipboardError {
  constructor(override readonly cause: unknown) {
    super("clipboard I/O error");
  }
}

export class ClipboardUtf8Error extends ClipboardError {
  constructor(override readonly cause: unknown) {
    super("could not convert provider output to UTF-8");
  }
}

export class ClipboardCommandFailedError extends ClipboardError {
  constructor() {
    super("clipboard provider command failed");
  }
}

export class ClipboardStdinWriteFailedError extends ClipboardError {
  constructor() {
    super("failed to write to clipboard provider's stdin");
  }
}

export class ClipboardMissingStdoutError extends ClipboardError {
  constructor() {
    super("clipboard provider did not return any contents");
  }
}

export class ClipboardReadingNotSupportedError extends ClipboardError {
  constructor() {
    super("clipboard provider does not support reading");
  }
}

export class ClipboardPlatformError extends ClipboardError {
  constructor(readonly messageText: string) {
    super(`clipboard error: ${messageText}`);
  }
}

export interface ClipboardProvider {
  readonly name: string;
  getContents(clipboardType: ClipboardType): Effect.Effect<string, ClipboardError>;
  setContents(content: string, clipboardType: ClipboardType): Effect.Effect<void, ClipboardError>;
}

export class NoClipboard implements ClipboardProvider {
  readonly name = "none";
  getContents(_clipboardType: ClipboardType): Effect.Effect<string, ClipboardError> {
    return Effect.fail(new ClipboardReadingNotSupportedError());
  }
  setContents(
    _content: string,
    _clipboardType: ClipboardType,
  ): Effect.Effect<void, ClipboardError> {
    return Effect.void;
  }
}
