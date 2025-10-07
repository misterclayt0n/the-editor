use the_editor_lsp_types::types as lsp;
use the_editor_renderer::{
  Color,
  TextSection,
  TextSegment,
  TextStyle,
};

use crate::{
  core::graphics::Rect,
  ui::{
    UI_FONT_SIZE,
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
  },
};

/// Maximum width for the hover popup
const MAX_POPUP_WIDTH: f32 = 600.0;

/// Hover popup component showing LSP hover information
pub struct Hover {
  /// Hover contents: (server_name, hover_markdown)
  contents:      Vec<(String, String)>,
  /// Animation progress (0.0 = just appeared, 1.0 = fully visible)
  anim_progress: f32,
}

impl Hover {
  pub const ID: &'static str = "hover";

  /// Create a new hover popup from LSP hover responses
  pub fn new(hovers: Vec<(String, lsp::Hover)>) -> Self {
    let contents = hovers
      .into_iter()
      .map(|(server_name, hover)| {
        let markdown = hover_contents_to_string(hover.contents);
        (server_name, markdown)
      })
      .collect();

    Self {
      contents,
      anim_progress: 0.0,
    }
  }
}

impl Component for Hover {
  fn render(&mut self, _area: Rect, surface: &mut Surface, ctx: &mut Context) {
    if self.contents.is_empty() {
      return;
    }

    // Update animation progress
    const ANIM_SPEED: f32 = 30.0;
    if self.anim_progress < 1.0 {
      self.anim_progress = (self.anim_progress + ctx.dt * ANIM_SPEED).min(1.0);
    }

    // Smoothstep easing
    let t = self.anim_progress;
    let eased_t = t * t * (3.0 - 2.0 * t);

    // Animation effects
    let alpha = eased_t;
    let slide_offset = (1.0 - eased_t) * 8.0;
    let scale = 0.95 + (eased_t * 0.05);

    // Get theme colors
    let theme = &ctx.editor.theme;
    let bg_style = theme.get("ui.popup");
    let text_style = theme.get("ui.text");

    let mut bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 0.98));
    let mut text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));

    // Apply alpha
    bg_color.a *= alpha;
    text_color.a *= alpha;

    // Get hover content
    let (_server_name, markdown) = &self.contents[0];

    // Calculate popup dimensions
    let padding = 12.0;
    let line_height = UI_FONT_SIZE + 4.0;

    // Wrap text and calculate dimensions
    let lines = wrap_text(markdown, MAX_POPUP_WIDTH, surface.cell_width());
    let num_lines = lines.len().min(20); // Limit to 20 lines
    let max_line_width = lines
      .iter()
      .take(num_lines)
      .map(|l| l.len())
      .max()
      .unwrap_or(0) as f32
      * surface.cell_width();

    let popup_width = (max_line_width + padding * 2.0).max(200.0).min(MAX_POPUP_WIDTH);
    let popup_height = (num_lines as f32 * line_height) + (padding * 2.0);

    // Calculate cursor position
    let (cursor_x, cursor_y) = {
      let (view, doc) = crate::current_ref!(ctx.editor);
      let text = doc.text();
      let cursor_pos = doc.selection(view.id).primary().cursor(text.slice(..));

      // Convert char position to line/column
      let line = text.char_to_line(cursor_pos);
      let line_start = text.line_to_char(line);
      let col = cursor_pos - line_start;

      // Get view scroll offset
      let view_offset = doc.view_offset(view.id);
      let anchor_line = text.char_to_line(view_offset.anchor.min(text.len_chars()));

      // Calculate screen coordinates
      let font_size = ctx.editor.font_size_override.unwrap_or(ctx.editor.config().font_size);
      let font_width = surface.cell_width().max(1.0);
      let gutter_width = 6;
      let gutter_offset = gutter_width as f32 * font_width;

      const VIEW_PADDING_LEFT: f32 = 10.0;
      const VIEW_PADDING_TOP: f32 = 10.0;
      const LINE_SPACING: f32 = 2.0;

      let base_x = VIEW_PADDING_LEFT + gutter_offset;
      let base_y = VIEW_PADDING_TOP;

      let rel_row = line.saturating_sub(anchor_line);
      let x = base_x + (col as f32) * font_width;
      // Position BELOW the cursor line (+ 2 lines down)
      let y = base_y + (rel_row as f32) * (font_size + LINE_SPACING) + font_size * 2.0;

      (x, y)
    };

    // Apply animation transforms
    let anim_y = cursor_y + slide_offset; // Slide from below
    let anim_width = popup_width * scale;
    let anim_height = popup_height * scale;
    let anim_x = cursor_x - (popup_width - anim_width) / 2.0;

    // Draw background
    let corner_radius = 6.0;
    surface.draw_rounded_rect(anim_x, anim_y, anim_width, anim_height, corner_radius, bg_color);

    // Draw border
    let mut border_color = Color::new(0.3, 0.3, 0.35, 0.8);
    border_color.a *= alpha;
    surface.draw_rounded_rect_stroke(
      anim_x,
      anim_y,
      anim_width,
      anim_height,
      corner_radius,
      1.0,
      border_color,
    );

    // Render hover content
    surface.with_overlay_region(anim_x, anim_y, anim_width, anim_height, |surface| {
      let text_x = anim_x + padding;
      let mut text_y = anim_y + padding + UI_FONT_SIZE; // Add font size for baseline

      for line in lines.iter().take(num_lines) {
        let section = TextSection {
          position: (text_x, text_y),
          texts:    vec![TextSegment {
            content: line.clone(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          }],
        };

        surface.draw_text(section);
        text_y += line_height;
      }
    });
  }

  fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
    // Close on any key press and let the event pass through to the editor
    if let Event::Key(_) = event {
      return EventResult::Ignored(Some(Box::new(|compositor, _ctx| {
        compositor.remove(Self::ID);
      })));
    }

    EventResult::Ignored(None)
  }

  fn id(&self) -> Option<&'static str> {
    Some(Self::ID)
  }

  fn is_animating(&self) -> bool {
    self.anim_progress < 1.0
  }
}

/// Convert LSP HoverContents to a markdown string
fn hover_contents_to_string(contents: lsp::HoverContents) -> String {
  fn marked_string_to_markdown(contents: lsp::MarkedString) -> String {
    match contents {
      lsp::MarkedString::String(contents) => contents,
      lsp::MarkedString::LanguageString(string) => {
        if string.language == "markdown" {
          string.value
        } else {
          format!("```{}\n{}\n```", string.language, string.value)
        }
      },
    }
  }

  match contents {
    lsp::HoverContents::Scalar(contents) => marked_string_to_markdown(contents),
    lsp::HoverContents::Array(contents) => contents
      .into_iter()
      .map(marked_string_to_markdown)
      .collect::<Vec<_>>()
      .join("\n\n"),
    lsp::HoverContents::Markup(contents) => contents.value,
  }
}

/// Wrap text to fit within max_width
fn wrap_text(text: &str, max_width: f32, char_width: f32) -> Vec<String> {
  let max_chars = (max_width / char_width) as usize;
  let mut lines = Vec::new();

  for paragraph in text.lines() {
    if paragraph.is_empty() {
      lines.push(String::new());
      continue;
    }

    // Simple word wrapping
    let mut current_line = String::new();
    for word in paragraph.split_whitespace() {
      if current_line.is_empty() {
        current_line = word.to_string();
      } else if current_line.len() + word.len() + 1 <= max_chars {
        current_line.push(' ');
        current_line.push_str(word);
      } else {
        lines.push(current_line);
        current_line = word.to_string();
      }
    }
    if !current_line.is_empty() {
      lines.push(current_line);
    }
  }

  lines
}
