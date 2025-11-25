use ropey::Rope;
use the_editor_renderer::{
  Color,
  Key,
  ScrollDelta,
  TextSection,
  TextSegment,
  TextStyle,
};
use the_editor_stdx::rope::RopeSliceExt;

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
    popup_positioning::{
      calculate_cursor_position,
      constrain_popup_height,
      position_popup_centered_on_cursor,
    },
  },
};

/// Signature help layout constants tuned for a compact, Zed-like look.
const MAX_CONTENT_CHARS: usize = 56;
const MIN_CONTENT_CHARS: usize = 20;
const MAX_DOC_VISIBLE_LINES: usize = 8;
const POPUP_MIN_WIDTH: f32 = 200.0;
const POPUP_MAX_WIDTH: f32 = 520.0;
const POPUP_PADDING: f32 = 10.0;
const DOC_SECTION_GAP: f32 = 6.0;
const VIEWPORT_SIDE_MARGIN: f32 = 12.0;
/// Pixel gap between cursor and popup (matches completion popup)
const CURSOR_POPUP_MARGIN: f32 = 4.0;

/// Signature help popup component
pub struct SignatureHelp {
  /// Language for syntax highlighting
  language:               String,
  /// Active signature index
  active_signature:       usize,
  /// All available signatures
  signatures:             Vec<crate::handlers::signature_help::Signature>,
  /// Appearance animation
  animation:              crate::core::animation::AnimationHandle<f32>,
  /// Whether the popup is visible
  visible:                bool,
  /// Scroll offset for long documentation blocks
  doc_scroll:             usize,
  /// Cached doc metadata for scroll handling
  last_doc_visible_lines: usize,
  last_doc_total_lines:   usize,
  /// Deferred scroll amount to apply once layout is ready
  pending_doc_scroll:     i32,
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
      doc_scroll: 0,
      last_doc_visible_lines: 0,
      last_doc_total_lines: 0,
      pending_doc_scroll: 0,
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
    self.doc_scroll = 0;
    self.last_doc_visible_lines = 0;
    self.last_doc_total_lines = 0;
    self.pending_doc_scroll = 0;
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

  pub fn active_signature_index(&self) -> usize {
    self
      .active_signature
      .min(self.signatures.len().saturating_sub(1))
  }

  fn scroll_docs(&mut self, delta: &ScrollDelta) {
    let lines = super::markdown::scroll_lines_from_delta(delta);
    if lines == 0 {
      return;
    }
    self.enqueue_doc_scroll(lines);
  }

  fn enqueue_doc_scroll(&mut self, lines: i32) {
    if self.last_doc_total_lines == 0 && self.last_doc_visible_lines == 0 {
      const MAX_PENDING: i32 = 64;
      self.pending_doc_scroll = (self.pending_doc_scroll + lines).clamp(-MAX_PENDING, MAX_PENDING);
    } else {
      let _ = self.scroll_docs_by_lines(lines);
    }
  }

  fn doc_page_scroll_amount(&self) -> usize {
    if self.last_doc_visible_lines == 0 {
      0
    } else {
      (self.last_doc_visible_lines.max(1) + 1) / 2
    }
  }

  fn scroll_docs_by_lines(&mut self, lines: i32) -> bool {
    if self.last_doc_visible_lines == 0 || self.last_doc_total_lines <= self.last_doc_visible_lines
    {
      return false;
    }

    let max_scroll = self
      .last_doc_total_lines
      .saturating_sub(self.last_doc_visible_lines.max(1));
    let previous = self.doc_scroll;
    if lines < 0 {
      let amount = (-lines) as usize;
      self.doc_scroll = (self.doc_scroll + amount).min(max_scroll);
    } else {
      let amount = lines as usize;
      self.doc_scroll = self.doc_scroll.saturating_sub(amount);
    }
    self.doc_scroll = self.doc_scroll.min(max_scroll);
    previous != self.doc_scroll
  }
}

impl Component for SignatureHelp {
  fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
    match event {
      Event::Scroll(delta) => {
        self.scroll_docs(delta);
        EventResult::Consumed(None)
      },
      Event::Key(key) => {
        if key.ctrl && !key.alt && !key.shift {
          match key.code {
            Key::Char('d') => {
              let amount = self.doc_page_scroll_amount().max(1);
              self.enqueue_doc_scroll(-(amount as i32));
              return EventResult::Consumed(None);
            },
            Key::Char('u') => {
              let amount = self.doc_page_scroll_amount().max(1);
              self.enqueue_doc_scroll(amount as i32);
              return EventResult::Consumed(None);
            },
            _ => {},
          }
        }

        if self.signatures.len() <= 1 {
          return EventResult::Ignored(None);
        }

        match (key.code, key.ctrl, key.alt, key.shift) {
          (Key::Char('p'), false, true, false) => {
            if self.active_signature == 0 {
              self.active_signature = self.signatures.len() - 1;
            } else {
              self.active_signature -= 1;
            }
            self.doc_scroll = 0;
            EventResult::Consumed(None)
          },
          (Key::Char('n'), false, true, false) => {
            self.active_signature = (self.active_signature + 1) % self.signatures.len();
            self.doc_scroll = 0;
            EventResult::Consumed(None)
          },
          _ => EventResult::Ignored(None),
        }
      },
      _ => EventResult::Ignored(None),
    }
  }

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
    let sig = self.current_signature().clone();

    // Calculate popup dimensions
    let padding = POPUP_PADDING;

    // Calculate cursor position using shared positioning utility
    let Some(cursor) = calculate_cursor_position(ctx, surface) else {
      return;
    };

    let font_state = surface.save_font_state();

    surface.configure_font(&font_state.family, UI_FONT_SIZE);
    let ui_char_width = surface.cell_width().max(UI_FONT_WIDTH.max(1.0));
    let line_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);
    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    let signature_index = self.signature_index();
    let index_width_chars = signature_index
      .as_ref()
      .map(|idx| idx.chars().count())
      .unwrap_or(0);

    let available_content_width =
      (viewport_width - VIEWPORT_SIDE_MARGIN * 2.0 - padding * 2.0).max(ui_char_width);
    let available_chars = ((available_content_width / ui_char_width).floor() as usize).max(1);
    let wrap_chars = if available_chars < MIN_CONTENT_CHARS {
      available_chars
    } else {
      available_chars.min(MAX_CONTENT_CHARS)
    };

    let signature_lines = wrap_signature_lines_with_syntax(
      &sig.signature,
      sig.active_param_range,
      wrap_chars,
      &self.language,
      ctx,
      text_color,
    );
    let signature_line_count = signature_lines.len().max(1);

    let doc_lines = if let Some(doc) = &sig.signature_doc {
      build_doc_render_lines(doc, wrap_chars, ui_char_width, ctx, text_color)
    } else {
      Vec::new()
    };

    let doc_line_count = doc_lines.len();
    let mut visible_doc_lines = doc_line_count.min(MAX_DOC_VISIBLE_LINES);

    let mut content_chars = signature_lines
      .iter()
      .map(|line| line.char_len)
      .max()
      .unwrap_or(0)
      .max(index_width_chars);

    let max_doc_width = doc_lines
      .iter()
      .map(|segments| super::markdown::estimate_line_width(segments, ui_char_width))
      .fold(0.0, f32::max);
    if max_doc_width > 0.0 {
      let max_doc_chars = (max_doc_width / ui_char_width).ceil() as usize;
      content_chars = content_chars.max(max_doc_chars);
    }

    let preferred_min = if available_chars >= MIN_CONTENT_CHARS {
      MIN_CONTENT_CHARS
    } else {
      available_chars
    };
    content_chars = content_chars.max(preferred_min).min(wrap_chars).max(1);

    let content_width = content_chars as f32 * ui_char_width;
    let mut popup_width = (content_width + padding * 2.0).clamp(POPUP_MIN_WIDTH, POPUP_MAX_WIDTH);
    let available_popup_width =
      (viewport_width - VIEWPORT_SIDE_MARGIN * 2.0).max(padding * 2.0 + ui_char_width);
    popup_width = popup_width.min(available_popup_width).min(viewport_width);

    let signature_block_height = signature_line_count as f32 * line_height;
    let mut popup_height = padding * 2.0 + signature_block_height;
    if visible_doc_lines > 0 {
      popup_height += DOC_SECTION_GAP + visible_doc_lines as f32 * line_height;
    }
    let min_popup_height = padding * 2.0 + signature_block_height;

    // Constrain popup height to fit available space using shared utility
    // Pass None for bias to maintain current behavior (choose side with more space)
    popup_height = constrain_popup_height(
      cursor,
      popup_height,
      min_popup_height,
      viewport_height,
      None,
    );

    // Recalculate visible_doc_lines if height was constrained
    if popup_height
      < padding * 2.0
        + signature_block_height
        + DOC_SECTION_GAP
        + visible_doc_lines as f32 * line_height
    {
      let available_for_docs = (popup_height - min_popup_height - DOC_SECTION_GAP).max(0.0);
      let max_lines_by_height = (available_for_docs / line_height).floor() as usize;
      visible_doc_lines = visible_doc_lines.min(max_lines_by_height);
      if visible_doc_lines == 0 {
        popup_height = min_popup_height;
      } else {
        popup_height = min_popup_height + DOC_SECTION_GAP + visible_doc_lines as f32 * line_height;
      }
    }

    self.last_doc_total_lines = doc_line_count;
    self.last_doc_visible_lines = visible_doc_lines;
    if visible_doc_lines == 0 {
      self.doc_scroll = 0;
      self.pending_doc_scroll = 0;
    } else {
      let max_scroll = doc_line_count.saturating_sub(visible_doc_lines);
      self.doc_scroll = self.doc_scroll.min(max_scroll);
      if self.pending_doc_scroll != 0 {
        let pending = self.pending_doc_scroll;
        self.pending_doc_scroll = 0;
        let _ = self.scroll_docs_by_lines(pending);
        self.doc_scroll = self.doc_scroll.min(max_scroll);
      }
    }

    // Position popup using shared positioning utility (generalized from completer)
    // Pass None for bias to maintain current behavior (choose side with more space)
    let popup_pos = position_popup_centered_on_cursor(
      cursor,
      popup_width,
      popup_height,
      viewport_width,
      viewport_height,
      slide_offset,
      scale,
      None,
    );

    let anim_width = popup_width * scale;
    let anim_height = popup_height * scale;
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
      let first_line_baseline = anim_y + padding + UI_FONT_SIZE;

      for (line_idx, line) in signature_lines.iter().enumerate() {
        let line_y = first_line_baseline + line_idx as f32 * line_height;
        let mut section = TextSection {
          position: (text_x, line_y),
          texts:    Vec::new(),
        };

        for segment in &line.segments {
          let mut color = segment.color;
          // Parameter highlighting takes precedence over syntax highlighting
          if segment.highlighted {
            color = Color::new(1.0, 1.0, 0.6, text_color.a);
          }
          // Apply alpha to color
          color.a *= alpha;
          if segment.text.is_empty() {
            continue;
          }
          section.texts.push(TextSegment {
            content: segment.text.clone(),
            style:   TextStyle {
              size: UI_FONT_SIZE,
              color,
            },
          });
        }

        if section.texts.is_empty() {
          section.texts.push(TextSegment {
            content: String::new(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          });
        }

        surface.draw_text(section);
      }

      if let Some(index_text) = signature_index {
        let index_width = index_text.chars().count() as f32 * ui_char_width;
        let index_x = (anim_x + anim_width - padding - index_width).max(text_x);
        surface.draw_text(TextSection {
          position: (index_x, first_line_baseline),
          texts:    vec![TextSegment {
            content: index_text,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          }],
        });
      }

      if !doc_lines.is_empty() && visible_doc_lines > 0 {
        let max_scroll = doc_lines.len().saturating_sub(visible_doc_lines);
        let start_line = self.doc_scroll.min(max_scroll);
        let doc_area_top =
          first_line_baseline + signature_line_count as f32 * line_height + DOC_SECTION_GAP;
        let mut doc_y = doc_area_top;
        let doc_box_top = doc_area_top - UI_FONT_SIZE;
        let doc_box_height = visible_doc_lines as f32 * line_height;

        for segments in doc_lines.iter().skip(start_line).take(visible_doc_lines) {
          // Ensure we respect bottom padding - stop rendering before the padding area
          if doc_y + line_height > anim_y + anim_height - padding {
            break;
          }

          if segments.is_empty() {
            doc_y += line_height;
            continue;
          }

          let texts = segments
            .iter()
            .map(|segment| {
              let mut seg = segment.clone();
              seg.style.color.a *= alpha;
              seg
            })
            .collect();

          surface.draw_text(TextSection {
            position: (text_x, doc_y),
            texts,
          });
          doc_y += line_height;
        }

        if doc_lines.len() > visible_doc_lines {
          let track_height = doc_box_height.max(8.0) - 4.0;
          let track_y = doc_box_top + 2.0;
          let track_x = anim_x + anim_width - padding - 2.0;
          let scroll_ratio = if max_scroll == 0 {
            0.0
          } else {
            self.doc_scroll.min(max_scroll) as f32 / max_scroll as f32
          };
          let mut thumb_height = (visible_doc_lines as f32 / doc_lines.len() as f32) * track_height;
          thumb_height = thumb_height.clamp(6.0, track_height);
          let thumb_travel = (track_height - thumb_height).max(0.0);
          let thumb_y = track_y + scroll_ratio * thumb_travel;
          let mut track_color = Color::new(0.8, 0.8, 0.8, 0.08);
          let mut thumb_color = Color::new(0.9, 0.9, 0.9, 0.25);
          track_color.a *= alpha;
          thumb_color.a *= alpha;

          surface.draw_rect(track_x, track_y, 1.0, track_height, track_color);
          surface.draw_rect(track_x - 1.0, thumb_y, 2.0, thumb_height, thumb_color);
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

#[derive(Clone)]
struct SignatureSegment {
  text:        String,
  highlighted: bool,
  color:       Color,
}

#[derive(Clone)]
struct SignatureLine {
  segments: Vec<SignatureSegment>,
  char_len: usize,
}

/// Wrap signature lines with syntax highlighting applied.
/// Parameter highlighting (yellow) takes precedence over syntax colors.
fn wrap_signature_lines_with_syntax(
  signature: &str,
  highlight_range: Option<(usize, usize)>,
  max_chars: usize,
  language: &str,
  ctx: &mut Context,
  default_text_color: Color,
) -> Vec<SignatureLine> {
  let max_chars = max_chars.max(1);
  let total_chars = signature.chars().count();

  if total_chars == 0 {
    return vec![SignatureLine {
      segments: vec![SignatureSegment {
        text:        String::new(),
        highlighted: false,
        color:       default_text_color,
      }],
      char_len: 0,
    }];
  }

  // Convert parameter highlight range from bytes to characters
  let highlight_chars = highlight_range.map(|(start, end)| {
    (
      signature[..start.min(signature.len())].chars().count(),
      signature[..end.min(signature.len())].chars().count(),
    )
  });

  // Apply syntax highlighting to the full signature
  let theme = &ctx.editor.theme;
  let loader = ctx.editor.syn_loader.load();
  let language_obj = loader.language_for_name(language.to_string());

  let rope = Rope::from(signature);
  let slice = rope.slice(..);

  // Get syntax highlight spans
  let syntax_spans = language_obj
    .and_then(|lang| crate::core::syntax::Syntax::new(slice, lang, &loader).ok())
    .map(|syntax| syntax.collect_highlights(slice, &loader, 0..slice.len_bytes()))
    .unwrap_or_else(Vec::new);

  // Convert syntax spans to character-based spans with colors
  let mut syntax_char_spans: Vec<(usize, usize, Color)> = Vec::with_capacity(syntax_spans.len());
  for (hl, byte_range) in syntax_spans.into_iter() {
    let style = theme.highlight(hl);
    let color = style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(default_text_color);
    let start_char = slice.byte_to_char(slice.floor_char_boundary(byte_range.start));
    let end_char = slice.byte_to_char(slice.ceil_char_boundary(byte_range.end));
    if start_char < end_char {
      syntax_char_spans.push((start_char, end_char, color));
    }
  }
  syntax_char_spans.sort_by_key(|(s, _e, _)| *s);

  // Wrap the signature into lines
  let mut chars = Vec::new();
  let mut byte_offsets = Vec::new();
  for (byte_idx, ch) in signature.char_indices() {
    byte_offsets.push(byte_idx);
    chars.push(ch);
  }
  byte_offsets.push(signature.len());

  let mut lines = Vec::new();
  let mut line_start = 0usize;
  let mut idx = 0usize;
  let mut last_break: Option<usize> = None;

  while idx < total_chars {
    let ch = chars[idx];
    idx += 1;

    if signature_wrap_break(ch) {
      last_break = Some(idx);
    }

    if idx - line_start >= max_chars && idx < total_chars {
      let break_idx = last_break.filter(|b| *b > line_start).unwrap_or(idx);
      lines.push(build_signature_line_with_syntax(
        signature,
        line_start,
        break_idx,
        &byte_offsets,
        highlight_chars,
        &syntax_char_spans,
        default_text_color,
      ));
      line_start = break_idx;
      last_break = None;
    }
  }

  if line_start < total_chars {
    lines.push(build_signature_line_with_syntax(
      signature,
      line_start,
      total_chars,
      &byte_offsets,
      highlight_chars,
      &syntax_char_spans,
      default_text_color,
    ));
  }

  lines
}

fn build_signature_line_with_syntax(
  signature: &str,
  start_char: usize,
  end_char: usize,
  byte_offsets: &[usize],
  highlight_chars: Option<(usize, usize)>,
  syntax_spans: &[(usize, usize, Color)],
  default_color: Color,
) -> SignatureLine {
  if start_char >= end_char {
    return SignatureLine {
      segments: vec![SignatureSegment {
        text:        String::new(),
        highlighted: false,
        color:       default_color,
      }],
      char_len: 0,
    };
  }

  let (highlight_start, highlight_end) = highlight_chars.unwrap_or((usize::MAX, usize::MAX));

  // Build segments by merging syntax highlighting and parameter highlighting
  let mut segments = Vec::new();
  let mut cursor = start_char;

  // Collect all boundaries (syntax spans and parameter highlight)
  let mut boundaries = Vec::new();
  for (s, e, _) in syntax_spans.iter() {
    if *s >= start_char && *s < end_char {
      boundaries.push((*s, false));
    }
    if *e > start_char && *e <= end_char {
      boundaries.push((*e, false));
    }
  }
  if highlight_start >= start_char && highlight_start < end_char {
    boundaries.push((highlight_start, true));
  }
  if highlight_end > start_char && highlight_end <= end_char {
    boundaries.push((highlight_end, true));
  }
  boundaries.push((start_char, false));
  boundaries.push((end_char, false));
  boundaries.sort_by_key(|(pos, _)| *pos);
  boundaries.dedup_by_key(|(pos, _)| *pos);

  for &(boundary, _) in boundaries.iter() {
    if boundary <= cursor || boundary > end_char {
      continue;
    }

    let segment_start = cursor;
    let segment_end = boundary.min(end_char);

    if segment_start >= segment_end {
      continue;
    }

    // Determine if this segment is highlighted (parameter highlight)
    let highlighted = segment_start >= highlight_start && segment_end <= highlight_end;

    // Find syntax color for this segment (use the color from the first overlapping
    // syntax span)
    let mut syntax_color = default_color;
    for (s, e, color) in syntax_spans.iter() {
      if segment_start < *e && segment_end > *s {
        syntax_color = *color;
        break;
      }
    }

    let byte_start = byte_offsets[segment_start];
    let byte_end = byte_offsets[segment_end];
    segments.push(SignatureSegment {
      text: signature[byte_start..byte_end].to_string(),
      highlighted,
      color: syntax_color,
    });

    cursor = segment_end;
  }

  // Handle any remaining text
  if cursor < end_char {
    let highlighted = cursor >= highlight_start && end_char <= highlight_end;
    let mut syntax_color = default_color;
    for (s, e, color) in syntax_spans.iter() {
      if cursor < *e && end_char > *s {
        syntax_color = *color;
        break;
      }
    }
    let byte_start = byte_offsets[cursor];
    let byte_end = byte_offsets[end_char];
    segments.push(SignatureSegment {
      text: signature[byte_start..byte_end].to_string(),
      highlighted,
      color: syntax_color,
    });
  }

  if segments.is_empty() {
    segments.push(SignatureSegment {
      text:        String::new(),
      highlighted: false,
      color:       default_color,
    });
  }

  SignatureLine {
    segments,
    char_len: end_char - start_char,
  }
}

fn signature_wrap_break(ch: char) -> bool {
  ch.is_whitespace()
    || matches!(
      ch,
      ',' | ';' | ':' | '(' | ')' | '<' | '>' | '[' | ']' | '{' | '}' | '-'
    )
}

/// Build documentation render lines with markdown and syntax highlighting
/// support. Uses shared markdown utilities.
fn build_doc_render_lines(
  markdown: &str,
  max_chars: usize,
  cell_width: f32,
  ctx: &mut Context,
  _default_text_color: Color,
) -> Vec<Vec<TextSegment>> {
  let wrap_width = max_chars as f32 * cell_width;
  super::markdown::build_markdown_lines(markdown, wrap_width, cell_width, ctx)
}










