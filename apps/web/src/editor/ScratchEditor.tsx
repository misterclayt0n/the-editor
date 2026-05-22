import { EditorEngine } from "@the-editor/editor";
import { MonacoEditorEngineLive } from "@the-editor/editor-engine-monaco";
import { Effect, Fiber } from "effect";
import { useEffect, useRef } from "react";

const scratchContent = `// Scratch buffer
import { Effect } from "effect";

console.log("hello from The Editor");
`;

export function ScratchEditor() {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const fiber = Effect.gen(function* () {
      const engine = yield* EditorEngine;
      const model = yield* engine.createModel({
        uri: "inmemory://scratch.ts",
        languageId: "typescript",
        value: scratchContent,
      });
      const editor = yield* engine.mount({
        element: host,
        model,
        theme: "vs-dark",
      });
      yield* editor.focus;
      yield* Effect.never;
    }).pipe(Effect.scoped, Effect.provide(MonacoEditorEngineLive), Effect.runFork);

    return () => {
      Effect.runFork(Fiber.interrupt(fiber));
    };
  }, []);

  return <div ref={hostRef} className="scratch-editor" />;
}
