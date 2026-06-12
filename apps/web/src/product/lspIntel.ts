/**
 * LSP-backed completion and hover, registered as ORDINARY intel providers —
 * the same registry a user's snippet package or custom oracle would use.
 * The host seam carries the manager's per-document requests.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import {
  CompletionProviders,
  HoverProviders,
  SignatureProviders,
  type CompletionContext,
  type CompletionResult,
  type HoverContext,
  type HoverResult,
  type SignatureContext,
  type SignatureResult,
} from "../intel/types";

export class HostLspIntel extends Context.Tag("the-editor/web/HostLspIntel")<
  HostLspIntel,
  {
    readonly completion: (
      context: CompletionContext,
    ) => Promise<CompletionResult | null>;
    readonly hover: (context: HoverContext) => Promise<HoverResult | null>;
    readonly diagnosticsHover: (context: HoverContext) => HoverResult | null;
    readonly triggerCharacters: (docId: string) => readonly string[];
    readonly signature: (context: SignatureContext) => Promise<SignatureResult | null>;
    readonly signatureTriggerCharacters: (docId: string) => readonly string[];
    readonly signatureRetriggerCharacters: (docId: string) => readonly string[];
  }
>() {}

export const lspIntel = definePackage("the-editor/lsp-intel", () =>
  Effect.gen(function* () {
    const completions = yield* CompletionProviders;
    const hovers = yield* HoverProviders;
    const signatures = yield* SignatureProviders;
    const host = yield* HostLspIntel;

    yield* completions.register({
      id: "lsp",
      triggerCharacters: (docId) => host.triggerCharacters(docId),
      provide: (context) => host.completion(context),
    });
    yield* signatures.register({
      id: "lsp",
      triggerCharacters: (docId) => host.signatureTriggerCharacters(docId),
      retriggerCharacters: (docId) => host.signatureRetriggerCharacters(docId),
      provide: (context) => host.signature(context),
    });
    // Diagnostics card first; the docs card stacks beneath it.
    yield* hovers.register({
      id: "lsp-diagnostics",
      provide: (context) => host.diagnosticsHover(context),
    });
    yield* hovers.register({
      id: "lsp",
      provide: (context) => host.hover(context),
    });
  }),
);
