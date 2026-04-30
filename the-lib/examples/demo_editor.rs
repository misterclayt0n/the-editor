use std::{
  io::{
    self,
    Write,
  },
  path::{
    Path,
    PathBuf,
  },
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
  execute,
  queue,
  style::{
    Attribute,
    Color,
    Print,
    SetAttribute,
    SetBackgroundColor,
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
use ropey::Rope;
use the_lib::{
  Tendril,
  auto_pairs::{
    AutoPairs,
    edit_for_backspace,
    edit_for_typed_char,
  },
  clipboard::ClipboardType,
  operation::{
    Edit,
    SelectionTransform,
  },
  grapheme::{
    grapheme_width,
    next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  indent::indent_for_newline,
  line_ending::get_line_ending,
  movement::{
    Direction,
    MotionMode,
    MovementContext,
    transform_selection,
  },
  render::{
    CellGridMetrics,
    CursorPlacement,
    CursorSemantics,
    FrameRow,
    PresentUpdate,
    RetainedFrame,
    RowKind,
    RowMetadata,
    SelectionSpan,
    SyntaxSpan,
    TextAnnotations,
    Viewport,
    WidgetId,
  },
  search::{
    build_regex,
    search_regex,
  },
  selection::{
    CursorPick,
    Selection,
  },
  surface::SurfaceViewSpec,
  syntax::IndentationHeuristic,
  transaction::{
    DeletionOverlapPolicy,
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
  },
};
use the_runtime::{
  DocumentHandle,
  RuntimeSession,
  SurfaceHandle,
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
  SaveAs { path: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderProfile {
  Baseline,
  Metadata,
  Annotations,
  Widgets,
  FullStress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorShape {
  Block,
  Bar,
  Underline,
}

impl CursorShape {
  fn next(self) -> Self {
    match self {
      Self::Block => Self::Bar,
      Self::Bar => Self::Underline,
      Self::Underline => Self::Block,
    }
  }

  fn label(self) -> &'static str {
    match self {
      Self::Block => "block",
      Self::Bar => "bar",
      Self::Underline => "underline",
    }
  }

  fn semantics(self) -> CursorSemantics {
    match self {
      Self::Block | Self::Underline => CursorSemantics::Cell,
      Self::Bar => CursorSemantics::InsertionPoint,
    }
  }
}

impl RenderProfile {
  fn next(self) -> Self {
    match self {
      Self::Baseline => Self::Metadata,
      Self::Metadata => Self::Annotations,
      Self::Annotations => Self::Widgets,
      Self::Widgets => Self::FullStress,
      Self::FullStress => Self::Baseline,
    }
  }

  fn label(self) -> &'static str {
    match self {
      Self::Baseline => "baseline",
      Self::Metadata => "metadata",
      Self::Annotations => "annotations",
      Self::Widgets => "widgets",
      Self::FullStress => "full-stress",
    }
  }

  fn show_metadata(self) -> bool {
    !matches!(self, Self::Baseline)
  }

  fn show_inline_annotations(self) -> bool {
    matches!(self, Self::Annotations | Self::FullStress)
  }

  fn show_substitutions(self) -> bool {
    matches!(self, Self::Annotations | Self::FullStress)
  }

  fn show_widgets(self) -> bool {
    matches!(self, Self::Widgets | Self::FullStress)
  }
}

struct App {
  session:           RuntimeSession,
  document:          DocumentHandle,
  surface:           SurfaceHandle,
  auto_pairs:        AutoPairs,
  mode:              Mode,
  status:            String,
  last_render:       Duration,
  last_present:      PresentUpdate,
  transaction_count: u64,
  render_view:       RetainedFrame,
  render_profile:    RenderProfile,
  cursor_shape:      CursorShape,
}

#[cfg(test)]
struct VisibleFrame {
  rows:        Vec<String>,
  syntax_rows: Vec<Vec<(usize, usize, Color)>>,
}

impl App {
  fn open(path: Option<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {
    let viewport = viewport_from_terminal()?.0;
    Self::open_with_viewport(path, viewport)
  }

  fn open_with_viewport(
    path: Option<PathBuf>,
    viewport: Viewport,
  ) -> Result<Self, Box<dyn std::error::Error>> {
    let mut session = RuntimeSession::default();
    let document = match path {
      Some(path) => session.open_file(&path)?,
      None => session.create_scratch(Rope::new())?,
    };

    let surface = session.create_surface_with_viewport(document, viewport)?;
    session
      .surface_mut(surface)
      .expect("surface must exist")
      .set_view(SurfaceViewSpec {
        measurements: CellGridMetrics::new(1, 1),
        ..SurfaceViewSpec::new(viewport)
      });

    Ok(Self {
      session,
      document,
      surface,
      auto_pairs: AutoPairs::default(),
      mode: Mode::Normal,
      status: String::from(
        "Ctrl-Q quit · Ctrl-S save · Ctrl-Shift-S/F3 save as · Ctrl-Z/Y undo/redo · Ctrl-C/X/V \
         copy/cut/paste · / search · F2 wrap · F6 stress · F7 render profile · F8 cursor shape",
      ),
      last_render: Duration::ZERO,
      last_present: PresentUpdate::default(),
      transaction_count: 0,
      render_view: RetainedFrame::default(),
      render_profile: RenderProfile::Baseline,
      cursor_shape: CursorShape::Block,
    })
  }

  fn document(&self) -> &the_lib::document::Document {
    self
      .session
      .document(self.document)
      .expect("document handle must remain valid")
  }

  fn surface(&self) -> &the_lib::surface::Surface {
    self
      .session
      .surface(self.surface)
      .expect("surface handle must remain valid")
  }

  fn surface_mut(&mut self) -> &mut the_lib::surface::Surface {
    self
      .session
      .surface_mut(self.surface)
      .expect("surface handle must remain valid")
  }

  fn apply_edit(&mut self, edit: Edit) -> Result<(), Box<dyn std::error::Error>> {
    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface.apply_edit(document, edit)
      })??;
    Ok(())
  }

  fn apply_selection(&mut self, selection: Selection) -> Result<(), Box<dyn std::error::Error>> {
    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface.apply_selection(document, selection)
      })??;
    Ok(())
  }

  fn document_path(&self) -> Option<&Path> {
    let resource = self.session.document_entry(self.document)?.resource?;
    Some(self.session.resource(resource)?.path())
  }

  fn is_dirty(&self) -> bool {
    self
      .session
      .document_entry(self.document)
      .is_some_and(|entry| entry.is_dirty())
  }

  fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let _terminal = TerminalGuard::enter()?;
    let mut stdout = io::stdout();

    loop {
      self.sync_view()?;
      self.draw(&mut stdout)?;

      let event = event::read()?;
      match event {
        Event::Resize(..) => continue,
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

  fn sync_view(&mut self) -> io::Result<()> {
    let (_, (cols, rows)) = viewport_from_terminal()?;
    let row_count = rows.saturating_sub(self.footer_rows() as u16).max(1) as usize;
    let metadata_cols = self.metadata_cols(cols as usize);
    let profile = self.render_profile;

    let text = self.document().text().slice(..);
    let last_doc_line = text.len_lines().saturating_sub(1);

    let mut view = self.surface().view_spec();
    view.viewport.row_count = row_count;
    view.viewport.wrap_width = cols.saturating_sub(metadata_cols as u16).max(1) as usize;
    view.viewport.first_col = if view.format.soft_wrap {
      0
    } else {
      view.viewport.first_col
    };

    let mut annotations = TextAnnotations::default();
    if profile.show_inline_annotations() {
      annotations
        .add_inline_text(0, "ⓘ ")
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    }
    if profile.show_substitutions()
      && let Some((char_idx, replacement)) = first_substitutable_char(text)
    {
      annotations
        .add_grapheme_substitution_text(char_idx, replacement)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    }
    view.annotations = annotations;
    view.cursor_semantics = self.cursor_shape.semantics();
    view.widgets = if profile.show_widgets() {
      vec![
        the_lib::render::BlockWidget::after_line(
          widget_id_top(),
          0.min(last_doc_line),
          view.viewport.wrap_width as u32,
          2,
        ),
        the_lib::render::BlockWidget::after_line(
          widget_id_bottom(),
          last_doc_line,
          view.viewport.wrap_width as u32,
          1,
        ),
      ]
    } else {
      Vec::new()
    };

    self.surface_mut().set_view(view);
    Ok(())
  }

  fn draw(&mut self, stdout: &mut io::Stdout) -> Result<(), Box<dyn std::error::Error>> {
    let (_, (cols, rows)) = viewport_from_terminal()?;
    let prompt = self.prompt_line();
    let footer_rows = self.footer_rows() as u16;
    let editor_rows = rows.saturating_sub(footer_rows).max(1) as usize;
    let present = self.present()?;

    queue!(stdout, Hide)?;
    self.draw_editor_rows(stdout, cols as usize, editor_rows, present)?;

    let status = self.status_line(cols as usize);
    let status_row = rows.saturating_sub(footer_rows);
    queue!(
      stdout,
      MoveTo(0, status_row),
      Clear(ClearType::CurrentLine),
      SetForegroundColor(Color::Black),
      SetBackgroundColor(Color::Grey),
      Print(truncate_pad(&status, cols as usize)),
      SetBackgroundColor(Color::Reset),
      SetForegroundColor(Color::Reset),
      SetAttribute(Attribute::Reset),
    )?;

    if let Some(prompt) = &prompt {
      queue!(
        stdout,
        MoveTo(0, rows.saturating_sub(1)),
        Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::Cyan),
        Print(truncate_pad(prompt, cols as usize)),
        SetForegroundColor(Color::Reset),
      )?;
    } else {
      queue!(
        stdout,
        MoveTo(0, rows.saturating_sub(1)),
        Clear(ClearType::CurrentLine)
      )?;
    }

    // Editor cursors are painted by draw_visible_row(). Keep the terminal
    // cursor hidden so cursor shape is controlled by the client renderer.
    queue!(stdout, Hide)?;

    stdout.flush()?;
    Ok(())
  }

  fn present(&mut self) -> Result<PresentUpdate, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let present = self
      .session
      .present_surface(self.surface, &mut self.render_view)?;
    self.last_render = start.elapsed();
    self.last_present = present;
    Ok(present)
  }

  #[cfg(test)]
  fn render_visible_frame(
    &mut self,
    editor_rows: usize,
  ) -> Result<VisibleFrame, Box<dyn std::error::Error>> {
    let _ = self.present()?;

    let mut visible = vec![String::new(); editor_rows];
    let mut syntax_rows = vec![Vec::new(); editor_rows];

    for surface_row in 0..editor_rows {
      if let Some(row) = self.render_view.row(surface_row) {
        visible[surface_row] = self.row_text(row, usize::MAX);
      }

      let mut row_syntax = self
        .render_view
        .syntax_spans(surface_row)
        .into_iter()
        .filter_map(|span| {
          self
            .syntax_color(span.kind)
            .map(|color| (span.start_col, span.end_col, color))
        })
        .collect::<Vec<_>>();
      row_syntax.sort_by_key(|(start, end, _)| (*start, *end));
      syntax_rows[surface_row] = row_syntax;
    }

    Ok(VisibleFrame {
      rows: visible,
      syntax_rows,
    })
  }

  fn draw_editor_rows<W: Write>(
    &self,
    stdout: &mut W,
    cols: usize,
    editor_rows: usize,
    present: PresentUpdate,
  ) -> io::Result<()> {
    let metadata_cols = self.metadata_cols(cols);
    let redraw = present.redraw_rows(editor_rows);

    for surface_row in redraw {
      self.draw_surface_row(stdout, surface_row, cols, metadata_cols)?;
    }

    Ok(())
  }

  fn draw_surface_row<W: Write>(
    &self,
    stdout: &mut W,
    surface_row: usize,
    cols: usize,
    metadata_cols: usize,
  ) -> io::Result<()> {
    queue!(
      stdout,
      MoveTo(0, surface_row as u16),
      Clear(ClearType::CurrentLine)
    )?;

    let Some(row) = self.render_view.row(surface_row) else {
      return Ok(());
    };

    let metadata = row.metadata.as_ref();
    self.draw_row_prefix(stdout, surface_row, metadata, metadata_cols)?;

    let text_width = cols.saturating_sub(metadata_cols);
    let text = self.row_text(row, text_width);
    let syntax = self.render_view.syntax_spans(surface_row);
    let selections = self.render_view.selection_spans(surface_row);
    let cursors = self.render_view.cursor_placements(surface_row);
    self.draw_visible_row(
      stdout,
      metadata_cols as u16,
      surface_row as u16,
      &text,
      &syntax,
      &selections,
      &cursors,
    )
  }

  fn draw_row_prefix<W: Write>(
    &self,
    stdout: &mut W,
    surface_row: usize,
    metadata: Option<&RowMetadata>,
    metadata_cols: usize,
  ) -> io::Result<()> {
    if metadata_cols == 0 {
      return Ok(());
    }

    let (label, color) = match metadata.map(|metadata| metadata.kind) {
      Some(RowKind::Document) => {
        if let Some(metadata) = metadata {
          if metadata.wrapped_continuation {
            (String::from("↪"), Color::DarkGrey)
          } else {
            (
              metadata
                .doc_line
                .map(|line| (line + 1).to_string())
                .unwrap_or_else(|| String::from("·")),
              Color::DarkGrey,
            )
          }
        } else {
          (String::from("·"), Color::DarkGrey)
        }
      },
      Some(RowKind::Widget { .. }) => (String::from("widg"), Color::DarkMagenta),
      None => (surface_row.to_string(), Color::DarkGrey),
    };

    queue!(
      stdout,
      MoveTo(0, surface_row as u16),
      SetForegroundColor(color),
      Print(truncate_pad(&label, metadata_cols)),
      SetForegroundColor(Color::Reset),
    )
  }

  fn row_text(&self, row: &FrameRow, width: usize) -> String {
    match row.metadata.as_ref().map(|metadata| metadata.kind) {
      Some(RowKind::Widget {
        widget_id,
        fragment,
      }) => {
        self
          .widget_text(widget_id, fragment as usize, width)
          .unwrap_or_default()
      },
      _ => {
        let text = row.row.text.to_string();
        if width == usize::MAX {
          text
        } else {
          truncate_pad(&text, width)
        }
      },
    }
  }

  fn widget_text(&self, widget_id: WidgetId, fragment: usize, width: usize) -> Option<String> {
    let placement = self.render_view.widget_placement(widget_id)?;
    let primary_cursor = self
      .render_view
      .cursors()
      .iter()
      .find(|cursor| cursor.cursor_id == self.surface().primary_cursor_id())
      .copied();
    let first_syntax = self.render_view.syntax().first().copied();
    let first_selection = self.render_view.selections().first().copied();

    let lines = if widget_id == widget_id_top() {
      vec![
        format!(
          "widget top · row {} · y={} h={} px · cursor x={} y={} h={}",
          placement.surface_row,
          placement.y_px,
          placement.height_px,
          primary_cursor.map(|cursor| cursor.x_px).unwrap_or(0),
          primary_cursor.map(|cursor| cursor.y_px).unwrap_or(0),
          primary_cursor.map(|cursor| cursor.height_px).unwrap_or(0),
        ),
        format!(
          "damage {}..{} full={} · syntax x/w {}/{} · selection x/w {}/{}",
          self.last_present.damage.start_row,
          self.last_present.damage.end_row,
          self.last_present.full,
          first_syntax.map(|span| span.x_px).unwrap_or(0),
          first_syntax.map(|span| span.width_px).unwrap_or(0),
          first_selection.map(|span| span.x_px).unwrap_or(0),
          first_selection.map(|span| span.width_px).unwrap_or(0),
        ),
      ]
    } else {
      vec![format!(
        "widget bottom · changed={} retained={} removed={} meta={} syntax={} selections={} \
         cursors={} widgets={}",
        self.last_present.changed_rows,
        self.last_present.retained_rows,
        self.last_present.removed_rows,
        self.last_present.row_metadata,
        self.last_present.syntax,
        self.last_present.selections,
        self.last_present.cursors,
        self.last_present.widgets,
      )]
    };

    lines.get(fragment).map(|line| {
      if width == usize::MAX {
        line.clone()
      } else {
        truncate_pad(line, width)
      }
    })
  }

  fn draw_visible_row<W: Write>(
    &self,
    stdout: &mut W,
    x: u16,
    y: u16,
    text: &str,
    syntax: &[SyntaxSpan],
    selections: &[SelectionSpan],
    cursors: &[CursorPlacement],
  ) -> io::Result<()> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct CellStyle {
      fg: Option<Color>,
      bg: Option<Color>,
    }

    let primary = self.surface().primary_cursor_id();
    let mut segment = String::new();
    let mut active_style = CellStyle { fg: None, bg: None };
    let mut current_col = 0;

    let style_at = |col: usize, next_col: usize| {
      let fg = syntax.iter().find_map(|span| {
        ranges_overlap(col, next_col, span.start_col, span.end_col)
          .then(|| self.syntax_color(span.kind))
          .flatten()
      });
      let selected = selections
        .iter()
        .any(|span| ranges_overlap(col, next_col, span.start_col, span.end_col));
      let cursor = (self.cursor_shape == CursorShape::Block)
        .then(|| cursors.iter().find(|cursor| cursor.col == col))
        .flatten();

      if let Some(cursor) = cursor {
        CellStyle {
          fg: Some(if cursor.cursor_id == primary {
            Color::Black
          } else {
            Color::White
          }),
          bg: Some(if cursor.cursor_id == primary {
            Color::White
          } else {
            Color::DarkRed
          }),
        }
      } else {
        CellStyle {
          fg,
          bg: selected.then_some(Color::DarkGrey),
        }
      }
    };

    let flush_segment =
      |stdout: &mut W, style: CellStyle, segment: &mut String| -> io::Result<()> {
        if segment.is_empty() {
          return Ok(());
        }
        let flushed = std::mem::take(segment);
        queue!(
          stdout,
          SetForegroundColor(style.fg.unwrap_or(Color::Reset)),
          SetBackgroundColor(style.bg.unwrap_or(Color::Reset)),
          Print(flushed),
        )?;
        Ok(())
      };

    queue!(stdout, MoveTo(x, y))?;
    for ch in text.chars() {
      let mut encoded = [0; 4];
      let next_col = current_col + grapheme_width(ch.encode_utf8(&mut encoded));
      let style = style_at(current_col, next_col.max(current_col + 1));

      if style != active_style {
        flush_segment(stdout, active_style, &mut segment)?;
        active_style = style;
      }

      segment.push(ch);
      current_col = next_col;
    }

    flush_segment(stdout, active_style, &mut segment)?;

    if self.cursor_shape == CursorShape::Block
      && let Some(cursor) = cursors.iter().find(|cursor| cursor.col == current_col)
    {
      let style = CellStyle {
        fg: Some(if cursor.cursor_id == primary {
          Color::Black
        } else {
          Color::White
        }),
        bg: Some(if cursor.cursor_id == primary {
          Color::White
        } else {
          Color::DarkRed
        }),
      };
      queue!(
        stdout,
        SetForegroundColor(style.fg.unwrap_or(Color::Reset)),
        SetBackgroundColor(style.bg.unwrap_or(Color::Reset)),
        Print(" "),
      )?;
    }

    queue!(
      stdout,
      SetForegroundColor(Color::Reset),
      SetBackgroundColor(Color::Reset),
      SetAttribute(Attribute::Reset),
    )?;

    self.draw_cursor_overlays(stdout, x, y, text, cursors)?;

    queue!(
      stdout,
      SetForegroundColor(Color::Reset),
      SetBackgroundColor(Color::Reset),
      SetAttribute(Attribute::Reset),
    )?;
    Ok(())
  }

  fn draw_cursor_overlays<W: Write>(
    &self,
    stdout: &mut W,
    x: u16,
    y: u16,
    text: &str,
    cursors: &[CursorPlacement],
  ) -> io::Result<()> {
    if self.cursor_shape == CursorShape::Block {
      return Ok(());
    }

    let primary = self.surface().primary_cursor_id();
    for cursor in cursors {
      let fg = if cursor.cursor_id == primary {
        Color::White
      } else {
        Color::DarkRed
      };
      let x = x.saturating_add(cursor.col as u16);
      match self.cursor_shape {
        CursorShape::Block => {},
        CursorShape::Bar => {
          queue!(
            stdout,
            MoveTo(x, y),
            SetForegroundColor(fg),
            SetBackgroundColor(Color::Reset),
            Print("▏"),
          )?;
        },
        CursorShape::Underline => {
          queue!(
            stdout,
            MoveTo(x, y),
            SetForegroundColor(fg),
            SetBackgroundColor(Color::Reset),
            SetAttribute(Attribute::Underlined),
            Print(cell_text_at_col(text, cursor.col)),
            SetAttribute(Attribute::NoUnderline),
          )?;
        },
      }
    }

    Ok(())
  }

  fn syntax_color(&self, kind: the_lib::syntax::HighlightKindId) -> Option<Color> {
    let name = self.session.syntax().registry().highlight_name(kind)?;
    let base = name.split('.').next().unwrap_or(name.as_str());

    Some(match base {
      "comment" => Color::DarkGreen,
      "string" => Color::Yellow,
      "constant" | "number" => Color::DarkYellow,
      "keyword" => Color::Magenta,
      "function" | "constructor" => Color::Blue,
      "type" | "tag" | "module" | "namespace" => Color::Cyan,
      "variable" => Color::Green,
      "attribute" => Color::DarkCyan,
      "operator" | "punctuation" => Color::DarkGrey,
      _ => return None,
    })
  }

  fn handle_key(&mut self, key: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
    if matches!(self.mode, Mode::Search { .. }) {
      return self.handle_search_mode_key(key);
    }

    if matches!(self.mode, Mode::SaveAs { .. }) {
      return self.handle_save_as_mode_key(key);
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
      // KeyCode::Char('n') => self.repeat_search(true)?,
      // KeyCode::Char('N') => self.repeat_search(false)?,
      KeyCode::Left => self.apply_transform(SelectionTransform::Horizontal {
        direction: Direction::Backward,
        count: 1,
        mode,
      })?,
      KeyCode::Right => self.apply_transform(SelectionTransform::Horizontal {
        direction: Direction::Forward,
        count: 1,
        mode,
      })?,
      KeyCode::Up => self.apply_transform(SelectionTransform::VisualVertical {
        direction: Direction::Backward,
        count: 1,
        mode,
      })?,
      KeyCode::Down => self.apply_transform(SelectionTransform::VisualVertical {
        direction: Direction::Forward,
        count: 1,
        mode,
      })?,
      KeyCode::PageUp => {
        let count = self.surface().viewport().row_count.max(1);
        self.apply_transform(SelectionTransform::VisualVertical {
          direction: Direction::Backward,
          count,
          mode,
        })?;
      },
      KeyCode::PageDown => {
        let count = self.surface().viewport().row_count.max(1);
        self.apply_transform(SelectionTransform::VisualVertical {
          direction: Direction::Forward,
          count,
          mode,
        })?;
      },
      KeyCode::F(2) => self.toggle_wrap(),
      KeyCode::F(3) => self.start_save_as(),
      KeyCode::F(6) => self.stress_transactions()?,
      KeyCode::F(7) => self.cycle_render_profile(),
      KeyCode::F(8) => self.cycle_cursor_shape(),
      KeyCode::Home => self.apply_transform(SelectionTransform::LineStart { mode })?,
      KeyCode::End => self.apply_transform(SelectionTransform::LineEnd { mode })?,
      KeyCode::Backspace => self.backspace()?,
      KeyCode::Delete => self.delete_forward()?,
      KeyCode::Enter => self.insert_newline()?,
      KeyCode::Tab => {
        let indent = self.document().indent_style().as_str().to_owned();
        self.insert_text(&indent)?;
      },
      KeyCode::Char(ch) => self.insert_char(ch)?,
      _ => {},
    }

    Ok(false)
  }

  fn handle_search_mode_key(&mut self, key: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
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
            .session
            .registers_mut()
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
    }
    Ok(false)
  }

  fn handle_save_as_mode_key(&mut self, key: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
    let Mode::SaveAs { mut path } = std::mem::replace(&mut self.mode, Mode::Normal) else {
      return Ok(false);
    };
    let mut keep_save_as = true;

    match key.code {
      KeyCode::Esc => {
        keep_save_as = false;
        self.status = String::from("save as cancelled");
      },
      KeyCode::Enter => {
        keep_save_as = false;
        if path.is_empty() {
          self.status = String::from("empty save path");
        } else {
          self.save_as(PathBuf::from(path.clone()))?;
        }
      },
      KeyCode::Backspace => {
        path.pop();
      },
      KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
        path.push(ch);
      },
      _ => {},
    }

    if keep_save_as {
      self.mode = Mode::SaveAs { path: path.clone() };
    }
    Ok(false)
  }

  fn handle_control_key(&mut self, key: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
    if key.modifiers.contains(KeyModifiers::SHIFT)
      && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
    {
      self.start_save_as();
      return Ok(false);
    }

    match key.code {
      KeyCode::Char('/') => {
        self.mode = Mode::Search {
          query: String::new(),
        };
        self.status = String::from("search: type regex, Enter to jump, Esc to cancel");
      },
      KeyCode::Char('q') => return Ok(true),
      KeyCode::Char('s') => self.save()?,
      KeyCode::Char('z') => {
        let undone = self
          .session
          .with_surface_document_mut(self.surface, |surface, document| surface.undo(document))??;
        if undone {
          self.status = String::from("undo");
        }
      },
      KeyCode::Char('y') => {
        let redone = self
          .session
          .with_surface_document_mut(self.surface, |surface, document| surface.redo(document))??;
        if redone {
          self.status = String::from("redo");
        }
      },
      KeyCode::Char('a') => {
        let len = self.document().text().len_chars();
        self.apply_selection(Selection::single(0, len))?;
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
      KeyCode::Char('g') => self.apply_transform(SelectionTransform::DocumentStart {
        mode: MotionMode::Move,
      })?,
      // KeyCode::Char('e') => self.apply_transform(SelectionTransform::DocumentEnd {
      //   mode: MotionMode::Move,
      // })?,
      _ => {},
    }

    Ok(false)
  }

  fn apply_transform(
    &mut self,
    transform: SelectionTransform,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let surface = self.surface();
    let viewport = surface.viewport();
    let text = self.document().text().slice(..);
    let cx = MovementContext {
      text_format: surface.text_format(),
      annotations: surface.text_annotations(),
      wrap_width:  viewport.wrap_width,
    };
    let result = transform_selection(
      text,
      surface.selection().clone(),
      surface.visual_goals(),
      transform,
      &cx,
    );

    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface.apply_motion_result(document, result)
      })??;
    Ok(())
  }

  fn insert_char(&mut self, ch: char) -> Result<(), Box<dyn std::error::Error>> {
    let edit = edit_for_typed_char(
      self.document().text(),
      self.surface().selection(),
      ch,
      &self.auto_pairs,
    )?;

    if let Some(edit) = edit {
      self.apply_edit(edit)?;
      self.transaction_count += 1;
      self.status = format!("inserted {ch:?}");
      return Ok(());
    }

    self.insert_text(&ch.to_string())
  }

  fn insert_text(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let range = self.surface().primary_range();
    let span = Span {
      start: range.from(),
      end:   range.to(),
    };
    let tx = Transaction::from_replacements(
      self.document().text().len_chars(),
      [Replacement::replace(span, text)],
      ReplacementOverlapPolicy::Reject,
    )?;
    let selection = Selection::point(span.start + text.chars().count());
    self.apply_edit(Edit {
      transaction: tx,
      selection,
    })?;
    self.transaction_count += 1;
    Ok(())
  }

  fn backspace(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let edit = edit_for_backspace(
      self.document().text(),
      self.surface().selection(),
      &self.auto_pairs,
    )?;

    if let Some(edit) = edit {
      self.apply_edit(edit)?;
      self.transaction_count += 1;
      self.status = String::from("auto-pair backspace");
      return Ok(());
    }

    let range = self.surface().primary_range();
    if !range.is_empty() {
      self.delete_span(range.from(), range.to())?;
      return Ok(());
    }

    if range.head == 0 {
      return Ok(());
    }

    let text = self.document().text().slice(..);
    let start = prev_grapheme_boundary(text, range.head);
    self.delete_span(start, range.head)
  }

  fn delete_forward(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let range = self.surface().primary_range();
    if !range.is_empty() {
      self.delete_span(range.from(), range.to())?;
      return Ok(());
    }

    let text = self.document().text().slice(..);
    if range.head >= text.len_chars() {
      return Ok(());
    }

    let end = next_grapheme_boundary(text, range.head);
    self.delete_span(range.head, end)
  }

  fn delete_span(&mut self, start: usize, end: usize) -> Result<(), Box<dyn std::error::Error>> {
    let tx = Transaction::delete_spans(
      self.document().text().len_chars(),
      [Span { start, end }],
      DeletionOverlapPolicy::Reject,
    )?;
    self.apply_edit(Edit {
      transaction: tx,
      selection:   Selection::point(start),
    })?;
    self.transaction_count += 1;
    self.status = String::from("deleted");
    Ok(())
  }

  fn insert_newline(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let surface = self.surface();
    let text = self.document().text().slice(..);
    let cursor = surface.primary_range().cursor(text);
    let line = text.char_to_line(cursor);
    let indent = indent_for_newline(
      None,
      None,
      &IndentationHeuristic::Simple,
      &self.document().indent_style(),
      surface.text_format().tab_width as usize,
      text,
      line,
      cursor,
      line,
    );

    let newline = format!("{}{}", self.document().line_ending().as_str(), indent);
    self.insert_text(&newline)
  }

  fn copy_selection_or_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let values = self.selection_or_line_values();
    self.status = self.write_yank_targets(values, "copied")?;
    Ok(())
  }

  fn cut_selection_or_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let values = self.selection_or_line_values();
    let status = self.write_yank_targets(values, "cut")?;

    if self.surface().primary_range().is_empty() {
      let text = self.document().text().slice(..);
      let cursor = self.surface().primary_range().cursor(text);
      let line = text.char_to_line(cursor);
      let start = text.line_to_char(line);
      let end = if line + 1 < text.len_lines() {
        text.line_to_char(line + 1)
      } else {
        text.len_chars()
      };
      self.delete_span(start, end)?;
    } else {
      let range = self.surface().primary_range();
      self.delete_span(range.from(), range.to())?;
    }

    self.status = status;
    Ok(())
  }

  fn paste_register(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    match self.session.read_clipboard(ClipboardType::Clipboard) {
      Ok(text) if text.is_empty() => {
        self.status = String::from("clipboard is empty");
        Ok(())
      },
      Ok(text) => {
        self.insert_text(&text)?;
        self.status = format!("pasted {} clipboard", self.session.clipboard_name());
        Ok(())
      },
      Err(_) => {
        let Some(values) = self.session.registers().read('"') else {
          self.status = String::from("clipboard unavailable and register \" is empty");
          return Ok(());
        };

        let joined = values
          .iter()
          .map(|value| value.as_str())
          .collect::<Vec<_>>()
          .join(self.document().line_ending().as_str());
        self.insert_text(&joined)?;
        self.status = String::from("pasted register \"");
        Ok(())
      },
    }
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

    let surface = self.surface();
    let Some(selection) = search_regex(
      self.document().text().slice(..),
      surface.selection(),
      CursorPick::Id(surface.primary_cursor_id()),
      &regex,
      MotionMode::Move,
      direction,
      true,
    ) else {
      self.status = format!("no match for /{query}");
      return Ok(());
    };

    self.apply_selection(selection)?;
    self.status = format!("searched /{query}");
    Ok(())
  }

  #[allow(dead_code)]
  fn repeat_search(&mut self, forward: bool) -> Result<(), Box<dyn std::error::Error>> {
    let Some(query) = self.session.registers().first('/') else {
      self.status = String::from("no previous search");
      return Ok(());
    };
    let query = query.to_string();
    self.run_search(&query, forward)
  }

  fn toggle_wrap(&mut self) {
    let format = {
      let mut format = self.surface().text_format().clone();
      format.soft_wrap = !format.soft_wrap;
      format
    };

    let _ = self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface.set_text_format_reconcile_viewport(document, format.clone())
      });

    self.status = if format.soft_wrap {
      String::from("soft wrap enabled")
    } else {
      String::from("soft wrap disabled")
    };
  }

  fn cycle_render_profile(&mut self) {
    self.render_profile = self.render_profile.next();
    self.status = format!("render profile: {}", self.render_profile.label());
  }

  fn cycle_cursor_shape(&mut self) {
    self.cursor_shape = self.cursor_shape.next();
    self.status = format!("cursor shape: {}", self.cursor_shape.label());
  }

  fn stress_transactions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    for _ in 0..1000 {
      self.insert_text("x")?;
    }
    self.status = format!(
      "1000 transactions in {:.2?} · revision {}",
      start.elapsed(),
      self.document().revision()
    );
    Ok(())
  }

  fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = self.document_path().map(Path::to_path_buf) else {
      self.start_save_as();
      return Ok(());
    };

    self.session.save(self.document)?;
    self.status = format!("saved {}", path.display());
    Ok(())
  }

  fn save_as(&mut self, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    self.session.save_as(self.document, path)?;
    let path = self
      .document_path()
      .map(Path::to_path_buf)
      .unwrap_or_else(|| PathBuf::from("[unknown]"));
    self.status = format!("saved as {}", path.display());
    Ok(())
  }

  fn start_save_as(&mut self) {
    let path = self
      .document_path()
      .map(|path| path.display().to_string())
      .unwrap_or_default();
    self.mode = Mode::SaveAs { path: path.clone() };
    self.status = String::from("save as: type a path, Enter to save, Esc to cancel");
  }

  fn prompt_line(&self) -> Option<String> {
    match &self.mode {
      Mode::Normal => None,
      Mode::Search { query } => Some(format!("/{query}")),
      Mode::SaveAs { path } => Some(format!("save as: {path}")),
    }
  }

  fn footer_rows(&self) -> usize {
    1 + usize::from(self.prompt_line().is_some())
  }

  fn metadata_cols(&self, terminal_cols: usize) -> usize {
    if self.render_profile.show_metadata() && terminal_cols > 20 {
      8
    } else {
      0
    }
  }

  fn selection_or_line_values(&self) -> Vec<Tendril> {
    if self.surface().primary_range().is_empty() {
      return vec![Tendril::from(self.current_line_text().as_str())];
    }

    self
      .surface()
      .selection()
      .fragments(self.document().text().slice(..))
      .map(|fragment| Tendril::from(fragment.as_ref()))
      .collect()
  }

  fn join_yank_values(&self, values: &[Tendril]) -> String {
    values
      .iter()
      .map(|value| value.as_str())
      .collect::<Vec<_>>()
      .join(self.document().line_ending().as_str())
  }

  fn write_yank_targets(
    &mut self,
    values: Vec<Tendril>,
    verb: &str,
  ) -> Result<String, Box<dyn std::error::Error>> {
    let clipboard_text = self.join_yank_values(&values);
    self.session.registers_mut().write('"', values);

    match self
      .session
      .write_clipboard(&clipboard_text, ClipboardType::Clipboard)
    {
      Ok(()) => {
        Ok(format!(
          "{verb} to register \" and {} clipboard",
          self.session.clipboard_name()
        ))
      },
      Err(err) => {
        Ok(format!(
          "{verb} to register \" (clipboard unavailable: {err})"
        ))
      },
    }
  }

  fn current_line_text(&self) -> String {
    let text = self.document().text().slice(..);
    let cursor = self.surface().primary_range().cursor(text);
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
    let text = self.document().text().slice(..);
    let cursor = self.surface().primary_range().cursor(text);
    let line = text.char_to_line(cursor);
    let line_start = text.line_to_char(line);
    let col = cursor.saturating_sub(line_start);
    let mode = match &self.mode {
      Mode::Normal => "NORMAL",
      Mode::Search { .. } => "SEARCH",
      Mode::SaveAs { .. } => "SAVE-AS",
    };
    let name = self
      .document_path()
      .map(|path| path.display().to_string())
      .unwrap_or_else(|| String::from("[scratch]"));
    let modified = if self.is_dirty() { " [+]" } else { "" };
    let selection_len = self.surface().primary_range().len();
    let present = self.last_present;

    let info = format!(
      " {mode} {name}{modified} | profile {} | cursor {} | ln {}, col {} | sel {} | rev {} | tx \
       {} | render {:?} | gen {} | Δ c/rm/r {} / {} / {} | meta {} | dmg {}..{}{} ",
      self.render_profile.label(),
      self.cursor_shape.label(),
      line + 1,
      col + 1,
      selection_len,
      self.document().revision(),
      self.transaction_count,
      self.last_render,
      present.generation,
      present.changed_rows,
      present.retained_rows,
      present.removed_rows,
      present.row_metadata,
      present.damage.start_row,
      present.damage.end_row,
      if present.full { " full" } else { "" },
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

fn widget_id_top() -> WidgetId {
  WidgetId::new(1).expect("widget ids are non-zero")
}

fn widget_id_bottom() -> WidgetId {
  WidgetId::new(2).expect("widget ids are non-zero")
}

fn first_substitutable_char(text: ropey::RopeSlice<'_>) -> Option<(usize, &'static str)> {
  text.chars().enumerate().find_map(|(index, ch)| {
    match ch {
      '\t' => Some((index, "→")),
      ' ' => Some((index, "·")),
      _ => None,
    }
  })
}

fn ranges_overlap(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> bool {
  start_a < end_b && start_b < end_a
}

fn cell_text_at_col(text: &str, target_col: usize) -> String {
  let mut col = 0;
  for ch in text.chars() {
    let mut encoded = [0; 4];
    let width = grapheme_width(ch.encode_utf8(&mut encoded)).max(1);
    let next_col = col + width;
    if target_col >= col && target_col < next_col {
      return ch.to_string();
    }
    col = next_col;
  }
  String::from(" ")
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

#[cfg(test)]
mod tests {
  use super::*;

  fn strip_ansi(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
      if ch != '\u{1b}' {
        out.push(ch);
        continue;
      }

      if chars.next_if_eq(&'[').is_none() {
        continue;
      }

      while let Some(next) = chars.next() {
        if ('@'..='~').contains(&next) {
          break;
        }
      }
    }

    out
  }

  fn demo_app() -> App {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/demo_editor.rs");
    App::open_with_viewport(Some(path), Viewport::new(0, 20, 0, 120))
      .expect("demo app should open example source")
  }

  #[test]
  fn visible_rust_rows_receive_mapped_syntax_colors() {
    let mut app = demo_app();
    let frame = app
      .render_visible_frame(20)
      .expect("visible frame should render");

    let first_nonempty_row = frame
      .rows
      .iter()
      .position(|row| !row.trim().is_empty())
      .expect("expected at least one non-empty row");

    assert!(
      !frame.syntax_rows[first_nonempty_row].is_empty(),
      "expected mapped syntax colors on first non-empty row {}; rows={:?}; doc_syntax={:?}",
      first_nonempty_row,
      frame.rows.iter().take(5).collect::<Vec<_>>(),
      app.document().syntax(),
    );
  }

  #[test]
  fn drawing_colored_rust_row_emits_non_default_ansi_color() {
    let mut app = demo_app();
    let frame = app
      .render_visible_frame(20)
      .expect("visible frame should render");

    let row = frame
      .rows
      .iter()
      .enumerate()
      .find(|(index, text)| !text.trim().is_empty() && !frame.syntax_rows[*index].is_empty())
      .map(|(index, _)| index)
      .expect("expected at least one colored visible row");

    let mut out = Vec::new();
    app
      .draw_visible_row(
        &mut out,
        0,
        0,
        &frame.rows[row],
        &app.render_view.syntax_spans(row),
        &[],
        &[],
      )
      .expect("row should draw");

    let output = String::from_utf8(out).expect("terminal output should be utf-8");
    assert!(
      output.contains("\u{1b}[38;5;") || output.contains("\u{1b}[3") || output.contains("\u{1b}[9"),
      "expected ANSI color escape in output: {output:?}"
    );

    let plain = strip_ansi(&output);
    assert!(
      plain.contains(&frame.rows[row]),
      "expected row text in output: {output:?}"
    );
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();
  let mut app = App::open(args.path)?;
  app.run()
}
