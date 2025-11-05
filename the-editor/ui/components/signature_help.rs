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

/// Maximum width for the signature help popup
const MAX_POPUP_WIDTH: usize = 80; // characters
const MIN_POPUP_CHARS: usize = 24;

/// Signature help popup component
pub struct SignatureHelp {
  /// Language for syntax highlighting
  language:         String,
  /// Active signature index
  active_signature: usize,
  /// All available signatures
  signatures:       Vec<crate::handlers::signature_help::Signature>,
  /// Appearance animation
  animation:        crate::core::animation::AnimationHandle<f32>,
  /// Whether the popup is visible
  visible:          bool,
}

impl SignatureHelp {
  pub const ID: &'static str = "signature-help";

  pub fn new(
    language: String,
    active_signature: usize,
    signatures: Vec<crate::handlers::signature_help::Signature>,
  ) -> Self {
    // Create appearance animation using popup preset
    let (duration, easing) = crate::core::animation::presets::POPUP;
    let animation = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);

    Self {
      language,
      active_signature,
      signatures,
      animation,
      visible: true,
    }
  }

  pub fn update(
    &mut self,
    language: String,
    active_signature: usize,
    signatures: Vec<crate::handlers::signature_help::Signature>,
  ) {
    self.language = language;
    self.active_signature = active_signature.min(signatures.len().saturating_sub(1));
    self.signatures = signatures;
    // Reset animation if signatures changed (quick re-animation from 80%)
    if self.animation.is_complete() {
      let (duration, easing) = crate::core::animation::presets::FAST;
      self.animation = crate::core::animation::AnimationHandle::new(0.8, 1.0, duration, easing);
    }
  }

  fn signature_index(&self) -> Option<String> {
    if self.signatures.len() > 1 {
      Some(format!(
        "({}/{})",
        self.active_signature + 1,
        self.signatures.len()
      ))
    } else {
      None
    }
  }

  fn current_signature(&self) -> &crate::handlers::signature_help::Signature {
    let idx = self
      .active_signature
      .min(self.signatures.len().saturating_sub(1));
    &self.signatures[idx]
  }
}

impl Component for SignatureHelp {
  fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    if self.signatures.len() <= 1 {
      return EventResult::Ignored(None);
    }

    use the_editor_renderer::Key;

    match (key.code, key.ctrl, key.alt, key.shift) {
      (Key::Char('p'), false, true, false) => {
        if self.active_signature == 0 {
          self.active_signature = self.signatures.len() - 1;
        } else {
          self.active_signature -= 1;
        }
        EventResult::Consumed(None)
      },
      (Key::Char('n'), false, true, false) => {
        self.active_signature = (self.active_signature + 1) % self.signatures.len();
        EventResult::Consumed(None)
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, ctx: &mut Context) {
    if self.signatures.is_empty() || !self.visible {
      return;
    }

    let doc_cell_w = surface.cell_width().max(1.0);

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

    // Get active signature
    let sig = self.current_signature();

    // Calculate popup dimensions
    let padding = 12.0;

    // Calculate fresh cursor position using document font metrics
    let cursor_position = {
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

      if rel_row >= view.inner_height() {
        None
      } else {
        // Get font metrics
        let font_size = ctx
          .editor
          .font_size_override
          .unwrap_or(ctx.editor.config().font_size);
        let font_width = doc_cell_w;
        const LINE_SPACING: f32 = 4.0;
        let line_height = font_size + LINE_SPACING;

        // Get view's screen offset (handles splits correctly)
        let inner = view.inner_area(doc);
        let view_x = inner.x as f32 * font_width;
        let view_y = inner.y as f32 * line_height;

        // Calculate final screen position
        let x = view_x + (screen_col as f32 * font_width);
        let line_top = view_y + (rel_row as f32 * line_height);

        Some((x, line_top, line_height))
      }
    };

    let Some((cursor_x, line_top, doc_line_height)) = cursor_position else {
      return;
    };

    let font_state = surface.save_font_state();

    surface.configure_font(&font_state.family, UI_FONT_SIZE);
    let ui_char_width = surface.cell_width().max(UI_FONT_WIDTH.max(1.0));
    let line_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);

    let signature_index = self.signature_index();
    let index_width_chars = signature_index
      .as_ref()
      .map(|idx| idx.chars().count())
      .unwrap_or(0);

    let sig_char_count = sig.signature.chars().count();
    let mut content_chars = sig_char_count.clamp(MIN_POPUP_CHARS, MAX_POPUP_WIDTH);
    content_chars = content_chars.max(index_width_chars.clamp(0, MAX_POPUP_WIDTH));

    if let Some(doc) = &sig.signature_doc {
      if let Some(max_doc_width) = wrap_doc_text(doc, MAX_POPUP_WIDTH)
        .iter()
        .map(|line| line.chars().count())
        .max()
      {
        content_chars = content_chars.max(max_doc_width.clamp(MIN_POPUP_CHARS, MAX_POPUP_WIDTH));
      }
    }

    let wrap_chars = content_chars.clamp(MIN_POPUP_CHARS, MAX_POPUP_WIDTH);
    let doc_lines = if let Some(doc) = &sig.signature_doc {
      wrap_doc_text(doc, wrap_chars)
    } else {
      Vec::new()
    };

    let content_width = content_chars as f32 * ui_char_width;
    let popup_width = (content_width + padding * 2.0).clamp(240.0, 800.0);

    let mut popup_height = padding * 2.0 + line_height;
    if !doc_lines.is_empty() {
      popup_height += 8.0 + doc_lines.len() as f32 * line_height;
    }

    // Get viewport dimensions for bounds checking
    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    // Apply animation transforms
    let anim_width = popup_width * scale;
    let anim_height = popup_height * scale;

    // Try to position above cursor first (signature help prefers above)
    let mut popup_y = line_top - popup_height - 4.0 - slide_offset;

    // Ensure popup fits within viewport
    if popup_y < 0.0 || popup_y + anim_height > viewport_height {
      // Try positioning below cursor instead
      let y_below = line_top + doc_line_height + 4.0 + slide_offset;
      if y_below >= 0.0 && y_below + anim_height <= viewport_height {
        popup_y = y_below;
      } else {
        // Not enough space in preferred positions, clamp to viewport
        popup_y = popup_y
          .max(0.0)
          .min((viewport_height - anim_height).max(0.0));
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

    // Render signature text with highlighted parameter
    surface.with_overlay_region(anim_x, anim_y, anim_width, anim_height, |surface| {
      let text_x = anim_x + padding;
      let text_y = anim_y + padding + UI_FONT_SIZE; // Add font size for baseline

      // Build a single section with multiple segments
      let mut section = TextSection {
        position: (text_x, text_y),
        texts:    Vec::new(),
      };

      if let Some((start, end)) = sig.active_param_range {
        // Split into: before | highlighted | after
        let before = &sig.signature[..start];
        let highlighted = &sig.signature[start..end];
        let after = &sig.signature[end..];

        if !before.is_empty() {
          section.texts.push(TextSegment {
            content: before.to_string(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          });
        }

        // For highlighted text, we can use a lighter color since we can't set
        // background
        section.texts.push(TextSegment {
          content: highlighted.to_string(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: Color::new(1.0, 1.0, 0.6, text_color.a), // Yellowish highlight
          },
        });

        if !after.is_empty() {
          section.texts.push(TextSegment {
            content: after.to_string(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          });
        }
      } else {
        // No parameter highlighting
        section.texts.push(TextSegment {
          content: sig.signature.clone(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: text_color,
          },
        });
      }

      surface.draw_text(section);

      if let Some(index_text) = signature_index {
        let index_width = index_text.chars().count() as f32 * ui_char_width;
        let index_x = (anim_x + anim_width - padding - index_width).max(text_x);
        surface.draw_text(TextSection {
          position: (index_x, text_y),
          texts:    vec![TextSegment {
            content: index_text,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          }],
        });
      }

      if !doc_lines.is_empty() {
        let mut doc_y = text_y + line_height + 8.0;
        for line in doc_lines {
          if doc_y > anim_y + anim_height - padding {
            break;
          }

          if line.is_empty() {
            doc_y += line_height;
            continue;
          }

          surface.draw_text(TextSection {
            position: (text_x, doc_y),
            texts:    vec![TextSegment {
              content: line,
              style:   TextStyle {
                size:  UI_FONT_SIZE,
                color: text_color,
              },
            }],
          });
          doc_y += line_height;
        }
      }
    });

    surface.restore_font_state(font_state);
  }

  fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
    None
  }

  fn id(&self) -> Option<&'static str> {
    Some(Self::ID)
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete()
  }
}

fn wrap_doc_text(doc: &str, max_chars: usize) -> Vec<String> {
  let mut lines = Vec::new();
  for raw_line in doc.lines() {
    let wrapped = wrap_text(raw_line, max_chars);
    if wrapped.is_empty() {
      lines.push(String::new());
    } else {
      lines.extend(wrapped);
    }
  }
  lines
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
  if max_chars == 0 {
    return vec![String::new()];
  }

  if text.trim().is_empty() {
    return vec![String::new()];
  }

  let mut lines = Vec::new();
  let mut current = String::new();

  for word in text.split_whitespace() {
    let word_len = word.chars().count();

    if word_len > max_chars {
      if !current.is_empty() {
        lines.push(current.clone());
        current.clear();
      }

      let mut buffer = String::with_capacity(max_chars);
      for ch in word.chars() {
        if buffer.chars().count() >= max_chars {
          lines.push(buffer.clone());
          buffer.clear();
        }
        buffer.push(ch);
      }
      if !buffer.is_empty() {
        lines.push(buffer);
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
