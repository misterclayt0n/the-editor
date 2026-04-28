use thiserror::Error;

use crate::{
  document::{
    Document,
    DocumentError,
  },
  edit::Edit,
  history::{
    History,
    HistoryError,
    HistoryJump,
    UndoKind,
  },
  movement::{
    MotionResult,
    VisualGoals,
  },
  render::{
    BlockWidget,
    FrameRequest,
    RenderFrameDelta,
    RenderSurface,
    SurfaceMeasurements,
    SyntaxHighlight,
    TextAnnotations,
    TextFormat,
    Viewport,
    visual_pos_at_char,
  },
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
    SelectionError,
  },
};

#[derive(Debug, Error)]
pub enum SurfaceError {
  #[error(transparent)]
  Document(#[from] DocumentError),

  #[error(transparent)]
  History(#[from] HistoryError),

  #[error(transparent)]
  Selection(#[from] SelectionError),

  #[error("selection is out of bounds for document length {len}")]
  SelectionOutOfBounds { len: usize },

  #[error("active cursor {id} is not present in the surface selection")]
  ActiveCursorNotFound { id: u64 },

  #[error("document/history length mismatch: expected {expected}, got {got}")]
  DocumentHistoryLengthMismatch { expected: usize, got: usize },

  #[error("history jump transaction #{index} expected document length {expected}, got {got}")]
  HistoryJumpLengthMismatch {
    index:    usize,
    expected: usize,
    got:      usize,
  },
}

pub type Result<T> = std::result::Result<T, SurfaceError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealMode {
  None,
  Minimal,
  Center,
}

#[derive(Debug)]
pub struct Surface {
  selection:     Selection,
  active_cursor: Option<CursorId>,
  visual_goals:  VisualGoals,
  history:       History,
  viewport:      Viewport,
  render:        RenderSurface,
}

impl Surface {
  pub fn new(document: &Document, render: RenderSurface, viewport: Viewport) -> Result<Self> {
    Self::with_selection(document, Selection::point(0), render, viewport)
  }

  pub fn with_selection(
    document: &Document,
    selection: Selection,
    mut render: RenderSurface,
    viewport: Viewport,
  ) -> Result<Self> {
    validate_selection_bounds(&selection, document.text().len_chars())?;

    let history = History::new(document.text().len_chars(), selection.clone())?;
    render.set_selection(Some(selection.clone()));

    Ok(Self {
      active_cursor: selection.cursor_ids().first().copied(),
      selection,
      visual_goals: VisualGoals::new(),
      history,
      viewport,
      render,
    })
  }

  pub fn selection(&self) -> &Selection {
    &self.selection
  }

  pub fn active_cursor(&self) -> Option<CursorId> {
    self.active_cursor
  }

  pub fn visual_goals(&self) -> &VisualGoals {
    &self.visual_goals
  }

  pub fn history(&self) -> &History {
    &self.history
  }

  pub fn viewport(&self) -> Viewport {
    self.viewport
  }

  pub fn set_viewport(&mut self, viewport: Viewport) {
    self.viewport = viewport;
  }

  pub fn render_surface(&self) -> &RenderSurface {
    &self.render
  }

  pub fn render_generation(&self) -> u64 {
    self.render.generation()
  }

  pub fn text_format(&self) -> &TextFormat {
    self.render.text_format()
  }

  pub fn set_text_format(&mut self, format: TextFormat) {
    self.render.set_text_format(format);
  }

  pub fn text_annotations(&self) -> &TextAnnotations {
    self.render.text_annotations()
  }

  pub fn set_text_annotations(&mut self, annotations: TextAnnotations) {
    self.render.set_text_annotations(annotations);
  }

  pub fn syntax_highlights(&self) -> &[SyntaxHighlight] {
    self.render.syntax_highlights()
  }

  pub fn set_syntax_highlights(&mut self, highlights: Vec<SyntaxHighlight>) {
    self.render.set_syntax_highlights(highlights);
  }

  pub fn measurements(&self) -> SurfaceMeasurements {
    self.render.measurements()
  }

  pub fn set_measurements(&mut self, measurements: SurfaceMeasurements) {
    self.render.set_measurements(measurements);
  }

  pub fn widgets(&self) -> &[BlockWidget] {
    self.render.widgets()
  }

  pub fn set_widgets(&mut self, widgets: Vec<BlockWidget>) {
    self.render.set_widgets(widgets);
  }

  pub fn set_active_cursor(&mut self, active_cursor: Option<CursorId>) -> Result<()> {
    if let Some(id) = active_cursor
      && self.selection.by_id(id).is_none()
    {
      return Err(SurfaceError::ActiveCursorNotFound { id: id.get() });
    }

    self.active_cursor = active_cursor;
    Ok(())
  }

  pub fn primary_cursor_id(&self) -> CursorId {
    self
      .active_cursor
      .filter(|id| self.selection.by_id(*id).is_some())
      .unwrap_or(self.selection.cursor_ids()[0])
  }

  pub fn primary_index(&self) -> usize {
    self
      .selection
      .index_of(self.primary_cursor_id())
      .expect("primary cursor must be present in selection")
  }

  pub fn primary_cursor(&self) -> CursorRange {
    self
      .selection
      .pick(crate::selection::CursorPick::Id(self.primary_cursor_id()))
      .expect("primary cursor must be present in selection")
  }

  pub fn primary_range(&self) -> Range {
    self.primary_cursor().range
  }

  pub fn apply_edit(&mut self, document: &mut Document, edit: Edit) -> Result<()> {
    if edit.transaction.changes().is_empty() {
      return self.apply_selection(document, edit.selection);
    }

    self.preflight_edit(document, &edit)?;

    let before = document.text().clone();
    document.apply_transaction(&edit.transaction)?;
    self
      .history
      .commit(&before, edit.transaction.clone(), edit.selection.clone())
      .expect("preflighted surface edit commit must succeed");

    self.finish_selection_update(
      document,
      edit.selection,
      VisualGoals::new(),
      RevealMode::Minimal,
    );
    Ok(())
  }

  pub fn apply_selection(&mut self, document: &Document, selection: Selection) -> Result<()> {
    validate_selection_bounds(&selection, document.text().len_chars())?;
    self.finish_selection_update(document, selection, VisualGoals::new(), RevealMode::Minimal);
    Ok(())
  }

  pub fn apply_motion_result(&mut self, document: &Document, result: MotionResult) -> Result<()> {
    validate_selection_bounds(&result.selection, document.text().len_chars())?;

    let mut goals = result.visual_goals;
    goals.retain_ids(result.selection.cursor_ids().iter().copied());
    self.finish_selection_update(document, result.selection, goals, RevealMode::Minimal);
    Ok(())
  }

  pub fn undo(&mut self, document: &mut Document) -> Result<bool> {
    let Some(jump) = self.history.undo() else {
      return Ok(false);
    };
    self.apply_history_jump(document, jump)?;
    Ok(true)
  }

  pub fn redo(&mut self, document: &mut Document) -> Result<bool> {
    let Some(jump) = self.history.redo() else {
      return Ok(false);
    };
    self.apply_history_jump(document, jump)?;
    Ok(true)
  }

  pub fn earlier(&mut self, document: &mut Document, kind: UndoKind) -> Result<bool> {
    let jump = self.history.earlier(kind)?;
    if jump.is_empty() {
      return Ok(false);
    }
    self.apply_history_jump(document, jump)?;
    Ok(true)
  }

  pub fn later(&mut self, document: &mut Document, kind: UndoKind) -> Result<bool> {
    let jump = self.history.later(kind)?;
    if jump.is_empty() {
      return Ok(false);
    }
    self.apply_history_jump(document, jump)?;
    Ok(true)
  }

  pub fn apply_history_jump(&mut self, document: &mut Document, jump: HistoryJump) -> Result<()> {
    self.preflight_history_jump(document, &jump)?;

    for transaction in &jump.transactions {
      document.apply_transaction(transaction)?;
    }
    self.history.apply_jump(&jump)?;

    self.finish_selection_update(
      document,
      jump.selection,
      VisualGoals::new(),
      RevealMode::Minimal,
    );
    Ok(())
  }

  pub fn reveal_primary_cursor(&mut self, document: &Document, mode: RevealMode) {
    if matches!(mode, RevealMode::None) {
      return;
    }

    let char_idx = self.primary_range().cursor(document.text().slice(..));
    let Some(position) = visual_pos_at_char(
      document.text().slice(..),
      self.render.text_format(),
      self.viewport.wrap_width,
      self.render.text_annotations(),
      char_idx,
    ) else {
      return;
    };

    match mode {
      RevealMode::None => {},
      RevealMode::Minimal => {
        if self.viewport.row_count > 0 {
          if position.row < self.viewport.first_row {
            self.viewport.first_row = position.row;
          } else {
            let last_visible = self.viewport.first_row + self.viewport.row_count.saturating_sub(1);
            if position.row > last_visible {
              self.viewport.first_row = position
                .row
                .saturating_add(1)
                .saturating_sub(self.viewport.row_count);
            }
          }
        }

        if self.viewport.wrap_width > 0 {
          if position.col < self.viewport.first_col {
            self.viewport.first_col = position.col;
          } else {
            let last_visible = self.viewport.first_col + self.viewport.wrap_width.saturating_sub(1);
            if position.col > last_visible {
              self.viewport.first_col = position
                .col
                .saturating_add(1)
                .saturating_sub(self.viewport.wrap_width);
            }
          }
        }
      },
      RevealMode::Center => {
        if self.viewport.row_count > 0 {
          self.viewport.first_row = position.row.saturating_sub(self.viewport.row_count / 2);
        }
        if self.viewport.wrap_width > 0 {
          self.viewport.first_col = position.col.saturating_sub(self.viewport.wrap_width / 2);
        }
      },
    }
  }

  pub fn render(&mut self, document: &Document, since_generation: Option<u64>) -> RenderFrameDelta {
    self.render.render_frame(
      document.text().slice(..),
      document.revision(),
      FrameRequest {
        viewport: self.viewport,
        since_generation,
      },
    )
  }

  fn finish_selection_update(
    &mut self,
    document: &Document,
    selection: Selection,
    visual_goals: VisualGoals,
    reveal: RevealMode,
  ) {
    let previous_active = self.active_cursor;
    self.selection = selection;
    self.visual_goals = visual_goals;
    self.repair_active_cursor(previous_active);
    self.sync_render_selection();
    self.reveal_primary_cursor(document, reveal);
  }

  fn sync_render_selection(&mut self) {
    self.render.set_selection(Some(self.selection.clone()));
  }

  fn repair_active_cursor(&mut self, previous_active: Option<CursorId>) {
    self.active_cursor = match previous_active {
      Some(id) if self.selection.by_id(id).is_some() => Some(id),
      Some(_) => self.selection.cursor_ids().first().copied(),
      None => None,
    };
  }

  fn preflight_edit(&self, document: &Document, edit: &Edit) -> Result<()> {
    self.ensure_document_matches_history(document)?;

    let expected = document.text().len_chars();
    if edit.transaction.changes().len() != expected {
      return Err(SurfaceError::DocumentHistoryLengthMismatch {
        expected,
        got: edit.transaction.changes().len(),
      });
    }

    validate_selection_bounds(&edit.selection, edit.transaction.changes().len_after())?;
    Ok(())
  }

  fn preflight_history_jump(&self, document: &Document, jump: &HistoryJump) -> Result<()> {
    self.ensure_document_matches_history(document)?;

    let mut len = document.text().len_chars();
    for (index, transaction) in jump.transactions.iter().enumerate() {
      if transaction.changes().len() != len {
        return Err(SurfaceError::HistoryJumpLengthMismatch {
          index,
          expected: len,
          got: transaction.changes().len(),
        });
      }
      len = transaction.changes().len_after();
    }

    validate_selection_bounds(&jump.selection, len)?;
    Ok(())
  }

  fn ensure_document_matches_history(&self, document: &Document) -> Result<()> {
    let expected = self.history.current_len();
    let got = document.text().len_chars();
    if expected != got {
      return Err(SurfaceError::DocumentHistoryLengthMismatch { expected, got });
    }
    Ok(())
  }
}

fn validate_selection_bounds(selection: &Selection, len: usize) -> Result<()> {
  if selection
    .ranges()
    .iter()
    .any(|range| range.anchor > len || range.head > len)
  {
    return Err(SurfaceError::SelectionOutOfBounds { len });
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;
  use crate::{
    movement::{
      MotionResult,
      VisualGoal,
      VisualGoals,
    },
    render::{
      SurfaceId,
      TextAnnotations,
      TextFormat,
    },
    selection::Range,
    transaction::Transaction,
  };

  fn render_surface() -> RenderSurface {
    RenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    )
  }

  fn viewport() -> Viewport {
    Viewport::new(0, 2, 0, 4)
  }

  #[test]
  fn new_surface_initializes_history_and_syncs_render_selection() {
    let document = Document::new(Rope::from("hello\nworld\n"));
    let surface = Surface::new(&document, render_surface(), viewport()).unwrap();

    assert_eq!(surface.selection().ranges(), &[Range::point(0)]);
    assert_eq!(
      surface.active_cursor(),
      surface.selection().cursor_ids().first().copied()
    );
    assert_eq!(surface.history().current_len(), document.text().len_chars());
    assert_eq!(
      surface.render_surface().selection(),
      Some(surface.selection())
    );
  }

  #[test]
  fn apply_edit_updates_document_history_selection_and_render_state() {
    let mut document = Document::new(Rope::from("hello"));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();
    let original_active = surface.active_cursor().unwrap();

    let edit = Edit {
      transaction: Transaction::insert_at(document.text().len_chars(), 5, " world").unwrap(),
      selection:   Selection::new_with_ids_unchecked(
        smallvec::smallvec![Range::point(11)],
        smallvec::smallvec![original_active],
      ),
    };

    surface.apply_edit(&mut document, edit).unwrap();

    assert_eq!(document.text().to_string(), "hello world");
    assert_eq!(surface.history().current_len(), 11);
    assert_eq!(surface.selection().ranges(), &[Range::point(11)]);
    assert_eq!(surface.active_cursor(), Some(original_active));
    assert_eq!(
      surface.render_surface().selection(),
      Some(surface.selection())
    );
  }

  #[test]
  fn apply_selection_repairs_removed_active_cursor_and_clears_visual_goals() {
    let document = Document::new(Rope::from("abcdef"));
    let mut surface = Surface::with_selection(
      &document,
      Selection::new(smallvec::smallvec![Range::point(1), Range::point(4)]).unwrap(),
      render_surface(),
      viewport(),
    )
    .unwrap();
    let second = surface.selection().cursor_ids()[1];

    surface.set_active_cursor(Some(second)).unwrap();
    surface.visual_goals.insert(second, VisualGoal { col: 9 });
    surface
      .apply_selection(
        &document,
        Selection::new(smallvec::smallvec![Range::point(2)]).unwrap(),
      )
      .unwrap();

    assert_eq!(
      surface.active_cursor(),
      Some(surface.selection().cursor_ids()[0])
    );
    assert_ne!(surface.active_cursor(), Some(second));
    assert_eq!(surface.visual_goals(), &VisualGoals::new());
    assert_eq!(
      surface.primary_cursor_id(),
      surface.selection().cursor_ids()[0]
    );
  }

  #[test]
  fn apply_motion_result_keeps_visual_goals() {
    let document = Document::new(Rope::from("abc\ndef\n"));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();
    let cursor_id = surface.primary_cursor_id();

    let mut goals = VisualGoals::new();
    goals.insert(cursor_id, VisualGoal { col: 3 });

    surface
      .apply_motion_result(&document, MotionResult {
        selection:    Selection::new_with_ids_unchecked(
          smallvec::smallvec![Range::point(4)],
          smallvec::smallvec![cursor_id],
        ),
        visual_goals: goals.clone(),
      })
      .unwrap();

    assert_eq!(surface.selection().ranges(), &[Range::point(4)]);
    assert_eq!(surface.visual_goals(), &goals);
    assert_eq!(surface.active_cursor(), Some(cursor_id));
  }

  #[test]
  fn reveal_primary_cursor_scrolls_minimally() {
    let document = Document::new(Rope::from("a\nb\nc\nd\n"));
    let mut surface = Surface::new(&document, render_surface(), Viewport::new(0, 2, 0, 4)).unwrap();

    surface
      .apply_selection(
        &document,
        Selection::new(smallvec::smallvec![Range::point(6)]).unwrap(),
      )
      .unwrap();

    assert_eq!(surface.viewport().first_row, 2);
  }

  #[test]
  fn undo_and_redo_restore_document_and_selection() {
    let mut document = Document::new(Rope::from("hello"));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();

    let len = document.text().len_chars();
    surface
      .apply_edit(&mut document, Edit {
        transaction: Transaction::insert_at(len, 5, "!").unwrap(),
        selection:   Selection::new(smallvec::smallvec![Range::point(6)]).unwrap(),
      })
      .unwrap();

    assert!(surface.undo(&mut document).unwrap());
    assert_eq!(document.text().to_string(), "hello");
    assert_eq!(surface.selection().ranges(), &[Range::point(0)]);

    assert!(surface.redo(&mut document).unwrap());
    assert_eq!(document.text().to_string(), "hello!");
    assert_eq!(surface.selection().ranges(), &[Range::point(6)]);
  }
}
