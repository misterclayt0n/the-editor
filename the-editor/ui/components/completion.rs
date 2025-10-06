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
  handlers::completion::{
    CompletionItem,
    CompletionProvider,
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
  /// Animation progress (0.0 = just appeared, 1.0 = fully visible)
  anim_progress:   f32,
}

impl Completion {
  pub const ID: &'static str = "completion";

  /// Create a new completion popup
  pub fn new(items: Vec<CompletionItem>, trigger_offset: usize, filter: String) -> Self {
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
      anim_progress: 0.0, // Start animation from 0
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
      }
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
          }
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
    self.filtered.get(self.cursor).map(|&(idx, _)| &self.items[idx as usize])
  }

  /// Get the currently selected completion item mutably
  pub fn selection_mut(&mut self) -> Option<&mut CompletionItem> {
    self.filtered.get(self.cursor).map(|&(idx, _)| &mut self.items[idx as usize])
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
  pub fn replace_provider_items(&mut self, provider: CompletionProvider, new_items: Vec<CompletionItem>) {
    // Remove old items from this provider
    self.items.retain(|item| item.provider() != provider);

    // Add new items
    self.items.extend(new_items);

    // Re-score
    self.score(false);
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
      || item
        .tags
        .as_ref()
        .map_or(false, |tags| tags.contains(&lsp::CompletionItemTag::DEPRECATED))
  }

  /// Apply the selected completion item
  fn apply_completion(&self, ctx: &mut Context, item: &CompletionItem) {
    use crate::core::transaction::Transaction;
    use crate::lsp::util::lsp_pos_to_pos;
    use the_editor_lsp_types::types as lsp;

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
        // because the user may have typed more characters while the completion was pending
        let cursor = doc.selection(view.id).primary().cursor(doc.text().slice(..));

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
            let text = lsp_item.item.insert_text.as_ref().unwrap_or(&lsp_item.item.label);
            (start, cursor, text.clone())
          },
        };

        // Create and apply main completion transaction
        let transaction = Transaction::change(doc.text(), [(start, end, Some(text.into()))].iter().cloned());
        doc.apply(&transaction, view.id);

        // Apply additional text edits (e.g., auto-imports)
        if let Some(ref additional_edits) = lsp_item.item.additional_text_edits {
          if !additional_edits.is_empty() {
            log::info!("Applying {} additional text edits for auto-import", additional_edits.len());

            // Convert LSP text edits to transaction
            let text = doc.text();
            let mut changes = Vec::new();

            for edit in additional_edits {
              let start = lsp_pos_to_pos(text, edit.range.start, offset_encoding)
                .unwrap_or_else(|| {
                  log::error!("Invalid additional edit start position");
                  0
                });
              let end = lsp_pos_to_pos(text, edit.range.end, offset_encoding)
                .unwrap_or_else(|| {
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
        let cursor = doc.selection(view.id).primary().cursor(doc.text().slice(..));
        let start = self.trigger_offset;
        let end = cursor;

        let transaction = Transaction::change(doc.text(), [(start, end, Some(other.label.clone().into()))].iter().cloned());
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
      (Key::Up, _, _, _) | (Key::Char('p'), true, _, _) => {
        self.move_up(1);
        EventResult::Consumed(None)
      }
      // Down - move down
      (Key::Down, _, _, _) | (Key::Char('n'), true, _, _) => {
        self.move_down(1);
        EventResult::Consumed(None)
      }
      // PageUp - page up
      (Key::PageUp, _, _, _) | (Key::Char('u'), true, _, _) => {
        self.move_up(MAX_VISIBLE_ITEMS / 2);
        EventResult::Consumed(None)
      }
      // PageDown - page down
      (Key::PageDown, _, _, _) | (Key::Char('d'), true, _, _) => {
        self.move_down(MAX_VISIBLE_ITEMS / 2);
        EventResult::Consumed(None)
      }
      // Home - to start
      (Key::Home, _, _, _) => {
        self.cursor = 0;
        self.scroll_offset = 0;
        self.doc_resolved = false;
        EventResult::Consumed(None)
      }
      // End - to end
      (Key::End, _, _, _) => {
        if !self.filtered.is_empty() {
          self.cursor = self.filtered.len() - 1;
          self.ensure_cursor_in_view();
          self.doc_resolved = false;
        }
        EventResult::Consumed(None)
      }
      // Escape - don't consume, let editor handle mode switch
      // The editor_view will close completion and switch to normal mode
      (Key::Escape, _, _, _) => EventResult::Ignored(None),
      // Ctrl+c - explicitly close completion without mode switch
      // Return a callback to signal we want to close (editor_view handles it)
      (Key::Char('c'), true, _, _) => {
        EventResult::Consumed(Some(Box::new(|_compositor, _ctx| {
          // Empty callback - just signals completion should close
          // EditorView will set self.completion = None
        })))
      }
      // Enter / Tab - accept completion
      (Key::Enter, _, _, _) | (Key::Tab, _, _, false) => {
        if let Some(item) = self.selection() {
          // Apply the selected completion
          self.apply_completion(ctx, item);
        }
        // Return a callback to signal we want to close (editor_view handles it)
        EventResult::Consumed(Some(Box::new(|_compositor, _ctx| {
          // Empty callback - just signals completion should close
          // EditorView will set self.completion = None
        })))
      }
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, ctx: &mut Context) {
    if self.filtered.is_empty() {
      return;
    }

    // Update animation progress (fast lerp, completes in ~0.1s)
    const ANIM_SPEED: f32 = 30.0; // Higher = faster
    if self.anim_progress < 1.0 {
      self.anim_progress = (self.anim_progress + ctx.dt * ANIM_SPEED).min(1.0);
    }

    // Smoothstep easing for smooth animation
    let t = self.anim_progress;
    let eased_t = t * t * (3.0 - 2.0 * t);

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

    let mut bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 0.98));
    let mut text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
    let mut selected_bg = selected_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.25, 0.3, 0.45, 1.0));
    let mut selected_fg = selected_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));

    // Apply animation alpha to all colors
    bg_color.a *= alpha;
    text_color.a *= alpha;
    selected_bg.a *= alpha;
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

    // Calculate cursor position from trigger_offset
    // For now, use current cursor position as approximation
    // TODO: Calculate exact position based on trigger_offset when cursor has moved
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
      let gutter_width = 6; // Approximate gutter width
      let gutter_offset = gutter_width as f32 * font_width;

      const VIEW_PADDING_LEFT: f32 = 10.0;
      const VIEW_PADDING_TOP: f32 = 10.0;
      const LINE_SPACING: f32 = 2.0;

      let base_x = VIEW_PADDING_LEFT + gutter_offset;
      let base_y = VIEW_PADDING_TOP;

      let rel_row = line.saturating_sub(anchor_line);
      let x = base_x + (col as f32) * font_width;
      // Position below the cursor line
      let y = base_y + (rel_row as f32) * (font_size + LINE_SPACING) + font_size + LINE_SPACING;

      (x, y)
    };

    // Apply animation transforms
    let anim_y = cursor_y + slide_offset;
    let anim_width = menu_width * scale;
    let anim_height = menu_height * scale;
    // Center the scaled popup at the cursor position
    let anim_x = cursor_x - (menu_width - anim_width) / 2.0;

    // Draw background
    let corner_radius = 6.0;
    surface.draw_rounded_rect(anim_x, anim_y, anim_width, anim_height, corner_radius, bg_color);

    // Draw border (with animated alpha)
    let mut border_color = Color::new(0.3, 0.3, 0.35, 0.8);
    border_color.a *= alpha;
    surface.draw_rounded_rect_stroke(anim_x, anim_y, anim_width, anim_height, corner_radius, 1.0, border_color);

    // Render items (using animated transforms)
    surface.with_overlay_region(anim_x, anim_y, anim_width, anim_height, |surface| {
      let visible_range = self.scroll_offset..self.scroll_offset + visible_items;
      for (row, &(idx, _score)) in self.filtered[visible_range.clone()].iter().enumerate() {
        let item = &self.items[idx as usize];
        let is_selected = self.scroll_offset + row == self.cursor;

        let (label, kind, deprecated) = match item {
          CompletionItem::Lsp(lsp_item) => (
            lsp_item.item.label.as_str(),
            Self::format_kind(lsp_item.item.kind),
            Self::is_deprecated(&lsp_item.item),
          ),
          CompletionItem::Other(other) => (other.label.as_str(), other.kind.as_deref().unwrap_or(""), false),
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
          let mut c = Color::new(selected_fg.r * 0.7, selected_fg.g * 0.7, selected_fg.b * 0.7, 1.0);
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

    // TODO: Render documentation panel if there's room and item is selected
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
}
