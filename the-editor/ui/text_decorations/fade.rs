use the_editor_renderer::Color;

use crate::{
  core::{context_fade::RelevantRanges, doc_formatter::FormattedGrapheme, position::Position},
  ui::{compositor::Surface, text_decorations::Decoration},
};

/// A decoration that fades out text that is not in the relevant ranges
pub struct FadeDecoration {
  /// The ranges that should remain visible (not faded)
  relevant_ranges: RelevantRanges,
  /// The opacity to apply to faded text (0.0 = invisible, 1.0 = fully visible)
  fade_alpha: f32,
}

impl FadeDecoration {
  pub fn new(relevant_ranges: RelevantRanges) -> Self {
    Self {
      relevant_ranges,
      fade_alpha: 0.3, // 30% opacity for faded text
    }
  }

  pub fn with_fade_alpha(mut self, alpha: f32) -> Self {
    self.fade_alpha = alpha.clamp(0.0, 1.0);
    self
  }
}

impl Decoration for FadeDecoration {
  fn decorate_grapheme(&mut self, grapheme: &FormattedGrapheme) -> usize {
    // Check if this grapheme is in a relevant range
    let is_relevant = self.relevant_ranges.contains(grapheme.char_idx);

    // If not relevant, modify the style to be faded
    if !is_relevant {
      // The grapheme's style will be modified during rendering
      // We can't directly modify it here, but we can track state if needed
    }

    // Return the next position we want to be called for
    // For fade, we want to check every grapheme
    grapheme.char_idx + 1
  }

  fn decorate_line(&mut self, _surface: &mut Surface, _pos: (usize, u16)) {
    // We could modify the entire line here if needed
    // For now, we'll handle fading at the grapheme level
  }

  fn render_virt_lines(
    &mut self,
    _surface: &mut Surface,
    _pos: (usize, u16),
    _virt_off: Position,
  ) -> Position {
    // No virtual lines needed for fade decoration
    Position::new(0, 0)
  }
}

/// Helper function to apply fade to a color
pub fn apply_fade_to_color(color: Color, fade_alpha: f32) -> Color {
  Color {
    r: color.r,
    g: color.g,
    b: color.b,
    a: (color.a * fade_alpha).min(255.0),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::selection::Range;

  #[test]
  fn test_fade_decoration_creation() {
    let mut ranges = RelevantRanges::new();
    ranges.add_range(Range::new(10, 20));
    let fade = FadeDecoration::new(ranges);
    assert_eq!(fade.fade_alpha, 0.3);
  }

  #[test]
  fn test_fade_alpha_clamping() {
    let ranges = RelevantRanges::new();
    let fade = FadeDecoration::new(ranges.clone()).with_fade_alpha(2.0);
    assert_eq!(fade.fade_alpha, 1.0);

    let fade = FadeDecoration::new(ranges).with_fade_alpha(-0.5);
    assert_eq!(fade.fade_alpha, 0.0);
  }

  #[test]
  fn test_apply_fade_to_color() {
    let color = Color {
      r: 255.0,
      g: 128.0,
      b: 64.0,
      a: 255.0,
    };
    let faded = apply_fade_to_color(color, 0.5);
    assert_eq!(faded.r, 255.0);
    assert_eq!(faded.g, 128.0);
    assert_eq!(faded.b, 64.0);
    assert_eq!(faded.a, 127.5);
  }
}
