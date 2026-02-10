//! Render-adjacent helpers and visual layout utilities.
//!
//! This module hosts visual layout computations (soft-wrap, annotations, plan
//! construction) that depend on formatting and rendering state. It
//! intentionally lives alongside core logic so consumers can access
//! `the_lib::render::*` without pulling a separate crate.

pub mod doc_formatter;
pub mod grapheme;
pub mod graphics;
pub mod gutter;
pub mod highlight_adapter;
pub mod overlay;
pub mod plan;
pub mod text_annotations;
pub mod text_format;
pub mod theme;
pub mod ui;
pub mod ui_theme;
pub mod visual_position;

pub use grapheme::{
  FormattedGrapheme,
  GraphemeSource,
};
pub use gutter::{
  GutterConfig,
  GutterLineNumbersConfig,
  GutterType,
  LineNumberMode,
};
pub use highlight_adapter::SyntaxHighlightAdapter;
pub use overlay::{
  OverlayNode,
  OverlayRect,
  OverlayRectKind,
  OverlayText,
};
pub use plan::{
  HighlightProvider,
  NoHighlights,
  RenderCache,
  RenderCursor,
  RenderGutterLine,
  RenderGutterSpan,
  RenderLine,
  RenderPlan,
  RenderSelection,
  RenderSpan,
  RenderStyles,
  RenderVisibleRow,
  build_plan,
  gutter_width_for_document,
};
pub use ui::*;
pub use visual_position::{
  char_at_visual_pos,
  char_idx_at_visual_block_offset,
  char_idx_at_visual_offset,
  visual_offset_from_block,
  visual_pos_at_char,
};

pub use crate::view::ViewState;
