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
    self.active_signature = active_signature;
    self.signatures = signatures;
    // Reset animation if signatures changed (quick re-animation from 80%)
    if self.animation.is_complete() {
      let (duration, easing) = crate::core::animation::presets::FAST;
      self.animation = crate::core::animation::AnimationHandle::new(0.8, 1.0, duration, easing);
    }
  }
}

impl Component for SignatureHelp {
  fn render(&mut self, _area: Rect, surface: &mut Surface, ctx: &mut Context) {
    if self.signatures.is_empty() || !self.visible {
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
    let selected_style = theme.get("ui.menu.selected");

    let mut bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 0.98));
    let mut text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
    let mut highlight_color = selected_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.25, 0.3, 0.45, 1.0));

    // Apply alpha
    bg_color.a *= alpha;
    text_color.a *= alpha;
    highlight_color.a *= alpha;

    // Get active signature
    let sig = &self.signatures[self.active_signature.min(self.signatures.len() - 1)];

    // Calculate popup dimensions
    let padding = 12.0;
    let line_height = UI_FONT_SIZE + 4.0;

    // Estimate width based on signature length (more generous)
    let sig_width = sig.signature.len().min(MAX_POPUP_WIDTH) as f32 * UI_FONT_WIDTH;
    let popup_width = (sig_width + padding * 4.0).max(300.0).min(800.0);

    // Calculate height - add extra line if there's documentation
    let num_lines = if sig.signature_doc.is_some() {
      2.0
    } else {
      1.0
    };
    let popup_height = (num_lines * line_height) + (padding * 2.0);

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
      let font_size = ctx
        .editor
        .font_size_override
        .unwrap_or(ctx.editor.config().font_size);
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
      // Position ABOVE the cursor line (unlike completion which goes below)
      let y = base_y + (rel_row as f32) * (font_size + LINE_SPACING) - popup_height - 4.0;

      (x, y)
    };

    // Apply animation transforms
    let anim_y = cursor_y - slide_offset; // Slide from above
    let anim_width = popup_width * scale;
    let anim_height = popup_height * scale;
    let anim_x = cursor_x - (popup_width - anim_width) / 2.0;

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
    });
  }

  fn handle_event(&mut self, _event: &Event, _ctx: &mut Context) -> EventResult {
    // Don't consume any events - let them bubble up to the editor
    // The editor will handle mode switches (Escape) and close the signature help
    // automatically
    EventResult::Ignored(None)
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
