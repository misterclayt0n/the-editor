/**
 * GeometryModel: the single owner of line↔pixel mapping once block widgets
 * or soft wrap exist. Every consumer of vertical geometry (layout, hit
 * testing, caret reveal, scroll anchoring, window math) must go through
 * this model — line-arithmetic shortcuts break the moment one widget
 * reserves rows or one line wraps.
 *
 * Widgets reserve whole rows after a line; soft wrap gives a line more than
 * one row of its own (`rowsPerLine`). The model keeps prefix sums so
 * lineTop/lineAt are O(log n), degenerating to plain arithmetic with no
 * widgets and no wrap.
 */

export type GeometryWidgetRow = {
  /** Widget rows are inserted after this line's row. */
  readonly afterLine: number;
  readonly heightRows: number;
};

export class GeometryModel {
  /** Widgets sorted by afterLine. */
  private readonly widgets: ReadonlyArray<GeometryWidgetRow>;
  /** prefixRows[i] = total widget rows of widgets[0..i]. */
  private readonly prefixRows: ReadonlyArray<number>;
  private readonly totalWidgetRows: number;

  /** linePrefix[i] = total text rows of lines [0..i]; null = 1 row per line. */
  private readonly linePrefix: ReadonlyArray<number> | null;
  private readonly totalLineRows: number;

  constructor(
    readonly lineCount: number,
    readonly lineHeight: number,
    readonly paddingTop: number,
    widgets: ReadonlyArray<GeometryWidgetRow> = [],
    readonly paddingBottom: number = paddingTop,
    /** Soft wrap: visual rows per line (each >= 1), length = lineCount. */
    rowsPerLine?: ReadonlyArray<number>,
  ) {
    if (rowsPerLine !== undefined && rowsPerLine.length === lineCount) {
      const prefix: number[] = new Array(lineCount);
      let total = 0;
      for (let i = 0; i < lineCount; i++) {
        total += Math.max(1, rowsPerLine[i]!);
        prefix[i] = total;
      }
      this.linePrefix = prefix;
      this.totalLineRows = total;
    } else {
      this.linePrefix = null;
      this.totalLineRows = lineCount;
    }
    const valid = widgets
      // afterLine === -1 is a leading widget: it sits before line 0 and shifts
      // every line down (widgetRowsBefore(line) counts it for all lines >= 0).
      .filter((w) => w.heightRows > 0 && w.afterLine >= -1 && w.afterLine < lineCount)
      .sort((a, b) => a.afterLine - b.afterLine);
    this.widgets = valid;
    const prefix: number[] = [];
    let total = 0;
    for (const widget of valid) {
      total += widget.heightRows;
      prefix.push(total);
    }
    this.prefixRows = prefix;
    this.totalWidgetRows = total;
  }

  /** Total TEXT rows of lines above `line` (soft wrap counts every visual
   * row; widget rows are separate). The layout's `startRow` input. */
  textRowsBefore(line: number): number {
    if (this.linePrefix === null) return line;
    return line <= 0 ? 0 : this.linePrefix[Math.min(line, this.lineCount) - 1]!;
  }

  /** Visual rows `line` itself occupies (1 unless soft-wrapped). */
  lineRows(line: number): number {
    if (this.linePrefix === null) return 1;
    if (line < 0 || line >= this.lineCount) return 1;
    return this.linePrefix[line]! - this.textRowsBefore(line);
  }

  /** Total widget rows inserted before (above) `line`'s own row. */
  widgetRowsBefore(line: number): number {
    // Widgets with afterLine < line sit above the line's row.
    let lo = 0;
    let hi = this.widgets.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this.widgets[mid]!.afterLine < line) lo = mid + 1;
      else hi = mid;
    }
    return lo === 0 ? 0 : this.prefixRows[lo - 1]!;
  }

  /** Absolute y of the top of `line`'s FIRST row. */
  lineTop(line: number): number {
    return (
      this.paddingTop + (this.textRowsBefore(line) + this.widgetRowsBefore(line)) * this.lineHeight
    );
  }

  /**
   * The line whose row contains `y`. Positions inside a widget's reserved
   * rows resolve to the widget's line (the row above), so clicks on widgets
   * never jump past them. Clamped to [0, lineCount).
   */
  lineAt(y: number): number {
    if (this.lineCount <= 0) return 0;
    const visualRow = Math.floor((y - this.paddingTop) / this.lineHeight);
    if (visualRow <= 0) return 0;
    // Find the largest line with visualRowOf(line) <= visualRow.
    let lo = 0;
    let hi = this.lineCount - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (this.textRowsBefore(mid) + this.widgetRowsBefore(mid) <= visualRow) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  }

  contentHeight(): number {
    return (
      this.paddingTop +
      (this.totalLineRows + this.totalWidgetRows) * this.lineHeight +
      this.paddingBottom
    );
  }
}
