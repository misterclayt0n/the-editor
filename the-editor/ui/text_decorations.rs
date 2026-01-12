use std::cmp::Ordering;

use the_editor_renderer::{Color, TextSection, TextSegment, TextStyle};

use crate::{
  core::{doc_formatter::FormattedGrapheme, position::Position},
  ui::{UI_FONT_SIZE, UI_FONT_WIDTH, compositor::Surface},
};

pub mod diagnostic_underlines;
pub mod diagnostics;
pub mod fade;
pub mod inlay_hints;

/// Decorations are the primary mechanism for extending the text rendering.
///
/// Any on-screen element which is anchored to the rendered text in some form
/// should be implemented using this trait. Translating char positions to
/// on-screen positions can be expensive and should not be done manually in the
/// ui loop. Instead such translations are automatically performed on the fly
/// while the text is being rendered. The results are provided to this trait by
/// the rendering infrastructure.
///
/// To reserve space for virtual text lines (which is then filled by this trait)
/// emit appropriate
/// [`LineAnnotation`](crate::core::text_annotations::LineAnnotation)s
pub trait Decoration {
  /// Called **before** a **visual** line is rendered. A visual line does not
  /// necessarily correspond to a single line in a document as soft wrapping can
  /// spread a single document line across multiple visual lines.
  ///
  /// This function is called before text is rendered as any decorations should
  /// never overlap the document text. That means that setting the foreground
  /// color here is (essentially) useless as the text color is overwritten by
  /// the rendered text. This _of course_ doesn't apply when rendering inside
  /// virtual lines below the line reserved by `LineAnnotation`s as no text
  /// will be rendered here.
  fn decorate_line(&mut self, _surface: &mut Surface, _pos: (usize, u16)) {
    // pos: (doc_line, visual_line)
  }

  /// Called **after** a **visual** line is rendered. A visual line does not
  /// necessarily correspond to a single line in a document as soft wrapping can
  /// spread a single document line across multiple visual lines.
  ///
  /// This function is called after text is rendered so that decorations can
  /// collect horizontal positions on the line (see
  /// [`Decoration::decorate_grapheme`]) first and use those positions while
  /// rendering virtual text.
  ///
  /// **Note**: To avoid overlapping decorations in the virtual lines, each
  /// decoration must return the number of virtual text lines it has taken up.
  /// Each `Decoration` receives an offset `virt_off` based on these return
  /// values where it can render virtual text.
  ///
  /// That means that a `render_virt_lines` implementation that returns `X` can
  /// render virtual text in the following area:
  /// ```no_compile
  /// let start = inner.y + pos.visual_line + virt_off;
  /// start .. start + X
  /// ```
  fn render_virt_lines(
    &mut self,
    _surface: &mut Surface,
    _pos: (usize, u16),
    _virt_off: Position,
  ) -> Position {
    // pos: (doc_line, visual_line)
    // Returns: (additional_rows, line_width)
    Position::new(0, 0)
  }

  /// Called when rendering starts or position tracking needs to reset
  ///
  /// # Returns
  ///
  /// The char idx of the next grapheme that should trigger `decorate_grapheme`
  fn reset_pos(&mut self, _pos: usize) -> usize {
    usize::MAX
  }

  /// Called when text is concealed/skipped that contains an anchor
  ///
  /// # Returns
  ///
  /// The char idx of the next grapheme that should trigger `decorate_grapheme`
  fn skip_concealed_anchor(&mut self, conceal_end_char_idx: usize) -> usize {
    self.reset_pos(conceal_end_char_idx)
  }

  /// This function is called **before** the grapheme at `char_idx` is rendered.
  ///
  /// # Returns
  ///
  /// The char idx of the next grapheme that this function should be called for
  fn decorate_grapheme(&mut self, _grapheme: &FormattedGrapheme) -> usize {
    usize::MAX
  }
}

impl<F: FnMut(&mut Surface, (usize, u16))> Decoration for F {
  fn decorate_line(&mut self, surface: &mut Surface, pos: (usize, u16)) {
    self(surface, pos);
  }
}

/// Manages multiple decorations and orchestrates their calls during rendering
#[derive(Default)]
pub struct DecorationManager<'a> {
  decorations: Vec<(Box<dyn Decoration + 'a>, usize)>,
}

impl<'a> DecorationManager<'a> {
  /// Create a new empty decoration manager
  pub fn new() -> Self {
    Self {
      decorations: Vec::new(),
    }
  }

  /// Add a decoration to be rendered
  pub fn add_decoration(&mut self, decoration: impl Decoration + 'a) {
    self.decorations.push((Box::new(decoration), 0));
  }

  /// Prepare all decorations for rendering by resetting their positions
  pub fn prepare_for_rendering(&mut self, first_visible_char: usize) {
    for (decoration, next_position) in &mut self.decorations {
      *next_position = decoration.reset_pos(first_visible_char);
    }
  }

  /// Call decorate_grapheme on all decorations that are interested in this
  /// grapheme
  pub fn decorate_grapheme(&mut self, grapheme: &FormattedGrapheme) {
    for (decoration, hook_char_idx) in &mut self.decorations {
      loop {
        match (*hook_char_idx).cmp(&grapheme.char_idx) {
          // this grapheme has been concealed or we are at the first grapheme
          Ordering::Less => {
            *hook_char_idx = decoration.skip_concealed_anchor(grapheme.char_idx);
          },
          Ordering::Equal => {
            *hook_char_idx = decoration.decorate_grapheme(grapheme);
          },
          Ordering::Greater => break,
        }
      }
    }
  }

  /// Call decorate_line on all decorations
  pub fn decorate_line(&mut self, surface: &mut Surface, pos: (usize, u16)) {
    for (decoration, _) in &mut self.decorations {
      decoration.decorate_line(surface, pos);
    }
  }

  /// Render virtual lines for all decorations
  ///
  /// Returns the total number of virtual lines rendered
  pub fn render_virtual_lines(
    &mut self,
    surface: &mut Surface,
    pos: (usize, u16),
    line_width: usize,
  ) -> u16 {
    let mut virt_off = Position::new(1, line_width); // start at 1 to render in first virtual line slot
    let mut total_lines = 0u16;

    for (decoration, _) in &mut self.decorations {
      let result = decoration.render_virt_lines(surface, pos, virt_off);
      virt_off += result;
      total_lines = total_lines.saturating_add(result.row as u16);
    }

    total_lines
  }
}

/// Helper methods for rendering decorations
pub trait DecorationRenderer {
  /// Draw a single grapheme at the specified position with the given style
  fn draw_decoration_grapheme(&mut self, grapheme: &str, color: Color, x: f32, y: f32);

  /// Draw text at a position with automatic truncation if it exceeds max_width
  /// Returns the actual width rendered in characters
  fn draw_truncated_text(
    &mut self,
    text: &str,
    x: f32,
    y: f32,
    max_width: usize,
    color: Color,
  ) -> usize;

  /// Draw text at a position with automatic truncation if it exceeds max_width
  /// Uses the specified font size instead of UI_FONT_SIZE
  /// Returns the actual width rendered in characters
  fn draw_truncated_text_with_font_size(
    &mut self,
    text: &str,
    x: f32,
    y: f32,
    max_width: usize,
    color: Color,
    font_size: f32,
  ) -> usize;

  /// Check if a column is within viewport bounds
  fn column_in_bounds(&self, col: usize) -> bool;
}

impl DecorationRenderer for Surface {
  fn draw_decoration_grapheme(&mut self, grapheme: &str, color: Color, x: f32, y: f32) {
    self.draw_text(TextSection {
      position: (x, y),
      texts: vec![TextSegment {
        content: grapheme.to_string(),
        style: TextStyle {
          size: UI_FONT_SIZE,
          color,
        },
      }],
    });
  }

  fn draw_truncated_text(
    &mut self,
    text: &str,
    x: f32,
    y: f32,
    max_width: usize,
    color: Color,
  ) -> usize {
    use unicode_segmentation::UnicodeSegmentation;

    if max_width == 0 {
      return 0;
    }

    // Calculate actual display width using grapheme clusters
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let mut total_width = 0;
    let mut grapheme_count = 0;

    for grapheme in &graphemes {
      let width = unicode_width::UnicodeWidthStr::width(*grapheme);
      if total_width + width > max_width {
        break;
      }
      total_width += width;
      grapheme_count += 1;
    }

    // Check if truncation is needed
    let needs_truncation = grapheme_count < graphemes.len();

    if !needs_truncation {
      // Render full text
      self.draw_text(TextSection {
        position: (x, y),
        texts: vec![TextSegment {
          content: text.to_string(),
          style: TextStyle {
            size: UI_FONT_SIZE,
            color,
          },
        }],
      });
      total_width
    } else {
      // Truncate and add ellipsis (reserve 1 char for "…")
      let truncate_to = if max_width > 1 { max_width - 1 } else { 0 };
      let mut truncated_width = 0;
      let mut truncated_count = 0;

      for grapheme in &graphemes {
        let width = unicode_width::UnicodeWidthStr::width(*grapheme);
        if truncated_width + width > truncate_to {
          break;
        }
        truncated_width += width;
        truncated_count += 1;
      }

      let truncated: String = graphemes[..truncated_count].concat();
      let display = format!("{}…", truncated);

      self.draw_text(TextSection {
        position: (x, y),
        texts: vec![TextSegment {
          content: display,
          style: TextStyle {
            size: UI_FONT_SIZE,
            color,
          },
        }],
      });
      truncated_width + 1 // +1 for ellipsis
    }
  }

  fn draw_truncated_text_with_font_size(
    &mut self,
    text: &str,
    x: f32,
    y: f32,
    max_width: usize,
    color: Color,
    font_size: f32,
  ) -> usize {
    use unicode_segmentation::UnicodeSegmentation;

    if max_width == 0 {
      return 0;
    }

    // Calculate actual display width using grapheme clusters
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let mut total_width = 0;
    let mut grapheme_count = 0;

    for grapheme in &graphemes {
      let width = unicode_width::UnicodeWidthStr::width(*grapheme);
      if total_width + width > max_width {
        break;
      }
      total_width += width;
      grapheme_count += 1;
    }

    // Check if truncation is needed
    let needs_truncation = grapheme_count < graphemes.len();

    if !needs_truncation {
      // Render full text
      self.draw_text(TextSection {
        position: (x, y),
        texts: vec![TextSegment {
          content: text.to_string(),
          style: TextStyle {
            size: font_size,
            color,
          },
        }],
      });
      total_width
    } else {
      // Truncate and add ellipsis (reserve 1 char for "…")
      let truncate_to = if max_width > 1 { max_width - 1 } else { 0 };
      let mut truncated_width = 0;
      let mut truncated_count = 0;

      for grapheme in &graphemes {
        let width = unicode_width::UnicodeWidthStr::width(*grapheme);
        if truncated_width + width > truncate_to {
          break;
        }
        truncated_width += width;
        truncated_count += 1;
      }

      let truncated: String = graphemes[..truncated_count].concat();
      let display = format!("{}…", truncated);

      self.draw_text(TextSection {
        position: (x, y),
        texts: vec![TextSegment {
          content: display,
          style: TextStyle {
            size: font_size,
            color,
          },
        }],
      });
      truncated_width + 1 // +1 for ellipsis
    }
  }

  fn column_in_bounds(&self, col: usize) -> bool {
    let viewport_width = (self.width() as f32 / UI_FONT_WIDTH).floor() as usize;
    col < viewport_width
  }
}
