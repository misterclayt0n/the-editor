use std::collections::HashMap;

use crate::core::graphics::CursorKind;
use the_editor_renderer::{
  Color,
  TextSection,
};

/// A render command that can be batched
#[derive(Debug, Clone)]
pub enum RenderCommand {
  /// Draw a rectangle
  Rect {
    x:      f32,
    y:      f32,
    width:  f32,
    height: f32,
    color:  Color,
  },
  /// Draw text with specific attributes
  Text { section: TextSection },
  /// Draw cursor
  Cursor {
    x:      f32,
    y:      f32,
    width:  f32,
    height: f32,
    color:  Color,
    kind:   CursorKind,
    primary: bool,
  },
  /// Draw selection background
  Selection {
    x:      f32,
    y:      f32,
    width:  f32,
    height: f32,
    color:  Color,
  },
}

/// Groups render commands by type and state for batching
pub struct CommandBatcher {
  /// Pending commands grouped by render state
  rect_batch:      Vec<RectCommand>,
  text_batch:      HashMap<TextBatchKey, Vec<TextCommand>>,
  selection_batch: Vec<SelectionCommand>,
  cursor_batch:    Vec<CursorCommand>,
}

#[derive(Debug, Clone)]
struct RectCommand {
  x:      f32,
  y:      f32,
  width:  f32,
  height: f32,
  color:  Color,
}

#[derive(Debug, Clone)]
struct TextCommand {
  text: String,
  x:    f32,
  y:    f32,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct TextBatchKey {
  font_size: u32,     // Store as fixed point for hashing
  color:     [u8; 4], // Store as bytes for hashing
}

#[derive(Debug, Clone)]
struct SelectionCommand {
  x:      f32,
  y:      f32,
  width:  f32,
  height: f32,
  color:  Color,
}

#[derive(Debug, Clone)]
struct CursorCommand {
  x:      f32,
  y:      f32,
  width:  f32,
  height: f32,
  color:  Color,
  kind:   CursorKind,
  primary: bool,
}

impl CommandBatcher {
  pub fn new() -> Self {
    Self {
      rect_batch:      Vec::new(),
      text_batch:      HashMap::new(),
      selection_batch: Vec::new(),
      cursor_batch:    Vec::new(),
    }
  }

  /// Add a render command to the batcher
  pub fn add_command(&mut self, command: RenderCommand) {
    match command {
      RenderCommand::Rect {
        x,
        y,
        width,
        height,
        color,
      } => {
        self.rect_batch.push(RectCommand {
          x,
          y,
          width,
          height,
          color,
        });
      },
      RenderCommand::Text { section } => {
        // Group text by font size and color for batching
        let key = Self::text_key(&section);
        let commands = self.text_batch.entry(key).or_default();

        // Extract text and position from the section
        let text = section
          .texts
          .iter()
          .map(|seg| seg.content.as_str())
          .collect::<Vec<_>>()
          .join("");
        let (x, y) = section.position;

        commands.push(TextCommand { text, x, y });
      },
      RenderCommand::Selection {
        x,
        y,
        width,
        height,
        color,
      } => {
        self.selection_batch.push(SelectionCommand {
          x,
          y,
          width,
          height,
          color,
        });
      },
      RenderCommand::Cursor {
        x,
        y,
        width,
        height,
        color,
        kind,
        primary,
      } => {
        self.cursor_batch.push(CursorCommand {
          x,
          y,
          width,
          height,
          color,
          kind,
          primary,
        });
      },
    }
  }

  /// Execute all batched commands on the renderer
  pub fn execute(&mut self, renderer: &mut the_editor_renderer::Renderer) {
    // Draw selections first (background layer)
    for sel in &self.selection_batch {
      renderer.draw_rect(sel.x, sel.y, sel.width, sel.height, sel.color);
    }

    // Draw rectangles
    for rect in &self.rect_batch {
      renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, rect.color);
    }

    // Draw text batches
    for (key, commands) in &self.text_batch {
      // Reconstruct text attributes from key
      let font_size = key.font_size as f32 / 100.0;
      let color = Color::rgba(
        key.color[0] as f32 / 255.0,
        key.color[1] as f32 / 255.0,
        key.color[2] as f32 / 255.0,
        key.color[3] as f32 / 255.0,
      );

      // Render text - avoid batching for now due to position bug
      for cmd in commands {
        renderer.draw_text(TextSection::simple(
          cmd.x,
          cmd.y,
          cmd.text.clone(),
          font_size,
          color,
        ));
      }
    }

    // Draw cursors last (top layer)
    if !self.cursor_batch.is_empty() {
      let mut primary_cursor: Option<&CursorCommand> = None;
      for cursor in &self.cursor_batch {
        if cursor.primary {
          primary_cursor = Some(cursor);
        } else {
          Self::render_single_cursor(renderer, cursor);
        }
      }
      if let Some(cursor) = primary_cursor {
        Self::render_single_cursor(renderer, cursor);
      }
    }

    // Clear batches for next frame
    self.clear();
  }

  /// Clear all pending commands
  pub fn clear(&mut self) {
    self.rect_batch.clear();
    self.text_batch.clear();
    self.selection_batch.clear();
    self.cursor_batch.clear();
  }

  /// Check if there are any pending commands
  pub fn is_empty(&self) -> bool {
    self.rect_batch.is_empty()
      && self.text_batch.is_empty()
      && self.selection_batch.is_empty()
      && self.cursor_batch.is_empty()
  }

  fn render_single_cursor(renderer: &mut the_editor_renderer::Renderer, cursor: &CursorCommand) {
    const SECONDARY_CURSOR_OPACITY: f32 = 0.5;

    // Reduce opacity for secondary (non-primary) cursors
    let mut cursor_color = cursor.color;
    if !cursor.primary {
      cursor_color.a *= SECONDARY_CURSOR_OPACITY;
    }

    match cursor.kind {
      CursorKind::Block => {
        renderer.draw_rect(cursor.x, cursor.y, cursor.width, cursor.height, cursor_color);
      },
      CursorKind::Bar => {
        const BAR_WIDTH: f32 = 2.0;
        renderer.draw_rect(
          cursor.x,
          cursor.y,
          BAR_WIDTH.min(cursor.width),
          cursor.height,
          cursor_color,
        );
      },
      CursorKind::Underline => {
        const UNDERLINE_HEIGHT: f32 = 2.0;
        renderer.draw_rect(
          cursor.x,
          cursor.y + cursor.height - UNDERLINE_HEIGHT,
          cursor.width,
          UNDERLINE_HEIGHT.min(cursor.height),
          cursor_color,
        );
      },
      CursorKind::Hidden => {},
    }
  }

  fn text_key(section: &TextSection) -> TextBatchKey {
    // Assume first text segment for batching key (single-style sections)
    let first_segment = section.texts.first();
    if let Some(segment) = first_segment {
      TextBatchKey {
        font_size: (segment.style.size * 100.0) as u32,
        color:     [
          (segment.style.color.r * 255.0) as u8,
          (segment.style.color.g * 255.0) as u8,
          (segment.style.color.b * 255.0) as u8,
          (segment.style.color.a * 255.0) as u8,
        ],
      }
    } else {
      // Default key for empty sections
      TextBatchKey {
        font_size: 1600, // 16.0 * 100
        color:     [255, 255, 255, 255],
      }
    }
  }
}

// NOTE: We have no use for this now, but may come in handy later.

// /// Frame timing for batching rapid movements
// pub struct FrameTimer {
//   last_render:    std::time::Instant,
//   min_frame_time: std::time::Duration,
//   pending_render: bool,
// }

// impl FrameTimer {
//   pub fn new(target_fps: u32) -> Self {
//     Self {
//       last_render:    std::time::Instant::now(),
//       min_frame_time: std::time::Duration::from_millis(1000 / target_fps as
// u64),       pending_render: true, // Start with a pending render for initial
// frame     }
//   }

//   /// Check if we should render this frame
//   pub fn should_render(&mut self) -> bool {
//     let now = std::time::Instant::now();
//     let elapsed = now.duration_since(self.last_render);

//     if elapsed >= self.min_frame_time && self.pending_render {
//       self.last_render = now;
//       self.pending_render = false;
//       true
//     } else {
//       false
//     }
//   }

//   /// Mark that a render is needed
//   pub fn request_render(&mut self) {
//     self.pending_render = true;
//   }

//   /// Force immediate render (e.g., for important events)
//   pub fn force_render(&mut self) -> bool {
//     self.last_render = std::time::Instant::now();
//     let was_pending = self.pending_render;
//     self.pending_render = false;
//     was_pending
//   }
// }
