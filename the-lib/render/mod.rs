//! Render-adjacent helpers and visual layout utilities.
//!
//! This module will host visual layout computations (soft-wrap, annotations)
//! that depend on formatting and rendering state. It intentionally lives
//! alongside core logic so consumers can access `the_lib::render::*` without
//! pulling a separate crate.

pub mod visual_position;
pub mod graphics;
pub mod plan;
pub mod highlight_adapter;
pub mod text_format;
pub mod grapheme;
pub mod text_annotations;
pub mod doc_formatter;

pub use grapheme::{FormattedGrapheme, GraphemeSource};
pub use plan::{
  build_plan,
  HighlightProvider,
  NoHighlights,
  RenderCache,
  RenderCursor,
  RenderLine,
  RenderPlan,
  RenderSelection,
  RenderSpan,
  RenderStyles,
};
pub use highlight_adapter::SyntaxHighlightAdapter;
pub use crate::view::ViewState;
pub use visual_position::{char_at_visual_pos, visual_pos_at_char};
