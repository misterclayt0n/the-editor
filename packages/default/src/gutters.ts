import type { RenderFrame } from "@the-editor/contracts/runtime";
import type { EditorModule } from "@the-editor/editor";

export function defaultGutters(): EditorModule {
  return (editor) => {
    editor.gutters.registerProvider({
      id: "default.lineNumbers",
      title: "Line Numbers",
      lane: "lineNumbers",
      laneSpec: {
        id: "lineNumbers",
        order: 100,
        widthCells: 2,
        paddingRightPx: 22,
      },
      provide(ctx) {
        const rows = ctx.frame?.rows ?? [];
        const maxLine = Math.max(ctx.lineCount ?? 0, rows.reduce((max, row) => Math.max(max, row.docLine ?? 0), 0) + 1);
        const width = Math.max(2, String(maxLine).length);
        return rows.map((row) => ({
          id: `line:${row.surfaceRow}`,
          surfaceRow: row.surfaceRow,
          text: row.startsLogicalLine && row.docLine !== null ? String(row.docLine + 1).padStart(width, " ") : "",
          align: "right" as const,
          foreground: isActiveLine(ctx.frame, row.surfaceRow) ? "#e7b84a" : undefined,
          background: isActiveLine(ctx.frame, row.surfaceRow) ? "rgba(255, 255, 255, 0.045)" : undefined,
          opacity: isActiveLine(ctx.frame, row.surfaceRow) ? 0.95 : 0.58,
        }));
      },
    });
  };
}

function isActiveLine(frame: RenderFrame | undefined, surfaceRow: number): boolean {
  return frame?.cursors.some((cursor) => cursor.active && cursor.surfaceRow === surfaceRow) ?? false;
}
