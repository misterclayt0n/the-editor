//! Color types and utilities

/// RGBA color representation using normalized floats (0.0 to 1.0)
#[derive(Debug, Clone, Copy)]
pub struct Color {
  /// Red component (0.0 to 1.0)
  pub r: f32,
  /// Green component (0.0 to 1.0)
  pub g: f32,
  /// Blue component (0.0 to 1.0)
  pub b: f32,
  /// Alpha component (0.0 = transparent, 1.0 = opaque)
  pub a: f32,
}

impl Color {
  /// Create a new color with RGBA components
  ///
  /// Components should be in the range 0.0 to 1.0
  pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
    Self { r, g, b, a }
  }

  /// Create an opaque color with RGB components
  ///
  /// Alpha is set to 1.0 (fully opaque)
  pub fn rgb(r: f32, g: f32, b: f32) -> Self {
    Self { r, g, b, a: 1.0 }
  }

  /// Create a color from a hex value
  ///
  /// # Example
  ///
  /// ```rust
  /// # use the_editor_renderer::Color;
  /// let red = Color::from_hex(0xFF0000);
  /// let green = Color::from_hex(0x00FF00);
  /// let blue = Color::from_hex(0x0000FF);
  /// ```
  pub fn from_hex(hex: u32) -> Self {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    Self { r, g, b, a: 1.0 }
  }

  // Common colors
  //

  pub const WHITE: Self = Self {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
  };

  pub const BLACK: Self = Self {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
  };

  pub const RED: Self = Self {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
  };

  pub const GREEN: Self = Self {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
  };

  pub const BLUE: Self = Self {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
  };

  pub const GRAY: Self = Self {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 1.0,
  };
}
