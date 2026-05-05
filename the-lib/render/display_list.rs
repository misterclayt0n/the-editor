use std::num::NonZeroU64;

use super::layers::{
  CursorPlacement,
  SelectionSpan,
  SyntaxSpan,
};
use crate::Tendril;

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct SurfaceId(NonZeroU64);

impl SurfaceId {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
  pub first_row:  usize,
  pub row_count:  usize,
  pub first_col:  usize,
  pub wrap_width: usize,
}

impl Viewport {
  pub fn new(first_row: usize, row_count: usize, first_col: usize, wrap_width: usize) -> Self {
    Self {
      first_row,
      row_count,
      first_col,
      wrap_width,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRow {
  pub surface_row:          usize,
  pub text:                 Tendril,
  pub doc_line:             Option<usize>,
  pub starts_logical_line:  bool,
  pub wrapped_continuation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFrame {
  pub surface_id: SurfaceId,
  pub rows:       Vec<RenderRow>,
  pub selections: Vec<SelectionSpan>,
  pub cursors:    Vec<CursorPlacement>,
  pub syntax:     Vec<SyntaxSpan>,
}
