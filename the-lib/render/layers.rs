#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GutterLayerDelta;

use crate::selection::CursorId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionSpan {
  pub cursor_id:  CursorId,
  pub surface_row: usize,
  pub start_col:  usize,
  pub end_col:    usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SelectionLayerDelta {
  pub spans: Vec<SelectionSpan>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CursorLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OverlayLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WidgetLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LayerDeltaSet {
  pub gutters:    Option<GutterLayerDelta>,
  pub selections: Option<SelectionLayerDelta>,
  pub cursors:    Option<CursorLayerDelta>,
  pub overlays:   Option<OverlayLayerDelta>,
  pub widgets:    Option<WidgetLayerDelta>,
}
