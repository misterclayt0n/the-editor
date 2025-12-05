use std::{
  cmp::Reverse,
  sync::Arc,
};

use nucleo::{
  Config,
  Utf32Str,
  pattern::{
    Atom,
    AtomKind,
    CaseMatching,
    Normalization,
  },
};
use ropey::Rope;
use the_editor_lsp_types::types as lsp;
use the_editor_renderer::{
  Color,
  TextSection,
  TextSegment,
  TextStyle,
};
use the_editor_stdx::rope::RopeSliceExt;

use crate::{
  core::{
    ViewId,
    document::SavePoint,
    graphics::{
      CursorKind,
      Rect,
    },
    position::Position,
    transaction::Transaction,
  },
  editor::CompleteAction,
  handlers::{
    completion::{
      CompletionItem,
      CompletionProvider,
      LspCompletionItem,
    },
    completion_resolve::ResolveHandler,
  },
  snippets::{
    active::ActiveSnippet,
    elaborate::Snippet,
    render::RenderedSnippet,
  },
  ui::{
    UI_FONT_SIZE,
    UI_FONT_WIDTH,
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
    popup_positioning::{
      calculate_cursor_position,
      position_popup_near_cursor,
    },
  },
};

/// Minimum width for documentation preview panel
const MIN_DOC_WIDTH: u16 = 30;

/// Maximum width for completion menu
const MAX_MENU_WIDTH: u16 = 60;

/// Maximum visible completion items
const MAX_VISIBLE_ITEMS: usize = 15;
/// Pixel gap between cursor baseline and popup
const CURSOR_POPUP_MARGIN: f32 = 4.0;

struct CompletionApplyPlan {
  transaction:            Transaction,
  snippet:                Option<RenderedSnippet>,
  trigger_signature_help: bool,
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
  let mut lines = Vec::new();
  let mut current_line = String::new();
  let mut current_width = 0;

  for word in text.split_whitespace() {
    let word_len = word.len();

    // Break long tokens that exceed the available width
    if word_len > max_width {
      if !current_line.is_empty() {
        lines.push(std::mem::take(&mut current_line));
        current_width = 0;
      }

      let mut chunk = String::new();
      for ch in word.chars() {
        if chunk.chars().count() >= max_width {
          lines.push(std::mem::take(&mut chunk));
        }
        chunk.push(ch);
      }
      if !chunk.is_empty() {
        current_line = chunk;
        current_width = current_line.chars().count();
      }
      continue;
    }

    if current_width + word_len + (current_width != 0) as usize > max_width
      && !current_line.is_empty()
    {
      // Start new line
      lines.push(std::mem::take(&mut current_line));
      current_line = word.to_string();
      current_width = word_len;
    } else {
      // Add to current line
      if !current_line.is_empty() {
        current_line.push(' ');
        current_width += 1;
      }
      current_line.push_str(word);
      current_width += word_len;
    }
  }

  if !current_line.is_empty() {
    lines.push(current_line);
  }

  lines
}

fn truncate_to_width(text: &str, max_width: f32, char_width: f32) -> String {
  if max_width <= 0.0 {
    return String::new();
  }

  let char_width = char_width.max(1.0);
  let max_chars = (max_width / char_width).floor() as usize;
  if max_chars == 0 {
    return String::new();
  }

  let mut chars = text.chars();
  let count = text.chars().count();
  if count <= max_chars {
    return text.to_string();
  }

  if max_chars == 1 {
    return "…".to_string();
  }

  let mut truncated = String::with_capacity(max_chars);
  for _ in 0..(max_chars - 1) {
    if let Some(ch) = chars.next() {
      truncated.push(ch);
    } else {
      break;
    }
  }
  truncated.push('…');
  truncated
}

fn build_completion_doc_lines(
  markdown: &str,
  max_chars: usize,
  ctx: &mut Context,
  base_text_color: Color,
) -> Vec<Vec<TextSegment>> {
  let mut render_lines: Vec<Vec<TextSegment>> = Vec::new();
  let mut in_fence = false;
  let mut fence_lang: Option<String> = None;
  let mut fence_buf: Vec<String> = Vec::new();
  let max_chars = max_chars.max(4);

  for raw_line in markdown.lines() {
    if raw_line.starts_with("```") {
      if in_fence {
        let code = fence_buf.join("\n");
        render_lines.extend(highlight_completion_code_block_lines(
          fence_lang.as_deref(),
          &code,
          max_chars,
          ctx,
        ));
        render_lines.push(empty_doc_line(base_text_color));
        in_fence = false;
        fence_lang = None;
        fence_buf.clear();
      } else {
        in_fence = true;
        let lang = raw_line.trim_start_matches("```").trim();
        fence_lang = if lang.is_empty() {
          None
        } else {
          Some(lang.to_string())
        };
      }
      continue;
    }

    if in_fence {
      fence_buf.push(raw_line.to_string());
      continue;
    }

    if raw_line.trim().is_empty() {
      render_lines.push(empty_doc_line(base_text_color));
      continue;
    }

    let wrapped_lines = wrap_text(raw_line, max_chars);
    if wrapped_lines.is_empty() {
      render_lines.push(empty_doc_line(base_text_color));
    } else {
      for line in wrapped_lines {
        render_lines.push(vec![TextSegment {
          content: line,
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: base_text_color,
          },
        }]);
      }
    }
  }

  if in_fence {
    let code = fence_buf.join("\n");
    render_lines.extend(highlight_completion_code_block_lines(
      fence_lang.as_deref(),
      &code,
      max_chars,
      ctx,
    ));
  }

  if render_lines.is_empty() {
    render_lines.push(empty_doc_line(base_text_color));
  }

  render_lines
}

fn highlight_completion_code_block_lines(
  lang_hint: Option<&str>,
  code: &str,
  max_chars: usize,
  ctx: &mut Context,
) -> Vec<Vec<TextSegment>> {
  let theme = &ctx.editor.theme;
  let code_style = theme.get("markup.raw");
  let default_code_color = code_style
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
    .unwrap_or_else(Vec::new);

  let mut lines: Vec<Vec<TextSegment>> = Vec::new();
  let total_lines = rope.len_lines();

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
      line_string.pop();
    }

    let wrapped_line_strings = wrap_text(&line_string, max_chars);
    if wrapped_line_strings.is_empty() {
      lines.push(vec![TextSegment {
        content: String::new(),
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: default_code_color,
        },
      }]);
      continue;
    }

    for wrapped_line in wrapped_line_strings {
      if wrapped_line.is_empty() {
        lines.push(vec![TextSegment {
          content: String::new(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: default_code_color,
          },
        }]);
        continue;
      }

      let wrapped_rope = Rope::from(wrapped_line.as_str());
      let wrapped_slice = wrapped_rope.slice(..);

      let wrapped_spans = language
        .and_then(|lang| crate::core::syntax::Syntax::new(wrapped_slice, lang, &loader).ok())
        .map(|syntax| {
          syntax.collect_highlights(wrapped_slice, &loader, 0..wrapped_slice.len_bytes())
        })
        .unwrap_or_else(Vec::new);

      let mut wrapped_char_spans: Vec<(usize, usize, Color)> =
        Vec::with_capacity(wrapped_spans.len());
      for (hl, byte_range) in wrapped_spans.into_iter() {
        let style = theme.highlight(hl);
        let color = style
          .fg
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(default_code_color);
        let start_char =
          wrapped_slice.byte_to_char(wrapped_slice.floor_char_boundary(byte_range.start));
        let end_char = wrapped_slice.byte_to_char(wrapped_slice.ceil_char_boundary(byte_range.end));
        if start_char < end_char {
          wrapped_char_spans.push((start_char, end_char, color));
        }
      }
      wrapped_char_spans.sort_by_key(|(s, _e, _)| *s);

      let mut segments: Vec<TextSegment> = Vec::new();
      let mut cursor = 0usize;
      let wrapped_line_chars = wrapped_line.chars().count();

      for (s, e, color) in wrapped_char_spans.iter().cloned() {
        if e <= cursor || s >= wrapped_line_chars {
          continue;
        }
        let seg_start = s.max(cursor);
        let seg_end = e.min(wrapped_line_chars);

        if seg_start > cursor {
          let prefix = slice_chars_to_string(&wrapped_line, cursor, seg_start);
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

        let content = slice_chars_to_string(&wrapped_line, seg_start, seg_end);
        if !content.is_empty() {
          segments.push(TextSegment {
            content,
            style: TextStyle {
              size: UI_FONT_SIZE,
              color,
            },
          });
        }
        cursor = seg_end;
      }

      if cursor < wrapped_line_chars {
        let tail = slice_chars_to_string(&wrapped_line, cursor, wrapped_line_chars);
        if !tail.is_empty() {
          segments.push(TextSegment {
            content: tail,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: default_code_color,
            },
          });
        }
      }

      if segments.is_empty() {
        segments.push(TextSegment {
          content: wrapped_line,
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: default_code_color,
          },
        });
      }

      lines.push(segments);
    }
  }

  lines
}

fn slice_chars_to_string(s: &str, start: usize, end: usize) -> String {
  if start >= end {
    return String::new();
  }
  let mut buf = String::with_capacity(end.saturating_sub(start));
  for (i, ch) in s.chars().enumerate() {
    if i >= end {
      break;
    }
    if i >= start {
      buf.push(ch);
    }
  }
  buf
}

fn empty_doc_line(color: Color) -> Vec<TextSegment> {
  vec![TextSegment {
    content: String::new(),
    style:   TextStyle {
      size: UI_FONT_SIZE,
      color,
    },
  }]
}

/// Completion popup component
pub struct Completion {
  /// All completion items
  items:           Vec<CompletionItem>,
  /// Filtered item indices (sorted by score)
  filtered:        Vec<(u32, u32)>, // (index, score)
  /// Currently selected item index (into filtered list)
  cursor:          usize,
  /// Current filter string (text typed since trigger)
  filter:          String,
  /// Trigger offset in document (where completion started)
  trigger_offset:  usize,
  /// Savepoint for preview functionality
  savepoint:       Option<Arc<SavePoint>>,
  /// Whether preview is enabled
  preview_enabled: bool,
  /// Whether to replace (vs insert) mode
  replace_mode:    bool,
  /// Scroll offset for the list
  scroll_offset:   usize,
  /// Whether documentation has been resolved for current selection
  doc_resolved:    bool,
  /// Appearance animation
  animation:       crate::core::animation::AnimationHandle<f32>,
  /// Handler for resolving incomplete completion items
  resolve_handler: ResolveHandler,
}

impl Completion {
  pub const ID: &'static str = "completion";

  /// Create a new completion popup
  pub fn new(items: Vec<CompletionItem>, trigger_offset: usize, filter: String) -> Self {
    // Create appearance animation using popup preset
    let (duration, easing) = crate::core::animation::presets::POPUP;
    let animation = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);

    let mut completion = Self {
      items,
      filtered: Vec::new(),
      cursor: 0,
      filter,
      trigger_offset,
      savepoint: None,
      preview_enabled: false,
      replace_mode: false,
      scroll_offset: 0,
      doc_resolved: false,
      animation,
      resolve_handler: ResolveHandler::new(),
    };

    // Initial scoring
    completion.score(false);
    completion
  }

  /// Update the filter and re-score items
  pub fn update_filter(&mut self, c: Option<char>) {
    match c {
      Some(c) => self.filter.push(c),
      None => {
        self.filter.pop();
        if self.filter.is_empty() {
          self.filtered.clear();
          return;
        }
      },
    }

    self.score(c.is_some());
    self.cursor = 0;
    self.scroll_offset = 0;
    self.doc_resolved = false;
  }

  /// Score and filter items using fuzzy matching
  fn score(&mut self, incremental: bool) {
    let pattern = &self.filter;

    // Create nucleo pattern
    let atom = Atom::new(
      pattern,
      CaseMatching::Ignore,
      Normalization::Smart,
      AtomKind::Fuzzy,
      false,
    );

    let mut matcher = nucleo::Matcher::new(Config::DEFAULT);
    matcher.config.prefer_prefix = true;
    let mut buf = Vec::new();

    if incremental {
      // Incremental update: re-score existing matches
      self.filtered.retain_mut(|(index, score)| {
        let item = &self.items[*index as usize];
        let text = item.filter_text();
        match atom.score(Utf32Str::new(text, &mut buf), &mut matcher) {
          Some(new_score) => {
            *score = new_score as u32;
            true
          },
          None => false,
        }
      });
    } else {
      // Full re-score: score all items
      self.filtered.clear();
      for (i, item) in self.items.iter().enumerate() {
        let text = item.filter_text();
        if let Some(score) = atom.score(Utf32Str::new(text, &mut buf), &mut matcher) {
          self.filtered.push((i as u32, score as u32));
        }
      }
    }

    // Sort by score and provider priority
    // Higher scores first, preselected items first, higher priority first
    let items = &self.items;
    let pattern_len = pattern.len() as u32;
    let min_score = (7 + pattern_len * 14) / 3; // Helix's heuristic

    self.filtered.sort_unstable_by_key(|&(i, score)| {
      let item = &items[i as usize];
      (
        score <= min_score,
        Reverse(item.preselect()),
        item.provider_priority(),
        Reverse(score),
        i,
      )
    });
  }

  /// Get the currently selected completion item
  pub fn selection(&self) -> Option<&CompletionItem> {
    self
      .filtered
      .get(self.cursor)
      .map(|&(idx, _)| &self.items[idx as usize])
  }

  /// Get the currently selected completion item mutably
  pub fn selection_mut(&mut self) -> Option<&mut CompletionItem> {
    self
      .filtered
      .get(self.cursor)
      .map(|&(idx, _)| &mut self.items[idx as usize])
  }

  /// Check if the completion list is empty
  pub fn is_empty(&self) -> bool {
    self.filtered.is_empty()
  }

  /// Move cursor up
  pub fn move_up(&mut self, count: usize) {
    self.cursor = self.cursor.saturating_sub(count);
    self.doc_resolved = false;
    self.ensure_cursor_in_view();
  }

  /// Move cursor down
  pub fn move_down(&mut self, count: usize) {
    if !self.filtered.is_empty() {
      self.cursor = (self.cursor + count).min(self.filtered.len() - 1);
      self.doc_resolved = false;
      self.ensure_cursor_in_view();
    }
  }

  /// Ensure cursor is visible in the scrolled view
  fn ensure_cursor_in_view(&mut self) {
    if self.cursor < self.scroll_offset {
      self.scroll_offset = self.cursor;
    } else if self.cursor >= self.scroll_offset + MAX_VISIBLE_ITEMS {
      self.scroll_offset = self.cursor.saturating_sub(MAX_VISIBLE_ITEMS - 1);
    }
  }

  /// Replace items from a specific provider
  pub fn replace_provider_items(
    &mut self,
    provider: CompletionProvider,
    new_items: Vec<CompletionItem>,
  ) {
    // Remove old items from this provider
    self.items.retain(|item| item.provider() != provider);

    // Add new items
    self.items.extend(new_items);

    // Re-score
    self.score(false);
  }

  /// Replace a specific completion item with a resolved version
  /// Used by the resolve handler to update items with documentation
  pub fn replace_item(&mut self, old_item: &LspCompletionItem, new_item: CompletionItem) {
    // Find the item in our list
    for item in &mut self.items {
      if let CompletionItem::Lsp(lsp_item) = item {
        if lsp_item == old_item {
          *item = new_item;
          log::debug!("Replaced completion item with resolved version");
          return;
        }
      }
    }
    log::warn!("Could not find item to replace in completion list");
  }

  /// Trigger resolution for the currently selected item
  fn trigger_resolve(&mut self) {
    // Get the current selection index before borrowing resolve_handler
    let item_idx = if self.filtered.is_empty() {
      None
    } else {
      let (idx, _score) = self.filtered[self.cursor];
      Some(idx as usize)
    };

    if let Some(idx) = item_idx {
      if let Some(CompletionItem::Lsp(lsp_item)) = self.items.get_mut(idx) {
        self.resolve_handler.ensure_item_resolved(lsp_item);
      }
    }
  }

  /// Render the documentation popup for the selected completion item
  fn render_documentation(
    &self,
    item: &CompletionItem,
    completion_x: f32,
    completion_y: f32,
    completion_width: f32,
    completion_height: f32,
    alpha: f32,
    ui_char_width: f32,
    ui_line_height: f32,
    surface: &mut Surface,
    ctx: &mut Context,
  ) {
    // Extract documentation and detail from the item
    let (detail, doc) = match item {
      CompletionItem::Lsp(lsp_item) => {
        let detail = lsp_item.item.detail.as_deref();
        let doc = lsp_item.item.documentation.as_ref().and_then(|d| {
          match d {
            lsp::Documentation::String(s) => Some(s.as_str()),
            lsp::Documentation::MarkupContent(content) => Some(content.value.as_str()),
          }
        });
        (detail, doc)
      },
      CompletionItem::Other(_other) => {
        // Other items don't have documentation yet
        return;
      },
    };

    // If there's no documentation to show, return early
    if detail.is_none() && doc.is_none() {
      return;
    }

    // Get window dimensions
    let window_width = surface.width() as f32;
    let window_height = surface.height() as f32;

    // Constants for doc popup
    const MIN_DOC_WIDTH: f32 = 200.0;
    const MAX_DOC_WIDTH: f32 = 500.0;
    const MIN_DOC_HEIGHT: f32 = 100.0;
    const DOC_PADDING: f32 = 8.0;

    // Calculate available space on each side
    let space_on_right = window_width - (completion_x + completion_width);
    let space_on_left = completion_x;
    let space_below = window_height - (completion_y + completion_height);

    const SPACING: f32 = 8.0;

    // Determine best placement and calculate dimensions
    let (doc_x, doc_y, doc_width, doc_height) = if space_on_right >= MIN_DOC_WIDTH + SPACING {
      // Position to the right - available space is from completion edge to window
      // edge
      let available_width = space_on_right - SPACING;
      let doc_width = available_width.min(MAX_DOC_WIDTH);
      let doc_x = completion_x + completion_width + SPACING;
      let doc_y = completion_y;
      let doc_height = completion_height
        .max(MIN_DOC_HEIGHT)
        .min(window_height - doc_y);
      (doc_x, doc_y, doc_width, doc_height)
    } else if space_on_left >= MIN_DOC_WIDTH + SPACING {
      // Position to the left
      let available_width = space_on_left - SPACING;
      let doc_width = available_width.min(MAX_DOC_WIDTH);
      let doc_x = completion_x - doc_width - SPACING;
      let doc_y = completion_y;
      let doc_height = completion_height
        .max(MIN_DOC_HEIGHT)
        .min(window_height - doc_y);
      (doc_x, doc_y, doc_width, doc_height)
    } else if space_below >= MIN_DOC_HEIGHT + SPACING {
      // Position below completion
      let doc_x = completion_x;
      let doc_y = completion_y + completion_height + SPACING;
      let doc_width = completion_width
        .max(MIN_DOC_WIDTH)
        .min(MAX_DOC_WIDTH)
        .min(window_width - doc_x);
      let available_height = space_below - SPACING;
      let doc_height = available_height.min(MIN_DOC_HEIGHT * 2.0);
      (doc_x, doc_y, doc_width, doc_height)
    } else {
      // Not enough space anywhere, don't render
      return;
    };

    // Final safety check - ensure we're within viewport
    if doc_x < 0.0
      || doc_y < 0.0
      || doc_x + doc_width > window_width
      || doc_y + doc_height > window_height
      || doc_width < 100.0
      || doc_height < 50.0
    {
      return;
    }

    // Get theme colors (same as completion popup)
    let theme = &ctx.editor.theme;
    let bg_style = theme.get("ui.popup");
    let text_style = theme.get("ui.text");

    // Background should be opaque (don't apply animation alpha to background)
    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));

    // Apply animation alpha only to text
    let mut text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
    let base_text_color = text_color;

    surface.with_overlay_region(doc_x, doc_y, doc_width, doc_height, |surface| {
      // Draw background
      let corner_radius = 6.0;
      surface.draw_rounded_rect(doc_x, doc_y, doc_width, doc_height, corner_radius, bg_color);

      // Draw border (keep it opaque)
      let border_color = Color::new(0.3, 0.3, 0.35, 0.8);
      surface.draw_rounded_rect_stroke(
        doc_x,
        doc_y,
        doc_width,
        doc_height,
        corner_radius,
        1.0,
        border_color,
      );

      // Render documentation content
      let mut y_offset = doc_y + DOC_PADDING;
      let font_size = UI_FONT_SIZE;
      let line_height = ui_line_height.max(font_size + 4.0);
      let max_chars_per_line = ((doc_width - DOC_PADDING * 2.0) / ui_char_width)
        .floor()
        .max(4.0) as usize;
      let mut line_groups: Vec<Vec<TextSegment>> = Vec::new();

      if let Some(detail_text) = detail {
        let detail_color = Color::new(0.7, 0.8, 0.9, 1.0);
        let mut detail_lines =
          build_completion_doc_lines(detail_text, max_chars_per_line, ctx, detail_color);
        line_groups.append(&mut detail_lines);
        if doc.is_some() {
          line_groups.push(empty_doc_line(detail_color));
        }
      }

      if let Some(doc_text) = doc {
        let mut doc_lines =
          build_completion_doc_lines(doc_text, max_chars_per_line, ctx, base_text_color);
        line_groups.append(&mut doc_lines);
      }

      if line_groups.is_empty() {
        line_groups.push(empty_doc_line(base_text_color));
      }

      let max_lines_by_height = ((doc_height - DOC_PADDING * 2.0) / line_height)
        .floor()
        .max(0.0) as usize;
      if max_lines_by_height == 0 {
        return;
      }

      let max_text_y = doc_y + doc_height - DOC_PADDING;
      surface.push_scissor_rect(doc_x, doc_y, doc_width, doc_height);

      for segments in line_groups.into_iter().take(max_lines_by_height) {
        if y_offset > max_text_y {
          break;
        }

        let texts = segments
          .into_iter()
          .map(|mut segment| {
            segment.style.color.a *= alpha;
            segment
          })
          .collect();

        surface.draw_text(TextSection {
          position: (doc_x + DOC_PADDING, y_offset),
          texts,
        });
        y_offset += line_height;
      }

      surface.pop_scissor_rect();
    });
  }

  /// Format a completion item kind to a display string
  fn format_kind(kind: Option<lsp::CompletionItemKind>) -> &'static str {
    match kind {
      Some(lsp::CompletionItemKind::TEXT) => "text",
      Some(lsp::CompletionItemKind::METHOD) => "method",
      Some(lsp::CompletionItemKind::FUNCTION) => "function",
      Some(lsp::CompletionItemKind::CONSTRUCTOR) => "ctor",
      Some(lsp::CompletionItemKind::FIELD) => "field",
      Some(lsp::CompletionItemKind::VARIABLE) => "var",
      Some(lsp::CompletionItemKind::CLASS) => "class",
      Some(lsp::CompletionItemKind::INTERFACE) => "iface",
      Some(lsp::CompletionItemKind::MODULE) => "module",
      Some(lsp::CompletionItemKind::PROPERTY) => "prop",
      Some(lsp::CompletionItemKind::UNIT) => "unit",
      Some(lsp::CompletionItemKind::VALUE) => "value",
      Some(lsp::CompletionItemKind::ENUM) => "enum",
      Some(lsp::CompletionItemKind::KEYWORD) => "keyword",
      Some(lsp::CompletionItemKind::SNIPPET) => "snippet",
      Some(lsp::CompletionItemKind::COLOR) => "color",
      Some(lsp::CompletionItemKind::FILE) => "file",
      Some(lsp::CompletionItemKind::REFERENCE) => "ref",
      Some(lsp::CompletionItemKind::FOLDER) => "folder",
      Some(lsp::CompletionItemKind::ENUM_MEMBER) => "enumm",
      Some(lsp::CompletionItemKind::CONSTANT) => "const",
      Some(lsp::CompletionItemKind::STRUCT) => "struct",
      Some(lsp::CompletionItemKind::EVENT) => "event",
      Some(lsp::CompletionItemKind::OPERATOR) => "op",
      Some(lsp::CompletionItemKind::TYPE_PARAMETER) => "type",
      _ => "",
    }
  }

  /// Check if an LSP item is deprecated
  fn is_deprecated(item: &lsp::CompletionItem) -> bool {
    item.deprecated.unwrap_or(false)
      || item.tags.as_ref().map_or(false, |tags| {
        tags.contains(&lsp::CompletionItemTag::DEPRECATED)
      })
  }

  /// Apply the selected completion item
  ///
  /// This function applies the completion immediately without blocking.
  /// If the item needs resolution (for additional_text_edits like
  /// auto-imports), it spawns an async task to fetch and apply them
  /// afterwards.
  fn apply_completion(&self, ctx: &mut Context, item: &CompletionItem) {
    use the_editor_event::send_blocking;

    use crate::handlers::lsp::SignatureHelpEvent;

    let owned_item = item.clone();

    match owned_item {
      CompletionItem::Lsp(lsp_item) => {
        // First get the offset encoding and resolve future (if needed) before borrowing
        // doc
        let Some(language_server) = ctx.editor.language_server_by_id(lsp_item.provider) else {
          log::error!("Language server not found for completion");
          return;
        };

        let offset_encoding = language_server.offset_encoding();

        // Get the resolve future now if we need async resolution later
        let resolve_future = if !lsp_item.resolved
          && lsp_item.item.additional_text_edits.is_none()
          && matches!(
            language_server.capabilities().completion_provider,
            Some(lsp::CompletionOptions {
              resolve_provider: Some(true),
              ..
            })
          ) {
          Some(language_server.resolve_completion_item(&lsp_item.item))
        } else {
          None
        };

        // Now borrow doc mutably
        let (view, doc) = crate::current!(ctx.editor);

        let Some(plan) = self.plan_lsp_transaction(doc, view.id, &lsp_item.item, offset_encoding)
        else {
          return;
        };

        let placeholder_active = plan.snippet.is_some();
        let changes = plan
          .transaction
          .changes()
          .changes_iter()
          .collect::<Vec<_>>();

        // Apply the main completion transaction immediately
        doc.apply(&plan.transaction, view.id);

        if let Some(snippet) = plan.snippet {
          doc.active_snippet = match doc.active_snippet.take() {
            Some(active) => active.insert_subsnippet(snippet),
            None => ActiveSnippet::new(snippet),
          };
        }

        // Handle additional_text_edits - apply immediately if already resolved,
        // otherwise fetch asynchronously
        if let Some(additional_edits) = &lsp_item.item.additional_text_edits {
          if !additional_edits.is_empty() {
            log::info!(
              "Applying {} additional text edits for auto-import",
              additional_edits.len()
            );
            let transaction = crate::lsp::util::generate_transaction_from_edits(
              doc.text(),
              additional_edits.clone(),
              offset_encoding,
            );
            doc.apply(&transaction, view.id);
          }
        } else if let Some(future) = resolve_future {
          // Item not resolved - spawn async task to fetch additional_text_edits
          let doc_id = doc.id();
          let view_id = view.id;
          Self::spawn_resolve_additional_edits(future, offset_encoding, doc_id, view_id);
        }

        if plan.trigger_signature_help {
          send_blocking(
            &ctx.editor.handlers.signature_hints,
            SignatureHelpEvent::Trigger,
          );
        }

        // Save to history
        doc.append_changes_to_history(view);

        ctx.editor.last_completion = Some(CompleteAction::Applied {
          trigger_offset: self.trigger_offset,
          changes,
          placeholder: placeholder_active,
        });
      },
      CompletionItem::Other(other) => {
        let (view, doc) = crate::current!(ctx.editor);

        // For non-LSP completions, replace from trigger to cursor with the label
        let cursor = doc
          .selection(view.id)
          .primary()
          .cursor(doc.text().slice(..));
        let start = self.trigger_offset;
        let end = cursor;

        let transaction = Transaction::change(
          doc.text(),
          [(start, end, Some(other.label.clone().into()))]
            .iter()
            .cloned(),
        );
        let changes = transaction.changes().changes_iter().collect::<Vec<_>>();
        doc.apply(&transaction, view.id);

        // Save to history
        doc.append_changes_to_history(view);

        ctx.editor.last_completion = Some(CompleteAction::Applied {
          trigger_offset: self.trigger_offset,
          changes,
          placeholder: false,
        });
      },
    }
  }

  /// Spawn an async task to resolve completion item and apply
  /// additional_text_edits
  ///
  /// This is called when accepting a completion that hasn't been fully resolved
  /// yet. The main completion text is applied immediately, and this task
  /// fetches any additional edits (like auto-imports) asynchronously without
  /// blocking the UI.
  fn spawn_resolve_additional_edits(
    resolve_future: futures_util::future::BoxFuture<
      'static,
      crate::lsp::Result<lsp::CompletionItem>,
    >,
    offset_encoding: crate::lsp::OffsetEncoding,
    doc_id: crate::core::DocumentId,
    view_id: ViewId,
  ) {
    // Spawn async task to resolve and apply additional edits
    tokio::spawn(async move {
      match resolve_future.await {
        Ok(resolved) => {
          if let Some(additional_edits) = resolved.additional_text_edits.filter(|e| !e.is_empty()) {
            log::info!(
              "Async: Applying {} additional text edits for auto-import",
              additional_edits.len()
            );
            // Dispatch back to main thread to apply the edits
            crate::ui::job::dispatch(move |editor, _compositor| {
              let Some(doc) = editor.documents.get_mut(&doc_id) else {
                log::warn!("Document no longer exists for additional edits");
                return;
              };

              let transaction = crate::lsp::util::generate_transaction_from_edits(
                doc.text(),
                additional_edits,
                offset_encoding,
              );
              doc.apply(&transaction, view_id);

              // Append to history so the additional edits can be undone together
              // Check if view still exists before getting mutable reference
              if editor.tree.try_get(view_id).is_some() {
                let view = editor.tree.get_mut(view_id);
                doc.append_changes_to_history(view);
              }
            })
            .await;
          }
        },
        Err(err) => {
          log::error!("Async completion resolve failed: {}", err);
        },
      }
    });
  }

  fn plan_lsp_transaction(
    &self,
    doc: &mut crate::core::document::Document,
    view_id: ViewId,
    item: &lsp::CompletionItem,
    offset_encoding: crate::lsp::OffsetEncoding,
  ) -> Option<CompletionApplyPlan> {
    use crate::lsp::util::{
      generate_transaction_from_completion_edit,
      generate_transaction_from_snippet,
    };

    let selection = doc.selection(view_id).clone();
    let text = doc.text();
    let rope_slice = text.slice(..);
    let primary_cursor = selection.primary().cursor(rope_slice);

    let (edit_offset, new_text) = if let Some(edit) = &item.text_edit {
      match edit {
        lsp::CompletionTextEdit::Edit(edit) => {
          let Some(start) =
            crate::lsp::util::lsp_pos_to_pos(text, edit.range.start, offset_encoding)
          else {
            log::error!("Invalid LSP completion start position");
            return None;
          };
          let start_offset = start as i128 - primary_cursor as i128;
          (Some((start_offset, 0)), edit.new_text.clone())
        },
        lsp::CompletionTextEdit::InsertAndReplace(edit) => {
          let pos = if self.replace_mode {
            edit.replace.start
          } else {
            edit.insert.start
          };
          let Some(start) = crate::lsp::util::lsp_pos_to_pos(text, pos, offset_encoding) else {
            log::error!("Invalid LSP insert start position");
            return None;
          };
          let start_offset = start as i128 - primary_cursor as i128;
          (Some((start_offset, 0)), edit.new_text.clone())
        },
      }
    } else {
      let new_text = item
        .insert_text
        .clone()
        .unwrap_or_else(|| item.label.clone());
      (None, new_text)
    };

    let should_trigger_signature_help = new_text.contains('(');
    let is_snippet = matches!(item.kind, Some(lsp::CompletionItemKind::SNIPPET))
      || matches!(
        item.insert_text_format,
        Some(lsp::InsertTextFormat::SNIPPET)
      );

    if is_snippet {
      match Snippet::parse(&new_text) {
        Ok(snippet) => {
          let mut snippet_ctx = doc.snippet_ctx();
          let (transaction, rendered_snippet) = generate_transaction_from_snippet(
            text,
            &selection,
            edit_offset,
            self.replace_mode,
            snippet,
            &mut snippet_ctx,
          );
          return Some(CompletionApplyPlan {
            transaction,
            snippet: Some(rendered_snippet),
            trigger_signature_help: should_trigger_signature_help,
          });
        },
        Err(err) => {
          log::error!("Failed to parse snippet from completion: {}", err);
        },
      }
    }

    let transaction = generate_transaction_from_completion_edit(
      text,
      &selection,
      edit_offset,
      self.replace_mode,
      new_text,
    );
    Some(CompletionApplyPlan {
      transaction,
      snippet: None,
      trigger_signature_help: should_trigger_signature_help,
    })
  }
}

impl Component for Completion {
  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    use the_editor_renderer::Key;

    match (key.code, key.ctrl, key.alt, key.shift) {
      // Up - move up
      (Key::Up, ..) | (Key::Char('p'), true, ..) => {
        self.move_up(1);
        self.trigger_resolve();
        EventResult::Consumed(None)
      },
      // Down - move down
      (Key::Down, ..) | (Key::Char('n'), true, ..) => {
        self.move_down(1);
        self.trigger_resolve();
        EventResult::Consumed(None)
      },
      // PageUp - page up
      (Key::PageUp, ..) | (Key::Char('u'), true, ..) => {
        self.move_up(MAX_VISIBLE_ITEMS / 2);
        self.trigger_resolve();
        EventResult::Consumed(None)
      },
      // PageDown - page down
      (Key::PageDown, ..) | (Key::Char('d'), true, ..) => {
        self.move_down(MAX_VISIBLE_ITEMS / 2);
        self.trigger_resolve();
        EventResult::Consumed(None)
      },
      // Home - to start
      (Key::Home, ..) => {
        self.cursor = 0;
        self.scroll_offset = 0;
        self.doc_resolved = false;
        self.trigger_resolve();
        EventResult::Consumed(None)
      },
      // End - to end
      (Key::End, ..) => {
        if !self.filtered.is_empty() {
          self.cursor = self.filtered.len() - 1;
          self.ensure_cursor_in_view();
          self.doc_resolved = false;
          self.trigger_resolve();
        }
        EventResult::Consumed(None)
      },
      // Escape - don't consume, let editor handle mode switch
      // The editor_view will close completion and switch to normal mode
      (Key::Escape, ..) => EventResult::Ignored(None),
      // Ctrl+c - explicitly close completion without mode switch
      // Return a callback to signal we want to close (editor_view handles it)
      (Key::Char('c'), true, ..) => {
        EventResult::Consumed(Some(Box::new(|_compositor, _ctx| {
          // Empty callback - just signals completion should close
          // EditorView will set self.completion = None
        })))
      },
      // Enter / Tab - accept completion
      (Key::Enter | Key::NumpadEnter, ..) | (Key::Tab, _, _, false) => {
        if let Some(item) = self.selection() {
          // Apply the selected completion
          self.apply_completion(ctx, item);
        }
        // Return a callback to signal we want to close (editor_view handles it)
        EventResult::Consumed(Some(Box::new(|_compositor, _ctx| {
          // Empty callback - just signals completion should close
          // EditorView will set self.completion = None
        })))
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, ctx: &mut Context) {
    if self.filtered.is_empty() {
      return;
    }

    let font_state = surface.save_font_state();

    // Update animation with declarative system
    self.animation.update(ctx.dt);
    let eased_t = *self.animation.current();

    // Animation effects:
    // - Fade in (alpha)
    // - Slight upward slide
    // - Small scale (starts at 95%, grows to 100%)
    let alpha = eased_t;
    let slide_offset = (1.0 - eased_t) * 8.0; // Slide up 8px
    let scale = 0.95 + (eased_t * 0.05); // 95% -> 100%

    // Get theme colors
    let theme = &ctx.editor.theme;
    let bg_style = theme.get("ui.popup");
    let text_style = theme.get("ui.text");
    let selected_style = theme.get("ui.menu.selected");

    // Background colors stay opaque for solid appearance
    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));
    let selected_bg = selected_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.25, 0.3, 0.45, 1.0));

    // Text colors fade in with animation
    let mut text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
    let mut selected_fg = selected_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));

    // Apply animation alpha only to text
    text_color.a *= alpha;
    selected_fg.a *= alpha;

    // Calculate layout
    let visible_items = MAX_VISIBLE_ITEMS.min(self.filtered.len());
    let item_padding = 6.0;

    // Calculate cursor position using shared positioning utility
    let Some(cursor) = calculate_cursor_position(ctx, surface) else {
      return;
    };

    surface.configure_font(&font_state.family, UI_FONT_SIZE);
    let ui_char_width = surface.cell_width().max(UI_FONT_WIDTH.max(1.0));
    let ui_line_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);

    // First pass: find the longest label to determine kind column alignment
    let mut max_label_width: f32 = 0.0;
    for &(idx, _) in self.filtered.iter().take(20) {
      let item = &self.items[idx as usize];
      let label = match item {
        CompletionItem::Lsp(lsp_item) => &lsp_item.item.label,
        CompletionItem::Other(other) => &other.label,
      };
      let label_width = label.len() as f32 * ui_char_width;
      max_label_width = max_label_width.max(label_width);
    }

    // Second pass: determine menu width based on aligned layout
    let mut kind_column_offset = max_label_width + 20.0; // Extra spacing before kind
    let mut menu_width: f32 = 250.0; // minimum width
    for &(idx, _) in self.filtered.iter().take(20) {
      let item = &self.items[idx as usize];
      let kind = match item {
        CompletionItem::Lsp(lsp_item) => Self::format_kind(lsp_item.item.kind),
        CompletionItem::Other(other) => other.kind.as_deref().unwrap_or(""),
      };
      let item_width = kind_column_offset + (kind.len() as f32 * ui_char_width) + 16.0;
      menu_width = menu_width.max(item_width);
    }
    menu_width = menu_width.min(MAX_MENU_WIDTH as f32 * ui_char_width);
    let max_kind_offset = (menu_width - 32.0).max(0.0);
    if kind_column_offset > max_kind_offset {
      kind_column_offset = max_kind_offset;
    }

    let line_height = ui_line_height;
    let menu_height = (visible_items as f32 * line_height) + (item_padding * 2.0);

    // Get viewport dimensions for bounds checking
    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    // Position popup using shared positioning utility
    // Pass None for bias to maintain current behavior (choose side with more space)
    let popup_pos = position_popup_near_cursor(
      cursor,
      menu_width,
      menu_height,
      viewport_width,
      viewport_height,
      slide_offset,
      scale,
      None,
    );

    // Apply animation transforms
    let anim_width = menu_width * scale;
    let anim_height = menu_height * scale;
    let anim_x = popup_pos.x;
    let anim_y = popup_pos.y;

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

    // Draw border (keep it opaque)
    let border_color = Color::new(0.3, 0.3, 0.35, 0.8);
    surface.draw_rounded_rect_stroke(
      anim_x,
      anim_y,
      anim_width,
      anim_height,
      corner_radius,
      1.0,
      border_color,
    );

    // Render items (using animated transforms)
    surface.with_overlay_region(anim_x, anim_y, anim_width, anim_height, |surface| {
      surface.push_scissor_rect(anim_x, anim_y, anim_width, anim_height);

      let visible_range = self.scroll_offset..self.scroll_offset + visible_items;
      for (row, &(idx, _score)) in self.filtered[visible_range.clone()].iter().enumerate() {
        let item = &self.items[idx as usize];
        let is_selected = self.scroll_offset + row == self.cursor;

        let (label, kind, deprecated) = match item {
          CompletionItem::Lsp(lsp_item) => {
            (
              lsp_item.item.label.as_str(),
              Self::format_kind(lsp_item.item.kind),
              Self::is_deprecated(&lsp_item.item),
            )
          },
          CompletionItem::Other(other) => {
            (
              other.label.as_str(),
              other.kind.as_deref().unwrap_or(""),
              false,
            )
          },
        };

        let item_y = anim_y + item_padding + (row as f32 * line_height * scale);

        // Draw selection background
        if is_selected {
          surface.draw_rect(
            anim_x + 4.0 * scale,
            item_y - 2.0 * scale,
            anim_width - 8.0 * scale,
            line_height * scale,
            selected_bg,
          );
        }

        // Render label and kind
        let label_color = if is_selected {
          selected_fg
        } else if deprecated {
          let mut gray = Color::new(0.5, 0.5, 0.5, 1.0);
          gray.a *= alpha;
          gray
        } else {
          text_color
        };

        let kind_color = if is_selected {
          let mut c = Color::new(
            selected_fg.r * 0.7,
            selected_fg.g * 0.7,
            selected_fg.b * 0.7,
            1.0,
          );
          c.a *= alpha;
          c
        } else {
          let mut c = Color::new(0.6, 0.6, 0.7, 1.0);
          c.a *= alpha;
          c
        };

        let available_label_width = (kind_column_offset - 12.0).max(0.0);
        let label_text = truncate_to_width(label, available_label_width, ui_char_width);
        let available_kind_width = (menu_width - kind_column_offset - 16.0).max(0.0);
        let kind_text = truncate_to_width(kind, available_kind_width, ui_char_width);

        // Draw label
        surface.draw_text(TextSection {
          position: (anim_x + 8.0 * scale, item_y),
          texts:    vec![TextSegment {
            content: label_text,
            style:   TextStyle {
              size:  UI_FONT_SIZE * scale,
              color: label_color,
            },
          }],
        });

        // Draw kind at aligned column
        surface.draw_text(TextSection {
          position: (anim_x + 8.0 * scale + kind_column_offset * scale, item_y),
          texts:    vec![TextSegment {
            content: kind_text,
            style:   TextStyle {
              size:  UI_FONT_SIZE * scale,
              color: kind_color,
            },
          }],
        });
      }

      surface.pop_scissor_rect();
    });

    // Render documentation panel for selected item
    if let Some(selected_item) = self.selection() {
      self.render_documentation(
        selected_item,
        anim_x,
        anim_y,
        anim_width,
        anim_height,
        alpha,
        ui_char_width,
        line_height,
        surface,
        ctx,
      );
    }

    surface.restore_font_state(font_state);
  }

  fn cursor(&self, _area: Rect, _editor: &crate::Editor) -> (Option<Position>, CursorKind) {
    // No cursor for completion popup
    (None, CursorKind::Hidden)
  }

  fn should_update(&self) -> bool {
    true
  }

  fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
    if self.filtered.is_empty() {
      return Some((0, 0));
    }

    let visible_items = MAX_VISIBLE_ITEMS.min(self.filtered.len());
    let height = visible_items as u16 + 2;
    let width = MAX_MENU_WIDTH;

    Some((width, height))
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete()
  }
}
