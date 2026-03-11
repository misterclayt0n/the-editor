//! Render plan construction.
//!
//! A render plan is a backend-agnostic description of what to draw for a given
//! document and viewport. Clients consume the plan and handle actual drawing.
//!
//! # Example
//!
//! ```no_run
//! use ropey::Rope;
//! use the_lib::{
//!   document::{
//!     Document,
//!     DocumentId,
//!   },
//!   position::Position,
//!   render::{
//!     GutterConfig,
//!     NoHighlights,
//!     RenderCache,
//!     RenderStyles,
//!     build_plan,
//!     graphics::Rect,
//!     text_annotations::TextAnnotations,
//!     text_format::TextFormat,
//!   },
//!   view::ViewState,
//! };
//!
//! let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
//! let doc = Document::new(id, Rope::from("hello"));
//! let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
//! let text_fmt = TextFormat::default();
//! let mut annotations = TextAnnotations::default();
//! let mut highlights = NoHighlights;
//! let mut cache = RenderCache::default();
//! let styles = RenderStyles::default();
//! let gutter = GutterConfig::default();
//!
//! let plan = build_plan(
//!   &doc,
//!   view,
//!   &text_fmt,
//!   &gutter,
//!   &mut annotations,
//!   &mut highlights,
//!   &mut cache,
//!   styles,
//! );
//! assert_eq!(plan.lines.len(), 1);
//! ```

use std::{
  collections::BTreeMap,
  hash::{
    DefaultHasher,
    Hash,
    Hasher,
  },
  num::NonZeroUsize,
};

use the_core::grapheme::{
  Grapheme,
  GraphemeStr,
  next_grapheme_boundary,
};
use the_stdx::rope::RopeSliceExt;

use crate::{
  Tendril,
  diagnostics::DiagnosticSeverity,
  document::Document,
  editor::{
    PaneContentKind,
    TerminalId,
  },
  position::Position,
  render::{
    FormattedGrapheme,
    GraphemeSource,
    doc_formatter::{
      DocumentFormatter,
      prev_checkpoint,
    },
    graphics::{
      CursorKind,
      Rect,
      Style,
    },
    gutter::{
      GutterConfig,
      GutterType,
      LineNumberMode,
    },
    overlay::OverlayNode,
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    visual_position,
  },
  split_tree::PaneId,
  syntax::Highlight,
  view::ViewState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSpan {
  pub col:        u16,
  pub cols:       u16,
  pub text:       Tendril,
  pub highlight:  Option<Highlight>,
  pub is_virtual: bool,
}

impl RenderSpan {
  fn end_col(&self) -> u16 {
    self.col.saturating_add(self.cols)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderLine {
  pub row:   u16,
  pub spans: Vec<RenderSpan>,
}

impl RenderLine {
  fn new(row: u16) -> Self {
    Self {
      row,
      spans: Vec::new(),
    }
  }

  fn push_span(&mut self, span: RenderSpan) {
    if let Some(last) = self.spans.last_mut() {
      if last.is_virtual == span.is_virtual
        && last.highlight == span.highlight
        && last.end_col() == span.col
      {
        last.text.push_str(&span.text);
        last.cols = last.cols.saturating_add(span.cols);
        return;
      }
    }
    self.spans.push(span);
  }

  #[cfg(test)]
  fn text(&self) -> String {
    let mut s = String::new();
    for span in &self.spans {
      s.push_str(&span.text);
    }
    s
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderVisibleRow {
  pub row:               u16,
  pub doc_line:          usize,
  pub first_visual_line: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderGutterSpan {
  pub col:   u16,
  pub text:  Tendril,
  pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderGutterColumn {
  pub kind:  GutterType,
  pub col:   u16,
  pub width: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderGutterLine {
  pub row:   u16,
  pub spans: Vec<RenderGutterSpan>,
}

impl RenderGutterLine {
  fn new(row: u16) -> Self {
    Self {
      row,
      spans: Vec::new(),
    }
  }

  fn push_span(&mut self, span: RenderGutterSpan) {
    if span.text.is_empty() {
      return;
    }
    self.spans.push(span);
  }

  fn sort_spans(&mut self) {
    self.spans.sort_by_key(|span| span.col);
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCursor {
  pub id:    crate::selection::CursorId,
  pub pos:   Position,
  pub kind:  CursorKind,
  pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderSelectionKind {
  Primary,
  Match,
  Hover,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSelection {
  pub rect:  Rect,
  pub style: Style,
  pub kind:  RenderSelectionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderStyles {
  pub selection:          Style,
  pub cursor:             Style,
  pub active_cursor:      Style,
  pub cursor_kind:        CursorKind,
  pub active_cursor_kind: CursorKind,
  pub non_block_cursor_uses_head: bool,
  pub gutter:             Style,
  pub gutter_active:      Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionMatchHighlightOptions {
  pub enable_point_cursor_match: bool,
  pub max_needle_chars:          usize,
  pub max_matches:               usize,
}

impl Default for SelectionMatchHighlightOptions {
  fn default() -> Self {
    Self {
      enable_point_cursor_match: false,
      max_needle_chars:          128,
      max_matches:               1000,
    }
  }
}

impl Default for RenderStyles {
  fn default() -> Self {
    Self {
      selection:          Style::default(),
      cursor:             Style::default(),
      active_cursor:      Style::default(),
      cursor_kind:        CursorKind::Block,
      active_cursor_kind: CursorKind::Block,
      non_block_cursor_uses_head: true,
      gutter:             Style::default(),
      gutter_active:      Style::default(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderRowInsertion {
  pub base_row:      usize,
  pub inserted_rows: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderDamageReason {
  None,
  Full,
  Layout,
  Text,
  Decoration,
  Cursor,
  Scroll,
  Theme,
  PaneStructure,
}

impl RenderDamageReason {
  pub fn code(self) -> u8 {
    match self {
      Self::None => 0,
      Self::Full => 1,
      Self::Layout => 2,
      Self::Text => 3,
      Self::Decoration => 4,
      Self::Cursor => 5,
      Self::Scroll => 6,
      Self::Theme => 7,
      Self::PaneStructure => 8,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderLayerRowHashes {
  pub text_rows:       Vec<u64>,
  pub decoration_rows: Vec<u64>,
  pub cursor_rows:     Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderGenerationState {
  pub layout_generation:     u64,
  pub text_generation:       u64,
  pub decoration_generation: u64,
  pub cursor_generation:     u64,
  pub cursor_blink_generation: u64,
  pub scroll_generation:     u64,
  pub theme_generation:      u64,
  pub text_rows:             Vec<u64>,
  pub decoration_rows:       Vec<u64>,
  pub cursor_rows:           Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameGenerationState {
  pub frame_generation:          u64,
  pub pane_structure_generation: u64,
  pub pane_states:               BTreeMap<PaneId, RenderGenerationState>,
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
  pub viewport:                 Rect,
  pub scroll:                   Position,
  pub content_offset_x:         u16,
  pub layout_generation:        u64,
  pub text_generation:          u64,
  pub decoration_generation:    u64,
  pub cursor_generation:        u64,
  pub scroll_generation:        u64,
  pub theme_generation:         u64,
  pub damage_start_row:         u16,
  pub damage_end_row:           u16,
  pub damage_is_full:           bool,
  pub damage_reason:            RenderDamageReason,
  pub cursor_blink_enabled:     bool,
  pub cursor_blink_interval_ms: u16,
  pub cursor_blink_delay_ms:    u16,
  pub cursor_blink_generation:  u64,
  pub gutter_columns:           Vec<RenderGutterColumn>,
  pub visible_rows:             Vec<RenderVisibleRow>,
  pub gutter_lines:             Vec<RenderGutterLine>,
  pub lines:                    Vec<RenderLine>,
  pub cursors:                  Vec<RenderCursor>,
  pub selections:               Vec<RenderSelection>,
  pub overlays:                 Vec<OverlayNode>,
}

impl RenderPlan {
  pub fn empty(viewport: Rect, scroll: Position) -> Self {
    Self {
      viewport,
      scroll,
      content_offset_x: 0,
      layout_generation: 0,
      text_generation: 0,
      decoration_generation: 0,
      cursor_generation: 0,
      scroll_generation: 0,
      theme_generation: 0,
      damage_start_row: 0,
      damage_end_row: 0,
      damage_is_full: false,
      damage_reason: RenderDamageReason::None,
      cursor_blink_enabled: false,
      cursor_blink_interval_ms: 0,
      cursor_blink_delay_ms: 0,
      cursor_blink_generation: 0,
      gutter_columns: Vec::new(),
      visible_rows: Vec::new(),
      gutter_lines: Vec::new(),
      lines: Vec::new(),
      cursors: Vec::new(),
      selections: Vec::new(),
      overlays: Vec::new(),
    }
  }

  pub fn content_width(&self) -> usize {
    self.viewport.width.saturating_sub(self.content_offset_x) as usize
  }

  pub fn gutter_column(&self, kind: GutterType) -> Option<RenderGutterColumn> {
    self
      .gutter_columns
      .iter()
      .copied()
      .find(|column| column.kind == kind)
  }
}

impl Default for RenderPlan {
  fn default() -> Self {
    Self::empty(Rect::default(), Position::default())
  }
}

fn remap_relative_row(
  relative_row: usize,
  scroll_row: usize,
  viewport_height: usize,
  row_insertions: &[RenderRowInsertion],
) -> Option<u16> {
  let absolute_row = scroll_row.saturating_add(relative_row);
  let inserted_before = row_insertions
    .iter()
    .filter(|insertion| insertion.base_row < absolute_row)
    .map(|insertion| insertion.inserted_rows)
    .sum::<usize>();
  let shifted_relative_row = relative_row.saturating_add(inserted_before);
  (shifted_relative_row < viewport_height).then_some(shifted_relative_row as u16)
}

pub fn apply_row_insertions(plan: &mut RenderPlan, row_insertions: &[RenderRowInsertion]) {
  if row_insertions.is_empty() {
    return;
  }
  let scroll_row = plan.scroll.row;
  let viewport_height = plan.viewport.height as usize;

  plan.lines.retain_mut(|line| {
    let Some(row) =
      remap_relative_row(line.row as usize, scroll_row, viewport_height, row_insertions)
    else {
      return false;
    };
    line.row = row;
    true
  });

  plan.visible_rows.retain_mut(|row| {
    let Some(shifted_row) =
      remap_relative_row(row.row as usize, scroll_row, viewport_height, row_insertions)
    else {
      return false;
    };
    row.row = shifted_row;
    true
  });

  plan.gutter_lines.retain_mut(|line| {
    let Some(row) =
      remap_relative_row(line.row as usize, scroll_row, viewport_height, row_insertions)
    else {
      return false;
    };
    line.row = row;
    true
  });

  plan.selections.retain_mut(|selection| {
    let Some(row) = remap_relative_row(
      selection.rect.y as usize,
      scroll_row,
      viewport_height,
      row_insertions,
    ) else {
      return false;
    };
    selection.rect.y = row;
    true
  });

  plan.cursors.retain_mut(|cursor| {
    let Some(row) =
      remap_relative_row(cursor.pos.row, scroll_row, viewport_height, row_insertions)
    else {
      return false;
    };
    cursor.pos.row = row as usize;
    true
  });
}

#[derive(Debug, Clone)]
pub struct PaneRenderPlan {
  pub pane_id:     PaneId,
  pub rect:        Rect,
  pub pane_kind:   PaneContentKind,
  pub terminal_id: Option<TerminalId>,
  pub plan:        RenderPlan,
}

#[derive(Debug, Clone)]
pub struct FrameRenderPlan {
  pub active_pane:               PaneId,
  pub panes:                     Vec<PaneRenderPlan>,
  pub frame_generation:          u64,
  pub pane_structure_generation: u64,
  pub changed_pane_ids:          Vec<PaneId>,
  pub damage_is_full:            bool,
  pub damage_reason:             RenderDamageReason,
}

impl FrameRenderPlan {
  pub fn empty() -> Self {
    Self {
      active_pane: default_pane_id(),
      panes:       Vec::new(),
      frame_generation: 0,
      pane_structure_generation: 0,
      changed_pane_ids: Vec::new(),
      damage_is_full: false,
      damage_reason: RenderDamageReason::None,
    }
  }

  pub fn from_active_plan(plan: RenderPlan) -> Self {
    let pane_id = default_pane_id();
    let rect = plan.viewport;
    Self {
      active_pane: pane_id,
      panes:       vec![PaneRenderPlan {
        pane_id,
        rect,
        pane_kind: PaneContentKind::EditorBuffer,
        terminal_id: None,
        plan,
      }],
      frame_generation: 0,
      pane_structure_generation: 0,
      changed_pane_ids: Vec::new(),
      damage_is_full: false,
      damage_reason: RenderDamageReason::None,
    }
  }

  pub fn active_plan(&self) -> Option<&RenderPlan> {
    self
      .panes
      .iter()
      .find(|pane| pane.pane_id == self.active_pane)
      .map(|pane| &pane.plan)
  }

  pub fn active_plan_mut(&mut self) -> Option<&mut RenderPlan> {
    self
      .panes
      .iter_mut()
      .find(|pane| pane.pane_id == self.active_pane)
      .map(|pane| &mut pane.plan)
  }

  pub fn into_active_plan(self) -> Option<RenderPlan> {
    self
      .panes
      .into_iter()
      .find(|pane| pane.pane_id == self.active_pane)
      .map(|pane| pane.plan)
  }
}

impl Default for FrameRenderPlan {
  fn default() -> Self {
    Self::empty()
  }
}

fn default_pane_id() -> PaneId {
  PaneId::new(NonZeroUsize::new(1).expect("nonzero"))
}

fn hash_value<T: Hash>(value: &T) -> u64 {
  let mut hasher = DefaultHasher::new();
  value.hash(&mut hasher);
  hasher.finish()
}

fn combine_hashes(values: &[u64]) -> u64 {
  hash_value(&values)
}

fn update_row_hash(row_hashes: &mut [u64], row: usize, value: impl Hash) {
  let Some(slot) = row_hashes.get_mut(row) else {
    return;
  };
  let mut hasher = DefaultHasher::new();
  slot.hash(&mut hasher);
  value.hash(&mut hasher);
  *slot = hasher.finish();
}

fn full_damage_end_row(plan: &RenderPlan) -> u16 {
  plan.viewport.height.saturating_sub(1)
}

fn nonzero_row_range(rows: &[u64]) -> Option<(u16, u16)> {
  let start = rows.iter().position(|hash| *hash != 0)? as u16;
  let end = rows.iter().rposition(|hash| *hash != 0)? as u16;
  Some((start, end))
}

fn row_damage(reason: RenderDamageReason, full: bool, start_row: u16, end_row: u16) -> (u16, u16, bool, RenderDamageReason) {
  (start_row, end_row, full, reason)
}

pub fn diff_row_hashes(previous: &[u64], next: &[u64]) -> Option<(u16, u16)> {
  let max_len = previous.len().max(next.len());
  let mut first = None;
  let mut last = None;
  for idx in 0..max_len {
    let previous_hash = previous.get(idx).copied().unwrap_or_default();
    let next_hash = next.get(idx).copied().unwrap_or_default();
    if previous_hash != next_hash {
      first.get_or_insert(idx as u16);
      last = Some(idx as u16);
    }
  }
  first.zip(last)
}

pub fn base_render_layer_row_hashes(plan: &RenderPlan) -> RenderLayerRowHashes {
  let row_count = plan.viewport.height as usize;
  let mut text_rows = vec![0; row_count];
  let mut decoration_rows = vec![0; row_count];
  let mut cursor_rows = vec![0; row_count];

  for line in &plan.lines {
    let row = line.row as usize;
    for span in &line.spans {
      update_row_hash(
        &mut text_rows,
        row,
        (
          span.col,
          span.cols,
          span.text.as_str(),
          span.highlight.map(|highlight| highlight.get()),
          span.is_virtual,
        ),
      );
    }
  }

  for line in &plan.gutter_lines {
    let row = line.row as usize;
    for span in &line.spans {
      update_row_hash(&mut text_rows, row, (span.col, span.text.as_str()));
    }
  }

  for selection in &plan.selections {
    update_row_hash(
      &mut decoration_rows,
      selection.rect.y as usize,
      (
        selection.rect.x,
        selection.rect.y,
        selection.rect.width,
        selection.rect.height,
        selection.kind,
      ),
    );
  }

  for overlay in &plan.overlays {
    match overlay {
      OverlayNode::Rect(rect) => {
        let start_row = rect.rect.y as usize;
        let end_row = start_row.saturating_add(rect.rect.height as usize).max(start_row + 1);
        for row in start_row..end_row.min(row_count) {
          update_row_hash(
            &mut decoration_rows,
            row,
            (
              rect.rect.x,
              rect.rect.width,
              rect.kind,
              rect.radius,
            ),
          );
        }
      },
      OverlayNode::Text(text) => {
        update_row_hash(
          &mut decoration_rows,
          text.pos.row,
          (text.pos.col, text.text.as_str()),
        );
      },
    }
  }

  for cursor in &plan.cursors {
    update_row_hash(
      &mut cursor_rows,
      cursor.pos.row,
      (
        cursor.id,
        cursor.pos.col,
        cursor.kind,
      ),
    );
  }

  RenderLayerRowHashes {
    text_rows,
    decoration_rows,
    cursor_rows,
  }
}

pub fn hash_render_plan_layout(plan: &RenderPlan) -> u64 {
  let gutter_columns = plan
    .gutter_columns
    .iter()
    .map(|column| (column.kind, column.col, column.width))
    .collect::<Vec<_>>();
  let visible_rows = plan
    .visible_rows
    .iter()
    .map(|row| (row.row, row.doc_line, row.first_visual_line))
    .collect::<Vec<_>>();
  hash_value(&(
    plan.viewport.x,
    plan.viewport.y,
    plan.viewport.width,
    plan.viewport.height,
    plan.content_offset_x,
    gutter_columns,
    visible_rows,
  ))
}

pub fn finish_render_generations(
  plan: &mut RenderPlan,
  previous: Option<&RenderGenerationState>,
  theme_generation: u64,
  row_hashes: RenderLayerRowHashes,
) -> RenderGenerationState {
  let layout_generation = hash_render_plan_layout(plan);
  let text_generation = combine_hashes(&row_hashes.text_rows);
  let decoration_generation = combine_hashes(&row_hashes.decoration_rows);
  let cursor_generation = combine_hashes(&row_hashes.cursor_rows);
  let scroll_generation = hash_value(&(plan.scroll.row, plan.scroll.col));

  let damage = if let Some(previous) = previous {
    if previous.theme_generation != theme_generation {
      row_damage(RenderDamageReason::Theme, true, 0, full_damage_end_row(plan))
    } else if previous.layout_generation != layout_generation {
      row_damage(RenderDamageReason::Layout, true, 0, full_damage_end_row(plan))
    } else if previous.scroll_generation != scroll_generation {
      row_damage(RenderDamageReason::Scroll, true, 0, full_damage_end_row(plan))
    } else if previous.text_generation != text_generation {
      let (start, end) =
        diff_row_hashes(&previous.text_rows, &row_hashes.text_rows).unwrap_or((0, full_damage_end_row(plan)));
      row_damage(RenderDamageReason::Text, false, start, end)
    } else if previous.decoration_generation != decoration_generation {
      let (start, end) = diff_row_hashes(
        &previous.decoration_rows,
        &row_hashes.decoration_rows,
      )
      .unwrap_or((0, full_damage_end_row(plan)));
      row_damage(RenderDamageReason::Decoration, false, start, end)
    } else if previous.cursor_generation != cursor_generation
      || previous.cursor_blink_generation != plan.cursor_blink_generation
    {
      let (start, end) = diff_row_hashes(&previous.cursor_rows, &row_hashes.cursor_rows)
        .or_else(|| nonzero_row_range(&row_hashes.cursor_rows))
        .or_else(|| nonzero_row_range(&previous.cursor_rows))
        .unwrap_or((0, full_damage_end_row(plan)));
      row_damage(RenderDamageReason::Cursor, false, start, end)
    } else {
      row_damage(RenderDamageReason::None, false, 0, 0)
    }
  } else {
    row_damage(RenderDamageReason::Full, true, 0, full_damage_end_row(plan))
  };

  plan.layout_generation = layout_generation;
  plan.text_generation = text_generation;
  plan.decoration_generation = decoration_generation;
  plan.cursor_generation = cursor_generation;
  plan.scroll_generation = scroll_generation;
  plan.theme_generation = theme_generation;
  plan.damage_start_row = damage.0;
  plan.damage_end_row = damage.1;
  plan.damage_is_full = damage.2;
  plan.damage_reason = damage.3;

  RenderGenerationState {
    layout_generation,
    text_generation,
    decoration_generation,
    cursor_generation,
    cursor_blink_generation: plan.cursor_blink_generation,
    scroll_generation,
    theme_generation,
    text_rows: row_hashes.text_rows,
    decoration_rows: row_hashes.decoration_rows,
    cursor_rows: row_hashes.cursor_rows,
  }
}

pub fn finish_frame_generations(
  frame: &mut FrameRenderPlan,
  previous: Option<&FrameGenerationState>,
  pane_states: BTreeMap<PaneId, RenderGenerationState>,
) -> FrameGenerationState {
  let pane_structure_generation = hash_value(
    &frame
      .panes
      .iter()
      .map(|pane| {
        (
          pane.pane_id,
          pane.rect.x,
          pane.rect.y,
          pane.rect.width,
          pane.rect.height,
          pane.pane_kind,
          pane.terminal_id,
        )
      })
      .collect::<Vec<_>>(),
  );
  let frame_generation = hash_value(
    &(
      frame.active_pane,
      pane_structure_generation,
      frame
        .panes
        .iter()
        .map(|pane| {
          (
            pane.pane_id,
            pane.plan.layout_generation,
            pane.plan.text_generation,
            pane.plan.decoration_generation,
            pane.plan.cursor_generation,
            pane.plan.scroll_generation,
            pane.plan.theme_generation,
          )
        })
        .collect::<Vec<_>>(),
    ),
  );

  let mut changed_pane_ids = Vec::new();
  let mut damage_is_full = previous.is_none();
  let mut damage_reason = if previous.is_none() {
    RenderDamageReason::Full
  } else {
    RenderDamageReason::None
  };

  if let Some(previous) = previous {
    if previous.pane_structure_generation != pane_structure_generation {
      damage_is_full = true;
      damage_reason = RenderDamageReason::PaneStructure;
    }

    for pane in &frame.panes {
      let current = pane_states.get(&pane.pane_id);
      let previous_state = previous.pane_states.get(&pane.pane_id);
      if current != previous_state {
        changed_pane_ids.push(pane.pane_id);
        if damage_reason == RenderDamageReason::None {
          damage_reason = pane.plan.damage_reason;
        }
        damage_is_full |= pane.plan.damage_is_full;
      }
    }

    for pane_id in previous.pane_states.keys() {
      if !pane_states.contains_key(pane_id) {
        changed_pane_ids.push(*pane_id);
      }
    }
  } else {
    changed_pane_ids.extend(frame.panes.iter().map(|pane| pane.pane_id));
  }

  frame.frame_generation = frame_generation;
  frame.pane_structure_generation = pane_structure_generation;
  frame.changed_pane_ids = changed_pane_ids.clone();
  frame.damage_is_full = damage_is_full;
  frame.damage_reason = damage_reason;

  FrameGenerationState {
    frame_generation,
    pane_structure_generation,
    pane_states,
  }
}

#[derive(Debug, Default)]
pub struct RenderCache {
  text_version:           Option<u64>,
  annotations_generation: Option<u64>,
  format_signature:       Option<crate::render::text_format::TextFormatSignature>,
  by_char:                BTreeMap<usize, Position>,
  by_pos:                 BTreeMap<Position, usize>,
}

impl RenderCache {
  pub(crate) fn reset_if_stale(
    &mut self,
    text_version: u64,
    annotations_generation: u64,
    format_signature: crate::render::text_format::TextFormatSignature,
  ) {
    let stale = self.text_version != Some(text_version)
      || self.annotations_generation != Some(annotations_generation)
      || self.format_signature.as_ref() != Some(&format_signature);
    if stale {
      self.text_version = Some(text_version);
      self.annotations_generation = Some(annotations_generation);
      self.format_signature = Some(format_signature);
      self.by_char.clear();
      self.by_pos.clear();
    }
  }

  pub(crate) fn insert_origin(&mut self, char_idx: usize, pos: Position) {
    if let Some(prev) = self.by_char.insert(char_idx, pos) {
      self.by_pos.remove(&prev);
    }
    if let Some(prev) = self.by_pos.insert(pos, char_idx) {
      self.by_char.remove(&prev);
    }
  }

  pub(crate) fn nearest_origin(&self, target: Position) -> Option<(usize, Position)> {
    let (pos, char_idx) = self.by_pos.range(..=target).next_back()?;
    Some((*char_idx, *pos))
  }
}

const ORIGIN_CACHE_ROW_STRIDE: usize = 32;

pub trait HighlightProvider {
  fn highlight_at(&mut self, char_idx: usize) -> Option<Highlight>;
}

#[derive(Debug, Default)]
pub struct NoHighlights;

impl HighlightProvider for NoHighlights {
  fn highlight_at(&mut self, _char_idx: usize) -> Option<Highlight> {
    None
  }
}

/// Build a render plan for the given document and view state.
///
/// This uses `DocumentFormatter` to produce visual positions. When soft wrap
/// and line annotations are disabled it fast-starts at the scroll position via
/// `visual_position`. Otherwise it uses `RenderCache` to resume from the
/// nearest cached origin, and records periodic row checkpoints while walking.
#[allow(clippy::too_many_arguments)]
pub fn build_plan<'a, 't, H: HighlightProvider>(
  doc: &'a Document,
  view: ViewState,
  text_fmt: &'a TextFormat,
  gutter: &GutterConfig,
  annotations: &'t mut TextAnnotations<'a>,
  highlights: &mut H,
  cache: &mut RenderCache,
  styles: RenderStyles,
) -> RenderPlan {
  let mut plan = RenderPlan::empty(view.viewport, view.scroll);
  let text = doc.text().slice(..);

  let line_number_width = line_number_column_width(doc, gutter);
  plan.gutter_columns = build_gutter_columns(gutter, line_number_width);
  plan.content_offset_x = if view.viewport.width == 0 {
    0
  } else {
    gutter_columns_width(&plan.gutter_columns).min(view.viewport.width.saturating_sub(1))
  };
  let content_width = if text_fmt.viewport_width == 0 {
    view.viewport.width.max(1) as usize
  } else {
    text_fmt.viewport_width as usize
  };

  cache.reset_if_stale(
    doc.version(),
    annotations.generation(),
    text_fmt.signature(),
  );

  let row_start = view.scroll.row;
  let row_end = row_start + view.viewport.height as usize;
  let col_start = view.scroll.col;

  let has_line_annotations = annotations.has_line_annotations();
  let use_fast_start = !text_fmt.soft_wrap && !has_line_annotations;
  let (block_char_idx, origin) = if use_fast_start {
    let start_char =
      visual_position::char_at_visual_pos(text, text_fmt, annotations, view.scroll).unwrap_or(0);
    let (block_char_idx, _) = prev_checkpoint(text, start_char);
    let origin = if start_char == 0 {
      Position::new(0, 0)
    } else {
      visual_position::visual_pos_at_char(text, text_fmt, annotations, block_char_idx)
        .unwrap_or_else(|| Position::new(0, 0))
    };
    (block_char_idx, origin)
  } else if let Some((char_idx, pos)) = cache.nearest_origin(view.scroll) {
    (char_idx, pos)
  } else {
    (0, Position::new(0, 0))
  };
  cache.insert_origin(block_char_idx, origin);

  {
    let mut formatter = DocumentFormatter::new_at_prev_checkpoint(
      doc.text().slice(..),
      text_fmt,
      annotations,
      block_char_idx,
    );

    let mut current_row: Option<usize> = None;
    let mut current_line = RenderLine::new(0);
    let mut visible_rows: Vec<Option<RenderVisibleRow>> = vec![None; view.viewport.height as usize];
    let mut last_cached_origin_row = origin.row;

    for grapheme in &mut formatter {
      if grapheme.source.is_eof() {
        break;
      }

      let rel_pos = grapheme.visual_pos;
      let abs_row = origin.row + rel_pos.row;
      let abs_col = if rel_pos.row == 0 {
        origin.col + rel_pos.col
      } else {
        rel_pos.col
      };

      if !grapheme.source.is_virtual()
        && abs_col == 0
        && abs_row >= last_cached_origin_row.saturating_add(ORIGIN_CACHE_ROW_STRIDE)
      {
        cache.insert_origin(grapheme.char_idx, Position::new(abs_row, abs_col));
        last_cached_origin_row = abs_row;
      }

      if abs_row >= row_start && abs_row < row_end {
        let row = abs_row - row_start;
        if row < visible_rows.len() {
          let first_visual_line = grapheme.char_idx == text.line_to_char(grapheme.line_idx);
          match &mut visible_rows[row] {
            Some(meta) => {
              meta.first_visual_line |= first_visual_line;
            },
            None => {
              visible_rows[row] = Some(RenderVisibleRow {
                row: row as u16,
                doc_line: grapheme.line_idx,
                first_visual_line,
              });
            },
          }
        }
      }

      if grapheme.raw == Grapheme::Newline {
        // Render the newline as a space character, matching Helix behavior
        // where newlines are selectable/visible graphemes occupying 1 cell.
        if abs_row >= row_start && abs_row < row_end && abs_col >= col_start {
          let col = abs_col - col_start;
          if col < content_width {
            let row = abs_row - row_start;
            if current_row != Some(abs_row) {
              if let Some(prev_row) = current_row {
                if prev_row >= row_start && prev_row < row_end {
                  plan.lines.push(current_line);
                }
              }
              current_row = Some(abs_row);
              current_line = RenderLine::new(row as u16);
            }
            let highlight = match grapheme.source {
              GraphemeSource::VirtualText { highlight } => highlight,
              GraphemeSource::Document {
                highlight: Some(highlight),
                ..
              } => Some(highlight),
              GraphemeSource::Document {
                highlight: None, ..
              } => highlights.highlight_at(grapheme.char_idx),
            };
            current_line.push_span(RenderSpan {
              col: col as u16,
              cols: 1,
              text: " ".into(),
              highlight,
              is_virtual: false,
            });
          }
        }

        if let Some(row) = current_row {
          if row >= row_start && row < row_end {
            plan.lines.push(current_line);
          }
        }
        current_row = None;
        current_line = RenderLine::new(0);
        continue;
      }

      if abs_row < row_start {
        continue;
      }
      if abs_row >= row_end {
        break;
      }

      if abs_col < col_start {
        continue;
      }

      let col = abs_col - col_start;
      if col >= content_width {
        continue;
      }

      let row = abs_row - row_start;
      if current_row != Some(abs_row) {
        if current_row.is_some() {
          plan.lines.push(current_line);
        }
        current_row = Some(abs_row);
        current_line = RenderLine::new(row as u16);
      }

      if let Some((text, cols)) = grapheme_text(&grapheme) {
        let highlight = match grapheme.source {
          GraphemeSource::VirtualText { highlight } => highlight,
          GraphemeSource::Document {
            highlight: Some(highlight),
            ..
          } => Some(highlight),
          GraphemeSource::Document {
            highlight: None, ..
          } => highlights.highlight_at(grapheme.char_idx),
        };

        current_line.push_span(RenderSpan {
          col: col as u16,
          cols: cols as u16,
          text,
          highlight,
          is_virtual: grapheme.source.is_virtual(),
        });
      }
    }

    if current_row.is_some() {
      plan.lines.push(current_line);
    }
    plan.visible_rows = visible_rows.into_iter().flatten().collect();
    plan.gutter_lines = build_gutter_lines(
      &plan.visible_rows,
      doc,
      view,
      gutter,
      &plan.gutter_columns,
      styles,
    );
  }

  add_selections_and_cursor(&mut plan, doc, text_fmt, annotations, view, styles);

  plan
}

fn line_number_column_width(doc: &Document, gutter: &GutterConfig) -> usize {
  if !gutter.layout.contains(&GutterType::LineNumbers) {
    return 0;
  }
  let lines = doc.text().len_lines().max(1);
  gutter.line_numbers.min_width.max(lines.to_string().len())
}

fn build_gutter_columns(
  gutter: &GutterConfig,
  line_number_width: usize,
) -> Vec<RenderGutterColumn> {
  let mut out = Vec::with_capacity(gutter.layout.len());
  let mut col = 0u16;
  for kind in &gutter.layout {
    let width = match kind {
      GutterType::Diagnostics | GutterType::Diff | GutterType::Spacer => 1,
      GutterType::LineNumbers => line_number_width as u16,
    };
    if width == 0 {
      continue;
    }
    out.push(RenderGutterColumn {
      kind: *kind,
      col,
      width,
    });
    col = col.saturating_add(width);
  }
  out
}

fn gutter_columns_width(columns: &[RenderGutterColumn]) -> u16 {
  columns
    .iter()
    .map(|column| column.col.saturating_add(column.width))
    .max()
    .unwrap_or(0)
}

pub fn gutter_width_for_document(
  doc: &Document,
  viewport_width: u16,
  gutter: &GutterConfig,
) -> u16 {
  let columns = build_gutter_columns(gutter, line_number_column_width(doc, gutter));
  if viewport_width == 0 {
    return 0;
  }
  gutter_columns_width(&columns).min(viewport_width.saturating_sub(1))
}

fn build_gutter_lines(
  visible_rows: &[RenderVisibleRow],
  doc: &Document,
  view: ViewState,
  gutter: &GutterConfig,
  columns: &[RenderGutterColumn],
  styles: RenderStyles,
) -> Vec<RenderGutterLine> {
  if columns.is_empty() {
    return Vec::new();
  }

  let active_line = active_doc_line(doc, view);
  let mut out = Vec::with_capacity(visible_rows.len());

  for row in visible_rows {
    let mut line = RenderGutterLine::new(row.row);
    for column in columns {
      match column.kind {
        GutterType::LineNumbers => {
          if row.first_visual_line
            && let Some(text) = line_number_text(
              gutter.line_numbers.mode,
              row.doc_line,
              active_line,
              column.width as usize,
            )
          {
            let style = if active_line.is_some_and(|line| line == row.doc_line) {
              styles.gutter_active
            } else {
              styles.gutter
            };
            line.push_span(RenderGutterSpan {
              col: column.col,
              text: text.into(),
              style,
            });
          }
        },
        GutterType::Diagnostics | GutterType::Diff | GutterType::Spacer => {},
      }
    }
    line.sort_spans();
    out.push(line);
  }

  out
}

fn line_number_text(
  mode: LineNumberMode,
  doc_line: usize,
  active_line: Option<usize>,
  width: usize,
) -> Option<String> {
  if width == 0 {
    return None;
  }

  let absolute = doc_line.saturating_add(1);
  let value = match mode {
    LineNumberMode::Absolute => absolute,
    LineNumberMode::Relative => {
      match active_line {
        Some(active) if active == doc_line => absolute,
        Some(active) => active.abs_diff(doc_line),
        None => absolute,
      }
    },
  };
  Some(format!("{value:>width$}"))
}

fn active_doc_line(doc: &Document, view: ViewState) -> Option<usize> {
  let text = doc.text().slice(..);
  let selection = doc.selection();
  if let Some(active_cursor) = view.active_cursor
    && let Some((_, range)) = selection
      .iter_with_ids()
      .find(|(cursor_id, _)| *cursor_id == active_cursor)
  {
    return Some(text.char_to_line(range.cursor(text)));
  }
  let range = selection.ranges().first()?;
  Some(text.char_to_line(range.cursor(text)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDiagnosticGutterStyles {
  pub error:   Style,
  pub warning: Style,
  pub info:    Style,
  pub hint:    Style,
}

impl Default for RenderDiagnosticGutterStyles {
  fn default() -> Self {
    Self {
      error:   Style::default(),
      warning: Style::default(),
      info:    Style::default(),
      hint:    Style::default(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderGutterDiffKind {
  Added,
  Modified,
  Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDiffGutterStyles {
  pub added:    Style,
  pub modified: Style,
  pub removed:  Style,
}

impl Default for RenderDiffGutterStyles {
  fn default() -> Self {
    Self {
      added:    Style::default(),
      modified: Style::default(),
      removed:  Style::default(),
    }
  }
}

pub fn apply_diagnostic_gutter_markers(
  plan: &mut RenderPlan,
  diagnostics_by_line: &BTreeMap<usize, DiagnosticSeverity>,
  styles: RenderDiagnosticGutterStyles,
) {
  let Some(column) = plan.gutter_column(GutterType::Diagnostics) else {
    return;
  };

  for (meta, line) in plan.visible_rows.iter().zip(plan.gutter_lines.iter_mut()) {
    line
      .spans
      .retain(|span| span.col < column.col || span.col >= column.col.saturating_add(column.width));

    if !meta.first_visual_line {
      continue;
    }

    let Some(severity) = diagnostics_by_line.get(&meta.doc_line).copied() else {
      continue;
    };

    let style = match severity {
      DiagnosticSeverity::Error => styles.error,
      DiagnosticSeverity::Warning => styles.warning,
      DiagnosticSeverity::Information => styles.info,
      DiagnosticSeverity::Hint => styles.hint,
    };
    line.push_span(RenderGutterSpan {
      col: column.col,
      text: "●".into(),
      style,
    });
    line.sort_spans();
  }
}

pub fn apply_diff_gutter_markers(
  plan: &mut RenderPlan,
  diff_by_line: &BTreeMap<usize, RenderGutterDiffKind>,
  styles: RenderDiffGutterStyles,
) {
  let Some(column) = plan.gutter_column(GutterType::Diff) else {
    return;
  };

  for (meta, line) in plan.visible_rows.iter().zip(plan.gutter_lines.iter_mut()) {
    line
      .spans
      .retain(|span| span.col < column.col || span.col >= column.col.saturating_add(column.width));

    if !meta.first_visual_line {
      continue;
    }

    let Some(kind) = diff_by_line.get(&meta.doc_line).copied() else {
      continue;
    };

    let (text, style) = match kind {
      RenderGutterDiffKind::Added => ("+", styles.added),
      RenderGutterDiffKind::Modified => ("~", styles.modified),
      RenderGutterDiffKind::Removed => ("-", styles.removed),
    };
    line.push_span(RenderGutterSpan {
      col: column.col,
      text: text.into(),
      style,
    });
    line.sort_spans();
  }
}

fn grapheme_text(grapheme: &FormattedGrapheme<'_>) -> Option<(Tendril, usize)> {
  match grapheme.raw {
    Grapheme::Newline => None,
    Grapheme::Tab { width } => {
      let spaces = " ".repeat(width);
      Some((spaces.into(), width))
    },
    Grapheme::Other { ref g } => {
      let s = g.to_string();
      let width = grapheme.raw.width();
      Some((s.into(), width))
    },
  }
}

fn add_selections_and_cursor<'a>(
  plan: &mut RenderPlan,
  doc: &'a Document,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
  view: ViewState,
  styles: RenderStyles,
) {
  let row_visible_end_cols = visible_line_end_cols(plan, doc, text_fmt, annotations);
  let selection = doc.selection();
  for (cursor_id, range) in selection.iter_with_ids() {
    let from = range.from();
    let to = range.to();
    if from == to {
      // Even when empty, still render a cursor below.
    } else {
      let start =
        visual_position::visual_pos_at_char(doc.text().slice(..), text_fmt, annotations, from);
      let end =
        visual_position::visual_pos_at_char(doc.text().slice(..), text_fmt, annotations, to);
      let (Some(start), Some(end)) = (start, end) else {
        continue;
      };

      push_selection_rects(
        plan,
        start,
        end,
        styles.selection,
        RenderSelectionKind::Primary,
        &row_visible_end_cols,
      );
    }

    let cursor_kind = if view.active_cursor == Some(cursor_id) {
      styles.active_cursor_kind
    } else {
      styles.cursor_kind
    };
    let cursor_pos = match cursor_kind {
      CursorKind::Block | CursorKind::Hollow => range.cursor(doc.text().slice(..)),
      CursorKind::Bar | CursorKind::Underline | CursorKind::Hidden => {
        if styles.non_block_cursor_uses_head {
          range.head
        } else {
          range.cursor(doc.text().slice(..))
        }
      },
    };
    if let Some(pos) =
      visual_position::visual_pos_at_char(doc.text().slice(..), text_fmt, annotations, cursor_pos)
    {
      if let Some(pos) = clamp_position(plan, pos) {
        let cursor_style = if view.active_cursor == Some(cursor_id) {
          styles.active_cursor
        } else {
          styles.cursor
        };
        plan.cursors.push(RenderCursor {
          id: cursor_id,
          pos,
          kind: cursor_kind,
          style: cursor_style,
        });
      }
    }
  }
}

pub fn add_selection_match_highlights<'a>(
  plan: &mut RenderPlan,
  doc: &'a Document,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
  view: ViewState,
  style: Style,
  options: SelectionMatchHighlightOptions,
) {
  if plan.visible_rows.is_empty() || options.max_matches == 0 {
    return;
  }

  let text = doc.text();
  let text_slice = text.slice(..);
  let selection = doc.selection();
  let active_range = if let Some(active_cursor) = view.active_cursor {
    selection.range_by_id(active_cursor).copied()
  } else {
    selection.ranges().first().copied()
  };
  let Some(active_range) = active_range else {
    return;
  };

  let text_len = text.len_chars();
  let (needle_from, needle_to) = if active_range.is_empty() {
    if !options.enable_point_cursor_match {
      return;
    }
    let cursor = active_range.cursor(text_slice).min(text_len);
    if cursor >= text_len {
      return;
    }
    let next = next_grapheme_boundary(text_slice, cursor);
    if next <= cursor {
      return;
    }
    (cursor, next.min(text_len))
  } else {
    let (line_from, line_to) = active_range.line_range(text_slice);
    if line_from != line_to {
      return;
    }
    (
      active_range.from().min(text_len),
      active_range.to().min(text_len),
    )
  };

  if needle_to <= needle_from {
    return;
  }

  let needle_chars = needle_to - needle_from;
  if needle_chars > options.max_needle_chars {
    return;
  }

  let needle = text.slice(needle_from..needle_to).to_string();
  if needle.is_empty() {
    return;
  }
  if needle.chars().all(char::is_whitespace) {
    return;
  }
  if needle.contains('\n') || needle.contains('\r') {
    return;
  }

  let row_visible_end_cols = visible_line_end_cols(plan, doc, text_fmt, annotations);
  let mut visible_lines = BTreeMap::<usize, usize>::new();
  for row in &plan.visible_rows {
    visible_lines
      .entry(row.doc_line)
      .or_insert_with(|| text.line_to_char(row.doc_line));
  }

  let needle_len_bytes = needle.len();
  let mut emitted = 0usize;
  for (line_idx, line_start) in visible_lines {
    if emitted >= options.max_matches {
      break;
    }
    if line_idx >= text.len_lines() {
      break;
    }

    let mut line = text.line(line_idx).to_string();
    while line.ends_with(['\n', '\r']) {
      line.pop();
    }
    if line.is_empty() {
      continue;
    }

    let mut search_from = 0usize;
    while search_from <= line.len() {
      let Some(rel) = line[search_from..].find(&needle) else {
        break;
      };
      let byte_start = search_from + rel;
      let byte_end = byte_start + needle_len_bytes;
      let local_start = line[..byte_start].chars().count();
      let local_end = local_start + needle_chars;
      let abs_start = line_start + local_start;
      let abs_end = line_start + local_end;

      if abs_start == needle_from && abs_end == needle_to {
        search_from = byte_end;
        continue;
      }

      let start = visual_position::visual_pos_at_char(text_slice, text_fmt, annotations, abs_start);
      let end = visual_position::visual_pos_at_char(text_slice, text_fmt, annotations, abs_end);
      if let (Some(start), Some(end)) = (start, end) {
        push_selection_rects(
          plan,
          start,
          end,
          style,
          RenderSelectionKind::Match,
          &row_visible_end_cols,
        );
        emitted = emitted.saturating_add(1);
        if emitted >= options.max_matches {
          break;
        }
      }

      search_from = byte_end;
    }
  }
}

fn clamp_position(plan: &RenderPlan, pos: Position) -> Option<Position> {
  let row_start = plan.scroll.row;
  let row_end = row_start + plan.viewport.height as usize;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.content_width();

  if pos.row < row_start || pos.row >= row_end {
    return None;
  }
  if pos.col < col_start || pos.col >= col_end {
    return None;
  }

  Some(Position::new(pos.row - row_start, pos.col - col_start))
}

fn visible_line_end_cols<'a>(
  plan: &RenderPlan,
  doc: &'a Document,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
) -> Vec<usize> {
  if !text_fmt.soft_wrap && !annotations.has_line_annotations() {
    let mut row_end_cols = vec![plan.scroll.col; plan.viewport.height as usize];
    let text = doc.text().slice(..);

    for (row, end_col) in row_end_cols.iter_mut().enumerate() {
      let abs_row = plan.scroll.row + row;
      if abs_row >= text.len_lines() {
        break;
      }

      let line = text.line(abs_row);
      let mut visual_col = 0usize;
      let mut has_line_ending = false;
      for grapheme in line.graphemes() {
        let g = grapheme_str(grapheme);
        let g = Grapheme::new(g, visual_col, text_fmt.tab_width);
        if matches!(g, Grapheme::Newline) {
          has_line_ending = true;
        }
        visual_col += g.width();
      }

      // Make newline/eof selection visible, matching Helix behavior where
      // both line endings and EOF are selectable graphemes.
      if !has_line_ending {
        visual_col = visual_col.saturating_add(1);
      }

      *end_col = plan.scroll.col + visual_col;
    }

    return row_end_cols;
  }

  let mut row_end_cols = vec![plan.scroll.col; plan.viewport.height as usize];
  for line in &plan.lines {
    let row = line.row as usize;
    if row >= row_end_cols.len() {
      continue;
    }

    let end_col = line
      .spans
      .iter()
      .map(|span| plan.scroll.col + span.end_col() as usize)
      .max()
      .unwrap_or(plan.scroll.col);
    row_end_cols[row] = row_end_cols[row].max(end_col);
  }
  row_end_cols
}

fn grapheme_str<'a>(grapheme: ropey::RopeSlice<'a>) -> GraphemeStr<'a> {
  match grapheme.as_str() {
    Some(slice) => GraphemeStr::from(slice),
    None => GraphemeStr::from(grapheme.to_string()),
  }
}

fn row_visible_end_col(plan: &RenderPlan, row: usize, row_visible_end_cols: &[usize]) -> usize {
  let row_start = plan.scroll.row;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.content_width();
  let relative = row.saturating_sub(row_start);
  row_visible_end_cols
    .get(relative)
    .copied()
    .unwrap_or(col_start)
    .min(col_end)
}

fn push_selection_rects(
  plan: &mut RenderPlan,
  start: Position,
  end: Position,
  style: Style,
  kind: RenderSelectionKind,
  row_visible_end_cols: &[usize],
) {
  let row_start = plan.scroll.row;
  let row_end = row_start + plan.viewport.height as usize;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.content_width();

  let start_row = start.row;
  let end_row = end.row;

  if start_row == end_row {
    let row = start_row;
    if row < row_start || row >= row_end {
      return;
    }
    let from = start.col.min(end.col);
    let mut to = start.col.max(end.col);
    let from = from.max(col_start);
    to = to.min(row_visible_end_col(plan, row, row_visible_end_cols));
    if to <= from {
      return;
    }
    plan.selections.push(RenderSelection {
      rect: Rect::new(
        (from - col_start) as u16,
        (row - row_start) as u16,
        (to - from) as u16,
        1,
      ),
      style,
      kind,
    });
    return;
  }

  for row in start_row..=end_row {
    if row < row_start || row >= row_end {
      continue;
    }

    let row_end_col = row_visible_end_col(plan, row, row_visible_end_cols);
    let (from, to) = if row == start_row {
      (start.col, row_end_col)
    } else if row == end_row {
      (col_start, end.col.min(row_end_col))
    } else {
      (col_start, row_end_col)
    };

    let from = from.max(col_start);
    let to = to.min(col_end);
    if to <= from {
      continue;
    }

    plan.selections.push(RenderSelection {
      rect: Rect::new(
        (from - col_start) as u16,
        (row - row_start) as u16,
        (to - from) as u16,
        1,
      ),
      style,
      kind,
    });
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::RefCell,
    rc::Rc,
  };

  use ropey::Rope;
  use smallvec::smallvec;

  use super::*;
  use crate::{
    diagnostics::DiagnosticSeverity,
    document::{
      Document,
      DocumentId,
    },
    render::{
      GutterConfig,
      InlineDiagnostic,
      InlineDiagnosticFilter,
      InlineDiagnosticsConfig,
      InlineDiagnosticsLineAnnotation,
      SharedInlineDiagnosticsRenderData,
      SyntaxHighlightAdapter,
      graphics::Color,
      text_annotations::Overlay,
    },
    selection::{
      Range,
      Selection,
    },
    syntax::HighlightCache,
  };

  fn no_gutter() -> GutterConfig {
    GutterConfig {
      layout: Vec::new(),
      ..GutterConfig::default()
    }
  }

  #[test]
  fn build_plan_simple_text() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("abc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.lines.len(), 1);
    assert_eq!(plan.lines[0].text(), "abc");
  }

  #[test]
  fn build_plan_scrolls_rows() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\nc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(1, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.lines.len(), 1);
    assert_eq!(plan.lines[0].text(), "b ");
  }

  #[test]
  fn build_plan_selection_and_cursor_rects() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("abcd\nefgh\nijkl\n"));
    let selection = Selection::new(smallvec![Range::new(7, 12), Range::point(6)]).unwrap();
    doc.set_selection(selection).unwrap();

    let view = ViewState::new(Rect::new(0, 0, 8, 2), Position::new(1, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.selections.len(), 2);
    assert_eq!(plan.selections[0].rect, Rect::new(2, 0, 3, 1));
    assert_eq!(plan.selections[1].rect, Rect::new(0, 1, 2, 1));
    assert_eq!(plan.selections[0].kind, RenderSelectionKind::Primary);
    assert_eq!(plan.selections[1].kind, RenderSelectionKind::Primary);

    assert_eq!(plan.cursors.len(), 2);
    let cursor_positions: Vec<_> = plan.cursors.iter().map(|c| c.pos).collect();
    assert!(cursor_positions.contains(&Position::new(0, 1)));
    assert!(cursor_positions.contains(&Position::new(1, 1)));
  }

  #[test]
  fn build_plan_bar_cursor_uses_selection_head_position() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("printf\n"));
    doc.set_selection(Selection::single(0, 6)).unwrap();

    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();
    let mut cache = RenderCache::default();
    let styles = RenderStyles {
      cursor_kind: CursorKind::Bar,
      active_cursor_kind: CursorKind::Bar,
      ..RenderStyles::default()
    };

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.cursors.len(), 1);
    assert_eq!(plan.cursors[0].kind, CursorKind::Bar);
    assert_eq!(plan.cursors[0].pos, Position::new(0, 6));
  }

  #[test]
  fn add_selection_match_highlights_marks_secondary_matches() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("alpha beta alpha\n"));
    doc.set_selection(Selection::single(0, 5)).unwrap();

    let view = ViewState::new(Rect::new(0, 0, 20, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();

    let mut plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    add_selection_match_highlights(
      &mut plan,
      &doc,
      &text_fmt,
      &mut annotations,
      view,
      Style::default().bg(Color::Rgb(75, 42, 115)),
      SelectionMatchHighlightOptions::default(),
    );

    assert_eq!(
      plan
        .selections
        .iter()
        .filter(|selection| selection.kind == RenderSelectionKind::Primary)
        .count(),
      1
    );
    assert_eq!(
      plan
        .selections
        .iter()
        .filter(|selection| selection.kind == RenderSelectionKind::Match)
        .count(),
      1
    );
    assert_eq!(
      plan
        .selections
        .iter()
        .find(|selection| selection.kind == RenderSelectionKind::Match)
        .expect("match selection")
        .rect,
      Rect::new(11, 0, 5, 1)
    );
  }

  #[test]
  fn build_plan_applies_highlight_spans() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("abc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();

    let mut highlight_cache = HighlightCache::default();
    highlight_cache.update_range(
      0..doc.text().len_bytes(),
      vec![(crate::syntax::Highlight::new(1), 1..2)],
      doc.text().slice(..),
      doc.version(),
      1,
    );
    let mut highlights =
      SyntaxHighlightAdapter::from_cache(doc.text().slice(..), &highlight_cache, 0..1);

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = no_gutter();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    let span_highlights: Vec<_> = plan.lines[0]
      .spans
      .iter()
      .filter_map(|span| span.highlight)
      .collect();
    assert!(span_highlights.contains(&crate::syntax::Highlight::new(1)));
  }

  #[test]
  fn build_plan_applies_overlay_annotation_highlight() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("abc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let overlay = vec![Overlay::new(1, "X")];
    let mut annotations = TextAnnotations::default();
    let _ = annotations.add_overlay(&overlay, Some(crate::syntax::Highlight::new(7)));

    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = no_gutter();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    let has_overlay_highlight = plan.lines[0].spans.iter().any(|span| {
      span.highlight == Some(crate::syntax::Highlight::new(7)) && span.text.as_str().contains('X')
    });
    assert!(has_overlay_highlight);
  }

  #[test]
  fn build_plan_exposes_visible_row_metadata() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\n\nabcdef"));
    let view = ViewState::new(Rect::new(0, 0, 4, 4), Position::new(0, 0));
    let mut text_fmt = TextFormat::default();
    text_fmt.soft_wrap = true;
    text_fmt.viewport_width = 4;

    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.visible_rows.len(), 4);
    assert_eq!(plan.visible_rows[0].doc_line, 0);
    assert!(plan.visible_rows[0].first_visual_line);
    assert_eq!(plan.visible_rows[1].doc_line, 1);
    assert!(plan.visible_rows[1].first_visual_line);
    assert_eq!(plan.visible_rows[2].doc_line, 2);
    assert!(plan.visible_rows[2].first_visual_line);
    assert_eq!(plan.visible_rows[3].doc_line, 2);
    assert!(!plan.visible_rows[3].first_visual_line);
  }

  #[test]
  fn build_plan_generates_line_number_gutter_payload() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\n"));
    let view = ViewState::new(Rect::new(0, 0, 20, 2), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = GutterConfig::default();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert!(plan.content_offset_x > 0);
    assert_eq!(plan.gutter_lines.len(), 2);
    let line0_text = plan.gutter_lines[0]
      .spans
      .iter()
      .map(|span| span.text.as_str())
      .collect::<Vec<_>>()
      .join("");
    let line1_text = plan.gutter_lines[1]
      .spans
      .iter()
      .map(|span| span.text.as_str())
      .collect::<Vec<_>>()
      .join("");
    assert!(line0_text.contains('1'));
    assert!(line1_text.contains('2'));
  }

  #[test]
  fn apply_diagnostic_markers_to_gutter_column() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\n"));
    let view = ViewState::new(Rect::new(0, 0, 20, 2), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = GutterConfig::default();

    let mut plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );
    let mut diagnostics = BTreeMap::new();
    diagnostics.insert(1, DiagnosticSeverity::Warning);
    apply_diagnostic_gutter_markers(
      &mut plan,
      &diagnostics,
      RenderDiagnosticGutterStyles::default(),
    );

    let row1 = plan
      .gutter_lines
      .iter()
      .find(|line| line.row == 1)
      .expect("row 1 exists");
    assert!(row1.spans.iter().any(|span| span.text == "●"));
  }

  #[test]
  fn apply_diff_markers_to_gutter_column() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\n"));
    let view = ViewState::new(Rect::new(0, 0, 20, 2), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = GutterConfig::default();

    let mut plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );
    let mut diff = BTreeMap::new();
    diff.insert(0, RenderGutterDiffKind::Modified);
    apply_diff_gutter_markers(&mut plan, &diff, RenderDiffGutterStyles::default());

    let row0 = plan
      .gutter_lines
      .iter()
      .find(|line| line.row == 0)
      .expect("row 0 exists");
    assert!(row0.spans.iter().any(|span| span.text == "~"));
  }

  #[test]
  fn build_plan_populates_origin_cache_for_line_annotations() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let text = (0..80)
      .map(|index| format!("line {index}\n"))
      .collect::<String>();
    let doc = Document::new(id, Rope::from(text));
    let view = ViewState::new(Rect::new(0, 0, 80, 40), Position::new(0, 0));
    let mut text_fmt = TextFormat::default();
    text_fmt.soft_wrap = false;
    text_fmt.viewport_width = 80;

    let diagnostic_char_idx = doc.text().line_to_char(8).saturating_add(2);
    let diagnostics = vec![InlineDiagnostic::new(
      diagnostic_char_idx,
      DiagnosticSeverity::Warning,
      "line annotation cache test",
    )];
    let config = InlineDiagnosticsConfig {
      cursor_line:          InlineDiagnosticFilter::Disable,
      other_lines:          InlineDiagnosticFilter::Enable(DiagnosticSeverity::Hint),
      min_diagnostic_width: 12,
      prefix_len:           1,
      max_wrap:             4,
      max_diagnostics:      2,
    };
    let render_data: SharedInlineDiagnosticsRenderData = Rc::new(RefCell::new(Default::default()));
    let annotation = InlineDiagnosticsLineAnnotation::new(
      diagnostics,
      usize::MAX,
      None,
      80,
      0,
      config,
      render_data,
    );

    let mut annotations = TextAnnotations::default();
    let _ = annotations.add_line_annotation(Box::new(annotation));
    let mut highlights = NoHighlights;
    let gutter = no_gutter();
    let mut cache = RenderCache::default();

    let _ = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      RenderStyles::default(),
    );

    let checkpoint = cache
      .nearest_origin(Position::new(ORIGIN_CACHE_ROW_STRIDE + 1, 0))
      .expect("line annotations should populate row checkpoints");
    assert!(checkpoint.1.row >= ORIGIN_CACHE_ROW_STRIDE);
  }

  #[test]
  fn frame_render_plan_wraps_single_active_plan() {
    let mut plan = RenderPlan::empty(Rect::new(1, 2, 10, 5), Position::new(3, 4));
    plan.content_offset_x = 2;

    let frame = FrameRenderPlan::from_active_plan(plan.clone());
    assert_eq!(frame.panes.len(), 1);
    assert_eq!(frame.panes[0].pane_kind, PaneContentKind::EditorBuffer);
    assert_eq!(frame.panes[0].terminal_id, None);
    assert_eq!(
      frame
        .active_plan()
        .expect("active pane exists")
        .content_offset_x,
      2
    );
    assert_eq!(
      frame
        .into_active_plan()
        .expect("active pane exists")
        .viewport,
      plan.viewport
    );
  }

  #[test]
  fn empty_frame_render_plan_has_no_active_plan() {
    let frame = FrameRenderPlan::empty();
    assert!(frame.active_plan().is_none());
    assert!(frame.into_active_plan().is_none());
  }

  #[test]
  fn finish_render_generations_tracks_text_row_damage() {
    let mut initial = RenderPlan::empty(Rect::new(0, 0, 12, 3), Position::new(0, 0));
    initial.lines.push(RenderLine {
      row: 0,
      spans: vec![RenderSpan {
        col: 0,
        cols: 3,
        text: "abc".into(),
        highlight: None,
        is_virtual: false,
      }],
    });

    let initial_rows = base_render_layer_row_hashes(&initial);
    let previous = finish_render_generations(&mut initial, None, 0, initial_rows);
    assert!(initial.damage_is_full);
    assert_eq!(initial.damage_reason, RenderDamageReason::Full);

    let mut updated = initial.clone();
    updated.lines[0].spans[0].text = "abd".into();
    let updated_rows = base_render_layer_row_hashes(&updated);
    let next = finish_render_generations(&mut updated, Some(&previous), 0, updated_rows);

    assert_eq!(updated.damage_reason, RenderDamageReason::Text);
    assert!(!updated.damage_is_full);
    assert_eq!(updated.damage_start_row, 0);
    assert_eq!(updated.damage_end_row, 0);
    assert_ne!(previous.text_generation, next.text_generation);
  }

  #[test]
  fn finish_frame_generations_tracks_changed_panes() {
    let pane_id = PaneId::new(std::num::NonZeroUsize::new(2).unwrap());
    let mut initial_plan = RenderPlan::empty(Rect::new(0, 0, 12, 3), Position::new(0, 0));
    initial_plan.lines.push(RenderLine {
      row: 0,
      spans: vec![RenderSpan {
        col: 0,
        cols: 3,
        text: "abc".into(),
        highlight: None,
        is_virtual: false,
      }],
    });
    let initial_plan_rows = base_render_layer_row_hashes(&initial_plan);
    let initial_pane_state =
      finish_render_generations(&mut initial_plan, None, 0, initial_plan_rows);

    let mut frame = FrameRenderPlan {
      active_pane: pane_id,
      panes: vec![PaneRenderPlan {
        pane_id,
        rect: Rect::new(0, 0, 12, 3),
        pane_kind: PaneContentKind::EditorBuffer,
        terminal_id: None,
        plan: initial_plan.clone(),
      }],
      frame_generation: 0,
      pane_structure_generation: 0,
      changed_pane_ids: Vec::new(),
      damage_is_full: false,
      damage_reason: RenderDamageReason::None,
    };
    let initial_frame_state = finish_frame_generations(
      &mut frame,
      None,
      BTreeMap::from([(pane_id, initial_pane_state.clone())]),
    );
    assert!(frame.damage_is_full);
    assert_eq!(frame.damage_reason, RenderDamageReason::Full);
    assert_eq!(frame.changed_pane_ids, vec![pane_id]);

    let mut updated_plan = initial_plan.clone();
    updated_plan.lines[0].spans[0].text = "xyz".into();
    let updated_plan_rows = base_render_layer_row_hashes(&updated_plan);
    let updated_pane_state = finish_render_generations(
      &mut updated_plan,
      Some(&initial_pane_state),
      0,
      updated_plan_rows,
    );
    let mut updated_frame = FrameRenderPlan {
      active_pane: pane_id,
      panes: vec![PaneRenderPlan {
        pane_id,
        rect: Rect::new(0, 0, 12, 3),
        pane_kind: PaneContentKind::EditorBuffer,
        terminal_id: None,
        plan: updated_plan,
      }],
      frame_generation: 0,
      pane_structure_generation: 0,
      changed_pane_ids: Vec::new(),
      damage_is_full: false,
      damage_reason: RenderDamageReason::None,
    };
    let next_frame_state = finish_frame_generations(
      &mut updated_frame,
      Some(&initial_frame_state),
      BTreeMap::from([(pane_id, updated_pane_state)]),
    );

    assert!(!updated_frame.damage_is_full);
    assert_eq!(updated_frame.damage_reason, RenderDamageReason::Text);
    assert_eq!(updated_frame.changed_pane_ids, vec![pane_id]);
    assert_ne!(
      initial_frame_state.frame_generation,
      next_frame_state.frame_generation
    );
  }
}
