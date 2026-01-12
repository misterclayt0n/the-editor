use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Bézier coefficient for approximating circular arcs with cubic Bézier curves
/// c = (√2 - 1) × 4/3 ≈ 0.5522847498
const BEZIER_CIRCLE_COEFF: f32 = 0.5522847498307935;

/// Powerline separator glyph type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerlineGlyph {
  /// Right-pointing triangle (U+E0B0)
  RightTriangle,
  /// Left-pointing triangle (U+E0B2)
  LeftTriangle,
  /// Right-pointing rounded (U+E0B4)
  RightRounded,
  /// Left-pointing rounded (U+E0B6)
  LeftRounded,
}

impl PowerlineGlyph {
  /// Get the PowerlineGlyph variant for a Unicode character
  pub fn from_char(ch: char) -> Option<Self> {
    match ch {
      '\u{E0B0}' => Some(Self::RightTriangle),
      '\u{E0B2}' => Some(Self::LeftTriangle),
      '\u{E0B4}' => Some(Self::RightRounded),
      '\u{E0B6}' => Some(Self::LeftRounded),
      _ => None,
    }
  }
}

/// Render a Powerline glyph to a pixmap using tiny-skia
pub fn render_powerline_glyph(glyph: PowerlineGlyph, width: u32, height: u32) -> Option<Pixmap> {
  match glyph {
    PowerlineGlyph::RightTriangle => render_right_triangle(width, height),
    PowerlineGlyph::LeftTriangle => render_left_triangle(width, height),
    PowerlineGlyph::RightRounded => render_right_rounded(width, height),
    PowerlineGlyph::LeftRounded => render_left_rounded(width, height),
  }
}

/// Render a right-pointing triangle (U+E0B0)
fn render_right_triangle(width: u32, height: u32) -> Option<Pixmap> {
  let mut pixmap = Pixmap::new(width, height)?;
  let mut pb = PathBuilder::new();

  let w = width as f32;
  let h = height as f32;

  // Triangle: (0, 0) -> (w, h/2) -> (0, h) -> close
  pb.move_to(0.0, 0.0);
  pb.line_to(w, h / 2.0);
  pb.line_to(0.0, h);
  pb.close();

  let path = pb.finish()?;

  let mut paint = Paint::default();
  paint.set_color_rgba8(255, 255, 255, 255);
  paint.anti_alias = true;

  pixmap.fill_path(
    &path,
    &paint,
    FillRule::Winding,
    Transform::identity(),
    None,
  );

  Some(pixmap)
}

/// Render a left-pointing triangle (U+E0B2)
fn render_left_triangle(width: u32, height: u32) -> Option<Pixmap> {
  let mut pixmap = Pixmap::new(width, height)?;
  let mut pb = PathBuilder::new();

  let w = width as f32;
  let h = height as f32;

  // Triangle: (w, 0) -> (0, h/2) -> (w, h) -> close
  pb.move_to(w, 0.0);
  pb.line_to(0.0, h / 2.0);
  pb.line_to(w, h);
  pb.close();

  let path = pb.finish()?;

  let mut paint = Paint::default();
  paint.set_color_rgba8(255, 255, 255, 255);
  paint.anti_alias = true;

  pixmap.fill_path(
    &path,
    &paint,
    FillRule::Winding,
    Transform::identity(),
    None,
  );

  Some(pixmap)
}

/// Render a right-pointing rounded separator (U+E0B4)
/// Uses Bézier curves to approximate rounded corners like ghostty
fn render_right_rounded(width: u32, height: u32) -> Option<Pixmap> {
  let mut pixmap = Pixmap::new(width, height)?;
  let mut pb = PathBuilder::new();

  let w = width as f32;
  let h = height as f32;
  let radius = w.min(h / 2.0);
  let c = BEZIER_CIRCLE_COEFF;

  // Start at top-left (0, 0)
  pb.move_to(0.0, 0.0);

  // Upper Bézier curve: quarter-circle from (0, 0) to (radius, radius)
  pb.cubic_to(
    radius * c,
    0.0, // Control point 1
    radius,
    radius - radius * c, // Control point 2
    radius,
    radius, // End point
  );

  // Vertical line down the middle
  pb.line_to(radius, h - radius);

  // Lower Bézier curve: from (radius, h-radius) to (0, h)
  pb.cubic_to(
    radius,
    h - radius + radius * c, // Control point 1
    radius * c,
    h, // Control point 2
    0.0,
    h, // End point
  );

  // Close path back to (0, 0)
  pb.close();

  let path = pb.finish()?;

  let mut paint = Paint::default();
  paint.set_color_rgba8(255, 255, 255, 255);
  paint.anti_alias = true;

  pixmap.fill_path(
    &path,
    &paint,
    FillRule::Winding,
    Transform::identity(),
    None,
  );

  Some(pixmap)
}

/// Render a left-pointing rounded separator (U+E0B6)
/// Uses Bézier curves to approximate rounded corners like ghostty
fn render_left_rounded(width: u32, height: u32) -> Option<Pixmap> {
  let mut pixmap = Pixmap::new(width, height)?;
  let mut pb = PathBuilder::new();

  let w = width as f32;
  let h = height as f32;
  let radius = w.min(h / 2.0);
  let c = BEZIER_CIRCLE_COEFF;

  // Start at top-right (w, 0)
  pb.move_to(w, 0.0);

  // Upper Bézier curve: quarter-circle from (w, 0) to (w-radius, radius)
  pb.cubic_to(
    w - radius * c,
    0.0, // Control point 1
    w - radius,
    radius - radius * c, // Control point 2
    w - radius,
    radius, // End point
  );

  // Vertical line down the middle
  pb.line_to(w - radius, h - radius);

  // Lower Bézier curve: from (w-radius, h-radius) to (w, h)
  pb.cubic_to(
    w - radius,
    h - radius + radius * c, // Control point 1
    w - radius * c,
    h, // Control point 2
    w,
    h, // End point
  );

  // Close path back to (w, 0)
  pb.close();

  let path = pb.finish()?;

  let mut paint = Paint::default();
  paint.set_color_rgba8(255, 255, 255, 255);
  paint.anti_alias = true;

  pixmap.fill_path(
    &path,
    &paint,
    FillRule::Winding,
    Transform::identity(),
    None,
  );

  Some(pixmap)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_render_right_triangle() {
    let pixmap = render_powerline_glyph(PowerlineGlyph::RightTriangle, 16, 32);
    assert!(pixmap.is_some());
    let pixmap = pixmap.unwrap();
    assert_eq!(pixmap.width(), 16);
    assert_eq!(pixmap.height(), 32);
  }

  #[test]
  fn test_render_left_triangle() {
    let pixmap = render_powerline_glyph(PowerlineGlyph::LeftTriangle, 16, 32);
    assert!(pixmap.is_some());
  }

  #[test]
  fn test_render_right_rounded() {
    let pixmap = render_powerline_glyph(PowerlineGlyph::RightRounded, 16, 32);
    assert!(pixmap.is_some());
  }

  #[test]
  fn test_render_left_rounded() {
    let pixmap = render_powerline_glyph(PowerlineGlyph::LeftRounded, 16, 32);
    assert!(pixmap.is_some());
  }

  #[test]
  fn test_from_char() {
    assert_eq!(
      PowerlineGlyph::from_char('\u{E0B0}'),
      Some(PowerlineGlyph::RightTriangle)
    );
    assert_eq!(
      PowerlineGlyph::from_char('\u{E0B2}'),
      Some(PowerlineGlyph::LeftTriangle)
    );
    assert_eq!(
      PowerlineGlyph::from_char('\u{E0B4}'),
      Some(PowerlineGlyph::RightRounded)
    );
    assert_eq!(
      PowerlineGlyph::from_char('\u{E0B6}'),
      Some(PowerlineGlyph::LeftRounded)
    );
    assert_eq!(PowerlineGlyph::from_char('A'), None);
  }
}
