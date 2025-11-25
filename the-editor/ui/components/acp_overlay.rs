//! ACP overlay component for displaying agent responses.
//!
//! This overlay shows the streaming response from an ACP agent in a popup
//! similar to the hover component. It displays:
//! - Header with "ACP" label and model name
//! - Context summary (what was sent)
//! - The response text (markdown with syntax highlighting)
//! - Tool call indicators with icons
//! - A streaming indicator when response is in progress

use the_editor_renderer::{
  Color,
  Key,
  ScrollDelta,
  TextSection,
  TextSegment,
  TextStyle,
};

use crate::{
  core::{
    graphics::Rect,
    position::Position,
  },
  ui::{
    UI_FONT_SIZE,
    UI_FONT_WIDTH,
    components::{
      popup::{
        PopupConstraints,
        PopupContent,
        PopupFrame,
        PopupLimits,
        PopupShell,
        PopupSize,
      },
    },
    compositor::{
      Callback,
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
    popup_positioning::calculate_cursor_position,
  },
};

const MAX_VISIBLE_LINES: usize = 18;
const ACP_MIN_WIDTH_CHARS: u16 = 40;
const ACP_MAX_WIDTH_CHARS: u16 = 80;
const ACP_MAX_HEIGHT_LINES: u16 = 20;

/// ACP overlay component for displaying agent responses.
pub struct AcpOverlay {
  popup: PopupShell<AcpOverlayContent>,
}

impl AcpOverlay {
  pub const ID: &'static str = "acp-overlay";

  pub fn new() -> Self {
    let content = AcpOverlayContent::new();
    let popup_limits = PopupLimits {
      min_width:  ACP_MIN_WIDTH_CHARS,
      max_width:  ACP_MAX_WIDTH_CHARS,
      min_height: 6,
      max_height: ACP_MAX_HEIGHT_LINES,
    };
    let popup = PopupShell::new(Self::ID, content)
      .auto_close(false) // Don't close on any key, only on Escape
      .with_limits(popup_limits);
    Self { popup }
  }

  fn close_callback() -> Callback {
    Box::new(move |compositor, _ctx| {
      compositor.remove(Self::ID);
    })
  }
}

impl Default for AcpOverlay {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for AcpOverlay {
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    // Set anchor to cursor position for positioning
    let anchor = current_cursor_anchor(ctx, surface);
    self.popup.set_anchor(anchor);
    Component::render(&mut self.popup, area, surface, ctx);
  }

  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    // Handle Escape to close
    if let Event::Key(key) = event
      && key.code == Key::Escape
      && !key.ctrl
      && !key.alt
      && !key.shift
    {
      return EventResult::Consumed(Some(Self::close_callback()));
    }

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
  let _cursor = calculate_cursor_position(ctx, surface)?;

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
  let anchor_row = inner.y as usize + rel_row + 1;

  Some(Position::new(anchor_row, anchor_col))
}

struct AcpOverlayContent {
  layout:             Option<AcpLayout>,
  scroll_offset:      usize,
  /// Cached response text length to detect changes
  last_response_len:  usize,
  /// Cached streaming state
  last_was_streaming: bool,
}

#[derive(Clone)]
struct AcpLayout {
  header_lines:   Vec<Vec<TextSegment>>,
  response_lines: Vec<Vec<TextSegment>>,
  visible_lines:  usize,
  line_height:    f32,
  content_width:  f32,
  wrap_width:     f32,
}

impl AcpLayout {
  fn total_lines(&self) -> usize {
    self.header_lines.len() + self.response_lines.len()
  }

  fn inner_height(&self) -> f32 {
    (self.visible_lines as f32) * self.line_height
  }
}

// ============================================================================
// ACP Markdown Renderer
// ============================================================================

/// Marker prefix for tool calls injected into the response text.
/// Format: `[TOOL:status:name]` where status is "start", "done", or "error"
pub const TOOL_MARKER_PREFIX: &str = "[TOOL:";

/// Parse and render ACP response markdown with proper formatting.
///
/// Handles:
/// - Paragraphs (double newlines)
/// - Fenced code blocks with syntax highlighting
/// - Tool call markers (rendered as styled lines)
/// - Proper word wrapping that preserves intentional line breaks
fn build_acp_render_lines(
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

  // Use "function" color for tool calls - typically a nice accent color
  let mut tool_color = theme
    .get("function")
    .fg
    .map(crate::ui::theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.5, 0.8, 0.6, 1.0));
  // Dim it slightly to be less prominent than regular text
  tool_color.r *= 0.85;
  tool_color.g *= 0.85;
  tool_color.b *= 0.85;

  let max_chars = (wrap_width / cell_width).floor().max(4.0) as usize;

  let mut render_lines: Vec<Vec<TextSegment>> = Vec::new();
  let mut in_fence = false;
  let mut fence_lang: Option<String> = None;
  let mut fence_buf: Vec<String> = Vec::new();
  let mut prev_was_empty = false;

  // Process line by line, tracking blank lines for paragraph breaks
  // We can't split by \n\n first because that breaks code blocks with blank lines
  for raw_line in markdown.lines() {
    let is_empty = raw_line.trim().is_empty();

    // Check for tool markers (outside of code blocks)
    if !in_fence {
      if let Some(tool_line) = parse_tool_marker(raw_line) {
        let (icon, text) = tool_line;
        render_lines.push(vec![TextSegment {
          content: format!("{} {}", icon, text),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: tool_color,
          },
        }]);
        prev_was_empty = false;
        continue;
      }
    }

    // Handle fenced code blocks
    if raw_line.starts_with("```") {
      if in_fence {
        // End of code block
        let code = fence_buf.join("\n");
        render_lines.extend(super::markdown::highlight_code_block(
          fence_lang.as_deref(),
          &code,
          max_chars,
          ctx,
        ));
        in_fence = false;
        fence_lang = None;
        fence_buf.clear();
      } else {
        // Start of code block
        in_fence = true;
        let lang = raw_line.trim_start_matches('`').trim();
        fence_lang = if lang.is_empty() {
          None
        } else {
          Some(lang.to_string())
        };
      }
      prev_was_empty = false;
      continue;
    }

    // Inside a code block - preserve everything including blank lines
    if in_fence {
      fence_buf.push(raw_line.to_string());
      continue;
    }

    // Handle blank lines as paragraph separators
    if is_empty {
      // Only add one blank line for paragraph break (collapse multiple)
      if !prev_was_empty && !render_lines.is_empty() {
        render_lines.push(vec![TextSegment {
          content: String::new(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: base_text_color,
          },
        }]);
      }
      prev_was_empty = true;
      continue;
    }

    prev_was_empty = false;

    // Regular text - wrap it properly
    let wrapped = super::markdown::wrap_text_preserve_breaks(raw_line, max_chars);
    for line in wrapped {
      render_lines.push(vec![TextSegment {
        content: line,
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: base_text_color,
        },
      }]);
    }
  }

  // Handle unclosed fence
  if in_fence {
    let code = fence_buf.join("\n");
    render_lines.extend(super::markdown::highlight_code_block(
      fence_lang.as_deref(),
      &code,
      max_chars,
      ctx,
    ));
  }

  render_lines
}

/// Parse a tool marker line and return (icon, description).
///
/// Format: `[TOOL:status:name]` or `[TOOL:status:name:details]`
fn parse_tool_marker(line: &str) -> Option<(&'static str, String)> {
  let trimmed = line.trim();
  if !trimmed.starts_with(TOOL_MARKER_PREFIX) {
    return None;
  }

  let content = trimmed
    .strip_prefix(TOOL_MARKER_PREFIX)?
    .strip_suffix(']')?;

  let parts: Vec<&str> = content.splitn(3, ':').collect();
  if parts.len() < 2 {
    return None;
  }

  let status = parts[0];
  let name = parts[1];
  let details = parts.get(2).unwrap_or(&"");

  let (icon, text) = match status {
    "start" => ("→", format!("{} {}", name, details).trim().to_string()),
    "done" => ("←", format!("{} {}", name, details).trim().to_string()),
    "error" => ("✗", format!("{}: {}", name, details).trim().to_string()),
    _ => ("•", format!("{} {}", name, details).trim().to_string()),
  };

  Some((icon, text))
}





impl AcpOverlayContent {
  fn new() -> Self {
    Self {
      layout:             None,
      scroll_offset:      0,
      last_response_len:  0,
      last_was_streaming: false,
    }
  }

  fn ensure_layout(
    &mut self,
    cell_width: f32,
    ctx: &mut Context,
    wrap_width: f32,
  ) -> Option<&AcpLayout> {
    let state = ctx.editor.acp_response.as_ref()?;
    let line_height = UI_FONT_SIZE + 4.0;

    let layout_is_stale = self.layout.as_ref().map_or(true, |layout| {
      (layout.wrap_width - wrap_width).abs() > f32::EPSILON
    });

    if layout_is_stale {
      let theme = &ctx.editor.theme;
      let text_style = theme.get("ui.text");
      let base_text_color = text_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));

      let header_style = theme.get("ui.text.focus");
      let header_color = header_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::new(0.6, 0.8, 1.0, 1.0));

      let dim_color = Color::new(
        base_text_color.r * 0.6,
        base_text_color.g * 0.6,
        base_text_color.b * 0.6,
        base_text_color.a,
      );

      // Build header lines
      let mut header_lines: Vec<Vec<TextSegment>> = Vec::new();

      // Line 1: "ACP" label + provider + model
      // Get provider from config command (first element, e.g., "opencode")
      let provider = ctx
        .editor
        .acp_config
        .command
        .first()
        .map(|s| s.as_str())
        .unwrap_or("agent");

      // Get model name: prefer stored model state, fallback to response state
      let model_name = ctx
        .editor
        .acp
        .as_ref()
        .and_then(|h| h.model_state())
        .map(|s| {
          // Find the human-readable name for the current model
          s.available_models
            .iter()
            .find(|m| m.model_id == s.current_model_id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| s.current_model_id.to_string())
        })
        .or_else(|| {
          let m = &state.model_name;
          if m.is_empty() || m == "default" {
            None
          } else {
            Some(m.clone())
          }
        });

      let header_text = match model_name {
        Some(model) => format!("ACP  {} ({})", provider, model),
        None => format!("ACP  {}", provider),
      };
      header_lines.push(vec![TextSegment {
        content: header_text,
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: header_color,
        },
      }]);

      // Line 2: Context summary
      if !state.context_summary.is_empty() {
        header_lines.push(vec![TextSegment {
          content: state.context_summary.clone(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: dim_color,
          },
        }]);
      }

      // Line 3: Separator
      let max_chars = (wrap_width / cell_width).floor().max(4.0) as usize;
      let separator = "─".repeat(max_chars.min(60));
      header_lines.push(vec![TextSegment {
        content: separator,
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: dim_color,
        },
      }]);

      // Build response lines using hover's markdown renderer
      let response_text = if state.response_text.is_empty() {
        if state.is_streaming {
          "Waiting for response...".to_string()
        } else {
          "No response yet.".to_string()
        }
      } else {
        let mut text = state.response_text.clone();
        if state.is_streaming {
          text.push('▌'); // Streaming cursor (inline)
        }
        text
      };

      let response_lines = build_acp_render_lines(&response_text, wrap_width, cell_width, ctx);

      // Calculate content dimensions
      let all_lines_count = header_lines.len() + response_lines.len();
      let visible_lines = all_lines_count.min(MAX_VISIBLE_LINES);

      let mut content_width = header_lines
        .iter()
        .chain(response_lines.iter())
        .take(visible_lines)
        .map(|segments| super::markdown::estimate_line_width(segments, cell_width))
        .fold(0.0, f32::max);

      if content_width <= 0.0 {
        content_width = 0.0;
      }
      content_width = content_width.min(wrap_width);
      let min_width = (30.0 * cell_width).min(wrap_width);
      content_width = content_width.max(min_width);

      self.layout = Some(AcpLayout {
        header_lines,
        response_lines,
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

    let total_lines = layout.total_lines();
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

impl PopupContent for AcpOverlayContent {
  fn measure(
    &mut self,
    _surface: &Surface,
    ctx: &mut Context,
    constraints: PopupConstraints,
  ) -> PopupSize {
    if constraints.max_width <= 0.0 || constraints.max_height <= 0.0 {
      return PopupSize::ZERO;
    }

    // If no ACP response, return minimal size
    if ctx.editor.acp_response.is_none() {
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
    // If no ACP response, don't render anything
    let Some(state) = ctx.editor.acp_response.as_ref() else {
      return;
    };

    // Check if content has changed - only invalidate layout when necessary
    let current_len = state.response_text.len();
    let current_streaming = state.is_streaming;
    if current_len != self.last_response_len || current_streaming != self.last_was_streaming {
      self.layout = None;
      self.last_response_len = current_len;
      self.last_was_streaming = current_streaming;
    }

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

      let total_lines = layout.total_lines();
      if total_lines == 0 {
        new_scroll_offset = 0;
      } else {
        let visible_lines = layout.visible_lines.min(total_lines).max(1);
        let max_scroll = total_lines.saturating_sub(visible_lines);
        new_scroll_offset = new_scroll_offset.min(max_scroll);
        let text_bottom_bound = inner.y + inner.height;

        // Combine header and response lines for rendering
        let all_lines: Vec<_> = layout
          .header_lines
          .iter()
          .chain(layout.response_lines.iter())
          .collect();

        for segments in all_lines.iter().skip(new_scroll_offset).take(visible_lines) {
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

        // Draw scrollbar if needed
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
        // Don't consume other keys - let them pass through
        // (Escape is handled in AcpOverlay::handle_event)
        EventResult::Ignored(None)
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn is_animating(&self) -> bool {
    // Consider animating while streaming to keep redrawing
    // This will be checked via ctx in render, so return false here
    false
  }
}
