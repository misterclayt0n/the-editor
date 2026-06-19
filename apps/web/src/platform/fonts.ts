import type { FontFileLoader } from "../fonts/dom";

type FontBridge = {
  readonly readBinaryFile?: (path: string) => Promise<ArrayBuffer | Uint8Array>;
};

const toArrayBuffer = (value: ArrayBuffer | Uint8Array): ArrayBuffer => {
  if (value instanceof ArrayBuffer) return value;
  return new Uint8Array(value).buffer;
};

export function createFontFileLoader(): FontFileLoader | undefined {
  const bridge = (window as unknown as { readonly theEditorFs?: FontBridge }).theEditorFs;
  if (bridge?.readBinaryFile === undefined) return undefined;
  return async (path) => toArrayBuffer(await bridge.readBinaryFile!(path));
}
