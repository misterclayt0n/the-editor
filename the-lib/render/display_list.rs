use std::num::NonZeroU64;

use super::{
  damage::Damage,
  layers::LayerDeltaSet,
};
use crate::Tendril;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceId(NonZeroU64);

impl SurfaceId {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowId(NonZeroU64);

impl RowId {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }

  pub(crate) fn from_non_zero(id: NonZeroU64) -> Self {
    Self(id)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(NonZeroU64);

impl WidgetId {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRequest {
  pub viewport:         Viewport,
  pub since_generation: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowPlacement {
  pub surface_row: usize,
  pub y_px:        u32,
  pub height_px:   u32,
}

impl RowPlacement {
  pub fn new(surface_row: usize, y_px: u32, height_px: u32) -> Self {
    Self {
      surface_row,
      y_px,
      height_px,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderRow {
  pub id:         RowId,
  pub generation: u64,
  pub placement:  RowPlacement,
  pub text:       Tendril,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowMove {
  pub id:   RowId,
  pub from: Option<RowPlacement>,
  pub to:   RowPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
  Document,
  Widget {
    widget_id: WidgetId,
    fragment:  u32,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowMetadata {
  pub row_id:               RowId,
  pub surface_row:          usize,
  pub kind:                 RowKind,
  pub doc_line:             Option<usize>,
  pub starts_logical_line:  bool,
  pub wrapped_continuation: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RowMetadataDelta {
  pub rows: Vec<RowMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFrameDelta {
  pub surface_id:    SurfaceId,
  pub generation:    u64,
  pub full:          bool,
  pub retained_rows: Vec<RowMove>,
  pub changed_rows:  Vec<RenderRow>,
  pub removed_rows:  Vec<RowId>,
  pub row_metadata:  Option<RowMetadataDelta>,
  pub layers:        LayerDeltaSet,
  pub damage:        Damage,
}
