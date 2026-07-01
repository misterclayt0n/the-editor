/**
 * ExtensionTranspiler wiring: the one bundler-specific line the capability
 * needs. `createEsbuildTranspiler` (in @the-editor/extensions) is plain
 * cross-platform code; it just needs a URL to the wasm binary when running
 * in a browser/Electron-renderer context. Vite resolves that URL for us —
 * this file exists only so that resolution has exactly one home.
 */
import { createEsbuildTranspiler, type ExtensionTranspiler } from "@the-editor/extensions";
// Vite's `?url` suffix returns a resolved, fetchable URL for the asset
// instead of trying to bundle/parse it.
import esbuildWasmUrl from "esbuild-wasm/esbuild.wasm?url";

export function createExtensionTranspiler(): ExtensionTranspiler {
  return createEsbuildTranspiler({ wasmURL: esbuildWasmUrl });
}
