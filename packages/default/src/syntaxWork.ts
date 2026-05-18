import { toDisposable, type EditorModule } from "@the-editor/editor";
import { getDefaultState } from "./state.js";

export function defaultSyntaxWork(): EditorModule {
  return (editor) => {
    let disposed = false;
    let running = false;

    const tick = () => {
      if (disposed || running) return;
      const panes = [...getDefaultState(editor).surfacePanes.values()];
      if (!panes.length) return;

      running = true;
      void editor.runtime.syntax.drainWork({ maxDocuments: 8, budgetMs: 8 })
        .then((result) => {
          if (disposed || result.advanced <= 0) return;
          const seen = new Set<number>();
          for (const pane of panes) {
            if (seen.has(pane.surfaceId)) continue;
            seen.add(pane.surfaceId);
            editor.workbench.invalidateSurface(pane.surfaceId, "content");
          }
        })
        .catch((error) => {
          console.error("syntax work failed", error);
        })
        .finally(() => {
          running = false;
        });
    };

    const interval = setInterval(tick, 50);
    tick();
    return toDisposable(() => {
      disposed = true;
      clearInterval(interval);
    });
  };
}
