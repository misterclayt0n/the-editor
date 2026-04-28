#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementBackendKind {
  CellGrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceMeasurements {
  pub backend:       MeasurementBackendKind,
  pub cell_width_px: u32,
  pub row_height_px: u32,
}

impl SurfaceMeasurements {
  pub const fn cell_grid(cell_width_px: u32, row_height_px: u32) -> Self {
    Self {
      backend:       MeasurementBackendKind::CellGrid,
      cell_width_px: if cell_width_px == 0 { 1 } else { cell_width_px },
      row_height_px: if row_height_px == 0 { 1 } else { row_height_px },
    }
  }
}

impl Default for SurfaceMeasurements {
  fn default() -> Self {
    Self::cell_grid(8, 16)
  }
}
