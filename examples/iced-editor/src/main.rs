use std::path::{
  Path,
  PathBuf,
};

use cosmic_text::{
  Attrs,
  Buffer,
  Family,
  FontSystem,
  Metrics,
  Shaping,
};
use iced::{
  Element,
  Fill,
  Font,
  Pixels,
  Subscription,
  Theme,
  keyboard::{
    self,
    Key,
    key::Named,
  },
  widget::{
    column,
    container,
    row,
    space,
    stack,
    text,
  },
};
use ropey::Rope;
use the_lib::{
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
  render::{
    PresentUpdate,
    RetainedFrame,
    RowKind,
    CellGridMetrics,
    Viewport,
  },
  selection::Selection,
  surface::SurfaceViewSpec,
  syntax::IndentationHeuristic,
  transaction::{
    Span,
    Transaction,
  },
};
use the_runtime::{
  DocumentHandle,
  RuntimeSession,
  SurfaceHandle,
};

const VIEW_ROWS: usize = 28;
const VIEW_COLS: usize = 96;
const FONT_SIZE_PX: f32 = 16.0;
const DEFAULT_TEXT: &str = include_str!("../../../the-lib/examples/demo_wrap_repro.txt");

fn main() -> iced::Result {
  let path = std::env::args().nth(1).map(PathBuf::from);

  iced::application(move || IcedEditor::open(path.clone()), update, view)
    .title(title)
    .theme(theme)
    .subscription(subscription)
    .run()
}

#[derive(Debug, Clone)]
enum Message {
  Keyboard(keyboard::Event),
}

#[derive(Debug, Clone, Copy)]
struct IcedCellMetrics {
  text_size_px:   f32,
  line_height_px: f32,
  cell_width_px:  u32,
  row_height_px:  u32,
}

struct IcedEditor {
  session:      RuntimeSession,
  document:     DocumentHandle,
  surface:      SurfaceHandle,
  render:       RetainedFrame,
  present:      PresentUpdate,
  status:       String,
  opened_path:  Option<PathBuf>,
  cell_metrics: IcedCellMetrics,
}

impl IcedEditor {
  fn open(path: Option<PathBuf>) -> Self {
    let mut session = RuntimeSession::default();

    let (document, opened_path, mut status) = match path.clone() {
      Some(path) => match session.open_file(&path) {
        Ok(document) => (document, Some(path), String::new()),
        Err(err) => {
          let fallback = fallback_text();
          let document = session
            .create_scratch(Rope::from(fallback.as_str()))
            .expect("scratch creation should succeed");
          (
            document,
            None,
            format!("failed to open {}: {err} · using scratch", path.display()),
          )
        },
      },
      None => {
        let fallback = fallback_text();
        let document = session
          .create_scratch(Rope::from(fallback.as_str()))
          .expect("scratch creation should succeed");
        (document, None, String::from("scratch document"))
      },
    };

    let cell_metrics = measure_cell_grid_metrics();
    let viewport = Viewport::new(0, VIEW_ROWS, 0, VIEW_COLS);
    let surface = session
      .create_surface_with_viewport(document, viewport)
      .expect("surface creation should succeed");
    session
      .surface_mut(surface)
      .expect("surface should exist")
      .set_view(SurfaceViewSpec {
        measurements: CellGridMetrics::new(
          cell_metrics.cell_width_px,
          cell_metrics.row_height_px,
        ),
        ..SurfaceViewSpec::new(viewport)
      });

    let mut editor = Self {
      session,
      document,
      surface,
      render: RetainedFrame::default(),
      present: PresentUpdate::default(),
      status: std::mem::take(&mut status),
      opened_path,
      cell_metrics,
    };
    editor.refresh_frame();
    editor
  }

  fn handle(&mut self, message: Message) {
    let result = match message {
      Message::Keyboard(event) => self.handle_keyboard_event(event),
    };

    match result {
      Ok(needs_refresh) => {
        if needs_refresh {
          self.refresh_frame();
        }
      },
      Err(err) => {
        self.status = err;
      },
    }
  }

  fn document_path(&self) -> Option<&Path> {
    self.opened_path.as_deref()
  }

  fn surface(&self) -> &the_lib::surface::Surface {
    self
      .session
      .surface(self.surface)
      .expect("surface handle should stay valid")
  }

  fn handle_keyboard_event(&mut self, event: keyboard::Event) -> Result<bool, String> {
    let keyboard::Event::KeyPressed {
      key,
      modified_key,
      physical_key,
      modifiers,
      text,
      ..
    } = event
    else {
      return Ok(false);
    };

    if modifiers.command() {
      if let Some(ch) = modified_key.to_latin(physical_key) {
        match ch {
          'z' => return self.undo(),
          'y' => return self.redo(),
          _ => {},
        }
      }
    }

    let mode = if modifiers.shift() {
      MotionMode::Extend
    } else {
      MotionMode::Move
    };

    match key.as_ref() {
      Key::Named(Named::ArrowLeft) => return self.apply_motion(move_left, 1, mode),
      Key::Named(Named::ArrowRight) => return self.apply_motion(move_right, 1, mode),
      Key::Named(Named::ArrowUp) => return self.apply_motion(move_up_visual, 1, mode),
      Key::Named(Named::ArrowDown) => return self.apply_motion(move_down_visual, 1, mode),
      Key::Named(Named::PageUp) => {
        let count = self.surface().viewport().row_count.max(1);
        return self.apply_motion(move_up_visual, count, mode);
      },
      Key::Named(Named::PageDown) => {
        let count = self.surface().viewport().row_count.max(1);
        return self.apply_motion(move_down_visual, count, mode);
      },
      Key::Named(Named::Home) => {
        if modifiers.command() || modifiers.macos_command() {
          return self.apply_motion_simple(move_document_start, mode);
        }
        return self.apply_motion_simple(move_line_start, mode);
      },
      Key::Named(Named::End) => {
        if modifiers.command() || modifiers.macos_command() {
          return self.apply_motion_simple(move_document_end, mode);
        }
        return self.apply_motion_simple(move_line_end, mode);
      },
      Key::Named(Named::Backspace) => return self.backspace(),
      Key::Named(Named::Delete) => return self.delete_forward(),
      Key::Named(Named::Enter) => return self.insert_newline(),
      Key::Named(Named::Tab) => {
        let indent = self
          .session
          .document(self.document)
          .expect("document handle should stay valid")
          .indent_style()
          .as_str()
          .to_owned();
        return self.insert_text(&indent);
      },
      Key::Named(Named::F2) => return self.toggle_wrap(),
      _ => {},
    }

    if modifiers.control() || modifiers.alt() {
      return Ok(false);
    }

    if let Some(text) = text {
      let inserted = text.as_str();
      if !inserted.is_empty() && inserted != "\u{7f}" && inserted != "\r" {
        return self.insert_text(inserted);
      }
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
  ) -> Result<bool, String> {
    let surface = self.surface();
    let viewport = surface.viewport();
    let text = self
      .session
      .document(self.document)
      .expect("document handle should stay valid")
      .text()
      .slice(..);
    let cx = MovementContext {
      text_format: surface.text_format(),
      annotations: surface.text_annotations(),
      wrap_width: viewport.wrap_width,
    };
    let result = motion(
      text,
      surface.selection().clone(),
      count,
      mode,
      surface.visual_goals(),
      &cx,
    );

    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface.apply_motion_result(document, result)
      })
      .map_err(|err| err.to_string())?
      .map_err(|err| err.to_string())?;
    Ok(true)
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
  ) -> Result<bool, String> {
    let surface = self.surface();
    let viewport = surface.viewport();
    let text = self
      .session
      .document(self.document)
      .expect("document handle should stay valid")
      .text()
      .slice(..);
    let cx = MovementContext {
      text_format: surface.text_format(),
      annotations: surface.text_annotations(),
      wrap_width: viewport.wrap_width,
    };
    let result = motion(
      text,
      surface.selection().clone(),
      mode,
      surface.visual_goals(),
      &cx,
    );

    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface.apply_motion_result(document, result)
      })
      .map_err(|err| err.to_string())?
      .map_err(|err| err.to_string())?;
    Ok(true)
  }

  fn toggle_wrap(&mut self) -> Result<bool, String> {
    let format = {
      let mut format = self.surface().text_format().clone();
      format.soft_wrap = !format.soft_wrap;
      format
    };

    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface.set_text_format_reconcile_viewport(document, format.clone());
        Ok::<(), String>(())
      })
      .map_err(|err| err.to_string())??;

    self.status = if format.soft_wrap {
      String::from("soft wrap enabled")
    } else {
      String::from("soft wrap disabled")
    };
    Ok(true)
  }

  fn insert_text(&mut self, inserted: &str) -> Result<bool, String> {
    let inserted_len = inserted.chars().count();
    let range = self.surface().primary_range();
    let span = Span {
      start: range.from(),
      end: range.to(),
    };
    let transaction = Transaction::replace(
      self
        .session
        .document(self.document)
        .expect("document handle should stay valid")
        .text()
        .len_chars(),
      span,
      inserted,
    )
    .map_err(|err| err.to_string())?;

    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface
          .apply_edit(document, Edit {
            transaction,
            selection: Selection::point(span.start + inserted_len),
          })
          .map_err(|err| err.to_string())
      })
      .map_err(|err| err.to_string())??;

    self.status = format!("inserted {} chars", inserted_len);
    Ok(true)
  }

  fn backspace(&mut self) -> Result<bool, String> {
    let range = self.surface().primary_range();
    if !range.is_empty() {
      return self.delete_span(range.from(), range.to());
    }
    if range.head == 0 {
      return Ok(false);
    }

    let text = self
      .session
      .document(self.document)
      .expect("document handle should stay valid")
      .text()
      .slice(..);
    let start = prev_grapheme_boundary(text, range.head);
    self.delete_span(start, range.head)
  }

  fn delete_forward(&mut self) -> Result<bool, String> {
    let range = self.surface().primary_range();
    if !range.is_empty() {
      return self.delete_span(range.from(), range.to());
    }

    let text = self
      .session
      .document(self.document)
      .expect("document handle should stay valid")
      .text()
      .slice(..);
    if range.head >= text.len_chars() {
      return Ok(false);
    }

    let end = next_grapheme_boundary(text, range.head);
    self.delete_span(range.head, end)
  }

  fn delete_span(&mut self, start: usize, end: usize) -> Result<bool, String> {
    let transaction = Transaction::delete(
      self
        .session
        .document(self.document)
        .expect("document handle should stay valid")
        .text()
        .len_chars(),
      Span { start, end },
    )
    .map_err(|err| err.to_string())?;

    self
      .session
      .with_surface_document_mut(self.surface, |surface, document| {
        surface
          .apply_edit(document, Edit {
            transaction,
            selection: Selection::point(start),
          })
          .map_err(|err| err.to_string())
      })
      .map_err(|err| err.to_string())??;

    self.status = String::from("deleted");
    Ok(true)
  }

  fn insert_newline(&mut self) -> Result<bool, String> {
    let surface = self.surface();
    let document = self
      .session
      .document(self.document)
      .expect("document handle should stay valid");
    let text = document.text().slice(..);
    let cursor = surface.primary_range().cursor(text);
    let line = text.char_to_line(cursor);
    let indent = indent_for_newline(
      None,
      None,
      &IndentationHeuristic::Simple,
      &document.indent_style(),
      surface.text_format().tab_width as usize,
      text,
      line,
      cursor,
      line,
    );
    let newline = format!(
      "{}{}",
      get_line_ending(&text).unwrap_or(document.line_ending()).as_str(),
      indent,
    );
    self.insert_text(&newline)
  }

  fn undo(&mut self) -> Result<bool, String> {
    let changed = self
      .session
      .with_surface_document_mut(self.surface, |surface, document| surface.undo(document))
      .map_err(|err| err.to_string())?
      .map_err(|err| err.to_string())?;
    if changed {
      self.status = String::from("undo");
    }
    Ok(changed)
  }

  fn redo(&mut self) -> Result<bool, String> {
    let changed = self
      .session
      .with_surface_document_mut(self.surface, |surface, document| surface.redo(document))
      .map_err(|err| err.to_string())?
      .map_err(|err| err.to_string())?;
    if changed {
      self.status = String::from("redo");
    }
    Ok(changed)
  }

  fn refresh_frame(&mut self) {
    match self
      .session
      .present_surface(self.surface, &mut self.render)
    {
      Ok(update) => {
        self.present = update;
      },
      Err(err) => {
        self.status = format!("render failed: {err}");
      },
    }
  }

  fn viewport_label(&self) -> String {
    let viewport = self.surface().viewport();
    format!(
      "rows {}..{} · cols {}..{} · wrap {}",
      viewport.first_row,
      viewport.first_row.saturating_add(viewport.row_count),
      viewport.first_col,
      viewport.first_col.saturating_add(viewport.wrap_width),
      if self.surface().text_format().soft_wrap {
        "on"
      } else {
        "off"
      },
    )
  }

  fn cell_grid_label(&self) -> String {
    format!(
      "Font::MONOSPACE @ {:.1}px · measured cell {}×{} px · line height {:.1}px · the-lib still assumes a uniform cell grid",
      self.cell_metrics.text_size_px,
      self.cell_metrics.cell_width_px,
      self.cell_metrics.row_height_px,
      self.cell_metrics.line_height_px,
    )
  }

  fn document_name(&self) -> String {
    self
      .document_path()
      .map(|path| path.display().to_string())
      .unwrap_or_else(|| String::from("[scratch]"))
  }

  fn document_revision(&self) -> u64 {
    self
      .session
      .document(self.document)
      .expect("document handle should stay valid")
      .revision()
  }

  fn render_row_text(&self, surface_row: usize) -> String {
    let Some(row) = self.render.row(surface_row) else {
      return String::new();
    };

    match row.metadata.as_ref().map(|metadata| metadata.kind) {
      Some(RowKind::Widget {
        widget_id,
        fragment,
      }) => format!("[widget {} · fragment {}]", widget_id.get(), fragment),
      _ => row.row.text.to_string(),
    }
  }

  fn gutter_label(&self, surface_row: usize) -> String {
    self
      .render
      .row(surface_row)
      .and_then(|row| row.metadata.as_ref())
      .map(|metadata| match metadata.kind {
        RowKind::Document => {
          if metadata.wrapped_continuation {
            String::from("   ↪")
          } else {
            format!("{:>4}", metadata.doc_line.map(|line| line + 1).unwrap_or(0))
          }
        },
        RowKind::Widget { .. } => String::from(" wid"),
      })
      .unwrap_or_else(|| String::from("    "))
  }
}

fn title(_: &IcedEditor) -> String {
  String::from("the-editor · iced demo")
}

fn theme(_: &IcedEditor) -> Theme {
  Theme::TokyoNight
}

fn subscription(_: &IcedEditor) -> Subscription<Message> {
  keyboard::listen().map(Message::Keyboard)
}

fn update(editor: &mut IcedEditor, message: Message) {
  editor.handle(message);
}

fn view(editor: &IcedEditor) -> Element<'_, Message> {
  let viewport = editor.surface().viewport();
  let primary_cursor = editor.surface().primary_cursor_id();

  let controls = column![
    text(editor.document_name()).size(24),
    text(editor.viewport_label()),
    text(editor.cell_grid_label()),
    text(
      "Type directly. Arrows move · Shift+Arrows select · Enter newline · Backspace/Delete edit"
    ),
    text(
      "Home/End line · PageUp/PageDown page · Ctrl/Cmd+Z undo · Ctrl/Cmd+Y redo · F2 wrap"
    ),
  ]
  .spacing(8);

  let mut rows = column![];
  for surface_row in 0..viewport.row_count {
    let text_row = text(editor.render_row_text(surface_row))
      .font(Font::MONOSPACE)
      .size(FONT_SIZE_PX)
      .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
        editor.cell_metrics.line_height_px,
      )))
      .width(Fill);

    let cursor_overlay = editor
      .render
      .cursor_placements(surface_row)
      .into_iter()
      .find(|cursor| cursor.cursor_id == primary_cursor)
      .map(|cursor| {
        row![
          space().width(cursor.x_px as f32),
          text("▏")
            .font(Font::MONOSPACE)
            .size(FONT_SIZE_PX)
            .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
              editor.cell_metrics.line_height_px,
            ))),
        ]
        .into()
      })
      .unwrap_or_else(|| space().into());

    let line = row![
      text(editor.gutter_label(surface_row))
        .font(Font::MONOSPACE)
        .size(FONT_SIZE_PX)
        .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
          editor.cell_metrics.line_height_px,
        )))
        .width(56),
      stack([
        text_row.into(),
        cursor_overlay,
      ])
      .width(Fill),
    ]
    .spacing(12);

    rows = rows.push(line);
  }

  let stats = text(format!(
    "rev {} · gen {} · changed {} · retained {} · removed {} · damage {}..{}{}",
    editor.document_revision(),
    editor.present.generation,
    editor.present.changed_rows,
    editor.present.retained_rows,
    editor.present.removed_rows,
    editor.present.damage.start_row,
    editor.present.damage.end_row,
    if editor.present.full { " · full" } else { "" },
  ));

  let content = column![
    controls,
    container(rows.spacing(4)).padding(12).width(Fill),
    stats,
    text(&editor.status),
  ]
  .spacing(16)
  .padding(16)
  .max_width(1400);

  container(content)
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .into()
}

fn measure_cell_grid_metrics() -> IcedCellMetrics {
  let line_height_px = iced::widget::text::LineHeight::default()
    .to_absolute(Pixels(FONT_SIZE_PX))
    .0;

  let mut font_system = FontSystem::new();
  let metrics = Metrics::new(FONT_SIZE_PX, line_height_px);
  let mut buffer = Buffer::new(&mut font_system, metrics);
  buffer.set_size(&mut font_system, None, None);
  buffer.set_text(
    &mut font_system,
    "M",
    &Attrs::new().family(Family::Monospace),
    Shaping::Advanced,
    None,
  );
  buffer.shape_until_scroll(&mut font_system, true);

  let cell_width_px = buffer
    .layout_runs()
    .next()
    .and_then(|run| run.glyphs.first())
    .map(|glyph| glyph.w.ceil().max(1.0) as u32)
    .unwrap_or(8);

  IcedCellMetrics {
    text_size_px: FONT_SIZE_PX,
    line_height_px,
    cell_width_px,
    row_height_px: line_height_px.ceil().max(1.0) as u32,
  }
}

fn fallback_text() -> String {
  format!(
    "the-editor + iced\n\nThis example keeps the client GUI in iced while the-editor owns the \ntext, surface, history, and visible render model.\n\n- Type directly into the editor\n- Use arrows, Home/End, and PageUp/PageDown\n- F2 toggles soft wrap\n- Undo and redo go through Surface\n\n---\n\n{}",
    DEFAULT_TEXT
  )
}
