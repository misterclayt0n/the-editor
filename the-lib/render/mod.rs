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
pub mod inline_diagnostics;
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
pub use inline_diagnostics::{
  InlineDiagnostic,
  InlineDiagnosticFilter,
  InlineDiagnosticRenderLine,
  InlineDiagnosticsConfig,
  InlineDiagnosticsLineAnnotation,
  InlineDiagnosticsRenderData,
  InlineDiagnosticsViewportLayout,
  SharedInlineDiagnosticsRenderData,
  render_inline_diagnostics_for_viewport,
};
pub use overlay::{
  OverlayNode,
  OverlayRect,
  OverlayRectKind,
  OverlayText,
};
pub use plan::{
  FrameGenerationState,
  FrameRenderPlan,
  HighlightProvider,
  NoHighlights,
  PaneRenderPlan,
  RenderCache,
  RenderCursor,
  RenderDamageReason,
  RenderDiagnosticGutterStyles,
  RenderDiffGutterStyles,
  RenderGenerationState,
  RenderGutterColumn,
  RenderGutterDiffKind,
  RenderGutterLine,
  RenderGutterSpan,
  RenderLayerRowHashes,
  RenderLine,
  RenderPlan,
  RenderRowInsertion,
  RenderSelection,
  RenderSelectionKind,
  RenderSpan,
  RenderStyles,
  RenderVisibleRow,
  SelectionMatchHighlightOptions,
  add_selection_match_highlights,
  apply_diagnostic_gutter_markers,
  apply_diff_gutter_markers,
  apply_row_insertions,
  base_render_layer_row_hashes,
  build_plan,
  diff_row_hashes,
  finish_frame_generations,
  finish_render_generations,
  gutter_width_for_document,
  hash_render_plan_layout,
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
