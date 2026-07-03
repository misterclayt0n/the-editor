/**
 * ClipboardPort: the renderer seam for native clipboard operations.
 * Terminals need the host bridge because the terminal's hidden input element
 * does not reliably participate in Chromium's default copy/paste pipeline.
 */

export type ClipboardTextReadOptions = {
  readonly maxBytes?: number;
};

export interface ClipboardPort {
  readText(options?: ClipboardTextReadOptions): Promise<string>;
  writeText(text: string): Promise<void>;
  readSelectionText?(options?: ClipboardTextReadOptions): Promise<string>;
  writeSelectionText?(text: string): Promise<void>;
  saveImageAsTempFile?(): Promise<string | null>;
}

const textEncoder = new TextEncoder();

const assertTextWithinLimit = (text: string, options?: ClipboardTextReadOptions): string => {
  if (options?.maxBytes === undefined) return text;
  if (textEncoder.encode(text).byteLength > options.maxBytes) {
    throw new Error("Clipboard text exceeds the terminal paste limit.");
  }
  return text;
};

const navigatorClipboardPort = (): ClipboardPort | undefined => {
  const clipboard = navigator.clipboard;
  if (clipboard === undefined) return undefined;
  return {
    readText: async (options) => assertTextWithinLimit(await clipboard.readText(), options),
    writeText: (text) => clipboard.writeText(text),
  };
};

declare global {
  interface Window {
    __testClipboardPort?: ClipboardPort;
    theEditorClipboard?: ClipboardPort;
  }
}

export function createClipboardPort(): ClipboardPort | undefined {
  const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
  if (isDev && window.__testClipboardPort !== undefined) return window.__testClipboardPort;
  if (window.theEditorClipboard !== undefined) return window.theEditorClipboard;
  return navigatorClipboardPort();
}
