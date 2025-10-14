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
use the_editor_lsp_types::types as lsp;
use the_editor_renderer::{
  Color,
  TextSection,
  TextSegment,
  TextStyle,
};

use crate::{
  core::{
    document::SavePoint,
    graphics::{
      CursorKind,
      Rect,
    },
    position::Position,
  },
  handlers::{
    completion::{
      CompletionItem,
      CompletionProvider,
      LspCompletionItem,
    },
    completion_resolve::ResolveHandler,
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
  },
};

/// Minimum width for documentation preview panel
const MIN_DOC_WIDTH: u16 = 30;

/// Maximum width for completion menu
const MAX_MENU_WIDTH: u16 = 60;

/// Maximum visible completion items
const MAX_VISIBLE_ITEMS: usize = 15;

/// Simple text wrapping function
/// Strip simple snippet syntax from completion text and return cursor offset
/// This is a temporary solution until we have full snippet support
/// Handles patterns like:
/// - ${1:} -> ("", cursor at position)
/// - ${1:text} -> ("text", cursor after text)
/// - $1 -> ("", cursor at position)
/// - Println(${1:}) -> ("Println()", cursor between parens)
/// Returns (stripped_text, cursor_offset_from_start)
fn strip_snippet_syntax(text: &str) -> (String, Option<usize>) {
  let mut result = String::with_capacity(text.len());
  let mut chars = text.chars().peekable();
  let mut first_tabstop_pos = None;

  while let Some(ch) = chars.next() {
    if ch == '$' {
      // Check if this is a snippet placeholder
      if chars.peek() == Some(&'{') {
        chars.next(); // consume '{'

        // Parse the tabstop number
        let mut tabstop_num = String::new();
        while let Some(&c) = chars.peek() {
          if c.is_ascii_digit() {
            tabstop_num.push(c);
            chars.next();
          } else {
            break;
          }
        }

        // Check for ':' which indicates default text
        if chars.peek() == Some(&':') {
          chars.next(); // consume ':'

          // Remember position of first tabstop ($1 or ${1:...})
          if first_tabstop_pos.is_none() && (tabstop_num == "1" || tabstop_num == "0") {
            first_tabstop_pos = Some(result.len());
          }

          // Collect text until '}'
          let mut depth = 1;
          while let Some(c) = chars.next() {
            if c == '{' {
              depth += 1;
              result.push(c);
            } else if c == '}' {
              depth -= 1;
              if depth == 0 {
                break;
              }
              result.push(c);
            } else {
              result.push(c);
            }
          }
        } else if chars.peek() == Some(&'}') {
          chars.next(); // consume '}'

          // Remember position of first tabstop
          if first_tabstop_pos.is_none() && (tabstop_num == "1" || tabstop_num == "0") {
            first_tabstop_pos = Some(result.len());
          }
        }
      } else {
        // $1 style - skip the number but remember position
        let mut tabstop_num = String::new();
        while let Some(&c) = chars.peek() {
          if c.is_ascii_digit() {
            tabstop_num.push(c);
            chars.next();
          } else {
            break;
          }
        }

        if first_tabstop_pos.is_none() && (tabstop_num == "1" || tabstop_num == "0") {
          first_tabstop_pos = Some(result.len());
        }
      }
    } else {
      result.push(ch);
    }
  }

  (result, first_tabstop_pos)
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
  let mut lines = Vec::new();
  let mut current_line = String::new();
  let mut current_width = 0;

  for word in text.split_whitespace() {
    let word_len = word.len();

    if current_width + word_len + 1 > max_width && !current_line.is_empty() {
      // Start new line
      lines.push(current_line);
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

    let mut matcher = nucleo::Matcher::new(Config::DEFAULT.match_paths());
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
      // Position to the right - available space is from completion edge to window edge
      let available_width = space_on_right - SPACING;
      let doc_width = available_width.min(MAX_DOC_WIDTH);
      let doc_x = completion_x + completion_width + SPACING;
      let doc_y = completion_y;
      let doc_height = completion_height.max(MIN_DOC_HEIGHT).min(window_height - doc_y);
      (doc_x, doc_y, doc_width, doc_height)
    } else if space_on_left >= MIN_DOC_WIDTH + SPACING {
      // Position to the left
      let available_width = space_on_left - SPACING;
      let doc_width = available_width.min(MAX_DOC_WIDTH);
      let doc_x = completion_x - doc_width - SPACING;
      let doc_y = completion_y;
      let doc_height = completion_height.max(MIN_DOC_HEIGHT).min(window_height - doc_y);
      (doc_x, doc_y, doc_width, doc_height)
    } else if space_below >= MIN_DOC_HEIGHT + SPACING {
      // Position below completion
      let doc_x = completion_x;
      let doc_y = completion_y + completion_height + SPACING;
      let doc_width = completion_width.max(MIN_DOC_WIDTH).min(MAX_DOC_WIDTH).min(window_width - doc_x);
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
    text_color.a *= alpha;

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
      let line_height = font_size + 4.0;

      // Render detail (in a code-like style if present)
      if let Some(detail_text) = detail {
        let mut detail_color = Color::new(0.7, 0.8, 0.9, 1.0);
        detail_color.a *= alpha;

        surface.draw_text(TextSection {
          position: (doc_x + DOC_PADDING, y_offset),
          texts:    vec![TextSegment {
            content: detail_text.to_string(),
            style:   TextStyle {
              size:  font_size,
              color: detail_color,
            },
          }],
        });
        y_offset += line_height * 2.0; // Extra spacing after detail
      }

      // Render documentation text (wrapped)
      if let Some(doc_text) = doc {
        // Simple line wrapping - split into words and wrap at doc_width
        let max_chars_per_line = ((doc_width - DOC_PADDING * 2.0) / (font_size * 0.6)) as usize;
        let lines = wrap_text(doc_text, max_chars_per_line);

        for line in lines
          .iter()
          .take(((doc_height - y_offset + doc_y - DOC_PADDING) / line_height) as usize)
        {
          surface.draw_text(TextSection {
            position: (doc_x + DOC_PADDING, y_offset),
            texts:    vec![TextSegment {
              content: line.to_string(),
              style:   TextStyle {
                size:  font_size,
                color: text_color,
              },
            }],
          });
          y_offset += line_height;
        }
      }
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
  fn apply_completion(&self, ctx: &mut Context, item: &CompletionItem) {
    use the_editor_lsp_types::types as lsp;

    use crate::{
      core::transaction::Transaction,
      lsp::util::lsp_pos_to_pos,
    };

    // For LSP items, get the offset encoding before borrowing editor
    let offset_encoding = match item {
      CompletionItem::Lsp(lsp_item) => {
        let language_server = match ctx.editor.language_server_by_id(lsp_item.provider) {
          Some(ls) => ls,
          None => {
            log::error!("Language server not found for completion");
            return;
          },
        };
        Some(language_server.offset_encoding())
      },
      CompletionItem::Other(_) => None,
    };

    let (view, doc) = crate::current!(ctx.editor);

    match item {
      CompletionItem::Lsp(lsp_item) => {
        let offset_encoding = offset_encoding.unwrap(); // We know it's Some from above

        // Get the text edit from the LSP item
        // IMPORTANT: Use current cursor position as end, not the LSP's range end,
        // because the user may have typed more characters while the completion was
        // pending
        let cursor = doc
          .selection(view.id)
          .primary()
          .cursor(doc.text().slice(..));

        let (start, end, text) = match &lsp_item.item.text_edit {
          Some(lsp::CompletionTextEdit::Edit(edit)) => {
            // Use the LSP-provided start position, but extend end to current cursor
            let start = lsp_pos_to_pos(doc.text(), edit.range.start, offset_encoding)
              .unwrap_or_else(|| {
                log::error!("Invalid LSP edit start position");
                self.trigger_offset
              });
            // Use cursor position to capture any characters typed while waiting
            (start, cursor, edit.new_text.clone())
          },
          Some(lsp::CompletionTextEdit::InsertAndReplace(edit)) => {
            // Use the insert range start, but extend end to current cursor
            let start = lsp_pos_to_pos(doc.text(), edit.insert.start, offset_encoding)
              .unwrap_or_else(|| {
                log::error!("Invalid LSP edit start position");
                self.trigger_offset
              });
            (start, cursor, edit.new_text.clone())
          },
          None => {
            // No text edit provided, fall back to inserting from trigger_offset to cursor
            let start = self.trigger_offset;
            let text = lsp_item
              .item
              .insert_text
              .as_ref()
              .unwrap_or(&lsp_item.item.label);
            (start, cursor, text.clone())
          },
        };

        // Check if this is a snippet that needs to be processed
        let (final_text, cursor_offset) = if matches!(
          lsp_item.item.insert_text_format,
          Some(lsp::InsertTextFormat::SNIPPET)
        ) {
          // For now, do a simple strip of snippet syntax since we don't have full snippet
          // support yet This handles common cases like ${1:} -> empty string,
          // ${1:text} -> text
          strip_snippet_syntax(&text)
        } else {
          (text, None)
        };

        // Check if we should trigger signature help after completion
        let should_trigger_signature_help = cursor_offset.is_some() && final_text.contains('(');

        // Create and apply main completion transaction
        let transaction = Transaction::change(
          doc.text(),
          [(start, end, Some(final_text.into()))].iter().cloned(),
        );
        doc.apply(&transaction, view.id);

        // If snippet had a cursor position, move cursor there
        if let Some(offset) = cursor_offset {
          let cursor_pos = start + offset;
          let selection = crate::core::selection::Selection::point(cursor_pos);
          doc.set_selection(view.id, selection);
        }

        // If we moved the cursor and the completion text contains '(', trigger
        // signature help
        if should_trigger_signature_help {
          use the_editor_event::send_blocking;

          use crate::handlers::lsp::SignatureHelpEvent;
          send_blocking(
            &ctx.editor.handlers.signature_hints,
            SignatureHelpEvent::Trigger,
          );
        }

        // Apply additional text edits (e.g., auto-imports)
        if let Some(ref additional_edits) = lsp_item.item.additional_text_edits {
          if !additional_edits.is_empty() {
            log::info!(
              "Applying {} additional text edits for auto-import",
              additional_edits.len()
            );

            // Convert LSP text edits to transaction
            let text = doc.text();
            let mut changes = Vec::new();

            for edit in additional_edits {
              let start =
                lsp_pos_to_pos(text, edit.range.start, offset_encoding).unwrap_or_else(|| {
                  log::error!("Invalid additional edit start position");
                  0
                });
              let end =
                lsp_pos_to_pos(text, edit.range.end, offset_encoding).unwrap_or_else(|| {
                  log::error!("Invalid additional edit end position");
                  start
                });

              changes.push((start, end, Some(edit.new_text.clone().into())));
            }

            // Apply all additional edits as a single transaction
            let additional_transaction = Transaction::change(doc.text(), changes.iter().cloned());
            doc.apply(&additional_transaction, view.id);
          }
        }
      },
      CompletionItem::Other(other) => {
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
        doc.apply(&transaction, view.id);
      },
    }

    // Save to history
    doc.append_changes_to_history(view);
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
      (Key::Enter, ..) | (Key::Tab, _, _, false) => {
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
    let line_height = UI_FONT_SIZE + 4.0;
    let item_padding = 6.0;
    let menu_height = (visible_items as f32 * line_height) + (item_padding * 2.0);

    // First pass: find the longest label to determine kind column alignment
    let mut max_label_width: f32 = 0.0;
    for &(idx, _) in self.filtered.iter().take(20) {
      let item = &self.items[idx as usize];
      let label = match item {
        CompletionItem::Lsp(lsp_item) => &lsp_item.item.label,
        CompletionItem::Other(other) => &other.label,
      };
      let label_width = label.len() as f32 * UI_FONT_WIDTH;
      max_label_width = max_label_width.max(label_width);
    }

    // Second pass: determine menu width based on aligned layout
    let kind_column_offset = max_label_width + 20.0; // Extra spacing before kind
    let mut menu_width: f32 = 250.0; // minimum width
    for &(idx, _) in self.filtered.iter().take(20) {
      let item = &self.items[idx as usize];
      let kind = match item {
        CompletionItem::Lsp(lsp_item) => Self::format_kind(lsp_item.item.kind),
        CompletionItem::Other(other) => other.kind.as_deref().unwrap_or(""),
      };
      let item_width = kind_column_offset + (kind.len() as f32 * UI_FONT_WIDTH) + 16.0;
      menu_width = menu_width.max(item_width);
    }
    menu_width = menu_width.min(MAX_MENU_WIDTH as f32 * UI_FONT_WIDTH);

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
      let y = view_y + (rel_row as f32 * line_height) + line_height;

      (x, y)
    };

    // Get viewport dimensions for bounds checking
    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    // Apply animation transforms
    let anim_width = menu_width * scale;
    let anim_height = menu_height * scale;

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
    let mut popup_x = cursor_x - (menu_width - anim_width) / 2.0;

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

        // Draw label
        surface.draw_text(TextSection {
          position: (anim_x + 8.0 * scale, item_y),
          texts:    vec![TextSegment {
            content: label.to_string(),
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
            content: kind.to_string(),
            style:   TextStyle {
              size:  UI_FONT_SIZE * scale,
              color: kind_color,
            },
          }],
        });
      }
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
        surface,
        ctx,
      );
    }
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
