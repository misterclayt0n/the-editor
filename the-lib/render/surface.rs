use std::ops::Range;

use ropey::RopeSlice;

use super::{
  display_list::{
    RenderFrame,
    SurfaceId,
    Viewport,
  },
  layer_projection::LayerProjector,
  layers::{
    CursorSemantics,
    SyntaxSpan,
  },
  layout_index::LayoutIndex,
  measurement::CellGridMetrics,
  text_format::TextFormat,
  visible_layout::VisibleLayoutBuilder,
};
use crate::{
  selection::Selection,
  syntax::SyntaxHighlightRun,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGridRenderSurface {
  id:               SurfaceId,
  format:           TextFormat,
  selection:        Option<Selection>,
  visible_syntax:   Vec<SyntaxSpan>,
  measurements:     CellGridMetrics,
  cursor_semantics: CursorSemantics,
}

impl CellGridRenderSurface {
  pub fn new(id: SurfaceId, format: TextFormat) -> Self {
    Self {
      id,
      format,
      selection: None,
      visible_syntax: Vec::new(),
      measurements: CellGridMetrics::default(),
      cursor_semantics: CursorSemantics::Cell,
    }
  }

  pub fn id(&self) -> SurfaceId {
    self.id
  }

  pub fn text_format(&self) -> &TextFormat {
    &self.format
  }

  pub fn set_text_format(&mut self, format: TextFormat) {
    self.format = format;
  }

  pub fn selection(&self) -> Option<&Selection> {
    self.selection.as_ref()
  }

  pub fn visible_syntax(&self) -> &[SyntaxSpan] {
    &self.visible_syntax
  }

  pub fn measurements(&self) -> CellGridMetrics {
    self.measurements
  }

  pub fn cursor_semantics(&self) -> CursorSemantics {
    self.cursor_semantics
  }

  pub fn set_selection(&mut self, selection: Option<Selection>) {
    self.selection = selection;
  }

  pub fn set_visible_syntax(&mut self, mut spans: Vec<SyntaxSpan>) {
    spans.sort_by(|a, b| {
      a.surface_row
        .cmp(&b.surface_row)
        .then_with(|| a.start_col.cmp(&b.start_col))
        .then_with(|| b.end_col.cmp(&a.end_col))
        .then_with(|| a.kind.cmp(&b.kind))
    });
    self.visible_syntax = spans;
  }

  pub fn set_measurements(&mut self, measurements: CellGridMetrics) {
    self.measurements = measurements;
  }

  pub fn set_cursor_semantics(&mut self, semantics: CursorSemantics) {
    self.cursor_semantics = semantics;
  }

  pub(crate) fn visible_document_ranges(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    viewport: Viewport,
    layout_index: &mut LayoutIndex,
  ) -> (Range<usize>, Range<usize>) {
    VisibleLayoutBuilder::new(text, text_revision, &self.format)
      .build(viewport, layout_index)
      .document_ranges()
  }

  pub(crate) fn project_syntax_runs(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    viewport: Viewport,
    runs: &[SyntaxHighlightRun],
    layout_index: &mut LayoutIndex,
  ) -> Vec<SyntaxSpan> {
    let layout =
      VisibleLayoutBuilder::new(text, text_revision, &self.format).build(viewport, layout_index);
    LayerProjector::project_syntax_runs(&layout, viewport, runs)
  }

  pub(crate) fn render_frame_with_index(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    active_cursor: Option<crate::selection::CursorId>,
    viewport: Viewport,
    layout_index: &mut LayoutIndex,
  ) -> RenderFrame {
    let layout =
      VisibleLayoutBuilder::new(text, text_revision, &self.format).build(viewport, layout_index);

    RenderFrame {
      surface_id: self.id,
      rows:       layout.build_rows(viewport),
      selections: LayerProjector::project_selections(&layout, self.selection.as_ref(), active_cursor, viewport),
      cursors:    LayerProjector::project_cursors(
        text,
        &layout,
        self.selection.as_ref(),
        active_cursor,
        viewport,
        self.cursor_semantics,
      ),
      syntax:     self.visible_syntax.clone(),
    }
  }
}
