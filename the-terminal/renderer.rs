//! Terminal cell extraction for rendering.
//!
//! This module provides utilities to extract renderable cells from the terminal state.

use alacritty_terminal::{
  term::{
    RenderableContent,
    cell::Flags as CellFlags,
  },
  vte::ansi::{
    Color,
    NamedColor,
  },
};

/// A single cell to be rendered.
#[derive(Debug, Clone)]
pub struct RenderCell {
  /// Column position (0-indexed).
  pub col: u16,
  /// Row position (0-indexed, from top of viewport).
  pub row: u16,
  /// Character to display.
  pub c: char,
  /// Foreground color as RGB.
  pub fg: (u8, u8, u8),
  /// Background color as RGB.
  pub bg: (u8, u8, u8),
  /// Cell flags for styling.
  pub flags: CellStyle,
  /// Whether this is a wide character (takes 2 cells).
  pub is_wide: bool,
}

/// Style flags for a cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct CellStyle {
  pub bold:          bool,
  pub italic:        bool,
  pub underline:     bool,
  pub strikethrough: bool,
  pub dim:           bool,
  pub inverse:       bool,
}

impl From<CellFlags> for CellStyle {
  fn from(flags: CellFlags) -> Self {
    Self {
      bold:          flags.contains(CellFlags::BOLD),
      italic:        flags.contains(CellFlags::ITALIC),
      underline:     flags.intersects(
        CellFlags::UNDERLINE | CellFlags::DOUBLE_UNDERLINE | CellFlags::UNDERCURL,
      ),
      strikethrough: flags.contains(CellFlags::STRIKEOUT),
      dim:           flags.contains(CellFlags::DIM),
      inverse:       flags.contains(CellFlags::INVERSE),
    }
  }
}

/// Terminal 16-color scheme with bright variants.
#[derive(Debug, Clone)]
pub struct ColorScheme {
  pub foreground:     (u8, u8, u8),
  pub background:     (u8, u8, u8),
  pub cursor:         (u8, u8, u8),
  pub black:          (u8, u8, u8),
  pub red:            (u8, u8, u8),
  pub green:          (u8, u8, u8),
  pub yellow:         (u8, u8, u8),
  pub blue:           (u8, u8, u8),
  pub magenta:        (u8, u8, u8),
  pub cyan:           (u8, u8, u8),
  pub white:          (u8, u8, u8),
  pub bright_black:   (u8, u8, u8),
  pub bright_red:     (u8, u8, u8),
  pub bright_green:   (u8, u8, u8),
  pub bright_yellow:  (u8, u8, u8),
  pub bright_blue:    (u8, u8, u8),
  pub bright_magenta: (u8, u8, u8),
  pub bright_cyan:    (u8, u8, u8),
  pub bright_white:   (u8, u8, u8),
}

impl Default for ColorScheme {
  fn default() -> Self {
    // Default dark theme colors
    Self {
      foreground:     (204, 204, 204),
      background:     (30, 30, 30),
      cursor:         (255, 255, 255),
      black:          (0, 0, 0),
      red:            (204, 0, 0),
      green:          (0, 204, 0),
      yellow:         (204, 204, 0),
      blue:           (0, 0, 204),
      magenta:        (204, 0, 204),
      cyan:           (0, 204, 204),
      white:          (204, 204, 204),
      bright_black:   (128, 128, 128),
      bright_red:     (255, 0, 0),
      bright_green:   (0, 255, 0),
      bright_yellow:  (255, 255, 0),
      bright_blue:    (0, 0, 255),
      bright_magenta: (255, 0, 255),
      bright_cyan:    (0, 255, 255),
      bright_white:   (255, 255, 255),
    }
  }
}

impl ColorScheme {
  /// Resolve an alacritty color to RGB.
  pub fn resolve(&self, color: Color, is_fg: bool) -> (u8, u8, u8) {
    match color {
      Color::Named(named) => self.resolve_named(named, is_fg),
      Color::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
      Color::Indexed(idx) => self.resolve_indexed(idx),
    }
  }

  fn resolve_named(&self, color: NamedColor, is_fg: bool) -> (u8, u8, u8) {
    match color {
      NamedColor::Foreground => self.foreground,
      NamedColor::Background => self.background,
      NamedColor::Cursor => self.cursor,
      NamedColor::Black => self.black,
      NamedColor::Red => self.red,
      NamedColor::Green => self.green,
      NamedColor::Yellow => self.yellow,
      NamedColor::Blue => self.blue,
      NamedColor::Magenta => self.magenta,
      NamedColor::Cyan => self.cyan,
      NamedColor::White => self.white,
      NamedColor::BrightBlack => self.bright_black,
      NamedColor::BrightRed => self.bright_red,
      NamedColor::BrightGreen => self.bright_green,
      NamedColor::BrightYellow => self.bright_yellow,
      NamedColor::BrightBlue => self.bright_blue,
      NamedColor::BrightMagenta => self.bright_magenta,
      NamedColor::BrightCyan => self.bright_cyan,
      NamedColor::BrightWhite => self.bright_white,
      _ => {
        if is_fg {
          self.foreground
        } else {
          self.background
        }
      }
    }
  }

  fn resolve_indexed(&self, idx: u8) -> (u8, u8, u8) {
    match idx {
      0 => self.black,
      1 => self.red,
      2 => self.green,
      3 => self.yellow,
      4 => self.blue,
      5 => self.magenta,
      6 => self.cyan,
      7 => self.white,
      8 => self.bright_black,
      9 => self.bright_red,
      10 => self.bright_green,
      11 => self.bright_yellow,
      12 => self.bright_blue,
      13 => self.bright_magenta,
      14 => self.bright_cyan,
      15 => self.bright_white,
      // 216-color cube (16-231)
      16..=231 => {
        let idx = idx - 16;
        let r = (idx / 36) % 6;
        let g = (idx / 6) % 6;
        let b = idx % 6;
        (
          if r == 0 { 0 } else { 55 + r * 40 },
          if g == 0 { 0 } else { 55 + g * 40 },
          if b == 0 { 0 } else { 55 + b * 40 },
        )
      }
      // Grayscale (232-255)
      232..=255 => {
        let gray = 8 + (idx - 232) * 10;
        (gray, gray, gray)
      }
    }
  }
}

/// Extract renderable cells from terminal content.
///
/// This function takes ownership of the RenderableContent since
/// its display_iter is a consuming iterator.
pub fn extract_cells(
  content: RenderableContent<'_>,
  colors: &ColorScheme,
  cols: usize,
  rows: usize,
) -> Vec<RenderCell> {
  let mut cells = Vec::with_capacity(cols * rows);

  for cell in content.display_iter {
    let point = cell.point;

    // Skip cells outside viewport
    if point.column.0 >= cols || point.line.0 as usize >= rows {
      continue;
    }

    // Skip wide character spacers - these are placeholder cells that follow
    // double-width characters (like CJK characters). The actual character
    // is in the cell before this one.
    let flags = cell.flags;
    if flags.contains(CellFlags::WIDE_CHAR_SPACER) {
      continue;
    }

    let c = cell.c;

    // Get colors, handling inverse
    let mut fg = colors.resolve(cell.fg, true);
    let mut bg = colors.resolve(cell.bg, false);

    if flags.contains(CellFlags::INVERSE) {
      std::mem::swap(&mut fg, &mut bg);
    }

    // Check if this is a wide character (takes 2 cells)
    let is_wide = flags.contains(CellFlags::WIDE_CHAR);

    cells.push(RenderCell {
      col: point.column.0 as u16,
      row: point.line.0 as u16,
      c,
      fg,
      bg,
      flags: CellStyle::from(flags),
      is_wide,
    });
  }

  cells
}

/// Terminal cursor information for rendering.
#[derive(Debug, Clone)]
pub struct CursorInfo {
  pub col:     u16,
  pub row:     u16,
  pub shape:   CursorShape,
  pub visible: bool,
}

/// Cursor shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
  Block,
  Underline,
  Beam,
}

impl Default for CursorShape {
  fn default() -> Self {
    Self::Block
  }
}

#[cfg(test)]
mod tests {
  use alacritty_terminal::{
    term::cell::Flags as CellFlags,
    vte::ansi::{
      Color,
      NamedColor,
      Rgb,
    },
  };

  use super::*;

  // ============================================================
  // Color Scheme Resolution Tests
  // ============================================================

  mod color_resolution {
    use super::*;

    #[test]
    fn test_resolve_named_colors() {
      let scheme = ColorScheme::default();

      assert_eq!(scheme.resolve(Color::Named(NamedColor::Black), true), scheme.black);
      assert_eq!(scheme.resolve(Color::Named(NamedColor::Red), true), scheme.red);
      assert_eq!(scheme.resolve(Color::Named(NamedColor::Green), true), scheme.green);
      assert_eq!(scheme.resolve(Color::Named(NamedColor::Yellow), true), scheme.yellow);
      assert_eq!(scheme.resolve(Color::Named(NamedColor::Blue), true), scheme.blue);
      assert_eq!(scheme.resolve(Color::Named(NamedColor::Magenta), true), scheme.magenta);
      assert_eq!(scheme.resolve(Color::Named(NamedColor::Cyan), true), scheme.cyan);
      assert_eq!(scheme.resolve(Color::Named(NamedColor::White), true), scheme.white);
    }

    #[test]
    fn test_resolve_bright_colors() {
      let scheme = ColorScheme::default();

      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightBlack), true),
        scheme.bright_black
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightRed), true),
        scheme.bright_red
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightGreen), true),
        scheme.bright_green
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightYellow), true),
        scheme.bright_yellow
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightBlue), true),
        scheme.bright_blue
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightMagenta), true),
        scheme.bright_magenta
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightCyan), true),
        scheme.bright_cyan
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::BrightWhite), true),
        scheme.bright_white
      );
    }

    #[test]
    fn test_resolve_foreground_background_cursor() {
      let scheme = ColorScheme::default();

      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::Foreground), true),
        scheme.foreground
      );
      assert_eq!(
        scheme.resolve(Color::Named(NamedColor::Background), false),
        scheme.background
      );
      assert_eq!(scheme.resolve(Color::Named(NamedColor::Cursor), true), scheme.cursor);
    }

    #[test]
    fn test_resolve_indexed_standard_16() {
      let scheme = ColorScheme::default();

      // Indexed 0-7 map to standard colors
      assert_eq!(scheme.resolve(Color::Indexed(0), true), scheme.black);
      assert_eq!(scheme.resolve(Color::Indexed(1), true), scheme.red);
      assert_eq!(scheme.resolve(Color::Indexed(2), true), scheme.green);
      assert_eq!(scheme.resolve(Color::Indexed(3), true), scheme.yellow);
      assert_eq!(scheme.resolve(Color::Indexed(4), true), scheme.blue);
      assert_eq!(scheme.resolve(Color::Indexed(5), true), scheme.magenta);
      assert_eq!(scheme.resolve(Color::Indexed(6), true), scheme.cyan);
      assert_eq!(scheme.resolve(Color::Indexed(7), true), scheme.white);

      // Indexed 8-15 map to bright colors
      assert_eq!(scheme.resolve(Color::Indexed(8), true), scheme.bright_black);
      assert_eq!(scheme.resolve(Color::Indexed(9), true), scheme.bright_red);
      assert_eq!(scheme.resolve(Color::Indexed(10), true), scheme.bright_green);
      assert_eq!(scheme.resolve(Color::Indexed(11), true), scheme.bright_yellow);
      assert_eq!(scheme.resolve(Color::Indexed(12), true), scheme.bright_blue);
      assert_eq!(scheme.resolve(Color::Indexed(13), true), scheme.bright_magenta);
      assert_eq!(scheme.resolve(Color::Indexed(14), true), scheme.bright_cyan);
      assert_eq!(scheme.resolve(Color::Indexed(15), true), scheme.bright_white);
    }

    #[test]
    fn test_resolve_216_color_cube() {
      let scheme = ColorScheme::default();

      // Index 16 = first cube color (0,0,0) -> black
      let (r, g, b) = scheme.resolve(Color::Indexed(16), true);
      assert_eq!((r, g, b), (0, 0, 0));

      // Index 231 = last cube color (5,5,5) -> near white
      let (r, g, b) = scheme.resolve(Color::Indexed(231), true);
      assert_eq!((r, g, b), (255, 255, 255));

      // Pure red: r=5, g=0, b=0 -> index 16 + 5*36 = 196
      let (r, g, b) = scheme.resolve(Color::Indexed(196), true);
      assert_eq!((r, g, b), (255, 0, 0));

      // Pure green: r=0, g=5, b=0 -> index 16 + 5*6 = 46
      let (r, g, b) = scheme.resolve(Color::Indexed(46), true);
      assert_eq!((r, g, b), (0, 255, 0));

      // Pure blue: r=0, g=0, b=5 -> index 16 + 5 = 21
      let (r, g, b) = scheme.resolve(Color::Indexed(21), true);
      assert_eq!((r, g, b), (0, 0, 255));
    }

    #[test]
    fn test_resolve_216_color_cube_formula() {
      let scheme = ColorScheme::default();

      // Test the color cube formula: for r,g,b in 0..6
      // index = 16 + r*36 + g*6 + b
      // color = (r==0 ? 0 : 55+r*40, g==0 ? 0 : 55+g*40, b==0 ? 0 : 55+b*40)
      for r in 0..6u8 {
        for g in 0..6u8 {
          for b in 0..6u8 {
            let idx = 16 + r * 36 + g * 6 + b;
            let expected_r = if r == 0 { 0 } else { 55 + r * 40 };
            let expected_g = if g == 0 { 0 } else { 55 + g * 40 };
            let expected_b = if b == 0 { 0 } else { 55 + b * 40 };

            let (actual_r, actual_g, actual_b) = scheme.resolve(Color::Indexed(idx), true);
            assert_eq!(
              (actual_r, actual_g, actual_b),
              (expected_r, expected_g, expected_b),
              "Color cube index {} (r={}, g={}, b={}) mismatch",
              idx,
              r,
              g,
              b
            );
          }
        }
      }
    }

    #[test]
    fn test_resolve_grayscale_ramp() {
      let scheme = ColorScheme::default();

      // Grayscale 232-255: gray = 8 + (idx - 232) * 10
      // Index 232 = gray level 8
      let (r, g, b) = scheme.resolve(Color::Indexed(232), true);
      assert_eq!(r, g);
      assert_eq!(g, b);
      assert_eq!(r, 8);

      // Index 243 = gray level 8 + 11*10 = 118
      let (r, g, b) = scheme.resolve(Color::Indexed(243), true);
      assert_eq!(r, g);
      assert_eq!(g, b);
      assert_eq!(r, 118);

      // Index 255 = gray level 8 + 23*10 = 238
      let (r, g, b) = scheme.resolve(Color::Indexed(255), true);
      assert_eq!(r, g);
      assert_eq!(g, b);
      assert_eq!(r, 238);
    }

    #[test]
    fn test_resolve_true_color_rgb() {
      let scheme = ColorScheme::default();

      let rgb = Rgb { r: 128, g: 64, b: 32 };
      assert_eq!(scheme.resolve(Color::Spec(rgb), true), (128, 64, 32));

      let rgb = Rgb { r: 0, g: 255, b: 128 };
      assert_eq!(scheme.resolve(Color::Spec(rgb), true), (0, 255, 128));

      let rgb = Rgb { r: 255, g: 255, b: 255 };
      assert_eq!(scheme.resolve(Color::Spec(rgb), true), (255, 255, 255));
    }
  }

  // ============================================================
  // Cell Style Flag Tests
  // ============================================================

  mod cell_style {
    use super::*;

    #[test]
    fn test_cell_style_from_flags_empty() {
      let style = CellStyle::from(CellFlags::empty());
      assert!(!style.bold);
      assert!(!style.italic);
      assert!(!style.underline);
      assert!(!style.strikethrough);
      assert!(!style.dim);
      assert!(!style.inverse);
    }

    #[test]
    fn test_cell_style_from_flags_bold() {
      let style = CellStyle::from(CellFlags::BOLD);
      assert!(style.bold);
      assert!(!style.italic);
      assert!(!style.underline);
    }

    #[test]
    fn test_cell_style_from_flags_italic() {
      let style = CellStyle::from(CellFlags::ITALIC);
      assert!(!style.bold);
      assert!(style.italic);
      assert!(!style.underline);
    }

    #[test]
    fn test_cell_style_from_flags_underline() {
      let style = CellStyle::from(CellFlags::UNDERLINE);
      assert!(style.underline);
    }

    #[test]
    fn test_cell_style_from_flags_double_underline() {
      let style = CellStyle::from(CellFlags::DOUBLE_UNDERLINE);
      assert!(style.underline);
    }

    #[test]
    fn test_cell_style_from_flags_undercurl() {
      let style = CellStyle::from(CellFlags::UNDERCURL);
      assert!(style.underline);
    }

    #[test]
    fn test_cell_style_from_flags_strikethrough() {
      let style = CellStyle::from(CellFlags::STRIKEOUT);
      assert!(style.strikethrough);
    }

    #[test]
    fn test_cell_style_from_flags_dim() {
      let style = CellStyle::from(CellFlags::DIM);
      assert!(style.dim);
    }

    #[test]
    fn test_cell_style_from_flags_inverse() {
      let style = CellStyle::from(CellFlags::INVERSE);
      assert!(style.inverse);
    }

    #[test]
    fn test_cell_style_from_flags_combined() {
      let flags = CellFlags::BOLD | CellFlags::ITALIC | CellFlags::STRIKEOUT;
      let style = CellStyle::from(flags);
      assert!(style.bold);
      assert!(style.italic);
      assert!(style.strikethrough);
      assert!(!style.underline);
      assert!(!style.dim);
      assert!(!style.inverse);
    }

    #[test]
    fn test_cell_style_from_flags_all() {
      let flags = CellFlags::BOLD
        | CellFlags::ITALIC
        | CellFlags::UNDERLINE
        | CellFlags::STRIKEOUT
        | CellFlags::DIM
        | CellFlags::INVERSE;
      let style = CellStyle::from(flags);
      assert!(style.bold);
      assert!(style.italic);
      assert!(style.underline);
      assert!(style.strikethrough);
      assert!(style.dim);
      assert!(style.inverse);
    }
  }

  // ============================================================
  // Cursor Shape Tests
  // ============================================================

  mod cursor_shape {
    use super::*;

    #[test]
    fn test_cursor_shape_default_is_block() {
      assert_eq!(CursorShape::default(), CursorShape::Block);
    }

    #[test]
    fn test_cursor_shapes_distinct() {
      assert_ne!(CursorShape::Block, CursorShape::Beam);
      assert_ne!(CursorShape::Block, CursorShape::Underline);
      assert_ne!(CursorShape::Beam, CursorShape::Underline);
    }
  }

  // ============================================================
  // Cell Extraction Tests
  // ============================================================

  mod cell_extraction {
    use super::*;
    use crate::test_utils::{
      feed_str,
      test_colors,
      test_term,
    };

    #[test]
    fn test_extract_cells_basic() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "Hello");

      let colors = test_colors();
      let content = term.renderable_content();
      let cells = extract_cells(content, &colors, 10, 5);

      // Find the 'H' cell
      let h_cell = cells.iter().find(|c| c.c == 'H');
      assert!(h_cell.is_some(), "Should find 'H' cell");
      let h = h_cell.unwrap();
      assert_eq!(h.col, 0);
      assert_eq!(h.row, 0);

      // Find the 'o' cell
      let o_cell = cells.iter().find(|c| c.c == 'o');
      assert!(o_cell.is_some());
      assert_eq!(o_cell.unwrap().col, 4);
    }

    #[test]
    fn test_extract_cells_multiline() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "ABC\nDEF\nGHI");

      let colors = test_colors();
      let content = term.renderable_content();
      let cells = extract_cells(content, &colors, 10, 5);

      // Check first line
      let a_cell = cells.iter().find(|c| c.c == 'A');
      assert!(a_cell.is_some());
      assert_eq!(a_cell.unwrap().row, 0);

      // Check second line
      let d_cell = cells.iter().find(|c| c.c == 'D');
      assert!(d_cell.is_some());
      assert_eq!(d_cell.unwrap().row, 1);

      // Check third line
      let g_cell = cells.iter().find(|c| c.c == 'G');
      assert!(g_cell.is_some());
      assert_eq!(g_cell.unwrap().row, 2);
    }

    #[test]
    fn test_extract_cells_with_colors() {
      let mut term = test_term(20, 5);
      // SGR 31 = red foreground
      feed_str(&mut term, "\x1b[31mRed\x1b[0m");

      let colors = test_colors();
      let content = term.renderable_content();
      let cells = extract_cells(content, &colors, 20, 5);

      let r_cell = cells.iter().find(|c| c.c == 'R');
      assert!(r_cell.is_some());
      let r = r_cell.unwrap();
      // Foreground should be red from color scheme
      assert_eq!(r.fg, colors.red);
    }

    #[test]
    fn test_extract_cells_inverse_swaps_colors() {
      let mut term = test_term(20, 5);
      // SGR 7 = inverse video
      feed_str(&mut term, "\x1b[7mI\x1b[0m");

      let colors = test_colors();
      let content = term.renderable_content();
      let cells = extract_cells(content, &colors, 20, 5);

      let i_cell = cells.iter().find(|c| c.c == 'I');
      assert!(i_cell.is_some());
      let i = i_cell.unwrap();

      // With inverse, fg and bg should be swapped
      // Default fg becomes bg, default bg becomes fg
      assert_eq!(i.fg, colors.background);
      assert_eq!(i.bg, colors.foreground);
    }

    #[test]
    fn test_extract_cells_viewport_bounds() {
      let mut term = test_term(5, 3);
      feed_str(&mut term, "ABCDEFGHIJ"); // More chars than fit in one row

      let colors = test_colors();
      let content = term.renderable_content();
      let cells = extract_cells(content, &colors, 5, 3);

      // All cells should be within viewport bounds
      for cell in &cells {
        assert!(cell.col < 5, "Cell col {} should be < 5", cell.col);
        assert!(cell.row < 3, "Cell row {} should be < 3", cell.row);
      }
    }

    #[test]
    fn test_extract_cells_preserves_style_flags() {
      let mut term = test_term(20, 5);
      // Bold and italic text
      feed_str(&mut term, "\x1b[1;3mBI\x1b[0m");

      let colors = test_colors();
      let content = term.renderable_content();
      let cells = extract_cells(content, &colors, 20, 5);

      let b_cell = cells.iter().find(|c| c.c == 'B');
      assert!(b_cell.is_some());
      let b = b_cell.unwrap();
      assert!(b.flags.bold);
      assert!(b.flags.italic);
    }
  }

  // ============================================================
  // Color Scheme Default Tests
  // ============================================================

  mod color_scheme_default {
    use super::*;

    #[test]
    fn test_default_scheme_has_distinct_colors() {
      let scheme = ColorScheme::default();

      // Foreground and background should be different
      assert_ne!(scheme.foreground, scheme.background);

      // Standard colors should be distinct
      assert_ne!(scheme.red, scheme.green);
      assert_ne!(scheme.red, scheme.blue);
      assert_ne!(scheme.green, scheme.blue);
    }

    #[test]
    fn test_default_scheme_bright_colors_brighter() {
      let scheme = ColorScheme::default();

      // Bright red should have higher values than regular red
      let (r1, _, _) = scheme.red;
      let (r2, _, _) = scheme.bright_red;
      assert!(r2 >= r1, "Bright red should be >= regular red");
    }
  }
}
