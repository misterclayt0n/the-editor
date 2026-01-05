use the_editor_lsp_types::types as lsp;
use the_editor_renderer::{
  Color,
  Key,
  ScrollDelta,
  TextSection,
  TextSegment,
};

use crate::{
  core::{
    graphics::Rect,
    position::Position,
  },
  ui::{
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
    popup_positioning::calculate_cursor_position,
  },
};

const MAX_VISIBLE_LINES: usize = 50;
const MIN_CONTENT_CHARS: usize = 50;
const HOVER_MIN_WIDTH_CHARS: u16 = 40;
const HOVER_MAX_WIDTH_CHARS: u16 = 100;
const HOVER_MAX_HEIGHT_LINES: u16 = 40;

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

fn current_cursor_anchor(ctx: &Context, surface: &mut Surface) -> Option<Position> {
  // Use shared cursor position calculation to ensure consistent positioning
  // PopupShell will use calculate_cursor_position internally, but we set anchor
  // to indicate we want cursor-relative positioning
  let _cursor = calculate_cursor_position(ctx, surface)?;

  // Convert to Position format for PopupShell (though it will recalculate using
  // shared module)
  // Skip if focused view is not a document (e.g., terminal)
  let (view, doc) = crate::try_current_ref!(ctx.editor)?;
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
      // Convert pixel width to cell width for consistent wrapping
      let max_width_cells = (wrap_width / cell_width.max(1.0)).floor().max(4.0) as u16;

      // Use cell-based markdown building for consistent wrapping
      let lines =
        super::markdown::build_markdown_lines_cells(&entry.markdown, max_width_cells, ctx);
      let visible_lines = lines.len().min(MAX_VISIBLE_LINES);

      // Calculate actual content width from wrapped lines
      let content_width_cells = lines
        .iter()
        .take(visible_lines)
        .map(|segments| super::markdown::line_width_cells(segments))
        .max()
        .unwrap_or(0);

      // Apply min width constraint
      let min_width_cells = (MIN_CONTENT_CHARS as u16).min(max_width_cells);
      let final_width_cells = content_width_cells
        .max(min_width_cells)
        .min(max_width_cells);
      let content_width = final_width_cells as f32 * cell_width;

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
    let lines = super::markdown::scroll_lines_from_delta(delta);
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

pub(crate) fn build_hover_render_lines(
  markdown: &str,
  max_width_cells: u16,
  ctx: &mut Context,
) -> Vec<Vec<TextSegment>> {
  super::markdown::build_markdown_lines_cells(markdown, max_width_cells, ctx)
}
