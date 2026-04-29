//! Compatibility adapters for document-visual mapping helpers.
//!
//! The authoritative seam for global visual mapping now lives in
//! [`super::layout_index`]. This module stays as a thin adapter for older
//! callers and documentation that still refer to `render/visual_position.rs`.

use ropey::RopeSlice;

use crate::render::{
  doc_formatter::Position,
  layout_index,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
};

#[allow(dead_code)]
pub(crate) fn visual_offset_from_block(
  text: RopeSlice<'_>,
  anchor: usize,
  pos: usize,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
) -> (Position, usize) {
  layout_index::visual_offset_from_block(text, anchor, pos, text_format, wrap_width, annotations)
}

#[allow(dead_code)]
pub(crate) fn char_idx_at_visual_block_offset(
  text: RopeSlice<'_>,
  anchor: usize,
  row: usize,
  column: usize,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
) -> (usize, usize) {
  layout_index::char_idx_at_visual_block_offset(
    text,
    anchor,
    row,
    column,
    text_format,
    wrap_width,
    annotations,
  )
}

#[allow(dead_code)]
pub(crate) fn char_idx_at_visual_offset(
  text: RopeSlice<'_>,
  anchor: usize,
  row_offset: isize,
  column: usize,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
) -> (usize, usize) {
  layout_index::char_idx_at_visual_offset(
    text,
    anchor,
    row_offset,
    column,
    text_format,
    wrap_width,
    annotations,
  )
}
