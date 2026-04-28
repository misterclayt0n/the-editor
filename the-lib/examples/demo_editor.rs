use std::{
  fs,
  io::{
    self,
    Write,
  },
  path::PathBuf,
  time::{
    Duration,
    Instant,
  },
};

use clap::Parser;
use crossterm::{
  cursor::{
    Hide,
    MoveTo,
    Show,
  },
  event::{
    self,
    Event,
    KeyCode,
    KeyEvent,
    KeyEventKind,
    KeyModifiers,
  },
  execute, queue,
  style::{
    Attribute,
    Color,
    Print,
    SetAttribute,
    SetForegroundColor,
  },
  terminal::{
    self,
    Clear,
    ClearType,
    EnterAlternateScreen,
    LeaveAlternateScreen,
    disable_raw_mode,
    enable_raw_mode,
  },
};
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
    RenderSurface,
    SurfaceId,
    SurfaceMeasurements,
    TextAnnotations,
    TextFormat,
    Viewport,
    visual_pos_at_char,
  },
  search::{
    build_regex,
    search_regex,
  },
  selection::{
    CursorPick,
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

#[derive(Parser, Debug)]
struct Args {
  /// File to open. If omitted, starts with an empty scratch buffer.
  path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
enum Mode {
  Normal,
  Search { query: String },
}

struct App {
  path:             Option<PathBuf>,
  document:         Document,
  surface:          Surface,
  registers:        Registers,
  auto_pairs:       AutoPairs,
  mode:             Mode,
  status:           String,
  dirty:            bool,
  last_render:      Duration,
  last_generation:  u64,
  transaction_count: u64,
}

impl App {
  fn open(path: Option<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {
    let text = match &path {
      Some(path) => fs::read_to_string(path)?,
      None => String::new(),
    };

    let document = Document::new(text.into());
    let mut render = RenderSurface::new(
      SurfaceId::new(1).expect("surface id must be non-zero"),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    render.set_measurements(SurfaceMeasurements::cell_grid(1, 1));

    let viewport = viewport_from_terminal()?.0;
    let surface = Surface::new(&document, render, viewport)?;

    Ok(Self {
      path,
      document,
      surface,
      registers: Registers::new(),
      auto_pairs: AutoPairs::default(),
      mode: Mode::Normal,
      status: String::from(
        "Ctrl-Q quit · Ctrl-S save · Ctrl-Z/Y undo/redo · Ctrl-C/X/V copy/cut/paste · / search · F2 wrap · F6 stress",
      ),
      dirty: false,
      last_render: Duration::ZERO,
      last_generation: 0,
      transaction_count: 0,
    })
  }

  fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let _terminal = TerminalGuard::enter()?;
    let mut stdout = io::stdout();

    loop {
      self.sync_viewport()?;
      self.draw(&mut stdout)?;

      let event = event::read()?;
      match event {
        Event::Resize(_, _) => continue,
        Event::Key(key) if key.kind == KeyEventKind::Press => {
          if self.handle_key(key)? {
            break;
          }
        },
        _ => {},
      }
    }

    Ok(())
  }

  fn sync_viewport(&mut self) -> io::Result<()> {
    let (viewport, _) = viewport_from_terminal()?;
    let mut current = self.surface.viewport();
    current.row_count = viewport.row_count;
    current.wrap_width = viewport.wrap_width;
    self.surface.set_viewport(current);
    Ok(())
  }

  fn draw(&mut self, stdout: &mut io::Stdout) -> Result<(), Box<dyn std::error::Error>> {
    let (_, (cols, rows)) = viewport_from_terminal()?;
    let editor_rows = rows.saturating_sub(1) as usize;

    let start = Instant::now();
    let frame = self.surface.render(&self.document, None);
    self.last_render = start.elapsed();
    self.last_generation = frame.generation;

    let mut visible = vec![String::new(); editor_rows];
    for row in frame.changed_rows {
      if row.placement.surface_row < visible.len() {
        visible[row.placement.surface_row] = row.text.to_string();
      }
    }

    queue!(stdout, Hide, MoveTo(0, 0), Clear(ClearType::All))?;

    for (row_idx, line) in visible.iter().enumerate() {
      let y = row_idx as u16;
      queue!(stdout, MoveTo(0, y), Print(line))?;
    }

    let status = self.status_line(cols as usize);
    queue!(
      stdout,
      MoveTo(0, rows.saturating_sub(1)),
      SetForegroundColor(Color::Black),
      SetAttribute(Attribute::Reverse),
      Print(truncate_pad(&status, cols as usize)),
      SetAttribute(Attribute::NoReverse),
      SetForegroundColor(Color::Reset),
    )?;

    if let Some((x, y)) = self.cursor_screen_pos(editor_rows, cols as usize) {
      queue!(stdout, MoveTo(x as u16, y as u16), Show)?;
    } else {
      queue!(stdout, Hide)?;
    }

    stdout.flush()?;
    Ok(())
  }

  fn cursor_screen_pos(&self, editor_rows: usize, cols: usize) -> Option<(usize, usize)> {
    let viewport = self.surface.viewport();
    let text = self.document.text().slice(..);
    let cursor = self.surface.primary_range().cursor(text);
    let pos = visual_pos_at_char(
      text,
      self.surface.text_format(),
      viewport.wrap_width,
      self.surface.text_annotations(),
      cursor,
    )?;

    let screen_row = pos.row.checked_sub(viewport.first_row)?;
    let screen_col = pos.col.checked_sub(viewport.first_col)?;
    if screen_row >= editor_rows || screen_col >= cols {
      return None;
    }
    Some((screen_col, screen_row))
  }

  fn handle_key(&mut self, key: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
    if matches!(self.mode, Mode::Search { .. }) {
      return self.handle_search_mode_key(key);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
      return self.handle_control_key(key);
    }

    let extend = key.modifiers.contains(KeyModifiers::SHIFT);
    let mode = if extend {
      MotionMode::Extend
    } else {
      MotionMode::Move
    };

    match key.code {
      KeyCode::Char('/') => {
        self.mode = Mode::Search {
          query: String::new(),
        };
        self.status = String::from("search: type regex, Enter to jump, Esc to cancel");
      },
      KeyCode::Char('n') => self.repeat_search(true)?,
      KeyCode::Char('N') => self.repeat_search(false)?,
      KeyCode::Left | KeyCode::Char('h') => self.apply_motion(move_left, 1, mode)?,
      KeyCode::Right | KeyCode::Char('l') => self.apply_motion(move_right, 1, mode)?,
      KeyCode::Up | KeyCode::Char('k') => self.apply_motion(move_up_visual, 1, mode)?,
      KeyCode::Down | KeyCode::Char('j') => self.apply_motion(move_down_visual, 1, mode)?,
      KeyCode::PageUp => {
        let count = self.surface.viewport().row_count.max(1);
        self.apply_motion(move_up_visual, count, mode)?;
      },
      KeyCode::PageDown => {
        let count = self.surface.viewport().row_count.max(1);
        self.apply_motion(move_down_visual, count, mode)?;
      },
      KeyCode::F(2) => self.toggle_wrap(),
      KeyCode::F(6) => self.stress_transactions()?,
      KeyCode::Home => self.apply_motion_simple(move_line_start, mode)?,
      KeyCode::End => self.apply_motion_simple(move_line_end, mode)?,
      KeyCode::Backspace => self.backspace()?,
      KeyCode::Delete => self.delete_forward()?,
      KeyCode::Enter => self.insert_newline()?,
      KeyCode::Tab => self.insert_text(self.document.indent_style().as_str())?,
      KeyCode::Char(ch) => self.insert_char(ch)?,
      _ => {},
    }

    Ok(false)
  }

  fn handle_search_mode_key(
    &mut self,
    key: KeyEvent,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let Mode::Search { mut query } = std::mem::replace(&mut self.mode, Mode::Normal) else {
      return Ok(false);
    };
    let mut keep_search = true;

    match key.code {
      KeyCode::Esc => {
        keep_search = false;
        self.status = String::from("search cancelled");
      },
      KeyCode::Enter => {
        keep_search = false;
        let committed = query.clone();
        if committed.is_empty() {
          self.status = String::from("empty search");
        } else {
          self
            .registers
            .write('/', vec![Tendril::from(committed.as_str())]);
          self.run_search(&committed, true)?;
        }
      },
      KeyCode::Backspace => {
        query.pop();
      },
      KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
        query.push(ch);
      },
      _ => {},
    }

    if keep_search {
      self.mode = Mode::Search {
        query: query.clone(),
      };
      self.status = format!("/{query}");
    }
    Ok(false)
  }

  fn handle_control_key(&mut self, key: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
    match key.code {
      KeyCode::Char('q') => return Ok(true),
      KeyCode::Char('s') => self.save()?,
      KeyCode::Char('z') => {
        if self.surface.undo(&mut self.document)? {
          self.status = String::from("undo");
          self.dirty = true;
        }
      },
      KeyCode::Char('y') => {
        if self.surface.redo(&mut self.document)? {
          self.status = String::from("redo");
          self.dirty = true;
        }
      },
      KeyCode::Char('a') => {
        let len = self.document.text().len_chars();
        self
          .surface
          .apply_selection(&self.document, Selection::single(0, len))?;
        self.status = String::from("selected all");
      },
      KeyCode::Char('c') => self.copy_selection_or_line()?,
      KeyCode::Char('x') => self.cut_selection_or_line()?,
      KeyCode::Char('v') => self.paste_register()?,
      KeyCode::Char('f') => {
        self.mode = Mode::Search {
          query: String::new(),
        };
        self.status = String::from("search: type regex, Enter to jump, Esc to cancel");
      },
      KeyCode::Char('g') => {
        self.apply_motion_simple(move_document_start, MotionMode::Move)?
      },
      KeyCode::Char('e') => {
        self.apply_motion_simple(move_document_end, MotionMode::Move)?
      },
      _ => {},
    }

    Ok(false)
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
  ) -> Result<(), Box<dyn std::error::Error>> {
    let viewport = self.surface.viewport();
    let text = self.document.text().slice(..);
    let cx = MovementContext {
      text_format: self.surface.text_format(),
      annotations: self.surface.text_annotations(),
      wrap_width: viewport.wrap_width,
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
  ) -> Result<(), Box<dyn std::error::Error>> {
    let viewport = self.surface.viewport();
    let text = self.document.text().slice(..);
    let cx = MovementContext {
      text_format: self.surface.text_format(),
      annotations: self.surface.text_annotations(),
      wrap_width: viewport.wrap_width,
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

  fn insert_char(&mut self, ch: char) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(edit) = edit_for_typed_char(
      self.document.text(),
      self.surface.selection(),
      ch,
      &self.auto_pairs,
    )? {
      self.surface.apply_edit(&mut self.document, edit)?;
      self.transaction_count += 1;
      self.dirty = true;
      self.status = format!("inserted {ch:?}");
      return Ok(());
    }

    self.insert_text(&ch.to_string())
  }

  fn insert_text(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
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
    let selection = Selection::point(span.start + text.chars().count());
    self.surface.apply_edit(
      &mut self.document,
      Edit {
        transaction: tx,
        selection,
      },
    )?;
    self.transaction_count += 1;
    self.dirty = true;
    Ok(())
  }

  fn backspace(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(edit) = edit_for_backspace(
      self.document.text(),
      self.surface.selection(),
      &self.auto_pairs,
    )? {
      self.surface.apply_edit(&mut self.document, edit)?;
      self.transaction_count += 1;
      self.dirty = true;
      self.status = String::from("auto-pair backspace");
      return Ok(());
    }

    let range = self.surface.primary_range();
    if !range.is_empty() {
      self.delete_span(range.from(), range.to())?;
      return Ok(());
    }

    if range.head == 0 {
      return Ok(());
    }

    let text = self.document.text().slice(..);
    let start = prev_grapheme_boundary(text, range.head);
    self.delete_span(start, range.head)
  }

  fn delete_forward(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let range = self.surface.primary_range();
    if !range.is_empty() {
      self.delete_span(range.from(), range.to())?;
      return Ok(());
    }

    let text = self.document.text().slice(..);
    if range.head >= text.len_chars() {
      return Ok(());
    }

    let end = next_grapheme_boundary(text, range.head);
    self.delete_span(range.head, end)
  }

  fn delete_span(&mut self, start: usize, end: usize) -> Result<(), Box<dyn std::error::Error>> {
    let tx = Transaction::delete_spans(
      self.document.text().len_chars(),
      [Span { start, end }],
      DeletionOverlapPolicy::Reject,
    )?;
    self.surface.apply_edit(
      &mut self.document,
      Edit {
        transaction: tx,
        selection:   Selection::point(start),
      },
    )?;
    self.transaction_count += 1;
    self.dirty = true;
    self.status = String::from("deleted");
    Ok(())
  }

  fn insert_newline(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

  fn copy_selection_or_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
    self.status = String::from("copied to register \"");
    Ok(())
  }

  fn cut_selection_or_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    if self.surface.primary_range().is_empty() {
      let text = self.document.text().slice(..);
      let cursor = self.surface.primary_range().cursor(text);
      let line = text.char_to_line(cursor);
      let start = text.line_to_char(line);
      let end = if line + 1 < text.len_lines() {
        text.line_to_char(line + 1)
      } else {
        text.len_chars()
      };
      self.registers.write('"', vec![Tendril::from(self.current_line_text().as_str())]);
      self.delete_span(start, end)?;
      self.status = String::from("cut line to register \"");
      return Ok(());
    }

    self.copy_selection_or_line()?;
    let range = self.surface.primary_range();
    self.delete_span(range.from(), range.to())?;
    self.status = String::from("cut selection to register \"");
    Ok(())
  }

  fn paste_register(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let Some(values) = self.registers.read('"') else {
      self.status = String::from("register \" is empty");
      return Ok(());
    };

    let joined = values
      .iter()
      .map(|value| value.as_str())
      .collect::<Vec<_>>()
      .join(self.document.line_ending().as_str());
    self.insert_text(&joined)?;
    self.status = String::from("pasted register \"");
    Ok(())
  }

  fn run_search(&mut self, query: &str, forward: bool) -> Result<(), Box<dyn std::error::Error>> {
    let regex = match build_regex(query, true) {
      Ok(regex) => regex,
      Err(err) => {
        self.status = format!("invalid regex: {err}");
        return Ok(());
      },
    };

    let direction = if forward {
      the_lib::movement::Direction::Forward
    } else {
      the_lib::movement::Direction::Backward
    };

    let Some(selection) = search_regex(
      self.document.text().slice(..),
      self.surface.selection(),
      CursorPick::Id(self.surface.primary_cursor_id()),
      &regex,
      MotionMode::Move,
      direction,
      true,
    ) else {
      self.status = format!("no match for /{query}");
      return Ok(());
    };

    self.surface.apply_selection(&self.document, selection)?;
    self.status = format!("searched /{query}");
    Ok(())
  }

  fn repeat_search(&mut self, forward: bool) -> Result<(), Box<dyn std::error::Error>> {
    let Some(query) = self.registers.first('/') else {
      self.status = String::from("no previous search");
      return Ok(());
    };
    let query = query.to_string();
    self.run_search(&query, forward)
  }

  fn toggle_wrap(&mut self) {
    let mut format = self.surface.text_format().clone();
    format.soft_wrap = !format.soft_wrap;
    self.surface.set_text_format(format.clone());
    self.status = if format.soft_wrap {
      String::from("soft wrap enabled")
    } else {
      String::from("soft wrap disabled")
    };
  }

  fn stress_transactions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    for _ in 0..1000 {
      self.insert_text("x")?;
    }
    self.status = format!(
      "1000 transactions in {:.2?} · revision {}",
      start.elapsed(),
      self.document.revision()
    );
    Ok(())
  }

  fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = &self.path else {
      self.status = String::from("scratch buffer has no path");
      return Ok(());
    };

    fs::write(path, self.document.text().to_string())?;
    self.status = format!("saved {}", path.display());
    self.dirty = false;
    Ok(())
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

  fn status_line(&self, width: usize) -> String {
    let text = self.document.text().slice(..);
    let cursor = self.surface.primary_range().cursor(text);
    let line = text.char_to_line(cursor);
    let line_start = text.line_to_char(line);
    let col = cursor.saturating_sub(line_start);
    let mode = match &self.mode {
      Mode::Normal => "NORMAL",
      Mode::Search { .. } => "SEARCH",
    };
    let name = self
      .path
      .as_ref()
      .map(|path| path.display().to_string())
      .unwrap_or_else(|| String::from("[scratch]"));
    let modified = if self.dirty { " [+]" } else { "" };
    let selection_len = self.surface.primary_range().len();

    let info = format!(
      " {mode} {name}{modified} | ln {}, col {} | sel {} | rev {} | tx {} | render {:?} | gen {} ",
      line + 1,
      col + 1,
      selection_len,
      self.document.revision(),
      self.transaction_count,
      self.last_render,
      self.last_generation,
    );

    if self.status.is_empty() {
      truncate_pad(&info, width)
    } else {
      truncate_pad(&format!("{info}:: {}", self.status), width)
    }
  }
}

struct TerminalGuard;

impl TerminalGuard {
  fn enter() -> io::Result<Self> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, Hide)?;
    Ok(Self)
  }
}

impl Drop for TerminalGuard {
  fn drop(&mut self) {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
  }
}

fn viewport_from_terminal() -> io::Result<(Viewport, (u16, u16))> {
  let (cols, rows) = terminal::size()?;
  let row_count = rows.saturating_sub(1).max(1) as usize;
  let wrap_width = cols.max(1) as usize;
  Ok((Viewport::new(0, row_count, 0, wrap_width), (cols, rows)))
}

fn truncate_pad(text: &str, width: usize) -> String {
  let mut out = String::new();
  for ch in text.chars().take(width) {
    out.push(ch);
  }
  let len = out.chars().count();
  if len < width {
    out.push_str(&" ".repeat(width - len));
  }
  out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();
  let mut app = App::open(args.path)?;
  app.run()
}
