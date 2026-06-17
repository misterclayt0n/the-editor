/**
 * GeometryModel: the single owner of line↔pixel mapping once block widgets
 * exist. Every consumer of vertical geometry (layout, hit testing, caret
 * reveal, scroll anchoring, window math) must go through this model —
 * line-arithmetic shortcuts break the moment one widget reserves rows.
 *
 * Widgets reserve whole rows after a line; the model keeps prefix sums so
 * lineTop is O(log widgets) and lineAt is O(log widgets), degenerating to
 * plain arithmetic with no widgets.
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

  constructor(
    readonly lineCount: number,
    readonly lineHeight: number,
    readonly paddingTop: number,
    widgets: ReadonlyArray<GeometryWidgetRow> = [],
    readonly paddingBottom: number = paddingTop,
  ) {
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

  /** Absolute y of the top of `line`'s row. */
  lineTop(line: number): number {
    return this.paddingTop + (line + this.widgetRowsBefore(line)) * this.lineHeight;
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
      if (mid + this.widgetRowsBefore(mid) <= visualRow) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  }

  contentHeight(): number {
    return (
      this.paddingTop +
      (this.lineCount + this.totalWidgetRows) * this.lineHeight +
      this.paddingBottom
    );
  }
}
