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

    let c = cell.c;

    // Get colors, handling inverse
    let flags = cell.flags;
    let mut fg = colors.resolve(cell.fg, true);
    let mut bg = colors.resolve(cell.bg, false);

    if flags.contains(CellFlags::INVERSE) {
      std::mem::swap(&mut fg, &mut bg);
    }

    cells.push(RenderCell {
      col: point.column.0 as u16,
      row: point.line.0 as u16,
      c,
      fg,
      bg,
      flags: CellStyle::from(flags),
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
