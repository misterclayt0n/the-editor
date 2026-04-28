use std::ops::Range;

use crate::{
  selection::CursorId,
  syntax::HighlightKindId,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GutterLayerDelta;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionSpan {
  pub cursor_id:   CursorId,
  pub surface_row: usize,
  pub start_col:   usize,
  pub end_col:     usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SelectionLayerDelta {
  pub spans: Vec<SelectionSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPlacement {
  pub cursor_id:   CursorId,
  pub surface_row: usize,
  pub col:         usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CursorLayerDelta {
  pub cursors: Vec<CursorPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxHighlight {
  pub range: Range<usize>,
  pub kind:  HighlightKindId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxSpan {
  pub surface_row: usize,
  pub start_col:   usize,
  pub end_col:     usize,
  pub kind:        HighlightKindId,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyntaxLayerDelta {
  pub spans: Vec<SyntaxSpan>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OverlayLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WidgetLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LayerDeltaSet {
  pub gutters:    Option<GutterLayerDelta>,
  pub selections: Option<SelectionLayerDelta>,
  pub cursors:    Option<CursorLayerDelta>,
  pub syntax:     Option<SyntaxLayerDelta>,
  pub overlays:   Option<OverlayLayerDelta>,
  pub widgets:    Option<WidgetLayerDelta>,
}
