use thiserror::Error;

use crate::{
  document::{
    Document,
    DocumentError,
  },
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
  operation::Edit,
  render::{
    CellGridMetrics,
    CellGridRenderSurface,
    CursorSemantics,
    RenderFrame,
    SyntaxSpan,
    TextFormat,
    Viewport,
    layout_index::{
      LayoutIndex,
      LayoutInputs,
      TextChangeSummary,
    },
  },
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
    SelectionError,
  },
  syntax::{
    LanguageRegistry,
    SyntaxGeneration,
  },
  transaction::TransactionError,
};

#[derive(Debug, Error)]
pub enum SurfaceError {
  #[error(transparent)]
  Document(#[from] DocumentError),

  #[error(transparent)]
  History(#[from] HistoryError),

  #[error(transparent)]
  Selection(#[from] SelectionError),

  #[error(transparent)]
  Transaction(#[from] TransactionError),

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceSyntaxInterest {
  pub viewport:   Viewport,
  pub char_range: std::ops::Range<usize>,
  pub byte_range: std::ops::Range<usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct VisibleSyntaxCache {
  document_revision: Option<u64>,
  generation:        Option<SyntaxGeneration>,
  viewport:          Option<Viewport>,
  spans:             Vec<SyntaxSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceViewSpec {
  pub viewport:         Viewport,
  pub format:           TextFormat,
  pub measurements:     CellGridMetrics,
  pub cursor_semantics: CursorSemantics,
}

impl SurfaceViewSpec {
  pub fn new(viewport: Viewport) -> Self {
    Self {
      viewport,
      format: TextFormat::default(),
      measurements: CellGridMetrics::default(),
      cursor_semantics: CursorSemantics::Cell,
    }
  }
}

#[derive(Debug)]
struct PendingHistoryCommit {
  before:           ropey::Rope,
  transaction:      crate::transaction::Transaction,
  selection_before: Selection,
  selection_after:  Selection,
}

#[derive(Debug)]
pub struct Surface {
  selection:      Selection,
  active_cursor:  Option<CursorId>,
  visual_goals:   VisualGoals,
  history:        History,
  pending_commit: Option<PendingHistoryCommit>,
  viewport:       Viewport,
  layout_index:   LayoutIndex,
  render:         CellGridRenderSurface,
  syntax_cache:   VisibleSyntaxCache,
}

impl Surface {
  pub fn new(
    document: &Document,
    render: CellGridRenderSurface,
    viewport: Viewport,
  ) -> Result<Self> {
    Self::with_selection(document, Selection::point(0), render, viewport)
  }

  pub fn with_selection(
    document: &Document,
    selection: Selection,
    mut render: CellGridRenderSurface,
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
      pending_commit: None,
      viewport,
      layout_index: LayoutIndex::default(),
      render,
      syntax_cache: VisibleSyntaxCache::default(),
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
    if self.viewport != viewport {
      self.viewport = viewport;
      self.invalidate_visible_syntax_cache();
    }
  }

  pub fn set_viewport_reconcile_document(&mut self, document: &Document, mut viewport: Viewport) {
    if self.render.text_format().soft_wrap {
      viewport.first_col = 0;
    }
    viewport.first_row = viewport
      .first_row
      .min(self.max_scroll_row(document, viewport));
    self.set_viewport(viewport);
  }

  pub fn view_spec(&self) -> SurfaceViewSpec {
    SurfaceViewSpec {
      viewport:         self.viewport,
      format:           self.render.text_format().clone(),
      measurements:     self.render.measurements(),
      cursor_semantics: self.render.cursor_semantics(),
    }
  }

  pub fn set_view(&mut self, spec: SurfaceViewSpec) {
    self.set_viewport(spec.viewport);
    self.set_text_format(spec.format);
    self.set_measurements(spec.measurements);
    self.set_cursor_semantics(spec.cursor_semantics);
  }

  pub fn render_surface(&self) -> &CellGridRenderSurface {
    &self.render
  }

  pub fn syntax_interest(&mut self, document: &Document) -> SurfaceSyntaxInterest {
    let text = document.text().slice(..);
    let (char_range, byte_range) = self.render.visible_document_ranges(
      text,
      document.revision(),
      self.viewport,
      &mut self.layout_index,
    );

    SurfaceSyntaxInterest {
      viewport: self.viewport,
      char_range,
      byte_range,
    }
  }

  pub fn visible_syntax(&self) -> &[SyntaxSpan] {
    self.render.visible_syntax()
  }

  pub fn text_format(&self) -> &TextFormat {
    self.render.text_format()
  }

  pub fn set_text_format(&mut self, format: TextFormat) {
    if self.render.text_format() == &format {
      return;
    }
    self.render.set_text_format(format);
    self.layout_index.invalidate_all();
    self.invalidate_visible_syntax_cache();
  }

  pub fn set_text_format_reconcile_viewport(&mut self, document: &Document, format: TextFormat) {
    if self.render.text_format() == &format {
      return;
    }
    self.render.set_text_format(format.clone());
    self.layout_index.invalidate_all();
    self.invalidate_visible_syntax_cache();

    if format.soft_wrap {
      self.viewport.first_col = 0;
    }

    self.reveal_primary_cursor(document, RevealMode::Minimal);
  }

  pub fn measurements(&self) -> CellGridMetrics {
    self.render.measurements()
  }

  pub fn set_measurements(&mut self, measurements: CellGridMetrics) {
    if self.render.measurements() == measurements {
      return;
    }
    self.render.set_measurements(measurements);
    self.invalidate_visible_syntax_cache();
  }

  pub fn cursor_semantics(&self) -> CursorSemantics {
    self.render.cursor_semantics()
  }

  pub fn set_cursor_semantics(&mut self, semantics: CursorSemantics) {
    self.render.set_cursor_semantics(semantics);
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
    let change_extent =
      summarize_text_change(edit.transaction.changes().changes()).unwrap_or(TextChangeExtent {
        start_old_char: before.len_chars(),
        end_old_char:   before.len_chars(),
        end_new_char:   before.len_chars(),
      });
    let start_line = before
      .slice(..)
      .char_to_line(change_extent.start_old_char.min(before.len_chars()));
    let old_end_line = before
      .slice(..)
      .char_to_line(change_extent.end_old_char.min(before.len_chars()))
      .max(start_line);
    document.apply_transaction(&edit.transaction)?;
    let new_text = document.text().slice(..);
    let new_end_line = new_text
      .char_to_line(change_extent.end_new_char.min(new_text.len_chars()))
      .max(start_line.min(new_text.len_lines().saturating_sub(1)));
    self.layout_index.note_text_change(TextChangeSummary {
      start_line: start_line.min(new_text.len_lines().saturating_sub(1)),
      old_end_line,
      new_end_line,
    });
    self.queue_history_commit(before, edit.transaction.clone(), edit.selection.clone())?;

    self.finish_selection_update(
      document,
      edit.selection,
      VisualGoals::new(),
      RevealMode::Minimal,
    );
    Ok(())
  }

  pub fn apply_selection(&mut self, document: &Document, selection: Selection) -> Result<()> {
    self.apply_selection_with_reveal(document, selection, RevealMode::Minimal)
  }

  pub fn apply_selection_without_reveal(
    &mut self,
    document: &Document,
    selection: Selection,
  ) -> Result<()> {
    self.apply_selection_with_reveal(document, selection, RevealMode::None)
  }

  pub fn apply_selection_with_reveal(
    &mut self,
    document: &Document,
    selection: Selection,
    reveal: RevealMode,
  ) -> Result<()> {
    validate_selection_bounds(&selection, document.text().len_chars())?;
    self.finish_selection_update(document, selection, VisualGoals::new(), reveal);
    Ok(())
  }

  pub fn apply_motion_result(&mut self, document: &Document, result: MotionResult) -> Result<()> {
    validate_selection_bounds(&result.selection, document.text().len_chars())?;

    let mut goals = result.visual_goals;
    goals.retain_ids(result.selection.cursor_ids().iter().copied());
    self.finish_selection_update(document, result.selection, goals, RevealMode::Minimal);
    Ok(())
  }

  pub fn commit_undo_checkpoint(&mut self) -> Result<()> {
    let Some(pending) = self.pending_commit.take() else {
      return Ok(());
    };
    self.history.commit(
      &pending.before,
      pending.transaction,
      pending.selection_before,
      pending.selection_after,
    )?;
    Ok(())
  }

  fn queue_history_commit(
    &mut self,
    before: ropey::Rope,
    transaction: crate::transaction::Transaction,
    selection_after: Selection,
  ) -> Result<()> {
    if transaction.changes().is_empty() {
      return Ok(());
    }

    match self.pending_commit.take() {
      None => {
        self.pending_commit = Some(PendingHistoryCommit {
          before,
          transaction,
          selection_before: self.selection.clone(),
          selection_after,
        });
      },
      Some(mut pending) => {
        pending.transaction = pending.transaction.compose(transaction)?;
        pending.selection_after = selection_after;
        self.pending_commit = Some(pending);
      },
    }
    Ok(())
  }

  pub fn undo(&mut self, document: &mut Document) -> Result<bool> {
    self.commit_undo_checkpoint()?;
    let Some(jump) = self.history.undo() else {
      return Ok(false);
    };
    self.apply_history_jump(document, jump)?;
    Ok(true)
  }

  pub fn redo(&mut self, document: &mut Document) -> Result<bool> {
    self.commit_undo_checkpoint()?;
    let Some(jump) = self.history.redo() else {
      return Ok(false);
    };
    self.apply_history_jump(document, jump)?;
    Ok(true)
  }

  pub fn earlier(&mut self, document: &mut Document, kind: UndoKind) -> Result<bool> {
    self.commit_undo_checkpoint()?;
    let jump = self.history.earlier(kind)?;
    if jump.is_empty() {
      return Ok(false);
    }
    self.apply_history_jump(document, jump)?;
    Ok(true)
  }

  pub fn later(&mut self, document: &mut Document, kind: UndoKind) -> Result<bool> {
    self.commit_undo_checkpoint()?;
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
    self.layout_index.invalidate_all();
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

    let text = document.text().slice(..);
    let char_idx = self.primary_range().cursor(text);
    let render = &self.render;
    let inputs = LayoutInputs {
      text,
      text_revision: document.revision(),
      format: render.text_format(),
      wrap_width: self.viewport.wrap_width,
    };
    let Some(position) = self.layout_index.position_for_char(inputs, char_idx) else {
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

        if self.render.text_format().soft_wrap {
          self.viewport.first_col = 0;
        } else if self.viewport.wrap_width > 0 {
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
        if self.render.text_format().soft_wrap {
          self.viewport.first_col = 0;
        } else if self.viewport.wrap_width > 0 {
          self.viewport.first_col = position.col.saturating_sub(self.viewport.wrap_width / 2);
        }
      },
    }
  }

  pub fn raw_frame_delta(
    &mut self,
    document: &Document,
    syntax_registry: Option<&LanguageRegistry>,
  ) -> RenderFrame {
    self.sync_visible_syntax(document, syntax_registry);
    self.render.render_frame_with_index(
      document.text().slice(..),
      document.revision(),
      self.active_cursor,
      self.viewport,
      &mut self.layout_index,
    )
  }

  pub fn render(
    &mut self,
    document: &Document,
    syntax_registry: Option<&LanguageRegistry>,
  ) -> RenderFrame {
    self.raw_frame_delta(document, syntax_registry)
  }

  fn max_scroll_row(&mut self, document: &Document, viewport: Viewport) -> usize {
    let text = document.text().slice(..);
    let inputs = LayoutInputs {
      text,
      text_revision: document.revision(),
      format: self.render.text_format(),
      wrap_width: viewport.wrap_width,
    };
    let last_row = self
      .layout_index
      .position_for_char(inputs, text.len_chars())
      .map(|position| position.row)
      .unwrap_or(0);
    last_row.saturating_sub(viewport.row_count.saturating_sub(1))
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

  fn sync_visible_syntax(
    &mut self,
    document: &Document,
    syntax_registry: Option<&LanguageRegistry>,
  ) {
    let generation = document.syntax_generation();
    let cache_valid = self.syntax_cache.document_revision == Some(document.revision())
      && self.syntax_cache.generation == generation
      && self.syntax_cache.viewport == Some(self.viewport);

    if cache_valid {
      self
        .render
        .set_visible_syntax(self.syntax_cache.spans.clone());
      return;
    }

    let spans = syntax_registry
      .and_then(|registry| {
        project_visible_syntax(
          document,
          &mut self.render,
          &mut self.layout_index,
          self.viewport,
          registry,
        )
      })
      .unwrap_or_default();

    self.syntax_cache = VisibleSyntaxCache {
      document_revision: Some(document.revision()),
      generation,
      viewport: Some(self.viewport),
      spans: spans.clone(),
    };
    self.render.set_visible_syntax(spans);
  }

  fn invalidate_visible_syntax_cache(&mut self) {
    self.syntax_cache = VisibleSyntaxCache::default();
    self.render.set_visible_syntax(Vec::new());
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
    let expected = self
      .pending_commit
      .as_ref()
      .map(|pending| pending.transaction.changes().len_after())
      .unwrap_or_else(|| self.history.current_len());
    let got = document.text().len_chars();
    if expected != got {
      return Err(SurfaceError::DocumentHistoryLengthMismatch { expected, got });
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextChangeExtent {
  start_old_char: usize,
  end_old_char:   usize,
  end_new_char:   usize,
}

fn summarize_text_change(changes: &[crate::transaction::Operation]) -> Option<TextChangeExtent> {
  let mut old_char = 0usize;
  let mut new_char = 0usize;
  let mut start_old = None;
  let mut end_old = 0usize;
  let mut end_new = 0usize;

  for change in changes {
    match change {
      crate::transaction::Operation::Retain(n) => {
        old_char += n;
        new_char += n;
      },
      crate::transaction::Operation::Delete(n) => {
        start_old.get_or_insert(old_char);
        old_char += n;
        end_old = old_char;
        end_new = new_char;
      },
      crate::transaction::Operation::Insert(text) => {
        start_old.get_or_insert(old_char);
        new_char += text.chars().count();
        end_old = old_char;
        end_new = new_char;
      },
    }
  }

  Some(TextChangeExtent {
    start_old_char: start_old?,
    end_old_char:   end_old,
    end_new_char:   end_new,
  })
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

fn project_visible_syntax(
  document: &Document,
  render: &mut CellGridRenderSurface,
  layout_index: &mut LayoutIndex,
  viewport: Viewport,
  registry: &LanguageRegistry,
) -> Option<Vec<SyntaxSpan>> {
  let state = document.active_syntax()?;
  let syntax = state.syntax();
  let text = document.text().slice(..);
  let (_char_range, byte_range) =
    render.visible_document_ranges(text, document.revision(), viewport, layout_index);
  if byte_range.is_empty() {
    return Some(Vec::new());
  }

  let runs = syntax.highlight_runs_in_byte_range(text, registry, byte_range);
  Some(render.project_syntax_runs(text, document.revision(), viewport, &runs, layout_index))
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
      TextFormat,
    },
    selection::{
      Range,
      Selection,
    },
    transaction::Transaction,
  };

  fn render_surface() -> CellGridRenderSurface {
    CellGridRenderSurface::new(SurfaceId::new(1).unwrap(), TextFormat::default())
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
    surface.commit_undo_checkpoint().unwrap();

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
  fn enabling_soft_wrap_resets_horizontal_scroll_and_reveals() {
    let document = Document::new(Rope::from("0123456789abcdef\n"));
    let mut surface = Surface::new(&document, render_surface(), Viewport::new(0, 2, 6, 4)).unwrap();

    surface
      .apply_selection(
        &document,
        Selection::new(smallvec::smallvec![Range::point(2)]).unwrap(),
      )
      .unwrap();

    let mut format = surface.text_format().clone();
    format.soft_wrap = true;
    surface.set_text_format_reconcile_viewport(&document, format);

    assert_eq!(surface.viewport().first_col, 0);
    assert_eq!(surface.viewport().first_row, 0);
  }

  #[test]
  fn pending_edits_are_grouped_until_undo_checkpoint() {
    let mut document = Document::new(Rope::from(""));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();

    surface
      .apply_edit(&mut document, Edit {
        transaction: Transaction::insert_at(0, 0, "a").unwrap(),
        selection:   Selection::new(smallvec::smallvec![Range::point(1)]).unwrap(),
      })
      .unwrap();
    surface
      .apply_edit(&mut document, Edit {
        transaction: Transaction::insert_at(1, 1, "b").unwrap(),
        selection:   Selection::new(smallvec::smallvec![Range::point(2)]).unwrap(),
      })
      .unwrap();

    surface.commit_undo_checkpoint().unwrap();
    assert!(surface.undo(&mut document).unwrap());
    assert_eq!(document.text().to_string(), "");
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

  #[test]
  fn undo_restores_selection_that_existed_before_an_edit_checkpoint() {
    let mut document = Document::new(Rope::from("int {\n\n}"));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();

    surface
      .apply_selection(&document, Selection::single(0, 3))
      .unwrap();
    let len = document.text().len_chars();
    surface
      .apply_edit(&mut document, Edit {
        transaction: Transaction::from_replacements(
          len,
          [
            crate::transaction::Replacement::insert(0, "\""),
            crate::transaction::Replacement::insert(3, "\""),
          ],
          crate::transaction::ReplacementOverlapPolicy::Reject,
        )
        .unwrap(),
        selection:   Selection::single(0, 5),
      })
      .unwrap();
    surface.commit_undo_checkpoint().unwrap();

    assert!(surface.undo(&mut document).unwrap());
    assert_eq!(document.text().to_string(), "int {\n\n}");
    assert_eq!(surface.selection().ranges(), &[Range::new(0, 3)]);
  }

  #[test]
  fn render_shows_cursor_in_empty_document() {
    let document = Document::new(Rope::from(""));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();

    let frame = surface.render(&document, None);

    assert_eq!(frame.rows.len(), 1);
    assert_eq!(frame.rows[0].text.to_string(), "");
    assert_eq!(frame.cursors.len(), 1);
    assert_eq!(frame.cursors[0].surface_row, 0);
    assert_eq!(frame.cursors[0].col, 0);
  }

  #[test]
  fn render_shows_cursor_on_trailing_empty_line() {
    let document = Document::new(Rope::from("abc\n"));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();
    surface
      .apply_selection(
        &document,
        Selection::new(smallvec::smallvec![Range::point(4)]).unwrap(),
      )
      .unwrap();

    let frame = surface.render(&document, None);

    assert_eq!(frame.rows.len(), 2);
    assert_eq!(frame.rows[1].text.to_string(), "");
    assert_eq!(frame.rows[1].doc_line, Some(1));
    assert!(frame.rows[1].starts_logical_line);
    assert!(!frame.rows[1].wrapped_continuation);
    assert_eq!(frame.cursors.len(), 1);
    assert_eq!(frame.cursors[0].surface_row, 1);
    assert_eq!(frame.cursors[0].col, 0);
  }

  #[test]
  fn render_selection_includes_newline_cell() {
    let document = Document::new(Rope::from("abc\n"));
    let mut surface = Surface::new(&document, render_surface(), viewport()).unwrap();
    surface
      .apply_selection(&document, Selection::single(0, 4))
      .unwrap();

    let frame = surface.render(&document, None);

    assert_eq!(frame.selections.len(), 1);
    assert_eq!(frame.selections[0].surface_row, 0);
    assert_eq!(frame.selections[0].start_col, 0);
    assert_eq!(frame.selections[0].end_col, 4);
  }
}
