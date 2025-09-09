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
    selection::Selection,
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
    // Ensure the active view has an initial cursor/selection to avoid panics
    // in motion commands that expect a selection to exist.
    self.document.set_selection(0, Selection::point(0));
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
    let doc_text = self.document.text();
    let selection = self.document.selection_ref(0);
    
    let normal = Color::rgb(0.85, 0.85, 0.9);
    let cursor_fg = Color::rgb(0.1, 0.1, 0.15); // Dark text on bright block.
    let cursor_bg = Color::rgb(0.2, 0.8, 0.7); // Teal block.
    let selection_bg = Color::rgba(0.3, 0.5, 0.8, 0.3); // Semi-transparent blue.
    let font_size = 22.0;
    let base_x = 50.0;
    let base_y = 380.0;

    // Draw selection background if there is a selection.
    if let Some(sel) = &selection {
      for range in sel.ranges() {
        if !range.is_empty() {
          let start_pos = range.from();
          let end_pos = range.to();
          
          let start_line = doc_text.char_to_line(start_pos);
          let end_line = doc_text.char_to_line(end_pos);
          
          if start_line == end_line {
            // Single line selection
            let line_start = doc_text.line_to_char(start_line);
            let start_col = start_pos - line_start;
            let end_col = end_pos - line_start;
            
            let y = base_y + (start_line as f32 * font_size);
            let padding: String = std::iter::repeat(' ').take(start_col).collect();
            let selection_width = end_col - start_col;
            let selection_chars: String = std::iter::repeat('█').take(selection_width).collect();
            
            renderer.draw_text(TextSection::simple(
              base_x,
              y,
              format!("{}{}", padding, selection_chars),
              font_size,
              selection_bg,
            ));
          } else {
            // Multi-line selection.
            for line in start_line..=end_line {
              let y = base_y + (line as f32 * font_size);
              let line_start_char = doc_text.line_to_char(line);
              let line_len = doc_text.line(line).len_chars();
              
              let (start_col, end_col) = if line == start_line {
                // First line: from selection start to end of line.
                (start_pos - line_start_char, line_len)
              } else if line == end_line {
                // Last line: from beginning to selection end.
                (0, end_pos - line_start_char)
              } else {
                // Middle lines: entire line.
                (0, line_len)
              };
              
              if end_col > start_col {
                let padding: String = std::iter::repeat(' ').take(start_col).collect();
                let selection_width = end_col - start_col;
                let selection_chars: String = std::iter::repeat('█').take(selection_width).collect();
                
                renderer.draw_text(TextSection::simple(
                  base_x,
                  y,
                  format!("{}{}", padding, selection_chars),
                  font_size,
                  selection_bg,
                ));
              }
            }
          }
        }
      }
    }

    // Draw the text.
    let full = doc_text.to_string();
    renderer.draw_text(TextSection::simple(base_x, base_y, full, font_size, normal));

    // Draw cursor.
    let pos = if let Some(sel) = &selection {
      sel.primary().cursor(doc_text.slice(..))
    } else {
      0
    };

    // Calculate cursor position accounting for newlines.
    let line_idx = doc_text.char_to_line(pos);
    let line_start = doc_text.line_to_char(line_idx);
    let col_in_line = pos - line_start;

    let pad: String = std::iter::repeat(' ').take(col_in_line).collect();
    let under_ch = doc_text.get_char(pos).unwrap_or(' ');

    let cursor_y = base_y + (line_idx as f32 * font_size);

    // Draw block background at cursor cell.
    renderer.draw_text(TextSection::simple(
      base_x,
      cursor_y,
      format!("{}{}", pad, '█'),
      font_size,
      cursor_bg,
    ));

    // Draw underlying character on top.
    renderer.draw_text(TextSection::simple(
      base_x,
      cursor_y,
      format!("{}{}", pad, under_ch),
      font_size,
      cursor_fg,
    ));

    let status_y = renderer.height() as f32 - 30.0;
    let mode_str = match self.mode {
      Mode::Normal => "NORMAL",
      Mode::Insert => "INSERT",
      Mode::Visual => "VISUAL",
    };
    renderer.draw_text(TextSection::simple(
      10.0,
      status_y,
      format!("{} | Size: {}x{}", mode_str, renderer.width(), renderer.height()),
      14.0,
      Color::rgb(0.6, 0.6, 0.7),
    ));
  }

  fn handle_event(&mut self, event: InputEvent, _renderer: &mut Renderer) -> bool {
    match event {
      InputEvent::Keyboard(key_press) => {
        if key_press.pressed {
          use the_editor_renderer::Key;
          // Dispatch renderer Key directly through keymap.
          match self.keymaps.get(self.mode, key_press.code) {
            KeymapResult::Matched(cmd) => {
              match cmd {
                crate::keymap::Command::Execute(f) => f(&mut self.document),
                crate::keymap::Command::EnterInsertMode => self.mode = Mode::Insert,
                crate::keymap::Command::ExitInsertMode => self.mode = Mode::Normal,
                crate::keymap::Command::EnterVisualMode => self.mode = Mode::Visual,
                crate::keymap::Command::ExitVisualMode => self.mode = Mode::Normal,
              }
              true
            },
            KeymapResult::Pending(_) => true, // Show pending UI later.
            KeymapResult::Cancelled(_) => true,
            KeymapResult::NotFound => {
              // If in insert, Enter inserts newline as a convenience.
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
          // Insert the incoming text at the current cursor(s).
          insert_text(&mut self.document, &text);
          true
        } else {
          // Treat as normal-mode keypresses (first char only).
          if let Some(ch) = text.chars().next() {
            use the_editor_renderer::Key;

            use crate::keymap::KeymapResult;
            match self.keymaps.get(self.mode, Key::Char(ch)) {
              KeymapResult::Matched(cmd) => {
                match cmd {
                  crate::keymap::Command::Execute(f) => f(&mut self.document),
                  crate::keymap::Command::EnterInsertMode => self.mode = Mode::Insert,
                  crate::keymap::Command::ExitInsertMode => self.mode = Mode::Normal,
                  crate::keymap::Command::EnterVisualMode => self.mode = Mode::Visual,
                  crate::keymap::Command::ExitVisualMode => self.mode = Mode::Normal,
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
