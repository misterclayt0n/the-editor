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
};

use super::{
  damage::Damage,
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
  layers::LayerDeltaSet,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSurface {
  id:                     SurfaceId,
  format:                 TextFormat,
  annotations:            TextAnnotations,
  format_generation:      u64,
  annotations_generation: u64,
  generation:             u64,
  state:                  SurfaceState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SurfaceState {
  next_row_id:                  u64,
  last_text_revision:           Option<u64>,
  last_viewport:                Option<Viewport>,
  last_format_generation:       u64,
  last_annotations_generation:  u64,
  visible_rows:                 Vec<RetainedRow>,
}

impl Default for SurfaceState {
  fn default() -> Self {
    Self {
      next_row_id: 1,
      last_text_revision: None,
      last_viewport: None,
      last_format_generation: 0,
      last_annotations_generation: 0,
      visible_rows: Vec::new(),
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
      format_generation: 0,
      annotations_generation: 0,
      generation: 0,
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
      self.format_generation = self.format_generation.wrapping_add(1);
    }
  }

  pub fn text_annotations(&self) -> &TextAnnotations {
    &self.annotations
  }

  pub fn set_text_annotations(&mut self, annotations: TextAnnotations) {
    if self.annotations != annotations {
      self.annotations = annotations;
      self.annotations_generation = self.annotations_generation.wrapping_add(1);
    }
  }

  pub fn render_frame(
    &mut self,
    text: RopeSlice<'_>,
    text_revision: u64,
    request: FrameRequest,
  ) -> RenderFrameDelta {
    let next_rows = build_visible_rows(
      text,
      &self.format,
      &self.annotations,
      request.viewport,
    );

    let force_full = self.should_force_full(&request);
    let diff = diff_rows(
      &mut self.state,
      next_rows,
      force_full,
      request.viewport,
    );

    let presentation_changed = force_full
      || !diff.retained_rows.is_empty()
      || !diff.changed_rows.is_empty()
      || !diff.removed_rows.is_empty();

    if self.generation == 0 {
      self.generation = 1;
    } else if presentation_changed {
      self.generation = self.generation.wrapping_add(1);
    }

    self.state.last_text_revision = Some(text_revision);
    self.state.last_viewport = Some(request.viewport);
    self.state.last_format_generation = self.format_generation;
    self.state.last_annotations_generation = self.annotations_generation;
    self.state.visible_rows = diff.next_visible_rows;

    let full = force_full || self.state.visible_rows.is_empty() && self.generation == 1;
    let damage = if full {
      Damage::full(request.viewport.row_count.min(self.state.visible_rows.len()))
    } else {
      diff.damage
    };

    RenderFrameDelta {
      surface_id: self.id,
      generation: self.generation,
      full,
      retained_rows: if full { Vec::new() } else { diff.retained_rows },
      changed_rows: if full {
        self
          .state
          .visible_rows
          .iter()
          .map(|row| row.row.clone())
          .collect()
      } else {
        diff.changed_rows
      },
      removed_rows: if full { Vec::new() } else { diff.removed_rows },
      layers: LayerDeltaSet::default(),
      damage,
    }
  }

  fn should_force_full(&self, request: &FrameRequest) -> bool {
    if self.state.visible_rows.is_empty() || self.generation == 0 {
      return true;
    }

    if request.since_generation != Some(self.generation) {
      return true;
    }

    if self.state.last_format_generation != self.format_generation {
      return true;
    }

    if self.state.last_annotations_generation != self.annotations_generation {
      return true;
    }

    self
      .state
      .last_viewport
      .is_none_or(|viewport| viewport.wrap_width != request.viewport.wrap_width)
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

fn build_visible_rows(
  text: RopeSlice<'_>,
  format: &TextFormat,
  annotations: &TextAnnotations,
  viewport: Viewport,
) -> Vec<BuiltRow> {
  if viewport.row_count == 0 {
    return Vec::new();
  }

  let mut formatter = DocumentFormatter::new(text, format, viewport.wrap_width, annotations);
  let mut virtual_ordinals = BTreeMap::new();
  let mut rows = Vec::new();
  let mut current = None::<RowBuilder>;

  while let Some(grapheme) = formatter.next() {
    let visual_row = grapheme.visual_pos.row;

    if current.as_ref().is_some_and(|row| row.visual_row != visual_row) {
      if let Some(row) = current.take() {
        if row.visual_row >= viewport.first_row
          && row.visual_row < viewport.first_row + viewport.row_count
        {
          rows.push(row.into_built_row(&mut virtual_ordinals));
          if rows.len() == viewport.row_count {
            return rows;
          }
        }
      }
    }

    if current.is_none() {
      current = Some(RowBuilder::new(
        visual_row,
        RowPlacement::new(visual_row.saturating_sub(viewport.first_row)),
        grapheme.char_idx,
      ));
    }

    current.as_mut().unwrap().push(&grapheme);

    if grapheme.source.is_eof() {
      break;
    }
  }

  if let Some(row) = current.take() {
    if row.visual_row >= viewport.first_row && row.visual_row < viewport.first_row + viewport.row_count
    {
      rows.push(row.into_built_row(&mut virtual_ordinals));
    }
  }

  if rows.is_empty() && viewport.first_row == 0 {
    rows.push(BuiltRow {
      key: RowKey::Document {
        line_idx: 0,
        first_doc_char: 0,
      },
      placement: RowPlacement::new(0),
      text: Tendril::new(),
    });
  }

  rows
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::RenderSurface;
  use crate::render::{
    FrameRequest,
    SurfaceId,
    TextAnnotations,
    TextFormat,
    Viewport,
  };

  fn request(first_row: usize, row_count: usize, wrap_width: usize, since: Option<u64>) -> FrameRequest {
    FrameRequest {
      viewport: Viewport::new(first_row, row_count, wrap_width),
      since_generation: since,
    }
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
