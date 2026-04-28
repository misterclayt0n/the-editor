use std::num::NonZeroU64;

use crate::Tendril;

use super::{
  damage::Damage,
  layers::LayerDeltaSet,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
  pub first_row:  usize,
  pub row_count:  usize,
  pub wrap_width: usize,
}

impl Viewport {
  pub fn new(first_row: usize, row_count: usize, wrap_width: usize) -> Self {
    Self {
      first_row,
      row_count,
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
}

impl RowPlacement {
  pub fn new(surface_row: usize) -> Self {
    Self { surface_row }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFrameDelta {
  pub surface_id:     SurfaceId,
  pub generation:     u64,
  pub full:           bool,
  pub retained_rows:  Vec<RowMove>,
  pub changed_rows:   Vec<RenderRow>,
  pub removed_rows:   Vec<RowId>,
  pub layers:         LayerDeltaSet,
  pub damage:         Damage,
}
