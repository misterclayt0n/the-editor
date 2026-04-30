use std::{
  collections::BTreeMap,
  ops::Range,
};

use super::{
  Damage,
  RenderFrameDelta,
  RenderRow,
  RowId,
  RowMetadata,
  WidgetId,
  layers::{
    CursorPlacement,
    SelectionSpan,
    SyntaxSpan,
    WidgetPlacement,
  },
};
use crate::selection::CursorId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRow {
  pub row:      RenderRow,
  pub metadata: Option<RowMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceFrame {
  pub generation: u64,
  pub rows:       Vec<FrameRow>,
  pub selections: Vec<SelectionSpan>,
  pub cursors:    Vec<CursorPlacement>,
  pub syntax:     Vec<SyntaxSpan>,
  pub widgets:    Vec<WidgetPlacement>,
}

impl SurfaceFrame {
  pub fn row(&self, surface_row: usize) -> Option<&FrameRow> {
    self
      .rows
      .iter()
      .find(|row| row.row.placement.surface_row == surface_row)
  }

  pub fn selections(&self) -> &[SelectionSpan] {
    &self.selections
  }

  pub fn cursors(&self) -> &[CursorPlacement] {
    &self.cursors
  }

  pub fn syntax(&self) -> &[SyntaxSpan] {
    &self.syntax
  }

  pub fn widgets(&self) -> &[WidgetPlacement] {
    &self.widgets
  }

  pub fn selection_spans(&self, surface_row: usize) -> Vec<SelectionSpan> {
    self
      .selections
      .iter()
      .copied()
      .filter(|span| span.surface_row == surface_row)
      .collect()
  }

  pub fn cursor_placements(&self, surface_row: usize) -> Vec<CursorPlacement> {
    self
      .cursors
      .iter()
      .copied()
      .filter(|cursor| cursor.surface_row == surface_row)
      .collect()
  }

  pub fn syntax_spans(&self, surface_row: usize) -> Vec<SyntaxSpan> {
    self
      .syntax
      .iter()
      .copied()
      .filter(|span| span.surface_row == surface_row)
      .collect()
  }

  pub fn widget_placement(&self, widget_id: WidgetId) -> Option<WidgetPlacement> {
    self
      .widgets
      .iter()
      .copied()
      .find(|widget| widget.widget_id == widget_id)
  }

  pub fn primary_cursor_screen_pos(
    &self,
    primary_cursor: CursorId,
    text_col_offset: usize,
    max_cols: usize,
    max_rows: usize,
  ) -> Option<(usize, usize)> {
    let cursor = self
      .cursors
      .iter()
      .find(|cursor| cursor.cursor_id == primary_cursor)?;
    if cursor.surface_row >= max_rows || cursor.col >= max_cols {
      return None;
    }
    Some((text_col_offset + cursor.col, cursor.surface_row))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentUpdate {
  pub generation:    u64,
  pub full:          bool,
  pub damage:        Damage,
  pub changed_rows:  usize,
  pub retained_rows: usize,
  pub removed_rows:  usize,
  pub row_metadata:  usize,
  pub selections:    usize,
  pub cursors:       usize,
  pub syntax:        usize,
  pub widgets:       usize,
}

impl Default for PresentUpdate {
  fn default() -> Self {
    Self {
      generation:    0,
      full:          false,
      damage:        Damage::empty(),
      changed_rows:  0,
      retained_rows: 0,
      removed_rows:  0,
      row_metadata:  0,
      selections:    0,
      cursors:       0,
      syntax:        0,
      widgets:       0,
    }
  }
}

impl PresentUpdate {
  pub fn redraw_rows(self, max_rows: usize) -> Range<usize> {
    if self.full {
      0..max_rows
    } else {
      self.damage.start_row.min(max_rows)..self.damage.end_row.min(max_rows)
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RetainedFrame {
  generation: u64,
  rows:       BTreeMap<RowId, FrameRow>,
  selections: Vec<SelectionSpan>,
  cursors:    Vec<CursorPlacement>,
  syntax:     Vec<SyntaxSpan>,
  widgets:    Vec<WidgetPlacement>,
}

impl RetainedFrame {
  pub fn generation(&self) -> u64 {
    self.generation
  }

  pub fn is_empty(&self) -> bool {
    self.generation == 0 && self.rows.is_empty()
  }

  pub fn row(&self, surface_row: usize) -> Option<&FrameRow> {
    self
      .rows
      .values()
      .find(|row| row.row.placement.surface_row == surface_row)
  }

  pub fn selections(&self) -> &[SelectionSpan] {
    &self.selections
  }

  pub fn cursors(&self) -> &[CursorPlacement] {
    &self.cursors
  }

  pub fn syntax(&self) -> &[SyntaxSpan] {
    &self.syntax
  }

  pub fn widgets(&self) -> &[WidgetPlacement] {
    &self.widgets
  }

  pub fn selection_spans(&self, surface_row: usize) -> Vec<SelectionSpan> {
    self
      .selections
      .iter()
      .copied()
      .filter(|span| span.surface_row == surface_row)
      .collect()
  }

  pub fn cursor_placements(&self, surface_row: usize) -> Vec<CursorPlacement> {
    self
      .cursors
      .iter()
      .copied()
      .filter(|cursor| cursor.surface_row == surface_row)
      .collect()
  }

  pub fn syntax_spans(&self, surface_row: usize) -> Vec<SyntaxSpan> {
    self
      .syntax
      .iter()
      .copied()
      .filter(|span| span.surface_row == surface_row)
      .collect()
  }

  pub fn widget_placement(&self, widget_id: WidgetId) -> Option<WidgetPlacement> {
    self
      .widgets
      .iter()
      .copied()
      .find(|widget| widget.widget_id == widget_id)
  }

  pub fn primary_cursor_screen_pos(
    &self,
    primary_cursor: CursorId,
    text_col_offset: usize,
    max_cols: usize,
    max_rows: usize,
  ) -> Option<(usize, usize)> {
    let cursor = self
      .cursors
      .iter()
      .find(|cursor| cursor.cursor_id == primary_cursor)?;
    if cursor.surface_row >= max_rows || cursor.col >= max_cols {
      return None;
    }
    Some((text_col_offset + cursor.col, cursor.surface_row))
  }

  pub fn snapshot(&self) -> SurfaceFrame {
    let mut rows = self.rows.values().cloned().collect::<Vec<_>>();
    rows.sort_by_key(|row| row.row.placement.surface_row);

    SurfaceFrame {
      generation: self.generation,
      rows,
      selections: self.selections.clone(),
      cursors: self.cursors.clone(),
      syntax: self.syntax.clone(),
      widgets: self.widgets.clone(),
    }
  }

  pub fn apply_delta(&mut self, delta: &RenderFrameDelta) -> PresentUpdate {
    if delta.full {
      self.rows.clear();
      self.selections.clear();
      self.cursors.clear();
      self.syntax.clear();
      self.widgets.clear();
    }

    for row_id in &delta.removed_rows {
      self.rows.remove(row_id);
    }

    for movement in &delta.retained_rows {
      if let Some(row) = self.rows.get_mut(&movement.id) {
        row.row.placement = movement.to;
      }
    }

    for row in &delta.changed_rows {
      let metadata = self
        .rows
        .get(&row.id)
        .and_then(|existing| existing.metadata.clone());
      self.rows.insert(row.id, FrameRow {
        row: row.clone(),
        metadata,
      });
    }

    if let Some(row_metadata) = &delta.row_metadata {
      for metadata in &row_metadata.rows {
        if let Some(row) = self.rows.get_mut(&metadata.row_id) {
          row.metadata = Some(metadata.clone());
        }
      }
    }

    if delta.full || delta.layers.selections.is_some() {
      self.selections = delta
        .layers
        .selections
        .as_ref()
        .map(|layer| layer.spans.clone())
        .unwrap_or_default();
    }
    if delta.full || delta.layers.cursors.is_some() {
      self.cursors = delta
        .layers
        .cursors
        .as_ref()
        .map(|layer| layer.cursors.clone())
        .unwrap_or_default();
    }
    if delta.full || delta.layers.syntax.is_some() {
      self.syntax = delta
        .layers
        .syntax
        .as_ref()
        .map(|layer| layer.spans.clone())
        .unwrap_or_default();
    }
    if delta.full || delta.layers.widgets.is_some() {
      self.widgets = delta
        .layers
        .widgets
        .as_ref()
        .map(|layer| layer.widgets.clone())
        .unwrap_or_default();
    }

    self.generation = delta.generation;

    PresentUpdate {
      generation:    delta.generation,
      full:          delta.full,
      damage:        delta.damage,
      changed_rows:  delta.changed_rows.len(),
      retained_rows: delta.retained_rows.len(),
      removed_rows:  delta.removed_rows.len(),
      row_metadata:  delta
        .row_metadata
        .as_ref()
        .map(|row_metadata| row_metadata.rows.len())
        .unwrap_or(0),
      selections:    self.selections.len(),
      cursors:       self.cursors.len(),
      syntax:        self.syntax.len(),
      widgets:       self.widgets.len(),
    }
  }
}

impl From<&RenderFrameDelta> for SurfaceFrame {
  fn from(delta: &RenderFrameDelta) -> Self {
    let mut retained = RetainedFrame::default();
    retained.apply_delta(delta);
    retained.snapshot()
  }
}
