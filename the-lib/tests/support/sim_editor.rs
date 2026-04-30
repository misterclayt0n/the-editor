use std::error::Error;

use the_lib::{
  Tendril,
  auto_pairs::{
    AutoPairs,
    edit_for_backspace,
    edit_for_typed_char,
  },
  document::Document,
  edit::Edit,
  grapheme::{
    next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  indent::indent_for_newline,
  line_ending::get_line_ending,
  movement::{
    MotionMode,
    MovementContext,
    move_document_end,
    move_document_start,
    move_down_visual,
    move_left,
    move_line_end,
    move_line_start,
    move_right,
    move_up_visual,
  },
  registers::Registers,
  render::{
    CellGridMetrics,
    CellGridRenderSurface,
    Damage,
    RenderFrameDelta,
    SurfaceId,
    TextAnnotations,
    TextFormat,
    Viewport,
  },
  search::{
    build_regex,
    search_regex,
  },
  selection::{
    CursorPick,
    Range,
    Selection,
  },
  surface::Surface,
  syntax::IndentationHeuristic,
  transaction::{
    DeletionOverlapPolicy,
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
  },
};

#[derive(Debug, Clone)]
pub enum Op {
  InsertChar(char),
  InsertText(&'static str),
  InsertNewline,
  Backspace,
  DeleteForward,
  MoveLeft,
  MoveRight,
  MoveUp,
  MoveDown,
  MoveLineStart,
  MoveLineEnd,
  MoveDocumentStart,
  MoveDocumentEnd,
  SelectAll,
  Copy,
  Cut,
  Paste,
  Search(&'static str),
  RepeatSearchForward,
  Undo,
  Redo,
  ToggleWrap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
  text:              String,
  selection:         Vec<(u64, usize, usize)>,
  active_cursor:     Option<u64>,
  revision:          u64,
  viewport:          Viewport,
  render_generation: u64,
  registers:         Vec<(char, Vec<String>)>,
}

pub struct SimEditor {
  pub document:   Document,
  pub surface:    Surface,
  pub registers:  Registers,
  pub auto_pairs: AutoPairs,
}

impl SimEditor {
  pub fn new(initial_text: &str) -> Self {
    let document = Document::new(initial_text.into());
    let mut render = CellGridRenderSurface::new(
      SurfaceId::new(1).unwrap(),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    render.set_measurements(CellGridMetrics::new(1, 1));

    let surface = Surface::new(&document, render, Viewport::new(0, 8, 0, 4)).unwrap();
    let mut editor = Self {
      document,
      surface,
      registers: Registers::new(),
      auto_pairs: AutoPairs::default(),
    };
    editor.assert_invariants("init");
    editor
  }

  pub fn run_script(initial_text: &str, ops: &[Op]) -> Self {
    let mut editor = Self::new(initial_text);

    for (index, op) in ops.iter().enumerate() {
      let before = editor.snapshot();
      if let Err(err) = editor.apply_op(op) {
        panic!(
          "script failed at step {index} with op {op:?}\nerror: {err}\nbefore: \
           {before:#?}\nafter: {:#?}",
          editor.snapshot()
        );
      }
      editor.assert_invariants(&format!("step {index} {op:?}"));
    }

    editor
  }

  pub fn snapshot(&self) -> Snapshot {
    Snapshot {
      text:              self.document.text().to_string(),
      selection:         self
        .surface
        .selection()
        .iter()
        .map(|cursor| (cursor.id.get(), cursor.range.anchor, cursor.range.head))
        .collect(),
      active_cursor:     self.surface.active_cursor().map(|id| id.get()),
      revision:          self.document.revision(),
      viewport:          self.surface.viewport(),
      render_generation: self.surface.render_generation(),
      registers:         self
        .registers
        .iter()
        .map(|(name, values)| {
          (
            name,
            values
              .iter()
              .map(|value| value.to_string())
              .collect::<Vec<_>>(),
          )
        })
        .collect(),
    }
  }

  pub fn assert_invariants(&mut self, label: &str) {
    let len = self.document.text().len_chars();

    for cursor in self.surface.selection().iter() {
      assert!(
        cursor.range.anchor <= len && cursor.range.head <= len,
        "{label}: selection out of bounds: {:#?}",
        self.snapshot()
      );
    }

    if let Some(active) = self.surface.active_cursor() {
      assert!(
        self.surface.selection().by_id(active).is_some(),
        "{label}: active cursor missing from selection: {:#?}",
        self.snapshot()
      );
    }

    assert_eq!(
      self.surface.history().current_len(),
      len,
      "{label}: history/document length mismatch: {:#?}",
      self.snapshot()
    );

    for (cursor_id, _) in self.surface.visual_goals().iter() {
      assert!(
        self.surface.selection().by_id(cursor_id).is_some(),
        "{label}: visual goal for missing cursor {cursor_id:?}: {:#?}",
        self.snapshot()
      );
    }

    assert_eq!(
      self.surface.render_surface().selection(),
      Some(self.surface.selection()),
      "{label}: render selection out of sync: {:#?}",
      self.snapshot()
    );

    let _delta = self
      .surface
      .render(&self.document, None, Some(self.surface.render_generation()));
    let stable = self
      .surface
      .render(&self.document, None, Some(self.surface.render_generation()));

    assert_stable_delta(label, &stable, &self.snapshot());
  }

  pub fn apply_op(&mut self, op: &Op) -> Result<(), Box<dyn Error>> {
    match op {
      Op::InsertChar(ch) => self.insert_char(*ch),
      Op::InsertText(text) => self.insert_text(text),
      Op::InsertNewline => self.insert_newline(),
      Op::Backspace => self.backspace(),
      Op::DeleteForward => self.delete_forward(),
      Op::MoveLeft => self.apply_motion(move_left, 1, MotionMode::Move),
      Op::MoveRight => self.apply_motion(move_right, 1, MotionMode::Move),
      Op::MoveUp => self.apply_motion(move_up_visual, 1, MotionMode::Move),
      Op::MoveDown => self.apply_motion(move_down_visual, 1, MotionMode::Move),
      Op::MoveLineStart => self.apply_motion_simple(move_line_start, MotionMode::Move),
      Op::MoveLineEnd => self.apply_motion_simple(move_line_end, MotionMode::Move),
      Op::MoveDocumentStart => self.apply_motion_simple(move_document_start, MotionMode::Move),
      Op::MoveDocumentEnd => self.apply_motion_simple(move_document_end, MotionMode::Move),
      Op::SelectAll => {
        let len = self.document.text().len_chars();
        self
          .surface
          .apply_selection(&self.document, Selection::single(0, len))?;
        Ok(())
      },
      Op::Copy => self.copy_selection_or_line(),
      Op::Cut => self.cut_selection_or_line(),
      Op::Paste => self.paste_register(),
      Op::Search(query) => self.search(query),
      Op::RepeatSearchForward => self.repeat_search_forward(),
      Op::Undo => {
        self.surface.undo(&mut self.document)?;
        Ok(())
      },
      Op::Redo => {
        self.surface.redo(&mut self.document)?;
        Ok(())
      },
      Op::ToggleWrap => {
        let mut format = self.surface.text_format().clone();
        format.soft_wrap = !format.soft_wrap;
        self
          .surface
          .set_text_format_reconcile_viewport(&self.document, format);
        Ok(())
      },
    }
  }

  fn apply_motion(
    &mut self,
    motion: impl Fn(
      ropey::RopeSlice<'_>,
      Selection,
      usize,
      MotionMode,
      &the_lib::movement::VisualGoals,
      &MovementContext<'_>,
    ) -> the_lib::movement::MotionResult,
    count: usize,
    mode: MotionMode,
  ) -> Result<(), Box<dyn Error>> {
    let viewport = self.surface.viewport();
    let text = self.document.text().slice(..);
    let cx = MovementContext {
      text_format: self.surface.text_format(),
      annotations: self.surface.text_annotations(),
      wrap_width:  viewport.wrap_width,
    };
    let result = motion(
      text,
      self.surface.selection().clone(),
      count,
      mode,
      self.surface.visual_goals(),
      &cx,
    );
    self.surface.apply_motion_result(&self.document, result)?;
    Ok(())
  }

  fn apply_motion_simple(
    &mut self,
    motion: impl Fn(
      ropey::RopeSlice<'_>,
      Selection,
      MotionMode,
      &the_lib::movement::VisualGoals,
      &MovementContext<'_>,
    ) -> the_lib::movement::MotionResult,
    mode: MotionMode,
  ) -> Result<(), Box<dyn Error>> {
    let viewport = self.surface.viewport();
    let text = self.document.text().slice(..);
    let cx = MovementContext {
      text_format: self.surface.text_format(),
      annotations: self.surface.text_annotations(),
      wrap_width:  viewport.wrap_width,
    };
    let result = motion(
      text,
      self.surface.selection().clone(),
      mode,
      self.surface.visual_goals(),
      &cx,
    );
    self.surface.apply_motion_result(&self.document, result)?;
    Ok(())
  }

  fn insert_char(&mut self, ch: char) -> Result<(), Box<dyn Error>> {
    if let Some(edit) = edit_for_typed_char(
      self.document.text(),
      self.surface.selection(),
      ch,
      &self.auto_pairs,
    )? {
      self.surface.apply_edit(&mut self.document, edit)?;
      return Ok(());
    }

    self.insert_text(&ch.to_string())
  }

  fn insert_text(&mut self, text: &str) -> Result<(), Box<dyn Error>> {
    let range = self.surface.primary_range();
    let span = Span {
      start: range.from(),
      end:   range.to(),
    };
    let tx = Transaction::from_replacements(
      self.document.text().len_chars(),
      [Replacement::replace(span, text)],
      ReplacementOverlapPolicy::Reject,
    )?;
    let selection = self
      .surface
      .selection()
      .clone()
      .transform(|_| Range::point(span.start + text.chars().count()));
    self.surface.apply_edit(&mut self.document, Edit {
      transaction: tx,
      selection,
    })?;
    Ok(())
  }

  fn insert_newline(&mut self) -> Result<(), Box<dyn Error>> {
    let text = self.document.text().slice(..);
    let cursor = self.surface.primary_range().cursor(text);
    let line = text.char_to_line(cursor);
    let indent = indent_for_newline(
      None,
      None,
      &IndentationHeuristic::Simple,
      &self.document.indent_style(),
      self.surface.text_format().tab_width as usize,
      text,
      line,
      cursor,
      line,
    );
    let newline = format!("{}{}", self.document.line_ending().as_str(), indent);
    self.insert_text(&newline)
  }

  fn backspace(&mut self) -> Result<(), Box<dyn Error>> {
    if let Some(edit) = edit_for_backspace(
      self.document.text(),
      self.surface.selection(),
      &self.auto_pairs,
    )? {
      self.surface.apply_edit(&mut self.document, edit)?;
      return Ok(());
    }

    let range = self.surface.primary_range();
    if !range.is_empty() {
      return self.delete_span(range.from(), range.to());
    }

    if range.head == 0 {
      return Ok(());
    }

    let text = self.document.text().slice(..);
    let start = prev_grapheme_boundary(text, range.head);
    self.delete_span(start, range.head)
  }

  fn delete_forward(&mut self) -> Result<(), Box<dyn Error>> {
    let range = self.surface.primary_range();
    if !range.is_empty() {
      return self.delete_span(range.from(), range.to());
    }

    let text = self.document.text().slice(..);
    if range.head >= text.len_chars() {
      return Ok(());
    }

    let end = next_grapheme_boundary(text, range.head);
    self.delete_span(range.head, end)
  }

  fn delete_span(&mut self, start: usize, end: usize) -> Result<(), Box<dyn Error>> {
    let tx = Transaction::delete_spans(
      self.document.text().len_chars(),
      [Span { start, end }],
      DeletionOverlapPolicy::Reject,
    )?;
    let selection = self
      .surface
      .selection()
      .clone()
      .transform(|_| Range::point(start));
    self.surface.apply_edit(&mut self.document, Edit {
      transaction: tx,
      selection,
    })?;
    Ok(())
  }

  fn copy_selection_or_line(&mut self) -> Result<(), Box<dyn Error>> {
    let values = if self.surface.primary_range().is_empty() {
      vec![Tendril::from(self.current_line_text().as_str())]
    } else {
      self
        .surface
        .selection()
        .fragments(self.document.text().slice(..))
        .map(|fragment| Tendril::from(fragment.as_ref()))
        .collect()
    };
    self.registers.write('"', values);
    Ok(())
  }

  fn cut_selection_or_line(&mut self) -> Result<(), Box<dyn Error>> {
    if self.surface.primary_range().is_empty() {
      let (start, end) = self.current_line_span();
      self
        .registers
        .write('"', vec![Tendril::from(self.current_line_text().as_str())]);
      return self.delete_span(start, end);
    }

    self.copy_selection_or_line()?;
    let range = self.surface.primary_range();
    self.delete_span(range.from(), range.to())
  }

  fn paste_register(&mut self) -> Result<(), Box<dyn Error>> {
    let Some(values) = self.registers.read('"') else {
      return Ok(());
    };

    let joined = values
      .iter()
      .map(|value| value.as_str())
      .collect::<Vec<_>>()
      .join(self.document.line_ending().as_str());
    self.insert_text(&joined)
  }

  fn search(&mut self, query: &str) -> Result<(), Box<dyn Error>> {
    let regex = build_regex(query, true).map_err(io_error)?;
    let selection = search_regex(
      self.document.text().slice(..),
      self.surface.selection(),
      CursorPick::Id(self.surface.primary_cursor_id()),
      &regex,
      MotionMode::Move,
      the_lib::movement::Direction::Forward,
      true,
    )
    .ok_or_else(|| io_error("search returned no match"))?;
    self.registers.write('/', vec![Tendril::from(query)]);
    self.surface.apply_selection(&self.document, selection)?;
    Ok(())
  }

  fn repeat_search_forward(&mut self) -> Result<(), Box<dyn Error>> {
    let Some(query) = self.registers.first('/') else {
      return Err(io_error("missing previous search").into());
    };
    let query = query.to_string();
    self.search(&query)
  }

  fn current_line_span(&self) -> (usize, usize) {
    let text = self.document.text().slice(..);
    let cursor = self.surface.primary_range().cursor(text);
    let line = text.char_to_line(cursor);
    let start = text.line_to_char(line);
    let end = if line + 1 < text.len_lines() {
      text.line_to_char(line + 1)
    } else {
      text.len_chars()
    };
    (start, end)
  }

  fn current_line_text(&self) -> String {
    let text = self.document.text().slice(..);
    let cursor = self.surface.primary_range().cursor(text);
    let line = text.char_to_line(cursor);
    let line_slice = text.line(line);
    let line_ending_chars = get_line_ending(&line_slice)
      .map(|line_ending| line_ending.len_chars())
      .unwrap_or(0);
    line_slice
      .slice(..line_slice.len_chars().saturating_sub(line_ending_chars))
      .to_string()
  }
}

pub fn io_error(message: impl Into<String>) -> std::io::Error {
  std::io::Error::other(message.into())
}

pub fn assert_stable_delta(label: &str, delta: &RenderFrameDelta, snapshot: &Snapshot) {
  assert!(
    !delta.full,
    "{label}: stable rerender unexpectedly full: {snapshot:#?}"
  );
  assert!(
    delta.retained_rows.is_empty()
      && delta.changed_rows.is_empty()
      && delta.removed_rows.is_empty(),
    "{label}: stable rerender changed rows: {delta:#?}\n{snapshot:#?}"
  );
  assert!(
    delta.row_metadata.is_none(),
    "{label}: stable rerender changed row metadata: {delta:#?}\n{snapshot:#?}"
  );
  assert!(
    delta.layers.selections.is_none()
      && delta.layers.cursors.is_none()
      && delta.layers.syntax.is_none()
      && delta.layers.widgets.is_none()
      && delta.layers.gutters.is_none()
      && delta.layers.overlays.is_none(),
    "{label}: stable rerender changed layers: {delta:#?}\n{snapshot:#?}"
  );
  assert_eq!(
    delta.damage,
    Damage::empty(),
    "{label}: stable rerender has damage: {delta:#?}\n{snapshot:#?}"
  );
}
