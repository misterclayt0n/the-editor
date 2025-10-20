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
    graphics::Rect,
    position::Position,
  },
  ui::{
    UI_FONT_SIZE,
    components::popup::{
      PopupConstraints,
      PopupContent,
      PopupFrame,
      PopupShell,
      PopupSize,
      PositionBias,
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

const MAX_VISIBLE_LINES: usize = 20;
const MIN_CONTENT_CHARS: usize = 10;

/// Hover popup component rendered inside a generic popup shell.
pub struct Hover {
  popup: PopupShell<HoverContent>,
}

impl Hover {
  pub const ID: &'static str = "hover";

  pub fn new(hovers: Vec<(String, lsp::Hover)>) -> Self {
    let content = HoverContent::new(hovers);
    let popup = PopupShell::new(Self::ID, content)
      .position_bias(PositionBias::Below)
      .auto_close(true);
    Self { popup }
  }
}

impl Component for Hover {
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    let anchor = current_cursor_anchor(ctx);
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

fn current_cursor_anchor(ctx: &Context) -> Option<Position> {
  let (view, doc) = crate::current_ref!(ctx.editor);
  let text = doc.text();
  let cursor_pos = doc.selection(view.id).primary().cursor(text.slice(..));

  let line = text.char_to_line(cursor_pos);
  let line_start = text.line_to_char(line);
  let col = cursor_pos - line_start;

  let view_offset = doc.view_offset(view.id);
  let anchor_line = text.char_to_line(view_offset.anchor.min(text.len_chars()));

  let rel_row = line.saturating_sub(anchor_line);
  let screen_col = col.saturating_sub(view_offset.horizontal_offset);

  if rel_row >= view.inner_height() {
    return None;
  }

  let inner = view.inner_area(doc);
  let anchor_col = inner.x as usize + screen_col;
  let anchor_row = inner.y as usize + rel_row + 1; // one line below cursor

  Some(Position::new(anchor_row, anchor_col))
}

struct HoverContent {
  entries: Vec<HoverEntry>,
  layout:  Option<HoverLayout>,
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
}

impl PopupContent for HoverContent {
  fn measure(
    &mut self,
    surface: &Surface,
    ctx: &mut Context,
    constraints: PopupConstraints,
  ) -> PopupSize {
    if constraints.max_width <= 0.0 || constraints.max_height <= 0.0 {
      return PopupSize::ZERO;
    }

    let wrap_width = constraints.max_width;
    let cell_width = surface.cell_width().max(1.0);

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
    let wrap_width = inner.width.max(1.0);
    let cell_width = frame.surface().cell_width().max(1.0);

    let Some(layout) = self.ensure_layout(cell_width, ctx, wrap_width) else {
      return;
    };

    let alpha = frame.alpha();
    let (text_x, mut text_y) = frame.inner_origin();
    text_y += UI_FONT_SIZE;

    let lines = layout.lines.iter().take(layout.visible_lines);
    for segments in lines {
      if text_y > inner.y + inner.height {
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

  for raw_line in markdown.lines() {
    if raw_line.starts_with("```") {
      if in_fence {
        let code = fence_buf.join("\n");
        render_lines.extend(highlight_code_block_lines(
          fence_lang.as_deref(),
          &code,
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

    let max_chars = (wrap_width / cell_width).floor().max(4.0) as usize;
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
    render_lines.extend(highlight_code_block_lines(
      fence_lang.as_deref(),
      &code,
      ctx,
    ));
  }

  render_lines
}

fn highlight_code_block_lines(
  lang_hint: Option<&str>,
  code: &str,
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
    let line_start_char = rope.line_to_char(line_idx);
    let line_end_char = line_start_char + line_string.chars().count();

    let mut segments: Vec<TextSegment> = Vec::new();
    let mut cursor = line_start_char;

    for (s, e, color) in char_spans.iter().cloned() {
      if e <= line_start_char || s >= line_end_char {
        continue;
      }
      let seg_start = s.max(line_start_char);
      let seg_end = e.min(line_end_char);

      if seg_start > cursor {
        let prefix = slice_chars_to_string(
          &line_string,
          cursor - line_start_char,
          seg_start - line_start_char,
        );
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

      let content = slice_chars_to_string(
        &line_string,
        seg_start - line_start_char,
        seg_end - line_start_char,
      );
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

    if cursor < line_end_char {
      let tail = slice_chars_to_string(
        &line_string,
        cursor - line_start_char,
        line_end_char - line_start_char,
      );
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
        content: line_string,
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: default_code_color,
        },
      });
    }

    lines.push(segments);
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
