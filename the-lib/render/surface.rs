use std::{
  collections::{
    BTreeMap,
    HashMap,
  },
  num::NonZeroU64,
};

use ropey::RopeSlice;

use crate::{
  Tendril,
  grapheme::Grapheme,
  selection::Selection,
  syntax::HighlightKindId,
};

use super::{
  damage::{
    Damage,
    DirtyState,
  },
  display_list::{
    FrameRequest,
    RenderFrameDelta,
    RenderRow,
    RowId,
    RowMove,
    RowPlacement,
    SurfaceId,
    Viewport,
  },
  doc_formatter::DocumentFormatter,
  layers::{
    CursorLayerDelta,
    CursorPlacement,
    LayerDeltaSet,
    SelectionLayerDelta,
    SelectionSpan,
    SyntaxHighlight,
    SyntaxLayerDelta,
    SyntaxSpan,
  },
  text_annotations::TextAnnotations,
  text_format::TextFormat,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSurface {
  id:                     SurfaceId,
  format:                 TextFormat,
  annotations:            TextAnnotations,
  selection:         Option<Selection>,
  syntax_highlights: Vec<SyntaxHighlight>,
  generation:        u64,
  dirty:             DirtyState,
  state:             SurfaceState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SurfaceState {
  next_row_id:        u64,
  last_text_revision: Option<u64>,
  last_viewport:      Option<Viewport>,
  layout_cache:       DocumentLayoutCache,
  visible_rows:       Vec<RetainedRow>,
  selection_spans:    Vec<SelectionSpan>,
  cursor_placements:  Vec<CursorPlacement>,
  syntax_spans:       Vec<SyntaxSpan>,
}

impl Default for SurfaceState {
  fn default() -> Self {
    Self {
      next_row_id: 1,
      last_text_revision: None,
      last_viewport: None,
      layout_cache: DocumentLayoutCache::default(),
      visible_rows: Vec::new(),
      selection_spans: Vec::new(),
      cursor_placements: Vec::new(),
      syntax_spans: Vec::new(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RetainedRow {
  key: RowKey,
  row: RenderRow,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RowKey {
  Document {
    line_idx:       usize,
    first_doc_char: usize,
  },
  Virtual {
    anchor_char: usize,
    ordinal:     u32,
  },
}

#[derive(Debug)]
struct RowBuilder {
  visual_row:       usize,
  placement:        RowPlacement,
  text:             String,
  first_doc:        Option<(usize, usize)>,
  virtual_anchor:   Option<usize>,
}

impl RowBuilder {
  fn new(visual_row: usize, placement: RowPlacement, first_char_idx: usize) -> Self {
    Self {
      visual_row,
      placement,
      text: String::new(),
      first_doc: None,
      virtual_anchor: Some(first_char_idx),
    }
  }

  fn push(&mut self, grapheme: &super::doc_formatter::FormattedGrapheme<'_>) {
    if !grapheme.is_virtual() && self.first_doc.is_none() {
      self.first_doc = Some((grapheme.line_idx, grapheme.char_idx));
    }

    match grapheme.raw {
      Grapheme::Newline => {},
      Grapheme::Tab { width } => self.text.push_str(&" ".repeat(width)),
      Grapheme::Other { ref g } => self.text.push_str(g.as_ref()),
    }
  }

  fn into_built_row(
    self,
    virtual_ordinals: &mut BTreeMap<usize, u32>,
  ) -> BuiltRow {
    let key = if let Some((line_idx, first_doc_char)) = self.first_doc {
      RowKey::Document {
        line_idx,
        first_doc_char,
      }
    } else {
      let anchor_char = self.virtual_anchor.unwrap_or(0);
      let ordinal = virtual_ordinals.entry(anchor_char).or_insert(0);
      let key = RowKey::Virtual {
        anchor_char,
        ordinal: *ordinal,
      };
      *ordinal += 1;
      key
    };

    BuiltRow {
      key,
      placement: self.placement,
      text: self.text.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedLayoutRow {
  key:  RowKey,
  text: Tendril,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct DocumentLayoutCache {
  text_revision: Option<u64>,
  wrap_width:    usize,
  rows:          Vec<CachedLayoutRow>,
}

#[derive(Debug)]
struct BuiltRow {
  key:       RowKey,
  placement: RowPlacement,
  text:      Tendril,
}

impl RenderSurface {
  pub fn new(id: SurfaceId, format: TextFormat, annotations: TextAnnotations) -> Self {
    Self {
      id,
      format,
      annotations,
      selection: None,
      syntax_highlights: Vec::new(),
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

  pub fn syntax_highlights(&self) -> &[SyntaxHighlight] {
    &self.syntax_highlights
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

  pub fn set_syntax_highlights(&mut self, mut highlights: Vec<SyntaxHighlight>) {
    highlights.sort_by_key(|highlight| (highlight.range.start, std::cmp::Reverse(highlight.range.end)));
    if self.syntax_highlights != highlights {
      self.syntax_highlights = highlights;
      self.dirty.syntax.mark();
    }
  }

  pub fn render_frame(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    request: FrameRequest,
  ) -> RenderFrameDelta {
    self.mark_external_dirty(text_revision, request.viewport);

    let stale_generation = request.since_generation != Some(self.generation);
    let first_frame = self.state.visible_rows.is_empty() || self.generation == 0;
    let viewport_changed = self.state.last_viewport != Some(request.viewport);

    if self.dirty.rows.dirty
      || first_frame
      || self.state.layout_cache.text_revision != Some(text_revision)
      || self.state.layout_cache.wrap_width != request.viewport.wrap_width
    {
      self.state.layout_cache = rebuild_document_layout_cache(
        text,
        &self.format,
        &self.annotations,
        text_revision,
        request.viewport.wrap_width,
      );
    }

    let force_full = first_frame || stale_generation || self.dirty.rows.full;

    let row_diff = if self.dirty.rows.dirty || first_frame || viewport_changed {
      let next_rows = build_visible_rows(&self.state.layout_cache, request.viewport);
      Some(diff_rows(&mut self.state, next_rows, force_full, request.viewport))
    } else {
      None
    };

    let next_visible_rows = row_diff
      .as_ref()
      .map(|diff| diff.next_visible_rows.clone())
      .unwrap_or_else(|| self.state.visible_rows.clone());

    let row_damage = row_diff.as_ref().map_or(Damage::empty(), |diff| diff.damage);
    let retained_rows = row_diff
      .as_ref()
      .map_or_else(Vec::new, |diff| diff.retained_rows.clone());
    let incremental_changed_rows = row_diff
      .as_ref()
      .map_or_else(Vec::new, |diff| diff.changed_rows.clone());
    let removed_rows = row_diff
      .as_ref()
      .map_or_else(Vec::new, |diff| diff.removed_rows.clone());

    let selection_spans = if self.dirty.selections.dirty
      || self.dirty.rows.dirty
      || first_frame
      || viewport_changed
    {
      build_selection_spans(
        text,
        &self.format,
        &self.annotations,
        self.selection.as_ref(),
        request.viewport,
      )
    } else {
      self.state.selection_spans.clone()
    };
    let selection_changed = force_full || self.state.selection_spans != selection_spans;
    let selection_layer = (force_full || selection_changed).then(|| SelectionLayerDelta {
      spans: selection_spans.clone(),
    });

    let cursor_placements = if self.dirty.cursors.dirty
      || self.dirty.rows.dirty
      || first_frame
      || viewport_changed
    {
      build_cursor_placements(
        text,
        &self.format,
        &self.annotations,
        self.selection.as_ref(),
        request.viewport,
      )
    } else {
      self.state.cursor_placements.clone()
    };
    let cursor_changed = force_full || self.state.cursor_placements != cursor_placements;
    let cursor_layer = (force_full || cursor_changed).then(|| CursorLayerDelta {
      cursors: cursor_placements.clone(),
    });

    let syntax_spans = if self.dirty.syntax.dirty
      || self.dirty.rows.dirty
      || first_frame
      || viewport_changed
    {
      build_syntax_spans(
        text,
        &self.format,
        &self.annotations,
        &self.syntax_highlights,
        request.viewport,
      )
    } else {
      self.state.syntax_spans.clone()
    };
    let syntax_changed = force_full || self.state.syntax_spans != syntax_spans;
    let syntax_layer = (force_full || syntax_changed).then(|| SyntaxLayerDelta {
      spans: syntax_spans.clone(),
    });

    let presentation_changed = force_full
      || !retained_rows.is_empty()
      || !incremental_changed_rows.is_empty()
      || !removed_rows.is_empty()
      || selection_changed
      || cursor_changed
      || syntax_changed;

    if self.generation == 0 {
      self.generation = 1;
    } else if presentation_changed {
      self.generation = self.generation.wrapping_add(1);
    }

    let full_changed_rows = next_visible_rows.iter().map(|row| row.row.clone()).collect::<Vec<_>>();
    let full = force_full || next_visible_rows.is_empty() && self.generation == 1;

    let selection_damage = if force_full || selection_changed {
      damage_for_selection_change(&self.state.selection_spans, &selection_spans)
    } else {
      Damage::empty()
    };
    let cursor_damage = if force_full || cursor_changed {
      damage_for_cursor_change(&self.state.cursor_placements, &cursor_placements)
    } else {
      Damage::empty()
    };
    let syntax_damage = if force_full || syntax_changed {
      damage_for_syntax_change(&self.state.syntax_spans, &syntax_spans)
    } else {
      Damage::empty()
    };

    let damage = if full {
      Damage::full(request.viewport.row_count.min(next_visible_rows.len()))
    } else {
      row_damage.union(selection_damage).union(cursor_damage).union(syntax_damage)
    };

    self.state.last_text_revision = Some(text_revision);
    self.state.last_viewport = Some(request.viewport);
    self.state.visible_rows = next_visible_rows;
    self.state.selection_spans = selection_spans;
    self.state.cursor_placements = cursor_placements;
    self.state.syntax_spans = syntax_spans;
    self.dirty.clear();

    RenderFrameDelta {
      surface_id: self.id,
      generation: self.generation,
      full,
      retained_rows: if full { Vec::new() } else { retained_rows },
      changed_rows: if full { full_changed_rows } else { incremental_changed_rows },
      removed_rows: if full { Vec::new() } else { removed_rows },
      layers: LayerDeltaSet {
        selections: selection_layer,
        cursors: cursor_layer,
        syntax: syntax_layer,
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

#[derive(Debug)]
struct RowDiff {
  next_visible_rows: Vec<RetainedRow>,
  retained_rows:     Vec<RowMove>,
  changed_rows:      Vec<RenderRow>,
  removed_rows:      Vec<RowId>,
  damage:            Damage,
}

fn diff_rows(
  state: &mut SurfaceState,
  next_rows: Vec<BuiltRow>,
  force_full: bool,
  _viewport: Viewport,
) -> RowDiff {
  let mut previous = state
    .visible_rows
    .drain(..)
    .map(|row| (row.key.clone(), row))
    .collect::<HashMap<_, _>>();

  let mut retained_rows = Vec::new();
  let mut changed_rows = Vec::new();
  let mut removed_rows = Vec::new();
  let mut next_visible_rows = Vec::with_capacity(next_rows.len());
  let mut damaged_rows = Vec::new();

  for built in next_rows {
    if let Some(previous_row) = previous.remove(&built.key) {
      let content_changed = previous_row.row.text != built.text;
      let moved = previous_row.row.placement != built.placement;
      let generation = if content_changed {
        previous_row.row.generation.wrapping_add(1)
      } else {
        previous_row.row.generation
      };

      let row = RenderRow {
        id: previous_row.row.id,
        generation,
        placement: built.placement,
        text: built.text,
      };

      if !force_full {
        if content_changed {
          damaged_rows.push(previous_row.row.placement.surface_row);
          damaged_rows.push(row.placement.surface_row);
          changed_rows.push(row.clone());
        } else if moved {
          damaged_rows.push(previous_row.row.placement.surface_row);
          damaged_rows.push(row.placement.surface_row);
          retained_rows.push(RowMove {
            id: row.id,
            from: Some(previous_row.row.placement),
            to: row.placement,
          });
        }
      }

      next_visible_rows.push(RetainedRow {
        key: built.key,
        row,
      });
    } else {
      let row = RenderRow {
        id: next_row_id(state),
        generation: 1,
        placement: built.placement,
        text: built.text,
      };

      if !force_full {
        damaged_rows.push(row.placement.surface_row);
        changed_rows.push(row.clone());
      }

      next_visible_rows.push(RetainedRow {
        key: built.key,
        row,
      });
    }
  }

  if !force_full {
    for removed in previous.into_values() {
      damaged_rows.push(removed.row.placement.surface_row);
      removed_rows.push(removed.row.id);
    }
  }

  let damage = if force_full {
    Damage::full(next_visible_rows.len())
  } else {
    Damage::from_rows(damaged_rows)
  };

  RowDiff {
    next_visible_rows,
    retained_rows,
    changed_rows,
    removed_rows,
    damage,
  }
}

fn next_row_id(state: &mut SurfaceState) -> RowId {
  let id = NonZeroU64::new(state.next_row_id).expect("row ids start at one");
  state.next_row_id = state.next_row_id.wrapping_add(1).max(1);
  RowId::from_non_zero(id)
}

fn rebuild_document_layout_cache(
  text: RopeSlice<'_>,
  format: &TextFormat,
  annotations: &TextAnnotations,
  text_revision: u64,
  wrap_width: usize,
) -> DocumentLayoutCache {
  let mut formatter = DocumentFormatter::new(text, format, wrap_width, annotations);
  let mut virtual_ordinals = BTreeMap::new();
  let mut rows = Vec::new();
  let mut current = None::<RowBuilder>;

  while let Some(grapheme) = formatter.next() {
    let visual_row = grapheme.visual_pos.row;

    if current.as_ref().is_some_and(|row| row.visual_row != visual_row) {
      if let Some(row) = current.take() {
        let built = row.into_built_row(&mut virtual_ordinals);
        rows.push(CachedLayoutRow {
          key: built.key,
          text: built.text,
        });
      }
    }

    if current.is_none() {
      current = Some(RowBuilder::new(
        visual_row,
        RowPlacement::new(visual_row),
        grapheme.char_idx,
      ));
    }

    current.as_mut().unwrap().push(&grapheme);

    if grapheme.source.is_eof() {
      break;
    }
  }

  if let Some(row) = current.take() {
    let built = row.into_built_row(&mut virtual_ordinals);
    rows.push(CachedLayoutRow {
      key: built.key,
      text: built.text,
    });
  }

  if rows.is_empty() {
    rows.push(CachedLayoutRow {
      key: RowKey::Document {
        line_idx: 0,
        first_doc_char: 0,
      },
      text: Tendril::new(),
    });
  }

  DocumentLayoutCache {
    text_revision: Some(text_revision),
    wrap_width,
    rows,
  }
}

fn build_visible_rows(cache: &DocumentLayoutCache, viewport: Viewport) -> Vec<BuiltRow> {
  if viewport.row_count == 0 {
    return Vec::new();
  }

  let end = viewport
    .first_row
    .saturating_add(viewport.row_count)
    .min(cache.rows.len());

  cache.rows[viewport.first_row.min(cache.rows.len())..end]
    .iter()
    .enumerate()
    .map(|(surface_row, row)| BuiltRow {
      key: row.key.clone(),
      placement: RowPlacement::new(surface_row),
      text: row.text.clone(),
    })
    .collect()
}

fn build_selection_spans(
  text: RopeSlice<'_>,
  format: &TextFormat,
  annotations: &TextAnnotations,
  selection: Option<&Selection>,
  viewport: Viewport,
) -> Vec<SelectionSpan> {
  let Some(selection) = selection else {
    return Vec::new();
  };

  let mut spans: Vec<SelectionSpan> = Vec::new();
  let mut formatter = DocumentFormatter::new(text, format, viewport.wrap_width, annotations);

  for grapheme in &mut formatter {
    if grapheme.source.is_eof() {
      break;
    }
    if grapheme.is_virtual() {
      continue;
    }

    let visual_row = grapheme.visual_pos.row;
    if visual_row < viewport.first_row || visual_row >= viewport.first_row + viewport.row_count {
      continue;
    }
    if grapheme.raw == Grapheme::Newline {
      continue;
    }

    let Some(cursor) = selection
      .iter()
      .find(|cursor| !cursor.range.is_empty() && cursor.range.contains(grapheme.char_idx))
    else {
      continue;
    };

    let span = SelectionSpan {
      cursor_id: cursor.id,
      surface_row: visual_row - viewport.first_row,
      start_col: grapheme.visual_pos.col,
      end_col: grapheme.visual_pos.col + grapheme.width(),
    };

    if let Some(last) = spans.last_mut()
      && last.cursor_id == span.cursor_id
      && last.surface_row == span.surface_row
      && last.end_col == span.start_col
    {
      last.end_col = span.end_col;
    } else {
      spans.push(span);
    }
  }

  spans
}

fn build_cursor_placements(
  text: RopeSlice<'_>,
  format: &TextFormat,
  annotations: &TextAnnotations,
  selection: Option<&Selection>,
  viewport: Viewport,
) -> Vec<CursorPlacement> {
  let Some(selection) = selection else {
    return Vec::new();
  };

  let mut cursors = Vec::new();
  for cursor in selection.iter() {
    let char_idx = cursor.range.cursor(text);
    let Some(pos) = super::visual_position::visual_pos_at_char(
      text,
      format,
      viewport.wrap_width,
      annotations,
      char_idx,
    ) else {
      continue;
    };

    if pos.row < viewport.first_row || pos.row >= viewport.first_row + viewport.row_count {
      continue;
    }

    cursors.push(CursorPlacement {
      cursor_id: cursor.id,
      surface_row: pos.row - viewport.first_row,
      col: pos.col,
    });
  }

  cursors
}

fn build_syntax_spans(
  text: RopeSlice<'_>,
  format: &TextFormat,
  annotations: &TextAnnotations,
  syntax_highlights: &[SyntaxHighlight],
  viewport: Viewport,
) -> Vec<SyntaxSpan> {
  if syntax_highlights.is_empty() {
    return Vec::new();
  }

  let mut spans: Vec<SyntaxSpan> = Vec::new();
  let mut next_highlight = 0;
  let mut active: Vec<(HighlightKindId, usize)> = Vec::new();
  let mut formatter = DocumentFormatter::new(text, format, viewport.wrap_width, annotations);

  for grapheme in &mut formatter {
    if grapheme.source.is_eof() {
      break;
    }
    if grapheme.is_virtual() || grapheme.raw == Grapheme::Newline {
      continue;
    }

    let visual_row = grapheme.visual_pos.row;
    if visual_row < viewport.first_row || visual_row >= viewport.first_row + viewport.row_count {
      continue;
    }

    while let Some(highlight) = syntax_highlights.get(next_highlight) {
      if highlight.range.start > grapheme.char_idx {
        break;
      }
      if highlight.range.end > grapheme.char_idx {
        active.push((highlight.kind, highlight.range.end));
      }
      next_highlight += 1;
    }

    active.retain(|(_, end)| *end > grapheme.char_idx);
    let Some((kind, _)) = active.last().copied() else {
      continue;
    };

    let span = SyntaxSpan {
      surface_row: visual_row - viewport.first_row,
      start_col: grapheme.visual_pos.col,
      end_col: grapheme.visual_pos.col + grapheme.width(),
      kind,
    };

    if let Some(last) = spans.last_mut()
      && last.surface_row == span.surface_row
      && last.kind == span.kind
      && last.end_col == span.start_col
    {
      last.end_col = span.end_col;
    } else {
      spans.push(span);
    }
  }

  spans
}

fn damage_for_selection_change(old: &[SelectionSpan], new: &[SelectionSpan]) -> Damage {
  Damage::from_rows(
    old.iter()
      .map(|span| span.surface_row)
      .chain(new.iter().map(|span| span.surface_row)),
  )
}

fn damage_for_cursor_change(old: &[CursorPlacement], new: &[CursorPlacement]) -> Damage {
  Damage::from_rows(
    old.iter()
      .map(|cursor| cursor.surface_row)
      .chain(new.iter().map(|cursor| cursor.surface_row)),
  )
}

fn damage_for_syntax_change(old: &[SyntaxSpan], new: &[SyntaxSpan]) -> Damage {
  Damage::from_rows(
    old.iter()
      .map(|span| span.surface_row)
      .chain(new.iter().map(|span| span.surface_row)),
  )
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::RenderSurface;
  use crate::{
    render::{
      FrameRequest,
      SurfaceId,
      SyntaxHighlight,
      TextAnnotations,
      TextFormat,
      Viewport,
    },
    selection::{
      Range,
      Selection,
    },
    syntax::HighlightKindId,
  };

  fn request(
    first_row: usize,
    row_count: usize,
    wrap_width: usize,
    since: Option<u64>,
  ) -> FrameRequest {
    FrameRequest {
      viewport: Viewport::new(first_row, row_count, wrap_width),
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
  fn first_frame_populates_doc_wide_layout_cache() {
    let rope = Rope::from("alpha beta gamma delta");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let mut surface = RenderSurface::new(
      SurfaceId::new(1).unwrap(),
      format,
      TextAnnotations::default(),
    );

    let _ = surface.render_frame(rope.slice(..), 1, request(0, 1, 5, None));

    assert_eq!(surface.state.layout_cache.text_revision, Some(1));
    assert_eq!(surface.state.layout_cache.wrap_width, 5);
    assert!(surface.state.layout_cache.rows.len() > 1);
  }

  #[test]
  fn first_frame_is_full() {
    let rope = Rope::from("a\nb\nc");
    let mut surface = RenderSurface::new(
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
  fn identical_second_frame_is_empty_incremental() {
    let rope = Rope::from("a\nb\nc");
    let mut surface = RenderSurface::new(
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
    let mut surface = RenderSurface::new(
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
    let mut surface = RenderSurface::new(
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
    let mut surface = RenderSurface::new(
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
    let mut surface = RenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    let mut annotations = TextAnnotations::default();
    annotations
      .add_virtual_line(crate::render::VirtualLineSpec::after(0).text("ghost").single_line());
    surface.set_text_annotations(annotations);

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 3, 20, Some(initial.generation)),
    );

    assert!(delta.full);
  }

  #[test]
  fn selection_change_updates_selection_and_cursor_layers_only() {
    let rope = Rope::from("abcd\nefgh");
    let mut surface = RenderSurface::new(
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
    assert!(delta.layers.syntax.is_none());
    let selection_delta = delta.layers.selections.expect("selection layer delta");
    assert_eq!(selection_delta.spans.len(), 1);
    assert_eq!(selection_delta.spans[0].surface_row, 0);
    assert_eq!(selection_delta.spans[0].start_col, 1);
    assert_eq!(selection_delta.spans[0].end_col, 3);

    let cursor_delta = delta.layers.cursors.expect("cursor layer delta");
    assert_eq!(cursor_delta.cursors.len(), 1);
    assert_eq!(cursor_delta.cursors[0].surface_row, 0);
    assert_eq!(cursor_delta.cursors[0].col, 2);
  }

  #[test]
  fn selection_and_cursor_layers_update_on_scroll_without_rebuilding_existing_rows() {
    let rope = Rope::from("aaaa\nbbbb\ncccc");
    let mut surface = RenderSurface::new(
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
    let mut surface = RenderSurface::new(
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
    let mut surface = RenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 1, 20, None));
    surface.set_syntax_highlights(vec![SyntaxHighlight {
      range: 1..3,
      kind: HighlightKindId::new(7),
    }]);

    let delta = surface.render_frame(
      rope.slice(..),
      1,
      request(0, 1, 20, Some(initial.generation)),
    );

    assert!(!delta.full);
    assert!(delta.changed_rows.is_empty());
    assert!(delta.layers.selections.is_none());
    assert!(delta.layers.cursors.is_none());
    let syntax_delta = delta.layers.syntax.expect("syntax layer delta");
    assert_eq!(syntax_delta.spans.len(), 1);
    assert_eq!(syntax_delta.spans[0].surface_row, 0);
    assert_eq!(syntax_delta.spans[0].start_col, 1);
    assert_eq!(syntax_delta.spans[0].end_col, 3);
    assert_eq!(syntax_delta.spans[0].kind, HighlightKindId::new(7));
  }

  #[test]
  fn syntax_layer_updates_on_scroll_without_rebuilding_existing_rows() {
    let rope = Rope::from("aaaa\nbbbb\ncccc");
    let mut surface = RenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    surface.set_syntax_highlights(vec![SyntaxHighlight {
      range: 5..7,
      kind: HighlightKindId::new(3),
    }]);

    let initial = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    assert!(initial.layers.syntax.is_some());

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
  fn stale_generation_forces_full_refresh() {
    let rope = Rope::from("a\nb");
    let mut surface = RenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let _ = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, None));
    let delta = surface.render_frame(rope.slice(..), 1, request(0, 2, 20, Some(999)));

    assert!(delta.full);
  }
}
