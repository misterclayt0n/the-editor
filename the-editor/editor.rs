use the_editor_renderer::{
  Application,
  Color,
  InputEvent,
  Renderer,
  TextSection,
  TextSegment,
};

use crate::{
  core::{
    commands::*,
    document::Document,
  },
  keymap::{
    KeymapResult,
    Keymaps,
    Mode,
    default,
  },
};

pub struct Editor {
  document: Document,
  mode:     Mode,
  keymaps:  Keymaps,
}

impl Editor {
  pub fn new() -> Self {
    let text =
      "Hello slkdj  world, this is a sample buffer: ççç你好\nalskjdlaskds\naskldjsalkjdl jaslkdjad";

    Self {
      document: Document::with_text(text),
      mode:     Mode::Normal,
      keymaps:  Keymaps::new(default::default()),
    }
  }
}

impl Application for Editor {
  fn init(&mut self, _renderer: &mut Renderer) {
    println!("Editor initialized!");
  }

  fn render(&mut self, renderer: &mut Renderer) {
    renderer.draw_text(TextSection::simple(
      50.0,
      50.0,
      "Welcome to The Editor!",
      48.0,
      Color::rgb(0.9, 0.9, 0.9),
    ));

    renderer.draw_text(TextSection::simple(
      50.0,
      120.0,
      "A modern text editor built with Rust",
      24.0,
      Color::rgb(0.7, 0.7, 0.8),
    ));

    renderer.draw_text(TextSection::simple(
      50.0,
      180.0,
      "Press ESC to exit",
      18.0,
      Color::rgb(0.5, 0.5, 0.6),
    ));

    renderer.draw_text(
      TextSection::new(50.0, 250.0).add_text(
        TextSegment::new("// Sample code")
          .with_color(Color::rgb(0.6, 0.8, 0.6))
          .with_size(20.0),
      ),
    );

    renderer.draw_text(
      TextSection::new(50.0, 280.0)
        .add_text(
          TextSegment::new("fn ")
            .with_color(Color::rgb(0.5, 0.7, 0.9))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("main")
            .with_color(Color::rgb(0.9, 0.9, 0.7))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("() {")
            .with_color(Color::rgb(0.9, 0.9, 0.85))
            .with_size(20.0),
        ),
    );

    renderer.draw_text(
      TextSection::new(50.0, 310.0)
        .add_text(
          TextSegment::new("    println!")
            .with_color(Color::rgb(0.5, 0.7, 0.9))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("(")
            .with_color(Color::rgb(0.9, 0.9, 0.85))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new("\"Hello, world!\"")
            .with_color(Color::rgb(0.8, 0.6, 0.4))
            .with_size(20.0),
        )
        .add_text(
          TextSegment::new(");")
            .with_color(Color::rgb(0.9, 0.9, 0.85))
            .with_size(20.0),
        ),
    );

    renderer.draw_text(TextSection::simple(
      50.0,
      340.0,
      "}",
      20.0,
      Color::rgb(0.9, 0.9, 0.85),
    ));

    // Draw current buffer text and overlay a block cursor without altering layout.
    // NOTE: draw base text once, then overlay a block glyph at the cursor cell
    // using a string of spaces to advance to the same monospaced column,
    // followed by the underlying character on top for contrast.
    // TODO: We should abstract this to the renderer it self, maybe turn cursor into
    // a component.
    let doc_text = self.document.text();
    let pos = if let Some(sel) = self.document.selection_ref(0) {
      sel.primary().cursor(doc_text.slice(..))
    } else {
      0
    };

    let normal = Color::rgb(0.85, 0.85, 0.9);
    let cursor_fg = Color::rgb(0.1, 0.1, 0.15); // dark text on bright block
    let cursor_bg = Color::rgb(0.2, 0.8, 0.7); // teal block
    let font_size = 22.0;

    let full = doc_text.to_string();
    renderer.draw_text(TextSection::simple(50.0, 380.0, full, font_size, normal));

    // Calculate cursor position accounting for newlines
    let line_idx = doc_text.char_to_line(pos);
    let line_start = doc_text.line_to_char(line_idx);
    let col_in_line = pos - line_start;

    let pad: String = std::iter::repeat(' ').take(col_in_line).collect();
    let under_ch = doc_text.get_char(pos).unwrap_or(' ');

    let cursor_y = 380.0 + (line_idx as f32 * font_size);

    // Draw block background at cursor cell
    renderer.draw_text(TextSection::simple(
      50.0,
      cursor_y,
      format!("{}{}", pad, '█'),
      font_size,
      cursor_bg,
    ));

    // Draw underlying character on top
    renderer.draw_text(TextSection::simple(
      50.0,
      cursor_y,
      format!("{}{}", pad, under_ch),
      font_size,
      cursor_fg,
    ));

    let status_y = renderer.height() as f32 - 30.0;
    renderer.draw_text(TextSection::simple(
      10.0,
      status_y,
      format!("Ready | Size: {}x{}", renderer.width(), renderer.height()),
      14.0,
      Color::rgb(0.6, 0.6, 0.7),
    ));
  }

  fn handle_event(&mut self, event: InputEvent, _renderer: &mut Renderer) -> bool {
    match event {
      InputEvent::Keyboard(key_press) => {
        if key_press.pressed {
          use the_editor_renderer::Key;
          // Dispatch renderer Key directly through keymap
          match self.keymaps.get(self.mode, key_press.code) {
            KeymapResult::Matched(cmd) => {
              match cmd {
                crate::keymap::Command::Execute(f) => f(&mut self.document),
                crate::keymap::Command::EnterInsertMode => self.mode = Mode::Insert,
                crate::keymap::Command::ExitInsertMode => self.mode = Mode::Normal,
              }
              true
            },
            KeymapResult::Pending(_) => true, // show pending UI later
            KeymapResult::Cancelled(_) => true,
            KeymapResult::NotFound => {
              // If in insert, Enter inserts newline as a convenience
              if self.mode == Mode::Insert && matches!(key_press.code, Key::Enter) {
                insert_text(&mut self.document, "\n");
                true
              } else {
                false
              }
            },
          }
        } else {
          false
        }
      },
      InputEvent::Mouse(mouse) => {
        println!("Mouse event at flinstons {:?}", mouse.position);
        false
      },
      InputEvent::Text(text) => {
        if self.mode == Mode::Insert {
          // Insert the incoming text at the current cursor(s)
          insert_text(&mut self.document, &text);
          true
        } else {
          // Treat as normal-mode keypresses (first char only)
          if let Some(ch) = text.chars().next() {
            use the_editor_renderer::Key;

            use crate::keymap::KeymapResult;
            match self.keymaps.get(self.mode, Key::Char(ch)) {
              KeymapResult::Matched(cmd) => {
                match cmd {
                  crate::keymap::Command::Execute(f) => f(&mut self.document),
                  crate::keymap::Command::EnterInsertMode => self.mode = Mode::Insert,
                  crate::keymap::Command::ExitInsertMode => self.mode = Mode::Normal,
                }
                true
              },
              KeymapResult::Pending(_) => true,
              KeymapResult::Cancelled(_) | KeymapResult::NotFound => false,
            }
          } else {
            false
          }
        }
      },
    }
  }

  fn resize(&mut self, width: u32, height: u32, _renderer: &mut Renderer) {
    println!("Window resized to {}x{}", width, height);
  }
}
