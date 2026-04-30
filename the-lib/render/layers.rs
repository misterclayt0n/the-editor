use super::display_list::WidgetId;
use crate::{
  selection::CursorId,
  syntax::HighlightKindId,
};

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GutterLayerDelta;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSpan {
  pub cursor_id:   CursorId,
  pub surface_row: usize,
  pub start_col:   usize,
  pub end_col:     usize,
  pub x_px:        u32,
  pub width_px:    u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectionLayerDelta {
  pub spans: Vec<SelectionSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CursorSemantics {
  /// Edge-based insertion position, suitable for a vertical bar caret.
  InsertionPoint,
  /// Cell-based cursor position, suitable for block and underline cursors.
  Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPlacement {
  pub cursor_id:   CursorId,
  pub surface_row: usize,
  pub col:         usize,
  pub x_px:        u32,
  pub y_px:        u32,
  pub width_px:    u32,
  pub height_px:   u32,
  pub semantics:   CursorSemantics,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CursorLayerDelta {
  pub cursors: Vec<CursorPlacement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxSpan {
  pub surface_row: usize,
  pub start_col:   usize,
  pub end_col:     usize,
  pub x_px:        u32,
  pub width_px:    u32,
  pub kind:        HighlightKindId,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyntaxLayerDelta {
  pub spans: Vec<SyntaxSpan>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OverlayLayerDelta;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockWidget {
  pub id:                 WidgetId,
  pub after_line:         usize,
  pub measured_width_px:  u32,
  pub measured_height_px: u32,
}

impl BlockWidget {
  pub fn after_line(
    id: WidgetId,
    after_line: usize,
    measured_width_px: u32,
    measured_height_px: u32,
  ) -> Self {
    Self {
      id,
      after_line,
      measured_width_px,
      measured_height_px,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetPlacement {
  pub widget_id:   WidgetId,
  pub surface_row: usize,
  pub row_span:    usize,
  pub x_px:        i64,
  pub y_px:        i64,
  pub width_px:    u32,
  pub height_px:   u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WidgetLayerDelta {
  pub widgets: Vec<WidgetPlacement>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LayerDeltaSet {
  pub gutters:    Option<GutterLayerDelta>,
  pub selections: Option<SelectionLayerDelta>,
  pub cursors:    Option<CursorLayerDelta>,
  pub syntax:     Option<SyntaxLayerDelta>,
  pub overlays:   Option<OverlayLayerDelta>,
  pub widgets:    Option<WidgetLayerDelta>,
}
