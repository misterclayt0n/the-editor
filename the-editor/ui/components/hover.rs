use the_editor_lsp_types::types as lsp;
use the_editor_renderer::{
  Color,
  TextSection,
  TextSegment,
  TextStyle,
};
use ropey::Rope;
use the_editor_stdx::rope::RopeSliceExt;

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
  contents:  Vec<(String, String)>,
  /// Appearance animation
  animation: crate::core::animation::AnimationHandle<f32>,
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

    // Create appearance animation using popup preset
    let (duration, easing) = crate::core::animation::presets::POPUP;
    let animation = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);

    Self {
      contents,
      animation,
    }
  }
}

impl Component for Hover {
  fn render(&mut self, _area: Rect, surface: &mut Surface, ctx: &mut Context) {
    if self.contents.is_empty() {
      return;
    }

    // Update animation with declarative system
    self.animation.update(ctx.dt);
    let eased_t = *self.animation.current();

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
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));
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

    // Parse markdown and build renderable lines (with syntax highlighted code blocks)
    let render_lines = build_hover_render_lines(markdown, surface.cell_width(), &ctx);

    let num_lines = render_lines.len().min(20); // Limit to 20 lines
    let max_line_width = render_lines
      .iter()
      .take(num_lines)
      .map(|segments| segments.iter().map(|seg| seg.content.chars().count()).sum::<usize>())
      .max()
      .unwrap_or(0) as f32
      * surface.cell_width();

    let popup_width = (max_line_width + padding * 2.0)
      .max(200.0)
      .min(MAX_POPUP_WIDTH);
    let popup_height = (num_lines as f32 * line_height) + (padding * 2.0);

    // Calculate fresh cursor position (not cached) with correct split offset
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

      // Calculate screen row/col accounting for scroll
      let rel_row = line.saturating_sub(anchor_line);
      let screen_col = col.saturating_sub(view_offset.horizontal_offset);

      // Check if cursor is visible
      if rel_row >= view.inner_height() {
        // Cursor scrolled out of view vertically
        return;
      }

      // Get font metrics
      let font_size = ctx
        .editor
        .font_size_override
        .unwrap_or(ctx.editor.config().font_size);
      let font_width = surface.cell_width().max(1.0);
      const LINE_SPACING: f32 = 4.0;
      let line_height = font_size + LINE_SPACING;

      // Get view's screen offset (handles splits correctly)
      let inner = view.inner_area(doc);
      let view_x = inner.x as f32 * font_width;
      let view_y = inner.y as f32 * line_height;

      // Calculate final screen position
      let x = view_x + (screen_col as f32 * font_width);
      // Position BELOW the cursor line (+ 2 lines down)
      let y = view_y + (rel_row as f32 * line_height) + line_height * 2.0;

      (x, y)
    };

    // Get viewport dimensions for bounds checking
    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    // Apply animation transforms
    let anim_width = popup_width * scale;
    let anim_height = popup_height * scale;

    // Try to position below cursor first
    let mut popup_y = cursor_y + slide_offset;

    // Check if popup would overflow bottom of viewport
    if popup_y + anim_height > viewport_height {
      // Try positioning above cursor instead
      let y_above = cursor_y - anim_height - slide_offset;
      if y_above >= 0.0 {
        popup_y = y_above;
      } else {
        // Not enough space above or below, clamp to viewport
        popup_y = popup_y.max(0.0).min(viewport_height - anim_height);
      }
    }

    // Center the scaled popup at the cursor position, then clamp to viewport
    let mut popup_x = cursor_x - (popup_width - anim_width) / 2.0;

    // Clamp X to viewport bounds
    popup_x = popup_x.max(0.0).min(viewport_width - anim_width);

    let anim_x = popup_x;
    let anim_y = popup_y;

    // Draw background
    let corner_radius = 6.0;
    surface.draw_rounded_rect(
      anim_x,
      anim_y,
      anim_width,
      anim_height,
      corner_radius,
      bg_color,
    );

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

      for segments in render_lines.iter().take(num_lines) {
        let section = TextSection {
          position: (text_x, text_y),
          texts:    segments.clone(),
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
    !self.animation.is_complete()
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
    lsp::HoverContents::Array(contents) => {
      contents
        .into_iter()
        .map(marked_string_to_markdown)
        .collect::<Vec<_>>()
        .join("\n\n")
    },
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

/// Build hover render lines with basic markdown handling and syntax-highlighted code blocks.
/// Returns a vector where each item is a line represented by pre-styled text segments.
fn build_hover_render_lines(markdown: &str, cell_width: f32, ctx: &Context) -> Vec<Vec<TextSegment>> {
  // Theme-derived base text color
  let theme = &ctx.editor.theme;
  let text_style = theme.get("ui.text");
  let base_text_color = text_style
    .fg
    .map(crate::ui::theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));

  // Collect lines of segments
  let mut render_lines: Vec<Vec<TextSegment>> = Vec::new();

  // Simple fenced code block parser
  let mut in_fence = false;
  let mut fence_lang: Option<String> = None;
  let mut fence_buf: Vec<String> = Vec::new();

  for raw_line in markdown.lines() {
    // Handle fence start/end
    if raw_line.starts_with("```") {
      if in_fence {
        // End of fence: highlight accumulated code
        let code = fence_buf.join("\n");
        render_lines.extend(highlight_code_block_lines(
          fence_lang.as_deref(),
          &code,
          ctx,
        ));
        // Add an empty spacer line after code block
        render_lines.push(vec![TextSegment {
          content: String::new(),
          style:   TextStyle { size: UI_FONT_SIZE, color: base_text_color },
        }]);
        // Reset state
        in_fence = false;
        fence_lang = None;
        fence_buf.clear();
      } else {
        // Start of fence
        in_fence = true;
        let lang = raw_line.trim_start_matches("```").trim();
        fence_lang = if lang.is_empty() { None } else { Some(lang.to_string()) };
      }
      continue;
    }

    if in_fence {
      fence_buf.push(raw_line.to_string());
      continue;
    }

    // Plain text: word-wrap into multiple lines
    for line in wrap_text(raw_line, MAX_POPUP_WIDTH, cell_width) {
      render_lines.push(vec![TextSegment {
        content: line,
        style:   TextStyle { size: UI_FONT_SIZE, color: base_text_color },
      }]);
    }
  }

  // If EOF reached while inside a fence, flush as code
  if in_fence {
    let code = fence_buf.join("\n");
    render_lines.extend(highlight_code_block_lines(
      fence_lang.as_deref(),
      &code,
      ctx,
    ));
  }

  render_lines
}

/// Highlight a code block into per-line segments. Falls back to plain code style on failure.
fn highlight_code_block_lines(lang_hint: Option<&str>, code: &str, ctx: &Context) -> Vec<Vec<TextSegment>> {
  let theme = &ctx.editor.theme;
  let code_style = theme.get("markup.raw");
  let default_code_color = code_style
    .fg
    .map(crate::ui::theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.8, 0.8, 0.8, 1.0));

  // Prepare rope
  let rope = Rope::from(code);
  let slice = rope.slice(..);

  // Resolve language from hint
  let loader = ctx.editor.syn_loader.load();
  let language = lang_hint
    .and_then(|name| loader.language_for_name(name.to_string()))
    .or_else(|| loader.language_for_match(slice));

  // Attempt to create syntax and collect highlights
  let spans = language
    .and_then(|lang| crate::core::syntax::Syntax::new(slice, lang, &loader).ok())
    .map(|syntax| syntax.collect_highlights(slice, &loader, 0..slice.len_bytes()))
    .unwrap_or_else(Vec::new);

  // Convert highlight spans to per-line colored segments
  let mut lines: Vec<Vec<TextSegment>> = Vec::new();
  let total_lines = rope.len_lines();

  // Precompute spans in char indices with resolved colors
  let mut char_spans: Vec<(usize, usize, Color)> = Vec::with_capacity(spans.len());
  for (hl, byte_range) in spans.into_iter() {
    let style = theme.highlight(hl);
    let color = style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(default_code_color);
    let start_char = slice.byte_to_char(slice.floor_char_boundary(byte_range.start));
    let end_char = slice.byte_to_char(slice.ceil_char_boundary(byte_range.end));
    if start_char < end_char {
      char_spans.push((start_char, end_char, color));
    }
  }
  char_spans.sort_by_key(|(s, _e, _)| *s);

  for line_idx in 0..total_lines {
    let line_slice = rope.line(line_idx);
    let mut line_string = line_slice.to_string();
    if line_string.ends_with('\n') {
      line_string.pop(); // remove trailing newline for rendering
    }
    let line_start_char = rope.line_to_char(line_idx);
    let line_end_char = line_start_char + line_string.chars().count();

    // Collect spans that intersect this line
    let mut segments: Vec<TextSegment> = Vec::new();
    let mut cursor = line_start_char;

    for (s, e, color) in char_spans.iter().cloned() {
      if e <= line_start_char || s >= line_end_char {
        continue;
      }
      let seg_start = s.max(line_start_char);
      let seg_end = e.min(line_end_char);

      // Push preceding plain segment if any
      if seg_start > cursor {
        let prefix = slice_chars_to_string(&line_string, cursor - line_start_char, seg_start - line_start_char);
        if !prefix.is_empty() {
          segments.push(TextSegment {
            content: prefix,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: default_code_color,
            },
          });
        }
      }

      // Push highlighted segment
      let content = slice_chars_to_string(&line_string, seg_start - line_start_char, seg_end - line_start_char);
      if !content.is_empty() {
        segments.push(TextSegment {
          content,
          style: TextStyle { size: UI_FONT_SIZE, color },
        });
      }
      cursor = seg_end;
    }

    // Trailing plain segment
    if cursor < line_end_char {
      let tail = slice_chars_to_string(&line_string, cursor - line_start_char, line_end_char - line_start_char);
      if !tail.is_empty() {
        segments.push(TextSegment {
          content: tail,
          style:   TextStyle { size: UI_FONT_SIZE, color: default_code_color },
        });
      }
    }

    // If no segments were produced (e.g. no spans), push the whole line as plain code
    if segments.is_empty() {
      segments.push(TextSegment {
        content: line_string,
        style:   TextStyle { size: UI_FONT_SIZE, color: default_code_color },
      });
    }

    lines.push(segments);
  }

  lines
}

/// Helper: slice a string by character indices [start, end) and return owned String
fn slice_chars_to_string(s: &str, start: usize, end: usize) -> String {
  if start >= end || start >= s.chars().count() {
    return String::new();
  }
  let mut buf = String::with_capacity(end.saturating_sub(start));
  for (i, ch) in s.chars().enumerate() {
    if i >= end { break; }
    if i >= start { buf.push(ch); }
  }
  buf
}
