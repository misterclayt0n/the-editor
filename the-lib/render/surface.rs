use std::ops::Range;

use ropey::RopeSlice;

use super::{
  damage::{
    Damage,
    DirtyState,
  },
  display_list::{
    FrameRequest,
    RenderFrameDelta,
    RowMetadata,
    SurfaceId,
    Viewport,
  },
  layer_projection::LayerProjector,
  layers::{
    BlockWidget,
    CursorLayerDelta,
    CursorPlacement,
    CursorSemantics,
    LayerDeltaSet,
    SelectionLayerDelta,
    SelectionSpan,
    SyntaxLayerDelta,
    SyntaxSpan,
    WidgetLayerDelta,
    WidgetPlacement,
  },
  layout_index::LayoutIndex,
  measurement::CellGridMetrics,
  row_diff::{
    RetainedRow,
    RowDiffEngine,
  },
  text_annotations::TextAnnotations,
  text_format::TextFormat,
  visible_layout::{
    VisibleLayout,
    VisibleLayoutBuilder,
  },
};
use crate::{
  selection::Selection,
  syntax::SyntaxHighlightRun,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGridRenderSurface {
  id:               SurfaceId,
  format:           TextFormat,
  annotations:      TextAnnotations,
  selection:        Option<Selection>,
  visible_syntax:   Vec<SyntaxSpan>,
  widgets:          Vec<BlockWidget>,
  measurements:     CellGridMetrics,
  cursor_semantics: CursorSemantics,
  generation:       u64,
  dirty:            DirtyState,
  state:            SurfaceState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SurfaceState {
  next_row_id:        u64,
  last_text_revision: Option<u64>,
  last_viewport:      Option<Viewport>,
  visible_layout:     VisibleLayout,
  visible_rows:       Vec<RetainedRow>,
  row_metadata:       Vec<RowMetadata>,
  selection_spans:    Vec<SelectionSpan>,
  cursor_placements:  Vec<CursorPlacement>,
  syntax_spans:       Vec<SyntaxSpan>,
  widget_placements:  Vec<WidgetPlacement>,
}

impl Default for SurfaceState {
  fn default() -> Self {
    Self {
      next_row_id:        1,
      last_text_revision: None,
      last_viewport:      None,
      visible_layout:     VisibleLayout::default(),
      visible_rows:       Vec::new(),
      row_metadata:       Vec::new(),
      selection_spans:    Vec::new(),
      cursor_placements:  Vec::new(),
      syntax_spans:       Vec::new(),
      widget_placements:  Vec::new(),
    }
  }
}

impl CellGridRenderSurface {
  pub fn new(id: SurfaceId, format: TextFormat, annotations: TextAnnotations) -> Self {
    Self {
      id,
      format,
      annotations,
      selection: None,
      visible_syntax: Vec::new(),
      widgets: Vec::new(),
      measurements: CellGridMetrics::default(),
      cursor_semantics: CursorSemantics::Cell,
      generation: 0,
      dirty: DirtyState::default(),
      state: SurfaceState::default(),
    }
  }

  pub fn id(&self) -> SurfaceId {
    self.id
  }

  pub fn generation(&self) -> u64 {
    self.generation
  }

  pub fn text_format(&self) -> &TextFormat {
    &self.format
  }

  pub fn set_text_format(&mut self, format: TextFormat) {
    if self.format != format {
      self.format = format;
      self.mark_rows_dirty(true);
    }
  }

  pub fn text_annotations(&self) -> &TextAnnotations {
    &self.annotations
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

  pub fn widgets(&self) -> &[BlockWidget] {
    &self.widgets
  }

  pub fn set_text_annotations(&mut self, annotations: TextAnnotations) {
    if self.annotations != annotations {
      self.annotations = annotations;
      self.mark_rows_dirty(true);
    }
  }

  pub fn set_selection(&mut self, selection: Option<Selection>) {
    if self.selection != selection {
      self.selection = selection;
      self.dirty.selections.mark();
      self.dirty.cursors.mark();
    }
  }

  pub fn set_visible_syntax(&mut self, mut spans: Vec<SyntaxSpan>) {
    spans.sort_by(|a, b| {
      a.surface_row
        .cmp(&b.surface_row)
        .then_with(|| a.start_col.cmp(&b.start_col))
        .then_with(|| b.end_col.cmp(&a.end_col))
        .then_with(|| a.kind.cmp(&b.kind))
    });
    if self.visible_syntax != spans {
      self.visible_syntax = spans;
      self.dirty.syntax.mark();
    }
  }

  pub fn set_widgets(&mut self, mut widgets: Vec<BlockWidget>) {
    widgets.sort_by_key(|widget| (widget.after_line, widget.id.get()));
    if self.widgets != widgets {
      self.widgets = widgets;
      self.mark_rows_dirty(true);
    }
  }

  pub fn set_measurements(&mut self, measurements: CellGridMetrics) {
    if self.measurements != measurements {
      let relayout = !self.widgets.is_empty();
      self.measurements = measurements;
      self.dirty.rows.mark(relayout);
      self.dirty.selections.mark();
      self.dirty.cursors.mark();
      self.dirty.syntax.mark();
      self.dirty.widgets.mark();
    }
  }

  pub fn set_cursor_semantics(&mut self, semantics: CursorSemantics) {
    if self.cursor_semantics != semantics {
      self.cursor_semantics = semantics;
      self.dirty.cursors.mark();
    }
  }

  pub(crate) fn visible_document_ranges(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    viewport: Viewport,
    layout_index: &mut LayoutIndex,
  ) -> (Range<usize>, Range<usize>) {
    self.ensure_visible_layout(text, text_revision, viewport, layout_index);
    self.state.visible_layout.document_ranges()
  }

  pub(crate) fn project_syntax_runs(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    viewport: Viewport,
    runs: &[SyntaxHighlightRun],
    layout_index: &mut LayoutIndex,
  ) -> Vec<SyntaxSpan> {
    self.ensure_visible_layout(text, text_revision, viewport, layout_index);
    LayerProjector::project_syntax_runs(
      &self.state.visible_layout,
      viewport,
      self.measurements,
      runs,
    )
  }

  fn ensure_visible_layout(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    viewport: Viewport,
    layout_index: &mut LayoutIndex,
  ) {
    let viewport_end = viewport.first_row.saturating_add(viewport.row_count);
    let cache_covers_viewport = self.state.visible_layout.window_start_row <= viewport.first_row
      && viewport_end <= self.state.visible_layout.window_end_row;

    if self.state.visible_rows.is_empty()
      || self.generation == 0
      || self.dirty.rows.full
      || self.dirty.widgets.dirty
      || (self.dirty.rows.dirty && self.state.last_viewport == Some(viewport))
      || self.state.visible_layout.text_revision != Some(text_revision)
      || self.state.visible_layout.wrap_width != viewport.wrap_width
      || !cache_covers_viewport
    {
      self.state.visible_layout = VisibleLayoutBuilder::new(
        text,
        text_revision,
        &self.format,
        &self.annotations,
        &self.widgets,
        self.measurements,
      )
      .build(viewport, layout_index);
    }
  }

  pub fn render_frame_stateless(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    request: FrameRequest,
  ) -> RenderFrameDelta {
    let mut layout_index = LayoutIndex::default();
    self.render_frame_with_index(text, text_revision, request, &mut layout_index)
  }

  #[doc(hidden)]
  pub fn render_frame(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    request: FrameRequest,
  ) -> RenderFrameDelta {
    self.render_frame_stateless(text, text_revision, request)
  }

  pub(crate) fn render_frame_with_index(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    request: FrameRequest,
    layout_index: &mut LayoutIndex,
  ) -> RenderFrameDelta {
    self.mark_external_dirty(text_revision, request.viewport);

    let stale_generation = request.since_generation != Some(self.generation);
    let first_frame = self.state.visible_rows.is_empty() || self.generation == 0;
    let viewport_changed = self.state.last_viewport != Some(request.viewport);

    self.ensure_visible_layout(text, text_revision, request.viewport, layout_index);

    let force_full = first_frame || stale_generation || self.dirty.rows.full;

    let row_diff = if self.dirty.rows.dirty || first_frame || viewport_changed {
      let next_rows = self
        .state
        .visible_layout
        .build_rows(request.viewport, self.measurements);
      let previous_rows = std::mem::take(&mut self.state.visible_rows);
      Some(RowDiffEngine::diff(
        &mut self.state.next_row_id,
        previous_rows,
        next_rows,
        force_full,
      ))
    } else {
      None
    };

    let next_visible_rows = row_diff
      .as_ref()
      .map(|diff| diff.next_visible_rows.clone())
      .unwrap_or_else(|| self.state.visible_rows.clone());
    let next_row_metadata = next_visible_rows
      .iter()
      .map(|row| row.metadata.clone())
      .collect::<Vec<_>>();
    let row_metadata_delta = RowDiffEngine::build_row_metadata_delta(
      &self.state.row_metadata,
      &next_row_metadata,
      force_full,
    );
    let row_metadata_changed = force_full || row_metadata_delta.is_some();

    let row_damage = row_diff
      .as_ref()
      .map_or(Damage::empty(), |diff| diff.damage);
    let retained_rows = row_diff
      .as_ref()
      .map_or_else(Vec::new, |diff| diff.retained_rows.clone());
    let incremental_changed_rows = row_diff
      .as_ref()
      .map_or_else(Vec::new, |diff| diff.changed_rows.clone());
    let removed_rows = row_diff
      .as_ref()
      .map_or_else(Vec::new, |diff| diff.removed_rows.clone());

    let selection_spans =
      if self.dirty.selections.dirty || self.dirty.rows.dirty || first_frame || viewport_changed {
        LayerProjector::project_selections(
          &self.state.visible_layout,
          self.selection.as_ref(),
          request.viewport,
          self.measurements,
        )
      } else {
        self.state.selection_spans.clone()
      };
    let selection_changed = force_full || self.state.selection_spans != selection_spans;
    let selection_layer = (force_full || selection_changed).then(|| {
      SelectionLayerDelta {
        spans: selection_spans.clone(),
      }
    });

    let cursor_placements =
      if self.dirty.cursors.dirty || self.dirty.rows.dirty || first_frame || viewport_changed {
        LayerProjector::project_cursors(
          text,
          &self.state.visible_layout,
          self.selection.as_ref(),
          request.viewport,
          self.measurements,
          self.cursor_semantics,
        )
      } else {
        self.state.cursor_placements.clone()
      };
    let cursor_changed = force_full || self.state.cursor_placements != cursor_placements;
    let cursor_layer = (force_full || cursor_changed).then(|| {
      CursorLayerDelta {
        cursors: cursor_placements.clone(),
      }
    });

    let syntax_spans =
      if self.dirty.syntax.dirty || self.dirty.rows.dirty || first_frame || viewport_changed {
        self.visible_syntax.clone()
      } else {
        self.state.syntax_spans.clone()
      };
    let syntax_changed = force_full || self.state.syntax_spans != syntax_spans;
    let syntax_layer = (force_full || syntax_changed).then(|| {
      SyntaxLayerDelta {
        spans: syntax_spans.clone(),
      }
    });

    let widget_placements =
      if self.dirty.widgets.dirty || self.dirty.rows.dirty || first_frame || viewport_changed {
        LayerProjector::project_widgets(
          &self.state.visible_layout,
          request.viewport,
          self.measurements,
        )
      } else {
        self.state.widget_placements.clone()
      };
    let widget_changed = force_full || self.state.widget_placements != widget_placements;
    let widget_layer = (force_full || widget_changed).then(|| {
      WidgetLayerDelta {
        widgets: widget_placements.clone(),
      }
    });

    let presentation_changed = force_full
      || !retained_rows.is_empty()
      || !incremental_changed_rows.is_empty()
      || !removed_rows.is_empty()
      || row_metadata_changed
      || selection_changed
      || cursor_changed
      || syntax_changed
      || widget_changed;

    if self.generation == 0 {
      self.generation = 1;
    } else if presentation_changed {
      self.generation = self.generation.wrapping_add(1);
    }

    let full_changed_rows = next_visible_rows
      .iter()
      .map(|row| row.row.clone())
      .collect::<Vec<_>>();
    let full = force_full || next_visible_rows.is_empty() && self.generation == 1;

    let row_metadata_damage = if force_full || row_metadata_changed {
      RowDiffEngine::damage_for_row_metadata_change(&self.state.row_metadata, &next_row_metadata)
    } else {
      Damage::empty()
    };
    let selection_damage = if force_full || selection_changed {
      LayerProjector::damage_for_selection_change(&self.state.selection_spans, &selection_spans)
    } else {
      Damage::empty()
    };
    let cursor_damage = if force_full || cursor_changed {
      LayerProjector::damage_for_cursor_change(&self.state.cursor_placements, &cursor_placements)
    } else {
      Damage::empty()
    };
    let syntax_damage = if force_full || syntax_changed {
      LayerProjector::damage_for_syntax_change(&self.state.syntax_spans, &syntax_spans)
    } else {
      Damage::empty()
    };
    let widget_damage = if force_full || widget_changed {
      LayerProjector::damage_for_widget_change(&self.state.widget_placements, &widget_placements)
    } else {
      Damage::empty()
    };

    let damage = if full {
      Damage::full(request.viewport.row_count.min(next_visible_rows.len()))
    } else {
      row_damage
        .union(row_metadata_damage)
        .union(selection_damage)
        .union(cursor_damage)
        .union(syntax_damage)
        .union(widget_damage)
    };

    self.state.last_text_revision = Some(text_revision);
    self.state.last_viewport = Some(request.viewport);
    self.state.visible_rows = next_visible_rows;
    self.state.row_metadata = next_row_metadata;
    self.state.selection_spans = selection_spans;
    self.state.cursor_placements = cursor_placements;
    self.state.syntax_spans = syntax_spans;
    self.state.widget_placements = widget_placements;
    self.dirty.clear();

    RenderFrameDelta {
      surface_id: self.id,
      generation: self.generation,
      full,
      retained_rows: if full { Vec::new() } else { retained_rows },
      changed_rows: if full {
        full_changed_rows
      } else {
        incremental_changed_rows
      },
      removed_rows: if full { Vec::new() } else { removed_rows },
      row_metadata: row_metadata_delta,
      layers: LayerDeltaSet {
        selections: selection_layer,
        cursors: cursor_layer,
        syntax: syntax_layer,
        widgets: widget_layer,
        ..LayerDeltaSet::default()
      },
      damage,
    }
  }

  fn mark_rows_dirty(&mut self, full: bool) {
    self.dirty.rows.mark(full);
    self.dirty.selections.mark();
    self.dirty.cursors.mark();
    self.dirty.syntax.mark();
    self.dirty.widgets.mark();
  }

  fn mark_external_dirty(&mut self, text_revision: u64, viewport: Viewport) {
    if self.state.last_text_revision.is_some()
      && self.state.last_text_revision != Some(text_revision)
    {
      self.mark_rows_dirty(false);
    }

    if self.state.last_viewport != Some(viewport) {
      let full = self
        .state
        .last_viewport
        .is_none_or(|previous| previous.wrap_width != viewport.wrap_width);
      self.mark_rows_dirty(full);
    }
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::CellGridRenderSurface;
  use crate::{
    render::{
      BlockWidget,
      CellGridMetrics,
      CursorSemantics,
      FrameRequest,
      RowKind,
      SurfaceId,
      SyntaxSpan,
      TextAnnotations,
      TextFormat,
      Viewport,
      WidgetId,
    },
    selection::{
      Range,
      Selection,
    },
    syntax::HighlightKindId,
  };

  fn syntax_span(surface_row: usize, start_col: usize, end_col: usize, kind: u32) -> SyntaxSpan {
    SyntaxSpan {
      surface_row,
      start_col,
      end_col,
      x_px: start_col as u32 * 8,
      width_px: (end_col - start_col) as u32 * 8,
      kind: HighlightKindId::new(kind),
    }
  }

  fn request(
    first_row: usize,
    row_count: usize,
    wrap_width: usize,
    since: Option<u64>,
  ) -> FrameRequest {
    FrameRequest {
      viewport:         Viewport::new(first_row, row_count, 0, wrap_width),
      since_generation: since,
    }
  }

  fn request_with_col(
    first_row: usize,
    row_count: usize,
    first_col: usize,
    wrap_width: usize,
    since: Option<u64>,
  ) -> FrameRequest {
    FrameRequest {
      viewport:         Viewport::new(first_row, row_count, first_col, wrap_width),
      since_generation: since,
    }
  }

  fn selection(ranges: &[(usize, usize)]) -> Selection {
    Selection::new(
      ranges
        .iter()
        .map(|&(anchor, head)| Range::new(anchor, head))
        .collect::<smallvec::SmallVec<[Range; 1]>>(),
    )
    .unwrap()
  }

  #[test]
  fn first_frame_populates_doc_wide_visible_layout() {
    let rope = Rope::from("alpha beta gamma delta");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      format,
      TextAnnotations::default(),
    );

    let _ = surface.render_frame(rope.slice(..), 1, request(0, 1, 5, None));

    assert_eq!(surface.state.visible_layout.text_revision, Some(1));
    assert_eq!(surface.state.visible_layout.wrap_width, 5);
    assert!(surface.state.visible_layout.rows.len() > 1);
  }

  #[test]
  fn first_frame_is_full() {
    let rope = Rope::from("a\nb\nc");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let delta = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));

    assert!(delta.full);
    assert_eq!(delta.changed_rows.len(), 2);
    assert!(delta.retained_rows.is_empty());
    assert!(delta.removed_rows.is_empty());
  }

  #[test]
  fn first_frame_emits_row_metadata_for_visible_rows() {
    let rope = Rope::from("alpha beta gamma");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      format,
      TextAnnotations::default(),
    );

    let delta = surface.render_frame(rope.slice(..), 1, request(0, 3, 5, None));

    let row_metadata = delta.row_metadata.expect("row metadata delta");
    assert_eq!(row_metadata.rows.len(), 3);
    assert_eq!(row_metadata.rows[0].surface_row, 0);
    assert_eq!(row_metadata.rows[0].kind, RowKind::Document);
    assert_eq!(row_metadata.rows[0].doc_line, Some(0));
    assert!(row_metadata.rows[0].starts_logical_line);
    assert!(!row_metadata.rows[0].wrapped_continuation);

    assert_eq!(row_metadata.rows[1].surface_row, 1);
    assert_eq!(row_metadata.rows[1].kind, RowKind::Document);
    assert_eq!(row_metadata.rows[1].doc_line, Some(0));
    assert!(!row_metadata.rows[1].starts_logical_line);
    assert!(row_metadata.rows[1].wrapped_continuation);
  }

  #[test]
  fn identical_second_frame_is_empty_incremental() {
    let rope = Rope::from("a\nb\nc");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 2, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert!(delta.changed_rows.is_empty());
    assert!(delta.retained_rows.is_empty());
    assert!(delta.removed_rows.is_empty());
    assert!(delta.damage.is_empty());
    assert_eq!(delta.generation, initial.generation);
  }

  #[test]
  fn vertical_scroll_reuses_overlapping_rows() {
    let rope = Rope::from("a\nb\nc");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    let old_bottom = initial.changed_rows[1].clone();
    let old_top = initial.changed_rows[0].clone();

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(1, 2, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert_eq!(delta.retained_rows.len(), 1);
    assert_eq!(delta.retained_rows[0].id, old_bottom.id);
    assert_eq!(delta.retained_rows[0].from.unwrap().surface_row, 1);
    assert_eq!(delta.retained_rows[0].to.surface_row, 0);
    assert_eq!(delta.changed_rows.len(), 1);
    assert_eq!(delta.changed_rows[0].placement.surface_row, 1);
    assert_eq!(delta.removed_rows, vec![old_top.id]);
  }

  #[test]
  fn visible_text_edit_bumps_only_changed_row_generation() {
    let rope = Rope::from("a\nb\nc");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 3, 20, None));
    let original_second = initial.changed_rows[1].clone();

    let edited = Rope::from("a\nB\nc");
    let delta = surface.render_frame(
      edited.slice(..),
      2,
      request(0, 3, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert_eq!(delta.changed_rows.len(), 1);
    assert_eq!(delta.changed_rows[0].id, original_second.id);
    assert_eq!(
      delta.changed_rows[0].generation,
      original_second.generation + 1
    );
  }

  #[test]
  fn wrap_width_change_forces_full_refresh() {
    let rope = Rope::from("hello world");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      format,
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 3, 20, None));
    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 3, 5, Some(initial.generation)),
    );

    assert!(delta.full);
  }

  #[test]
  fn annotation_change_forces_full_refresh() {
    let rope = Rope::from("a\nb");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    let mut annotations = TextAnnotations::default();
    annotations.add_inline_text(0, ">>").unwrap();
    surface.set_text_annotations(annotations);

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 2, 20, Some(initial.generation)),
    );

    assert!(delta.full);
  }

  #[test]
  fn selection_change_updates_selection_and_cursor_layers_only() {
    let rope = Rope::from("abcd\nefgh");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    surface.set_selection(Some(selection(&[(1, 3)])));

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 2, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert!(delta.changed_rows.is_empty());
    assert!(delta.row_metadata.is_none());
    assert!(delta.layers.syntax.is_none());
    let selection_delta = delta.layers.selections.expect("selection layer delta");
    assert_eq!(selection_delta.spans.len(), 1);
    assert_eq!(selection_delta.spans[0].surface_row, 0);
    assert_eq!(selection_delta.spans[0].start_col, 1);
    assert_eq!(selection_delta.spans[0].end_col, 3);
    assert_eq!(selection_delta.spans[0].x_px, 8);
    assert_eq!(selection_delta.spans[0].width_px, 16);

    let cursor_delta = delta.layers.cursors.expect("cursor layer delta");
    assert_eq!(cursor_delta.cursors.len(), 1);
    assert_eq!(cursor_delta.cursors[0].surface_row, 0);
    assert_eq!(cursor_delta.cursors[0].col, 2);
    assert_eq!(cursor_delta.cursors[0].x_px, 16);
    assert_eq!(cursor_delta.cursors[0].y_px, 0);
    assert_eq!(cursor_delta.cursors[0].width_px, 8);
    assert_eq!(cursor_delta.cursors[0].height_px, 16);
    assert_eq!(cursor_delta.cursors[0].semantics, CursorSemantics::Cell);
  }

  #[test]
  fn insertion_point_cursor_semantics_use_range_head() {
    let rope = Rope::from("abcd");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_cursor_semantics(CursorSemantics::InsertionPoint);
    surface.set_selection(Some(selection(&[(1, 3)])));

    let delta = surface.render_frame(rope.slice(..), 1, request(0, 1, 20, None));

    let cursor_delta = delta.layers.cursors.expect("cursor layer delta");
    assert_eq!(cursor_delta.cursors.len(), 1);
    assert_eq!(cursor_delta.cursors[0].col, 3);
    assert_eq!(cursor_delta.cursors[0].x_px, 24);
    assert_eq!(cursor_delta.cursors[0].width_px, 0);
    assert_eq!(
      cursor_delta.cursors[0].semantics,
      CursorSemantics::InsertionPoint
    );
  }

  #[test]
  fn selection_and_cursor_layers_update_on_scroll_without_rebuilding_existing_rows() {
    let rope = Rope::from("aaaa\nbbbb\ncccc");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_selection(Some(selection(&[(2, 7)])));

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    assert!(initial.layers.selections.is_some());

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(1, 2, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert_eq!(delta.changed_rows.len(), 1);
    let selection_delta = delta.layers.selections.expect("selection layer delta");
    assert_eq!(selection_delta.spans.len(), 1);
    assert_eq!(selection_delta.spans[0].surface_row, 0);
    assert_eq!(selection_delta.spans[0].start_col, 0);
    assert_eq!(selection_delta.spans[0].end_col, 2);

    let cursor_delta = delta.layers.cursors.expect("cursor layer delta");
    assert_eq!(cursor_delta.cursors.len(), 1);
    assert_eq!(cursor_delta.cursors[0].surface_row, 0);
    assert_eq!(cursor_delta.cursors[0].col, 1);
  }

  #[test]
  fn point_selection_emits_cursor_without_selection_span() {
    let rope = Rope::from("abcd");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_selection(Some(selection(&[(2, 2)])));

    let delta = surface.render_frame(rope.slice(..), 1, request(0, 1, 20, None));

    assert!(delta.layers.selections.is_some());
    assert_eq!(delta.layers.selections.unwrap().spans.len(), 0);
    let cursor_delta = delta.layers.cursors.expect("cursor layer delta");
    assert_eq!(cursor_delta.cursors.len(), 1);
    assert_eq!(cursor_delta.cursors[0].surface_row, 0);
    assert_eq!(cursor_delta.cursors[0].col, 2);
    assert!(!delta.damage.is_empty());
  }

  #[test]
  fn syntax_change_updates_only_syntax_layer() {
    let rope = Rope::from("abcd");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 1, 20, None));
    surface.set_visible_syntax(vec![syntax_span(0, 1, 3, 7)]);

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 1, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert!(delta.changed_rows.is_empty());
    assert!(delta.row_metadata.is_none());
    assert!(delta.layers.selections.is_none());
    assert!(delta.layers.cursors.is_none());
    let syntax_delta = delta.layers.syntax.expect("syntax layer delta");
    assert_eq!(syntax_delta.spans.len(), 1);
    assert_eq!(syntax_delta.spans[0].surface_row, 0);
    assert_eq!(syntax_delta.spans[0].start_col, 1);
    assert_eq!(syntax_delta.spans[0].end_col, 3);
    assert_eq!(syntax_delta.spans[0].x_px, 8);
    assert_eq!(syntax_delta.spans[0].width_px, 16);
    assert_eq!(syntax_delta.spans[0].kind, HighlightKindId::new(7));
  }

  #[test]
  fn horizontal_scroll_crops_rows_and_layer_columns() {
    let rope = Rope::from("abcdef");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_selection(Some(selection(&[(2, 5)])));
    surface.set_visible_syntax(vec![syntax_span(0, 1, 4, 9)]);

    let initial = surface.render_frame(rope.slice(..), 1, request_with_col(0, 1, 0, 4, None));

    surface.set_visible_syntax(vec![syntax_span(0, 0, 2, 9)]);
    let original_row = initial.changed_rows[0].clone();

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request_with_col(0, 1, 2, 4, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert_eq!(delta.changed_rows.len(), 1);
    assert_eq!(delta.changed_rows[0].id, original_row.id);
    assert_eq!(delta.changed_rows[0].text.as_str(), "cdef");

    let selection_delta = delta.layers.selections.expect("selection layer delta");
    assert_eq!(selection_delta.spans.len(), 1);
    assert_eq!(selection_delta.spans[0].start_col, 0);
    assert_eq!(selection_delta.spans[0].end_col, 3);

    let cursor_delta = delta.layers.cursors.expect("cursor layer delta");
    assert_eq!(cursor_delta.cursors.len(), 1);
    assert_eq!(cursor_delta.cursors[0].col, 2);

    let syntax_delta = delta.layers.syntax.expect("syntax layer delta");
    assert_eq!(syntax_delta.spans.len(), 1);
    assert_eq!(syntax_delta.spans[0].start_col, 0);
    assert_eq!(syntax_delta.spans[0].end_col, 2);
  }

  #[test]
  fn syntax_layer_updates_on_scroll_without_rebuilding_existing_rows() {
    let rope = Rope::from("aaaa\nbbbb\ncccc");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_visible_syntax(vec![syntax_span(1, 0, 2, 3)]);

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    assert!(initial.layers.syntax.is_some());

    surface.set_visible_syntax(vec![syntax_span(0, 0, 2, 3)]);

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(1, 2, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert_eq!(delta.changed_rows.len(), 1);
    let syntax_delta = delta.layers.syntax.expect("syntax layer delta");
    assert_eq!(syntax_delta.spans.len(), 1);
    assert_eq!(syntax_delta.spans[0].surface_row, 0);
    assert_eq!(syntax_delta.spans[0].start_col, 0);
    assert_eq!(syntax_delta.spans[0].end_col, 2);
    assert_eq!(syntax_delta.spans[0].kind, HighlightKindId::new(3));
  }

  #[test]
  fn row_metadata_updates_for_retained_rows_on_scroll() {
    let rope = Rope::from("a\nb\nc");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    let initial_metadata = initial.row_metadata.as_ref().expect("row metadata delta");
    let retained_row_id = initial_metadata.rows[1].row_id;

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(1, 2, 20, Some(initial.generation)),
    );

    let row_metadata = delta.row_metadata.expect("row metadata delta");
    assert_eq!(row_metadata.rows.len(), 2);
    assert_eq!(row_metadata.rows[0].row_id, retained_row_id);
    assert_eq!(row_metadata.rows[0].surface_row, 0);
    assert_eq!(row_metadata.rows[0].kind, RowKind::Document);
    assert_eq!(row_metadata.rows[0].doc_line, Some(1));
    assert!(row_metadata.rows[0].starts_logical_line);
    assert!(!row_metadata.rows[0].wrapped_continuation);

    assert_eq!(row_metadata.rows[1].surface_row, 1);
    assert_eq!(row_metadata.rows[1].kind, RowKind::Document);
    assert_eq!(row_metadata.rows[1].doc_line, Some(2));
  }

  #[test]
  fn measurement_change_updates_pixel_placement_without_rebuilding_text_rows() {
    let rope = Rope::from("abcd");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_selection(Some(selection(&[(1, 3)])));
    surface.set_visible_syntax(vec![syntax_span(0, 1, 3, 4)]);

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 1, 20, None));
    let original_row = initial.changed_rows[0].clone();
    surface.set_measurements(CellGridMetrics::new(10, 20));
    surface.set_visible_syntax(vec![SyntaxSpan {
      x_px: 10,
      width_px: 20,
      ..syntax_span(0, 1, 3, 4)
    }]);

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 1, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert!(delta.changed_rows.is_empty());
    assert_eq!(delta.retained_rows.len(), 1);
    assert_eq!(delta.retained_rows[0].id, original_row.id);
    assert_eq!(delta.retained_rows[0].to.y_px, 0);
    assert_eq!(delta.retained_rows[0].to.height_px, 20);

    let selection_delta = delta.layers.selections.expect("selection layer delta");
    assert_eq!(selection_delta.spans[0].x_px, 10);
    assert_eq!(selection_delta.spans[0].width_px, 20);

    let cursor_delta = delta.layers.cursors.expect("cursor layer delta");
    assert_eq!(cursor_delta.cursors[0].x_px, 20);
    assert_eq!(cursor_delta.cursors[0].height_px, 20);

    let syntax_delta = delta.layers.syntax.expect("syntax layer delta");
    assert_eq!(syntax_delta.spans[0].x_px, 10);
    assert_eq!(syntax_delta.spans[0].width_px, 20);
  }

  #[test]
  fn block_widget_reserves_rows_and_emits_widget_layer() {
    let rope = Rope::from("a\nb");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_widgets(vec![BlockWidget::after_line(
      WidgetId::new(1).unwrap(),
      0,
      40,
      24,
    )]);

    let delta = surface.render_frame(rope.slice(..), 1, request(0, 4, 20, None));

    assert!(delta.full);
    assert_eq!(delta.changed_rows.len(), 4);
    assert_eq!(delta.changed_rows[0].text.as_str(), "a");
    assert_eq!(delta.changed_rows[1].text.as_str(), "");
    assert_eq!(delta.changed_rows[2].text.as_str(), "");
    assert!(delta.changed_rows[3].text.as_str().starts_with("b"));

    let widget_layer = delta.layers.widgets.expect("widget layer delta");
    assert_eq!(widget_layer.widgets.len(), 1);
    assert_eq!(widget_layer.widgets[0].surface_row, 1);
    assert_eq!(widget_layer.widgets[0].row_span, 2);
    assert_eq!(widget_layer.widgets[0].x_px, 0);
    assert_eq!(widget_layer.widgets[0].y_px, 16);
    assert_eq!(widget_layer.widgets[0].width_px, 40);
    assert_eq!(widget_layer.widgets[0].height_px, 24);
  }

  #[test]
  fn widget_measurement_change_reflows_reserved_rows() {
    let rope = Rope::from("a\nb");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_widgets(vec![BlockWidget::after_line(
      WidgetId::new(1).unwrap(),
      0,
      40,
      24,
    )]);

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 4, 20, None));
    let old_bottom = initial.changed_rows[3].clone();
    surface.set_measurements(CellGridMetrics::new(8, 10));

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 5, 20, Some(initial.generation)),
    );

    assert!(delta.full);
    assert_eq!(delta.changed_rows.len(), 5);
    assert_eq!(delta.changed_rows[4].id, old_bottom.id);

    let widget_layer = delta.layers.widgets.expect("widget layer delta");
    assert_eq!(widget_layer.widgets.len(), 1);
    assert_eq!(widget_layer.widgets[0].row_span, 3);
    assert_eq!(widget_layer.widgets[0].y_px, 10);
  }

  #[test]
  fn widget_rows_emit_widget_row_metadata() {
    let rope = Rope::from("a\nb");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_widgets(vec![BlockWidget::after_line(
      WidgetId::new(1).unwrap(),
      0,
      40,
      24,
    )]);

    let delta = surface.render_frame(rope.slice(..), 1, request(0, 4, 20, None));

    let row_metadata = delta.row_metadata.expect("row metadata delta");
    assert_eq!(row_metadata.rows.len(), 4);
    assert_eq!(row_metadata.rows[1].kind, RowKind::Widget {
      widget_id: WidgetId::new(1).unwrap(),
      fragment:  0,
    });
    assert_eq!(row_metadata.rows[1].doc_line, Some(0));
    assert!(row_metadata.rows[1].starts_logical_line);
    assert!(!row_metadata.rows[1].wrapped_continuation);

    assert_eq!(row_metadata.rows[2].kind, RowKind::Widget {
      widget_id: WidgetId::new(1).unwrap(),
      fragment:  1,
    });
    assert_eq!(row_metadata.rows[2].doc_line, Some(0));
    assert!(!row_metadata.rows[2].starts_logical_line);
    assert!(!row_metadata.rows[2].wrapped_continuation);
  }

  #[test]
  fn stale_generation_forces_full_refresh() {
    let rope = Rope::from("a\nb");
    let mut surface = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let _ = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    let delta = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, Some(999)));

    assert!(delta.full);
  }
}
