use ropey::Rope;
use the_editor_lsp_types::types as lsp;
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
  core::{
    graphics::Rect,
    position::Position,
  },
  ui::{
    popup_positioning::calculate_cursor_position,
    UI_FONT_SIZE,
    UI_FONT_WIDTH,
    components::popup::{
      PopupConstraints,
      PopupContent,
      PopupFrame,
      PopupLimits,
      PopupShell,
      PopupSize,
    },
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
  },
};

const MAX_VISIBLE_LINES: usize = 12;
const MIN_CONTENT_CHARS: usize = 10;
const PIXELS_PER_SCROLL_LINE: f32 = 24.0;
const MAX_SCROLL_LINES_PER_TICK: i32 = 4;
const HOVER_MIN_WIDTH_CHARS: u16 = 18;
const HOVER_MAX_WIDTH_CHARS: u16 = 64;
const HOVER_MAX_HEIGHT_LINES: u16 = 20;

/// Hover popup component rendered inside a generic popup shell.
pub struct Hover {
  popup: PopupShell<HoverContent>,
}

impl Hover {
  pub const ID: &'static str = "hover";

  pub fn new(hovers: Vec<(String, lsp::Hover)>) -> Self {
    let content = HoverContent::new(hovers);
    let popup_limits = PopupLimits {
      min_width: HOVER_MIN_WIDTH_CHARS,
      max_width: HOVER_MAX_WIDTH_CHARS,
      max_height: HOVER_MAX_HEIGHT_LINES,
      ..PopupLimits::default()
    };
    let popup = PopupShell::new(Self::ID, content)
      .auto_close(true)
      .with_limits(popup_limits);
    Self { popup }
  }
}

impl Component for Hover {
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    // PopupShell now uses shared positioning module directly
    // Setting anchor ensures PopupShell knows to position relative to cursor
    let anchor = current_cursor_anchor(ctx, surface);
    self.popup.set_anchor(anchor);
    Component::render(&mut self.popup, area, surface, ctx);
  }

  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    Component::handle_event(&mut self.popup, event, ctx)
  }

  fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
    Component::required_size(&mut self.popup, viewport)
  }

  fn id(&self) -> Option<&'static str> {
    Some(Self::ID)
  }

  fn is_animating(&self) -> bool {
    Component::is_animating(&self.popup)
  }
}

fn current_cursor_anchor(ctx: &Context, surface: &Surface) -> Option<Position> {
  // Use shared cursor position calculation to ensure consistent positioning
  // PopupShell will use calculate_cursor_position internally, but we set anchor
  // to indicate we want cursor-relative positioning
  let _cursor = calculate_cursor_position(ctx, surface)?;
  
  // Convert to Position format for PopupShell (though it will recalculate using shared module)
  let (view, doc) = crate::current_ref!(ctx.editor);
  let text = doc.text();
  let cursor_pos = doc.selection(view.id).primary().cursor(text.slice(..));

  let line = text.char_to_line(cursor_pos);
  let view_offset = doc.view_offset(view.id);
  let anchor_line = text.char_to_line(view_offset.anchor.min(text.len_chars()));

  let rel_row = line.saturating_sub(anchor_line);
  if rel_row >= view.inner_height() {
    return None;
  }

  let line_start = text.line_to_char(line);
  let col = cursor_pos - line_start;
  let screen_col = col.saturating_sub(view_offset.horizontal_offset);

  let inner = view.inner_area(doc);
  let anchor_col = inner.x as usize + screen_col;
  // Position one line below cursor (matching original behavior)
  let anchor_row = inner.y as usize + rel_row + 1;

  Some(Position::new(anchor_row, anchor_col))
}

struct HoverContent {
  entries:       Vec<HoverEntry>,
  layout:        Option<HoverLayout>,
  scroll_offset: usize,
}

struct HoverEntry {
  #[allow(dead_code)]
  server:   String,
  markdown: String,
}

#[derive(Clone)]
struct HoverLayout {
  lines:         Vec<Vec<TextSegment>>,
  visible_lines: usize,
  line_height:   f32,
  content_width: f32,
  wrap_width:    f32,
}

impl HoverLayout {
  fn inner_height(&self) -> f32 {
    (self.visible_lines as f32) * self.line_height
  }
}

impl HoverContent {
  fn new(hovers: Vec<(String, lsp::Hover)>) -> Self {
    let entries = hovers
      .into_iter()
      .map(|(server, hover)| {
        HoverEntry {
          server,
          markdown: hover_contents_to_string(hover.contents),
        }
      })
      .collect::<Vec<_>>();

    Self {
      entries,
      layout: None,
      scroll_offset: 0,
    }
  }

  fn active_entry(&self) -> Option<&HoverEntry> {
    self.entries.first()
  }

  fn ensure_layout(
    &mut self,
    cell_width: f32,
    ctx: &mut Context,
    wrap_width: f32,
  ) -> Option<&HoverLayout> {
    let entry = self.active_entry()?;
    let line_height = UI_FONT_SIZE + 4.0;

    let layout_is_stale = self.layout.as_ref().map_or(true, |layout| {
      (layout.wrap_width - wrap_width).abs() > f32::EPSILON
    });

    if layout_is_stale {
      let lines = build_hover_render_lines(&entry.markdown, wrap_width, cell_width, ctx);
      let visible_lines = lines.len().min(MAX_VISIBLE_LINES);
      let mut content_width = lines
        .iter()
        .take(visible_lines)
        .map(|segments| estimate_line_width(segments, cell_width))
        .fold(0.0, f32::max);

      if content_width <= 0.0 {
        content_width = 0.0;
      }
      content_width = content_width.min(wrap_width);
      let min_width = (MIN_CONTENT_CHARS as f32 * cell_width).min(wrap_width);
      content_width = content_width.max(min_width);

      self.layout = Some(HoverLayout {
        lines,
        visible_lines,
        line_height,
        content_width,
        wrap_width,
      });
    }

    self.layout.as_ref()
  }

  fn scroll_by_delta(&mut self, delta: &ScrollDelta) -> bool {
    let lines = scroll_lines_from_delta(delta);
    if lines == 0 {
      return false;
    }
    self.scroll_by_lines(lines)
  }

  fn page_scroll_amount(&self) -> usize {
    self
      .layout
      .as_ref()
      .map(|layout| {
        let visible = layout.visible_lines.max(1);
        (visible + 1) / 2
      })
      .unwrap_or(0)
  }

  fn scroll_by_lines(&mut self, lines: i32) -> bool {
    let Some(layout) = self.layout.as_ref() else {
      return false;
    };

    let total_lines = layout.lines.len();
    let visible = layout.visible_lines.min(total_lines).max(1);
    if total_lines <= visible {
      return false;
    }

    let max_scroll = total_lines.saturating_sub(visible);
    let previous = self.scroll_offset;
    if lines < 0 {
      let amount = (-lines) as usize;
      self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    } else {
      let amount = lines as usize;
      self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }
    self.scroll_offset = self.scroll_offset.min(max_scroll);
    previous != self.scroll_offset
  }
}

impl PopupContent for HoverContent {
  fn measure(
    &mut self,
    _surface: &Surface,
    ctx: &mut Context,
    constraints: PopupConstraints,
  ) -> PopupSize {
    if constraints.max_width <= 0.0 || constraints.max_height <= 0.0 {
      return PopupSize::ZERO;
    }

    let wrap_width = constraints.max_width;
    let cell_width = UI_FONT_WIDTH.max(1.0);

    let Some(layout) = self.ensure_layout(cell_width, ctx, wrap_width) else {
      return PopupSize::ZERO;
    };

    let content_height = layout.inner_height().min(constraints.max_height);

    PopupSize {
      width:  layout.content_width.min(constraints.max_width),
      height: content_height,
    }
  }

  fn render(&mut self, frame: &mut PopupFrame<'_>, ctx: &mut Context) {
    let inner = frame.inner();
    let outer = frame.outer();
    let wrap_width = inner.width.max(1.0);
    let cell_width = UI_FONT_WIDTH.max(1.0);

    let alpha = frame.alpha();
    let (text_x, mut text_y) = frame.inner_origin();
    text_y += UI_FONT_SIZE;
    if self.scroll_offset > 0 {
      let padding_above = (inner.y - outer.y).max(0.0);
      text_y -= padding_above.min(UI_FONT_SIZE);
    }

    let mut new_scroll_offset = self.scroll_offset;

    {
      let Some(layout) = self.ensure_layout(cell_width, ctx, wrap_width) else {
        self.scroll_offset = 0;
        return;
      };

      let total_lines = layout.lines.len();
      if total_lines == 0 {
        new_scroll_offset = 0;
      } else {
        let visible_lines = layout.visible_lines.min(total_lines).max(1);
        let max_scroll = total_lines.saturating_sub(visible_lines);
        new_scroll_offset = new_scroll_offset.min(max_scroll);
        let text_bottom_bound = inner.y + inner.height;

        for segments in layout
          .lines
          .iter()
          .skip(new_scroll_offset)
          .take(visible_lines)
        {
          if text_y > text_bottom_bound {
            break;
          }

          let texts = segments
            .iter()
            .map(|segment| {
              let mut seg = segment.clone();
              seg.style.color.a *= alpha;
              seg
            })
            .collect();

          let section = TextSection {
            position: (text_x, text_y),
            texts,
          };

          frame.surface().draw_text(section);
          text_y += layout.line_height;
        }

        if total_lines > visible_lines {
          let track_height = inner.height.max(4.0) - 4.0;
          let track_y = inner.y + 2.0;
          let track_x = inner.x + inner.width - 2.0;
          let scroll_ratio = if max_scroll == 0 {
            0.0
          } else {
            new_scroll_offset.min(max_scroll) as f32 / max_scroll as f32
          };
          let mut thumb_height = (visible_lines as f32 / total_lines as f32) * track_height;
          thumb_height = thumb_height.clamp(8.0, track_height);
          let thumb_travel = (track_height - thumb_height).max(0.0);
          let thumb_y = track_y + scroll_ratio * thumb_travel;

          let mut track_color = Color::new(0.8, 0.8, 0.8, 0.08);
          let mut thumb_color = Color::new(0.9, 0.9, 0.9, 0.25);
          track_color.a *= alpha;
          thumb_color.a *= alpha;

          let surface = frame.surface();
          surface.draw_rect(track_x, track_y, 1.0, track_height, track_color);
          surface.draw_rect(track_x - 1.0, thumb_y, 2.0, thumb_height, thumb_color);
        }
      }
    }

    self.scroll_offset = new_scroll_offset;
  }

  fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
    match event {
      Event::Scroll(delta) => {
        let _ = self.scroll_by_delta(delta);
        EventResult::Consumed(None)
      },
      Event::Key(key) => {
        if key.ctrl && !key.alt && !key.shift {
          match key.code {
            Key::Char('d') => {
              let amount = self.page_scroll_amount();
              if amount > 0 {
                let _ = self.scroll_by_lines(-(amount as i32));
              }
              return EventResult::Consumed(None);
            },
            Key::Char('u') => {
              let amount = self.page_scroll_amount();
              if amount > 0 {
                let _ = self.scroll_by_lines(amount as i32);
              }
              return EventResult::Consumed(None);
            },
            _ => {},
          }
        }
        EventResult::Ignored(None)
      },
      _ => EventResult::Ignored(None),
    }
  }
}

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

fn build_hover_render_lines(
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

  let mut render_lines: Vec<Vec<TextSegment>> = Vec::new();
  let mut in_fence = false;
  let mut fence_lang: Option<String> = None;
  let mut fence_buf: Vec<String> = Vec::new();

  let max_chars = (wrap_width / cell_width).floor().max(4.0) as usize;

  for raw_line in markdown.lines() {
    if raw_line.starts_with("```") {
      if in_fence {
        let code = fence_buf.join("\n");
        render_lines.extend(highlight_code_block_lines(
          fence_lang.as_deref(),
          &code,
          max_chars,
          ctx,
        ));
        render_lines.push(vec![TextSegment {
          content: String::new(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: base_text_color,
          },
        }]);
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

    let wrapped_lines = wrap_text(raw_line, max_chars);
    if wrapped_lines.is_empty() {
      render_lines.push(vec![TextSegment {
        content: String::new(),
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: base_text_color,
        },
      }]);
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
    let max_chars = (wrap_width / cell_width).floor().max(4.0) as usize;
    render_lines.extend(highlight_code_block_lines(
      fence_lang.as_deref(),
      &code,
      max_chars,
      ctx,
    ));
  }

  render_lines
}

fn highlight_code_block_lines(
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

    // Wrap the line to fit within max_chars
    let wrapped_line_strings = wrap_text(&line_string, max_chars);
    
    // Process each wrapped segment independently
    // We highlight each wrapped segment separately, which means we lose some
    // syntax highlighting accuracy for wrapped lines, but ensures text fits within container
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

      // For wrapped segments, we apply syntax highlighting to the wrapped text directly
      // This is simpler than trying to map wrapped positions back to original spans
      let wrapped_rope = Rope::from(wrapped_line.as_str());
      let wrapped_slice = wrapped_rope.slice(..);
      
      // Re-apply syntax highlighting to the wrapped segment
      let wrapped_spans = language
        .and_then(|lang| crate::core::syntax::Syntax::new(wrapped_slice, lang, &loader).ok())
        .map(|syntax| syntax.collect_highlights(wrapped_slice, &loader, 0..wrapped_slice.len_bytes()))
        .unwrap_or_else(Vec::new);

      let mut wrapped_char_spans: Vec<(usize, usize, Color)> = Vec::with_capacity(wrapped_spans.len());
      for (hl, byte_range) in wrapped_spans.into_iter() {
        let style = theme.highlight(hl);
        let color = style
          .fg
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(default_code_color);
        let start_char = wrapped_slice.byte_to_char(wrapped_slice.floor_char_boundary(byte_range.start));
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

fn estimate_line_width(segments: &[TextSegment], cell_width: f32) -> f32 {
  segments
    .iter()
    .map(|segment| segment.content.chars().count() as f32 * cell_width)
    .sum()
}

fn slice_chars_to_string(s: &str, start: usize, end: usize) -> String {
  if start >= end || start >= s.chars().count() {
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

fn scroll_lines_from_delta(delta: &ScrollDelta) -> i32 {
  let raw = match delta {
    ScrollDelta::Lines { y, .. } => *y,
    ScrollDelta::Pixels { y, .. } => *y / PIXELS_PER_SCROLL_LINE,
  };

  if raw.abs() < f32::EPSILON {
    return 0;
  }

  let magnitude = raw.abs().ceil().min(MAX_SCROLL_LINES_PER_TICK as f32) as i32;
  if raw.is_sign_negative() {
    -magnitude
  } else {
    magnitude
  }
}
