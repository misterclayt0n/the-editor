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
        const maxLine = rows.reduce((max, row) => Math.max(max, row.docLine ?? 0), 0) + 1;
        const width = Math.max(2, String(maxLine).length);
        return rows.map((row) => ({
          id: `line:${row.surfaceRow}`,
          surfaceRow: row.surfaceRow,
          text: row.startsLogicalLine && row.docLine !== null ? String(row.docLine + 1).padStart(width, " ") : "",
          align: "right" as const,
          opacity: 0.58,
        }));
      },
    });
  };
}
