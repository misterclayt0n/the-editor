//! Shared markdown and code block rendering utilities.
//!
//! This module provides common functions for rendering markdown content,
//! including syntax-highlighted code blocks, used by hover, signature help,
//! and ACP overlay components.

use ropey::Rope;
use the_editor_renderer::{
  Color,
  TextSegment,
  TextStyle,
};

use crate::ui::{
  UI_FONT_SIZE,
  compositor::Context,
};

/// Highlight a code block with syntax highlighting.
///
/// This function properly handles:
/// - Char-based indexing (not byte-based)
/// - Sorted and non-overlapping spans
/// - Preservation of whitespace including tabs
/// - Line truncation for display
pub fn highlight_code_block(
  lang_hint: Option<&str>,
  code: &str,
  max_chars: usize,
  ctx: &mut Context,
) -> Vec<Vec<TextSegment>> {
  let theme = &ctx.editor.theme;

  let default_code_color = theme
    .get("markup.raw")
    .fg
    .map(crate::ui::theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.8, 0.8, 0.8, 1.0));

  let rope = Rope::from(code);
  let slice = rope.slice(..);

  let loader = ctx.editor.syn_loader.load();
  let language = lang_hint
    .and_then(|name| loader.language_for_name(name.to_string()))
    .or_else(|| loader.language_for_match(slice));

  let spans = language
    .and_then(|lang| crate::core::syntax::Syntax::new(slice, lang, &loader).ok())
    .map(|syntax| syntax.collect_highlights(slice, &loader, 0..slice.len_bytes()))
    .unwrap_or_default();

  // Convert byte ranges to char ranges and sort by start position
  let mut char_spans: Vec<(usize, usize, Color)> = Vec::with_capacity(spans.len());
  for (hl, byte_range) in spans.into_iter() {
    let style = theme.highlight(hl);
    let color = style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(default_code_color);
    // Convert byte indices to char indices
    let start_char = slice.byte_to_char(byte_range.start.min(slice.len_bytes()));
    let end_char = slice.byte_to_char(byte_range.end.min(slice.len_bytes()));
    if start_char < end_char {
      char_spans.push((start_char, end_char, color));
    }
  }
  char_spans.sort_by_key(|(s, ..)| *s);

  let mut result = Vec::new();
  let total_lines = rope.len_lines();

  for line_idx in 0..total_lines {
    let line_start_char = rope.line_to_char(line_idx);
    let line_slice = rope.line(line_idx);
    let mut line_string = line_slice.to_string();
    // Remove trailing newline if present
    if line_string.ends_with('\n') {
      line_string.pop();
    }
    let line_char_count = line_string.chars().count();
    let line_end_char = line_start_char + line_char_count;

    // Truncate long lines for display
    let display_line = if line_char_count > max_chars {
      let truncated: String = line_string.chars().take(max_chars - 1).collect();
      format!("{}…", truncated)
    } else {
      line_string.clone()
    };

    // Build segments for this line
    let mut segments: Vec<TextSegment> = Vec::new();
    let mut current_char = 0usize;

    for &(span_start, span_end, color) in &char_spans {
      // Skip spans that end before this line or start after
      if span_end <= line_start_char || span_start >= line_end_char {
        continue;
      }

      // Clamp span to line boundaries (in line-relative char indices)
      let rel_start = span_start.saturating_sub(line_start_char);
      let rel_end = (span_end - line_start_char).min(line_char_count);

      // Skip if this span starts before where we are (overlapping spans)
      if rel_start < current_char {
        continue;
      }

      // Add unhighlighted text before this span
      if rel_start > current_char {
        let text: String = line_string
          .chars()
          .skip(current_char)
          .take(rel_start - current_char)
          .collect();
        if !text.is_empty() {
          segments.push(TextSegment {
            content: text,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: default_code_color,
            },
          });
        }
      }

      // Add highlighted text
      if rel_end > rel_start {
        let text: String = line_string
          .chars()
          .skip(rel_start)
          .take(rel_end - rel_start)
          .collect();
        if !text.is_empty() {
          segments.push(TextSegment {
            content: text,
            style:   TextStyle {
              size: UI_FONT_SIZE,
              color,
            },
          });
        }
        current_char = rel_end;
      }
    }

    // Add remaining unhighlighted text
    if current_char < line_char_count {
      let text: String = line_string.chars().skip(current_char).collect();
      if !text.is_empty() {
        segments.push(TextSegment {
          content: text,
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: default_code_color,
          },
        });
      }
    }

    // If no segments, add the whole line with default color
    if segments.is_empty() {
      segments.push(TextSegment {
        content: display_line,
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: default_code_color,
        },
      });
    }

    result.push(segments);
  }

  result
}

/// Wrap text to fit within max_chars, preserving intentional structure.
///
/// Unlike simple word wrapping, this preserves leading whitespace and
/// only wraps lines that exceed the width.
pub fn wrap_text_preserve_breaks(text: &str, max_chars: usize) -> Vec<String> {
  if max_chars == 0 {
    return vec![String::new()];
  }

  let text = text.trim_end();
  if text.is_empty() {
    return vec![];
  }

  // If the line fits, return it as-is
  if text.chars().count() <= max_chars {
    return vec![text.to_string()];
  }

  // Need to wrap - use word boundaries
  let mut lines = Vec::new();
  let mut current = String::new();

  for word in text.split_whitespace() {
    let word_len = word.chars().count();

    // Handle very long words
    if word_len > max_chars {
      if !current.is_empty() {
        lines.push(std::mem::take(&mut current));
      }
      // Break long word into chunks
      let mut chunk = String::new();
      for ch in word.chars() {
        if chunk.chars().count() >= max_chars {
          lines.push(std::mem::take(&mut chunk));
        }
        chunk.push(ch);
      }
      if !chunk.is_empty() {
        current = chunk;
      }
      continue;
    }

    let current_len = current.chars().count();
    let needed = if current.is_empty() {
      word_len
    } else {
      word_len + 1
    };

    if current_len + needed > max_chars && !current.is_empty() {
      lines.push(std::mem::take(&mut current));
    }

    if !current.is_empty() {
      current.push(' ');
    }
    current.push_str(word);
  }

  if !current.is_empty() {
    lines.push(current);
  }

  lines
}

/// Build render lines from markdown text.
///
/// Handles:
/// - Fenced code blocks with syntax highlighting
/// - Paragraph breaks (blank lines)
/// - Proper text wrapping
pub fn build_markdown_lines(
  markdown: &str,
  wrap_width: f32,
  cell_width: f32,
  ctx: &mut Context,
) -> Vec<Vec<TextSegment>> {
  let theme = &ctx.editor.theme;
  let text_style = theme.get("ui.text");
  let base_text_color = text_style
    .fg
    .map(crate::ui::theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));

  let max_chars = (wrap_width / cell_width).floor().max(4.0) as usize;

  let mut render_lines: Vec<Vec<TextSegment>> = Vec::new();
  let mut in_fence = false;
  let mut fence_lang: Option<String> = None;
  let mut fence_buf: Vec<String> = Vec::new();
  let mut prev_was_empty = false;

  for raw_line in markdown.lines() {
    let is_empty = raw_line.trim().is_empty();

    // Handle fenced code blocks
    if raw_line.starts_with("```") {
      if in_fence {
        // End of code block
        let code = fence_buf.join("\n");
        render_lines.extend(highlight_code_block(
          fence_lang.as_deref(),
          &code,
          max_chars,
          ctx,
        ));
        in_fence = false;
        fence_lang = None;
        fence_buf.clear();
      } else {
        // Start of code block
        in_fence = true;
        let lang = raw_line.trim_start_matches('`').trim();
        fence_lang = if lang.is_empty() {
          None
        } else {
          Some(lang.to_string())
        };
      }
      prev_was_empty = false;
      continue;
    }

    // Inside a code block - preserve everything including blank lines
    if in_fence {
      fence_buf.push(raw_line.to_string());
      continue;
    }

    // Handle blank lines as paragraph separators
    if is_empty {
      // Only add one blank line for paragraph break (collapse multiple)
      if !prev_was_empty && !render_lines.is_empty() {
        render_lines.push(vec![TextSegment {
          content: String::new(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: base_text_color,
          },
        }]);
      }
      prev_was_empty = true;
      continue;
    }

    prev_was_empty = false;

    // Regular text - wrap it properly
    let wrapped = wrap_text_preserve_breaks(raw_line, max_chars);
    for line in wrapped {
      render_lines.push(vec![TextSegment {
        content: line,
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: base_text_color,
        },
      }]);
    }
  }

  // Handle unclosed fence
  if in_fence {
    let code = fence_buf.join("\n");
    render_lines.extend(highlight_code_block(
      fence_lang.as_deref(),
      &code,
      max_chars,
      ctx,
    ));
  }

  render_lines
}

/// Estimate the rendered width of a line of text segments.
pub fn estimate_line_width(segments: &[TextSegment], cell_width: f32) -> f32 {
  segments
    .iter()
    .map(|seg| seg.content.chars().count() as f32 * cell_width)
    .sum()
}

/// Calculate scroll lines from a scroll delta.
pub fn scroll_lines_from_delta(delta: &the_editor_renderer::ScrollDelta) -> i32 {
  use the_editor_renderer::ScrollDelta;

  match delta {
    ScrollDelta::Lines { y, .. } => {
      if *y < 0.0 {
        (*y).floor() as i32
      } else {
        (*y).ceil() as i32
      }
    },
    ScrollDelta::Pixels { y, .. } => {
      let line_height = UI_FONT_SIZE + 4.0;
      (*y / line_height).round() as i32
    },
  }
}
