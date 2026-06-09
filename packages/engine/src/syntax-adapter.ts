import type { SyntaxText, SyntaxTextEdit } from "@the-editor/syntax";
import type { TextBuffer, Transaction } from "./text/index";

export function syntaxTextFromTextBuffer(buffer: TextBuffer): SyntaxText {
  return { length: buffer.length, sliceString: (from, to) => buffer.sliceString(from, to) };
}

export function syntaxTextEditsFromTransaction(
  transaction: Transaction,
): readonly SyntaxTextEdit[] {
  return transaction.changes.toLegacyReplacements().map((replacement) => ({
    start: replacement.from,
    end: replacement.to,
    text: replacement.text,
  }));
}
