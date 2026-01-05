//! Markdown rendering with full pulldown-cmark support.
//!
//! This module provides a `Markdown` struct that parses and renders markdown
//! content with syntax-highlighted code blocks, styled headers, lists,
//! emphasis, and more. Used by completion, hover, signature help, and other
//! documentation popups.

use std::sync::Arc;

use arc_swap::ArcSwap;
use pulldown_cmark::{
  CodeBlockKind,
  Event,
  HeadingLevel,
  Options,
  Parser,
  Tag,
  TagEnd,
};
use ropey::Rope;
use the_editor_renderer::{
  Color,
  TextSegment,
  TextStyle,
};

use crate::{
  core::{
    syntax::Loader,
    theme::Theme,
  },
  ui::{
    UI_FONT_SIZE,
    compositor::Context,
  },
};

// ============================================================================
// Theme style keys (matching Helix conventions)
// ============================================================================

const TEXT_STYLE: &str = "ui.text";
const BLOCK_STYLE: &str = "markup.raw.inline";
const RULE_STYLE: &str = "punctuation.special";
const UNNUMBERED_LIST_STYLE: &str = "markup.list.unnumbered";
const NUMBERED_LIST_STYLE: &str = "markup.list.numbered";
const HEADING_STYLES: [&str; 6] = [
  "markup.heading.1",
  "markup.heading.2",
  "markup.heading.3",
  "markup.heading.4",
  "markup.heading.5",
  "markup.heading.6",
];
const INDENT: &str = "  ";

// ============================================================================
// Markdown struct
// ============================================================================

/// A parsed markdown document ready for rendering.
///
/// This struct holds the raw markdown content and a reference to the syntax
/// loader for highlighting code blocks. Call `parse()` to convert to renderable
/// lines.
pub struct Markdown {
  contents:      String,
  config_loader: Arc<ArcSwap<Loader>>,
}

impl Markdown {
  /// Create a new Markdown instance from raw markdown content.
  pub fn new(contents: String, config_loader: Arc<ArcSwap<Loader>>) -> Self {
    Self {
      contents,
      config_loader,
    }
  }

  /// Parse the markdown content into styled text segments.
  ///
  /// Each inner `Vec<TextSegment>` represents one line of output.
  /// Pass `Some(theme)` for styled output, or `None` for plain text.
  pub fn parse(&self, theme: Option<&Theme>) -> Vec<Vec<TextSegment>> {
    let get_color = |key: &str| -> Color {
      theme
        .and_then(|t| t.get(key).fg)
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0))
    };

    let text_color = get_color(TEXT_STYLE);
    let code_color = get_color(BLOCK_STYLE);
    let rule_color = get_color(RULE_STYLE);
    let unnumbered_list_color = get_color(UNNUMBERED_LIST_STYLE);
    let numbered_list_color = get_color(NUMBERED_LIST_STYLE);
    let heading_colors: Vec<Color> = HEADING_STYLES.iter().map(|k| get_color(k)).collect();

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&self.contents, options);

    let mut tags: Vec<Tag<'_>> = Vec::new();
    let mut current_line: Vec<TextSegment> = Vec::new();
    let mut lines: Vec<Vec<TextSegment>> = Vec::new();
    let mut list_stack: Vec<Option<u64>> = Vec::new();

    let get_indent = |level: usize| -> String {
      if level < 1 {
        String::new()
      } else {
        INDENT.repeat(level - 1)
      }
    };

    // Transform <code>...</code> HTML into Code events
    let mut in_html_code = false;
    let parser = parser.filter_map(|event| {
      match event {
        Event::Html(tag)
          if tag.starts_with("<code") && matches!(tag.chars().nth(5), Some(' ' | '>')) =>
        {
          in_html_code = true;
          None
        },
        Event::Html(tag) if *tag == *"</code>" => {
          in_html_code = false;
          None
        },
        Event::Text(text) if in_html_code => Some(Event::Code(text)),
        _ => Some(event),
      }
    });

    for event in parser {
      match event {
        Event::Start(Tag::List(list)) => {
          // If nested list, push current line first
          if !list_stack.is_empty() {
            push_line(&mut current_line, &mut lines);
          }
          list_stack.push(list);
        },
        Event::End(TagEnd::List(_)) => {
          list_stack.pop();
          // Add empty line after top-level list
          if list_stack.is_empty() {
            lines.push(vec![]);
          }
        },
        Event::Start(Tag::Item) => {
          tags.push(Tag::Item);

          // Get bullet style based on list type
          let (bullet, bullet_color) = list_stack
            .last()
            .unwrap_or(&None)
            .map_or(("• ".to_string(), unnumbered_list_color), |number| {
              (format!("{}. ", number), numbered_list_color)
            });

          // Increment list number if numbered
          if let Some(Some(v)) = list_stack.last_mut() {
            *v += 1;
          }

          let prefix = get_indent(list_stack.len()) + bullet.as_str();
          current_line.push(TextSegment {
            content: prefix,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: bullet_color,
            },
          });
        },
        Event::Start(tag) => {
          tags.push(tag);
          // Add indent for nested list content
          if current_line.is_empty() && !list_stack.is_empty() {
            current_line.push(TextSegment {
              content: get_indent(list_stack.len()),
              style:   TextStyle {
                size:  UI_FONT_SIZE,
                color: text_color,
              },
            });
          }
        },
        Event::End(tag) => {
          tags.pop();
          match tag {
            TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::CodeBlock | TagEnd::Item => {
              push_line(&mut current_line, &mut lines);
            },
            _ => (),
          }
          // Add empty line after headings, paragraphs, code blocks
          match tag {
            TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::CodeBlock => {
              lines.push(vec![]);
            },
            _ => (),
          }
        },
        Event::Text(text) => {
          if let Some(Tag::CodeBlock(kind)) = tags.last() {
            // Syntax-highlighted code block
            let language = match kind {
              CodeBlockKind::Fenced(lang) => lang.as_ref(),
              CodeBlockKind::Indented => "",
            };
            let code_lines =
              highlight_code_block_internal(&text, language, theme, &self.config_loader.load());
            lines.extend(code_lines);
          } else {
            // Regular text with appropriate styling
            let color = match tags.last() {
              Some(Tag::Heading { level, .. }) => {
                match level {
                  HeadingLevel::H1 => heading_colors[0],
                  HeadingLevel::H2 => heading_colors[1],
                  HeadingLevel::H3 => heading_colors[2],
                  HeadingLevel::H4 => heading_colors[3],
                  HeadingLevel::H5 => heading_colors[4],
                  HeadingLevel::H6 => heading_colors[5],
                }
              },
              // Note: We can't do italic/bold/strikethrough with just Color,
              // so we use distinct colors as visual hints
              Some(Tag::Emphasis) => {
                // Slightly dimmer for italic feel
                Color::new(
                  text_color.r * 0.9,
                  text_color.g * 0.9,
                  text_color.b * 1.0,
                  text_color.a,
                )
              },
              Some(Tag::Strong) => {
                // Slightly brighter for bold feel
                Color::new(
                  (text_color.r * 1.1).min(1.0),
                  (text_color.g * 1.1).min(1.0),
                  (text_color.b * 1.1).min(1.0),
                  text_color.a,
                )
              },
              Some(Tag::Strikethrough) => {
                // Dimmer for strikethrough
                Color::new(
                  text_color.r * 0.6,
                  text_color.g * 0.6,
                  text_color.b * 0.6,
                  text_color.a,
                )
              },
              _ => text_color,
            };

            current_line.push(TextSegment {
              content: text.to_string(),
              style:   TextStyle {
                size: UI_FONT_SIZE,
                color,
              },
            });
          }
        },
        Event::Code(text) | Event::Html(text) => {
          // Inline code
          current_line.push(TextSegment {
            content: text.to_string(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: code_color,
            },
          });
        },
        Event::SoftBreak | Event::HardBreak => {
          push_line(&mut current_line, &mut lines);
          // Add indent for list continuation
          if !list_stack.is_empty() {
            current_line.push(TextSegment {
              content: get_indent(list_stack.len()),
              style:   TextStyle {
                size:  UI_FONT_SIZE,
                color: text_color,
              },
            });
          }
        },
        Event::Rule => {
          lines.push(vec![TextSegment {
            content: "───".to_string(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: rule_color,
            },
          }]);
          lines.push(vec![]);
        },
        _ => {
          log::warn!("unhandled markdown event {:?}", event);
        },
      }
    }

    // Don't forget the last line
    if !current_line.is_empty() {
      lines.push(current_line);
    }

    // Remove trailing empty line if present
    if let Some(last) = lines.last() {
      if last.is_empty() {
        lines.pop();
      }
    }

    lines
  }

  /// Calculate the required size for rendering this markdown content.
  ///
  /// Returns `(width, height)` in display cells, accounting for wrapping at
  /// `max_width`.
  pub fn required_size(&self, max_width: usize) -> (usize, usize) {
    let lines = self.parse(None);
    let (w, h) = super::text_wrap::required_size(&lines, max_width as u16);
    (w as usize, h as usize)
  }
}

/// Helper to push current line segments to lines vec
fn push_line(current: &mut Vec<TextSegment>, lines: &mut Vec<Vec<TextSegment>>) {
  let segments = std::mem::take(current);
  if !segments.is_empty() {
    lines.push(segments);
  }
}

// ============================================================================
// Code block highlighting (internal)
// ============================================================================

/// Highlight a code block with syntax highlighting.
fn highlight_code_block_internal(
  code: &str,
  language: &str,
  theme: Option<&Theme>,
  loader: &Loader,
) -> Vec<Vec<TextSegment>> {
  let default_code_color = theme
    .and_then(|t| t.get(BLOCK_STYLE).fg)
    .map(crate::ui::theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.8, 0.8, 0.8, 1.0));

  let rope = Rope::from(code);
  let slice = rope.slice(..);

  // Try to get language config and create syntax highlighter
  let lang_config = if language.is_empty() {
    loader.language_for_match(slice)
  } else {
    loader.language_for_name(language.to_string())
  };

  let spans = lang_config
    .and_then(|lang| crate::core::syntax::Syntax::new(slice, lang, loader).ok())
    .map(|syntax| syntax.collect_highlights(slice, loader, 0..slice.len_bytes()))
    .unwrap_or_default();

  // Convert byte ranges to char ranges
  let mut char_spans: Vec<(usize, usize, Color)> = Vec::with_capacity(spans.len());
  for (hl, byte_range) in spans.into_iter() {
    let color = theme
      .map(|t| {
        t.highlight(hl)
          .fg
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(default_code_color)
      })
      .unwrap_or(default_code_color);

    let start_char = slice.byte_to_char(byte_range.start.min(slice.len_bytes()));
    let end_char = slice.byte_to_char(byte_range.end.min(slice.len_bytes()));
    if start_char < end_char {
      char_spans.push((start_char, end_char, color));
    }
  }
  char_spans.sort_by_key(|(s, ..)| *s);

  // Build output lines
  let mut result = Vec::new();
  let total_lines = rope.len_lines();

  for line_idx in 0..total_lines {
    let line_start_char = rope.line_to_char(line_idx);
    let line_slice = rope.line(line_idx);
    let mut line_string = line_slice.to_string();

    // Remove trailing newline
    if line_string.ends_with('\n') {
      line_string.pop();
    }
    // Replace tabs with spaces
    line_string = line_string.replace('\t', "    ");

    let line_char_count = line_string.chars().count();
    let line_end_char = line_start_char + line_char_count;

    let mut segments: Vec<TextSegment> = Vec::new();
    let mut current_char = 0usize;

    for &(span_start, span_end, color) in &char_spans {
      if span_end <= line_start_char || span_start >= line_end_char {
        continue;
      }

      let rel_start = span_start.saturating_sub(line_start_char);
      let rel_end = (span_end - line_start_char).min(line_char_count);

      if rel_start < current_char {
        continue;
      }

      // Add unhighlighted text before span
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

    // Add remaining text
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

    // Empty line gets empty segment
    if segments.is_empty() && !line_string.is_empty() {
      segments.push(TextSegment {
        content: line_string,
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

// ============================================================================
// Public helper functions
// ============================================================================

/// Highlight a code block with syntax highlighting.
///
/// This is the public API that takes a Context for access to theme and loader.
/// Uses word-aware wrapping when max_width_cells > 0.
pub fn highlight_code_block(
  lang_hint: Option<&str>,
  code: &str,
  max_width_cells: usize,
  ctx: &mut Context,
) -> Vec<Vec<TextSegment>> {
  let theme = &ctx.editor.theme;
  let loader = ctx.editor.syn_loader.load();

  let lines = highlight_code_block_internal(code, lang_hint.unwrap_or(""), Some(theme), &loader);

  // Apply word-aware wrapping if needed
  if max_width_cells > 0 {
    super::text_wrap::wrap_lines(&lines, max_width_cells as u16, false)
  } else {
    lines
  }
}

/// Wrap text to fit within max_chars, preserving intentional structure.
pub fn wrap_text_preserve_breaks(text: &str, max_chars: usize) -> Vec<String> {
  if max_chars == 0 {
    return vec![String::new()];
  }

  let text = text.trim_end();
  if text.is_empty() {
    return vec![];
  }

  if text.chars().count() <= max_chars {
    return vec![text.to_string()];
  }

  let mut lines = Vec::new();
  let mut current = String::new();

  for word in text.split_whitespace() {
    let word_len = word.chars().count();

    if word_len > max_chars {
      if !current.is_empty() {
        lines.push(std::mem::take(&mut current));
      }
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
/// This function parses markdown and wraps it using word-aware wrapping.
/// The `max_width_cells` parameter specifies the maximum width in display cells
/// (characters for monospace fonts).
pub fn build_markdown_lines(
  markdown: &str,
  wrap_width: f32,
  cell_width: f32,
  ctx: &mut Context,
) -> Vec<Vec<TextSegment>> {
  let md = Markdown::new(markdown.to_string(), ctx.editor.syn_loader.clone());
  let lines = md.parse(Some(&ctx.editor.theme));

  // Convert pixel width to cell width for wrapping
  let max_width_cells = (wrap_width / cell_width.max(1.0)).floor().max(4.0) as u16;

  // Use word-aware wrapping from text_wrap module
  super::text_wrap::wrap_lines(&lines, max_width_cells, false)
}

/// Build render lines from markdown with explicit cell width.
///
/// This is the preferred API - it takes max_width directly in cells,
/// avoiding the pixel-to-cell conversion that can cause inconsistencies.
pub fn build_markdown_lines_cells(
  markdown: &str,
  max_width_cells: u16,
  ctx: &mut Context,
) -> Vec<Vec<TextSegment>> {
  let md = Markdown::new(markdown.to_string(), ctx.editor.syn_loader.clone());
  let lines = md.parse(Some(&ctx.editor.theme));

  // Use word-aware wrapping from text_wrap module
  super::text_wrap::wrap_lines(&lines, max_width_cells, false)
}

/// Estimate the rendered width of a line of text segments in pixels.
pub fn estimate_line_width(segments: &[TextSegment], cell_width: f32) -> f32 {
  super::text_wrap::line_width_pixels(segments, cell_width)
}

/// Get the display width of a line in cells.
pub fn line_width_cells(segments: &[TextSegment]) -> u16 {
  super::text_wrap::line_width(segments)
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use super::*;

  // Helper to create a mock Markdown without needing full context
  fn parse_markdown_plain(content: &str) -> Vec<Vec<TextSegment>> {
    // We can't easily test with full syntax highlighting without a loader,
    // but we can test the parsing logic with None theme
    let loader = Arc::new(ArcSwap::from_pointee(Loader::default()));
    let md = Markdown::new(content.to_string(), loader);
    md.parse(None)
  }

  #[test]
  fn test_simple_paragraph() {
    let lines = parse_markdown_plain("Hello world");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].len(), 1);
    assert_eq!(lines[0][0].content, "Hello world");
  }

  #[test]
  fn test_multiple_paragraphs() {
    let lines = parse_markdown_plain("First paragraph\n\nSecond paragraph");
    // Should have: "First paragraph", empty line, "Second paragraph"
    assert!(lines.len() >= 2);
    assert_eq!(lines[0][0].content, "First paragraph");
  }

  #[test]
  fn test_heading() {
    let lines = parse_markdown_plain("# Heading 1\n\nSome text");
    assert!(!lines.is_empty());
    assert_eq!(lines[0][0].content, "Heading 1");
  }

  #[test]
  fn test_code_block() {
    let lines = parse_markdown_plain("```rust\nlet x = 1;\n```");
    // Code block should produce at least one line
    assert!(!lines.is_empty());
    // The content should contain "let x = 1;"
    let all_content: String = lines.iter().flatten().map(|s| s.content.as_str()).collect();
    assert!(all_content.contains("let x = 1;"));
  }

  #[test]
  fn test_inline_code() {
    let lines = parse_markdown_plain("Use `code` here");
    assert_eq!(lines.len(), 1);
    // Should have multiple segments: "Use ", "code", " here"
    let all_content: String = lines[0].iter().map(|s| s.content.as_str()).collect();
    assert_eq!(all_content, "Use code here");
  }

  #[test]
  fn test_unordered_list() {
    let lines = parse_markdown_plain("- Item 1\n- Item 2");
    assert!(lines.len() >= 2);
    // First item should contain bullet
    let first_line: String = lines[0].iter().map(|s| s.content.as_str()).collect();
    assert!(first_line.contains("Item 1"));
  }

  #[test]
  fn test_ordered_list() {
    let lines = parse_markdown_plain("1. First\n2. Second");
    assert!(lines.len() >= 2);
    let first_line: String = lines[0].iter().map(|s| s.content.as_str()).collect();
    assert!(first_line.contains("First"));
  }

  #[test]
  fn test_horizontal_rule() {
    let lines = parse_markdown_plain("Before\n\n---\n\nAfter");
    // Should contain the rule character
    let all_content: String = lines.iter().flatten().map(|s| s.content.as_str()).collect();
    assert!(all_content.contains("───"));
  }

  #[test]
  fn test_required_size_simple() {
    let loader = Arc::new(ArcSwap::from_pointee(Loader::default()));
    let md = Markdown::new("Hello world".to_string(), loader);
    let (width, height) = md.required_size(80);
    assert_eq!(width, 11); // "Hello world" is 11 chars
    assert_eq!(height, 1);
  }

  #[test]
  fn test_required_size_wrapping() {
    let loader = Arc::new(ArcSwap::from_pointee(Loader::default()));
    let md = Markdown::new("This is a longer line that should wrap".to_string(), loader);
    let (width, height) = md.required_size(20);
    assert!(width <= 20);
    assert!(height >= 2); // Should wrap to multiple lines
  }

  #[test]
  fn test_wrap_text_preserve_breaks_short() {
    let result = wrap_text_preserve_breaks("short", 80);
    assert_eq!(result, vec!["short"]);
  }

  #[test]
  fn test_wrap_text_preserve_breaks_long() {
    let result = wrap_text_preserve_breaks("this is a long line", 10);
    assert!(result.len() >= 2);
    for line in &result {
      assert!(line.chars().count() <= 10);
    }
  }

  #[test]
  fn test_wrap_line_short() {
    let segments = vec![TextSegment {
      content: "short".to_string(),
      style:   TextStyle {
        size:  UI_FONT_SIZE,
        color: Color::new(1.0, 1.0, 1.0, 1.0),
      },
    }];
    let result = super::text_wrap::wrap_line(&segments, 80, false);
    assert_eq!(result.len(), 1);
  }

  #[test]
  fn test_wrap_line_long() {
    let segments = vec![TextSegment {
      content: "this is a very long line that needs wrapping".to_string(),
      style:   TextStyle {
        size:  UI_FONT_SIZE,
        color: Color::new(1.0, 1.0, 1.0, 1.0),
      },
    }];
    let result = super::text_wrap::wrap_line(&segments, 20, false);
    assert!(result.len() >= 2);
  }
}
