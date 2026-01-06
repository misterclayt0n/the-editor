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
    components::popup::{
      DOC_POPUP_MAX_HEIGHT_LINES,
      DOC_POPUP_MAX_WIDTH_CHARS,
      DOC_POPUP_MIN_WIDTH_CHARS,
    },
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

fn truncate_to_width(text: &str, max_width: f32, char_width: f32) -> String {
  if max_width <= 0.0 {
    return String::new();
  }

  let char_width = char_width.max(1.0);
  let max_chars = (max_width / char_width).floor() as usize;
  if max_chars == 0 {
    return String::new();
  }

  let count = text.chars().count();
  if count <= max_chars {
    return text.to_string();
  }

  if max_chars == 1 {
    return "…".to_string();
  }

  let mut truncated: String = text.chars().take(max_chars - 1).collect();
  truncated.push('…');
  truncated
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

  /// Update the completion items (for progressive loading from LSP)
  /// This merges new items with existing ones, re-deduplicating by label and
  /// provider
  pub fn update_items(&mut self, new_items: Vec<CompletionItem>) {
    use std::collections::HashSet;

    // Build a set of existing item keys (label + provider) for deduplication
    let mut seen: HashSet<(String, CompletionProvider)> = self
      .items
      .iter()
      .map(|item| {
        let label = match item {
          CompletionItem::Lsp(lsp) => lsp.item.label.clone(),
          CompletionItem::Other(other) => other.label.clone(),
        };
        (label, item.provider())
      })
      .collect();

    // Add new items that aren't duplicates
    for item in new_items {
      let label = match &item {
        CompletionItem::Lsp(lsp) => lsp.item.label.clone(),
        CompletionItem::Other(other) => other.label.clone(),
      };
      let key = (label, item.provider());

      if !seen.contains(&key) {
        seen.insert(key);
        self.items.push(item);
      }
    }

    // Re-score with updated items
    self.score(false);

    // Mark docs as needing resolution again
    self.doc_resolved = false;
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

    // Get current document's language for syntax highlighting the detail
    let (_view, current_doc) = crate::current!(ctx.editor);
    let language = current_doc.language_name().unwrap_or("");

    // Combine detail and doc like Helix does:
    // - detail gets wrapped in a code block with the current language
    // - doc is appended as-is (it's already markdown from the LSP)
    let markdown_content = match (detail, doc) {
      (Some(detail), Some(doc)) => format!("```{}\n{}\n```\n{}", language, detail, doc),
      (Some(detail), None) => format!("```{}\n{}\n```", language, detail),
      (None, Some(doc)) => doc.to_string(),
      (None, None) => return,
    };

    // Get window dimensions
    let window_width = surface.width() as f32;
    let window_height = surface.height() as f32;

    // Constants for doc popup sizing (matching PopupShell's defaults)
    const DOC_PADDING: f32 = 12.0;
    const SPACING: f32 = 8.0;

    let line_height = ui_line_height.max(UI_FONT_SIZE + 4.0);

    // Calculate available space on each side
    let space_on_right = window_width - (completion_x + completion_width);
    let space_on_left = completion_x;
    let space_below = window_height - (completion_y + completion_height);

    // Convert shared constants to pixels (these are OUTER dimensions, matching
    // PopupShell) The char/line counts represent the outer popup size, not
    // inner content area
    let min_width_px = DOC_POPUP_MIN_WIDTH_CHARS as f32 * ui_char_width;
    let max_width_px = DOC_POPUP_MAX_WIDTH_CHARS as f32 * ui_char_width;
    let max_height_px = DOC_POPUP_MAX_HEIGHT_LINES as f32 * line_height;

    // Determine placement side and calculate max available dimensions
    enum Placement {
      Right { x: f32, y: f32 },
      Left { y: f32 },
      Below { x: f32, y: f32 },
    }

    let (placement, max_avail_width, max_avail_height) = if space_on_right >= min_width_px + SPACING
    {
      // Position to the right
      let x = completion_x + completion_width + SPACING;
      let y = completion_y;
      let avail_w = (space_on_right - SPACING).min(max_width_px);
      let avail_h = (window_height - y).min(max_height_px);
      (Placement::Right { x, y }, avail_w, avail_h)
    } else if space_on_left >= min_width_px + SPACING {
      // Position to the left
      let y = completion_y;
      let avail_w = (space_on_left - SPACING).min(max_width_px);
      let avail_h = (window_height - y).min(max_height_px);
      (Placement::Left { y }, avail_w, avail_h)
    } else if space_below >= line_height * 6.0 + SPACING {
      // Position below completion
      let x = completion_x;
      let y = completion_y + completion_height + SPACING;
      let avail_w = (window_width - x).min(max_width_px);
      let avail_h = (space_below - SPACING).min(max_height_px);
      (Placement::Below { x, y }, avail_w, avail_h)
    } else {
      // Not enough space anywhere
      return;
    };

    // Calculate inner content area (excluding padding)
    let inner_max_width = (max_avail_width - DOC_PADDING * 2.0).max(0.0);
    let inner_max_height = (max_avail_height - DOC_PADDING * 2.0).max(0.0);

    // Convert to cells for wrapping
    let max_width_cells = (inner_max_width / ui_char_width).floor().max(4.0) as u16;

    // Wrap content at max available width
    let line_groups =
      super::markdown::build_markdown_lines_cells(&markdown_content, max_width_cells, ctx);

    if line_groups.is_empty() {
      return;
    }

    // Measure actual content dimensions from wrapped lines (like hover does)
    let visible_lines = line_groups.len().min(DOC_POPUP_MAX_HEIGHT_LINES as usize);
    let content_width_cells = line_groups
      .iter()
      .take(visible_lines)
      .map(|segments| super::markdown::line_width_cells(segments))
      .max()
      .unwrap_or(0);

    // Apply min/max constraints to content width
    let min_width_cells = DOC_POPUP_MIN_WIDTH_CHARS.min(max_width_cells);
    let final_width_cells = content_width_cells
      .max(min_width_cells)
      .min(max_width_cells);
    let content_width = final_width_cells as f32 * ui_char_width;

    // Calculate final popup dimensions
    let popup_width = (content_width + DOC_PADDING * 2.0).min(max_avail_width);
    let content_height = (visible_lines as f32 * line_height).min(inner_max_height);
    let popup_height = content_height + DOC_PADDING * 2.0;

    // Calculate final position based on placement
    let (doc_x, doc_y) = match placement {
      Placement::Right { x, y } => (x, y),
      Placement::Left { y } => {
        // For left placement, position so right edge is at completion_x - SPACING
        let x = completion_x - popup_width - SPACING;
        (x.max(0.0), y)
      },
      Placement::Below { x, y } => (x, y),
    };

    // Final safety check - ensure we're within viewport
    if doc_x < 0.0
      || doc_y < 0.0
      || doc_x + popup_width > window_width
      || doc_y + popup_height > window_height
      || popup_width < min_width_px
      || popup_height < line_height * 3.0
    {
      return;
    }

    // Get theme colors
    let theme = &ctx.editor.theme;
    let bg_style = theme.get("ui.popup");

    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));

    surface.with_overlay_region(doc_x, doc_y, popup_width, popup_height, |surface| {
      // Draw background
      let corner_radius = 6.0;
      surface.draw_rounded_rect(
        doc_x,
        doc_y,
        popup_width,
        popup_height,
        corner_radius,
        bg_color,
      );

      // Draw border
      let border_color = Color::new(0.3, 0.3, 0.35, 0.8);
      surface.draw_rounded_rect_stroke(
        doc_x,
        doc_y,
        popup_width,
        popup_height,
        corner_radius,
        1.0,
        border_color,
      );

      // Render documentation content
      let text_x = doc_x + DOC_PADDING;
      let mut text_y = doc_y + DOC_PADDING;
      let max_text_y = doc_y + popup_height - DOC_PADDING;

      surface.push_scissor_rect(doc_x, doc_y, popup_width, popup_height);

      for segments in line_groups.into_iter().take(visible_lines) {
        if text_y > max_text_y {
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
          position: (text_x, text_y),
          texts,
        });
        text_y += line_height;
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
    // Use actual measured cell width, matching PopupShell's approach
    // Do NOT use UI_FONT_WIDTH as fallback - it causes size mismatch with hover
    let ui_char_width = surface.cell_width().max(1.0);
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
    // min_y is the bufferline height (top boundary where popups cannot be placed)
    let min_y = ctx.editor.viewport_pixel_offset.1;
    let popup_pos = position_popup_near_cursor(
      cursor,
      menu_width,
      menu_height,
      viewport_width,
      viewport_height,
      min_y,
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
