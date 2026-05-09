use crate::{
  selection::CursorId,
  syntax::HighlightKindId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSpan {
  pub cursor_id:   CursorId,
  pub surface_row: usize,
  pub start_col:   usize,
  pub end_col:     usize,
  pub active:      bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CursorSemantics {
  InsertionPoint,
  Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPlacement {
  pub cursor_id:   CursorId,
  pub surface_row: usize,
  pub col:         usize,
  pub semantics:   CursorSemantics,
  pub active:      bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxSpan {
  pub surface_row: usize,
  pub start_col:   usize,
  pub end_col:     usize,
  pub kind:        HighlightKindId,
}
