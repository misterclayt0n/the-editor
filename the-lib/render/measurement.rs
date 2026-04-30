/// Pixel metrics for the built-in monospace cell-grid renderer.
///
/// This is intentionally not a generic text measurement backend. The current
/// renderer assumes a uniform grid: every visual column has the same pixel
/// width and every surface row has the same pixel height. Clients with
/// proportional fonts or framework-owned text layout should treat this renderer
/// as the cell-grid path, not as a universal GUI text layout engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellGridMetrics {
  pub cell_width_px: u32,
  pub row_height_px: u32,
}

impl CellGridMetrics {
  pub const fn new(cell_width_px: u32, row_height_px: u32) -> Self {
    Self {
      cell_width_px: if cell_width_px == 0 { 1 } else { cell_width_px },
      row_height_px: if row_height_px == 0 { 1 } else { row_height_px },
    }
  }

  pub const fn cell_grid(cell_width_px: u32, row_height_px: u32) -> Self {
    Self::new(cell_width_px, row_height_px)
  }

  pub const fn col_to_x_px(self, col: usize) -> u32 {
    col as u32 * self.cell_width_px
  }

  pub const fn col_span_width_px(self, start_col: usize, end_col: usize) -> u32 {
    end_col.saturating_sub(start_col) as u32 * self.cell_width_px
  }

  pub const fn row_to_y_px(self, row: usize) -> u32 {
    row as u32 * self.row_height_px
  }
}

impl Default for CellGridMetrics {
  fn default() -> Self {
    Self::new(8, 16)
  }
}
