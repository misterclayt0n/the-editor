//! Crossterm backend variant that renders underlines as undercurls.

use std::io::{
  self,
  Write,
};

use crossterm::{
  cursor::{
    Hide,
    MoveTo,
    Show,
  },
  execute,
  queue,
  style::{
    Attribute as CAttribute,
    Color as CColor,
    Colors,
    Print,
    SetAttribute,
    SetBackgroundColor,
    SetColors,
    SetForegroundColor,
    SetUnderlineColor,
  },
  terminal::{
    self,
    Clear,
  },
};
use ratatui::{
  backend::{
    Backend,
    ClearType,
    WindowSize,
  },
  buffer::Cell,
  layout::Size,
  prelude::Rect,
  style::{
    Color,
    Modifier,
  },
};

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct UndercurlCrosstermBackend<W: Write> {
  writer: W,
}

impl<W: Write> UndercurlCrosstermBackend<W> {
  pub const fn new(writer: W) -> Self {
    Self { writer }
  }
}

impl<W: Write> Write for UndercurlCrosstermBackend<W> {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.writer.write(buf)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.writer.flush()
  }
}

impl<W: Write> Backend for UndercurlCrosstermBackend<W> {
  fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
  where
    I: Iterator<Item = (u16, u16, &'a Cell)>,
  {
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut underline_color = Color::Reset;
    let mut modifier = Modifier::empty();
    let mut underline_mode = UnderlineMode::None;
    let mut last_pos: Option<(u16, u16)> = None;

    for (x, y, cell) in content {
      if !matches!(last_pos, Some((px, py)) if x == px + 1 && y == py) {
        queue!(self.writer, MoveTo(x, y))?;
      }
      last_pos = Some((x, y));

      if cell.modifier != modifier {
        ModifierDiff {
          from: modifier,
          to:   cell.modifier,
        }
        .queue(&mut self.writer)?;
        modifier = cell.modifier;
      }

      let desired_underline_mode = if cell.modifier.contains(Modifier::UNDERLINED) {
        if cell.underline_color != Color::Reset {
          UnderlineMode::Curled
        } else {
          UnderlineMode::Straight
        }
      } else {
        UnderlineMode::None
      };

      if desired_underline_mode != underline_mode {
        match desired_underline_mode {
          UnderlineMode::None => queue!(self.writer, SetAttribute(CAttribute::NoUnderline))?,
          UnderlineMode::Straight => queue!(self.writer, SetAttribute(CAttribute::Underlined))?,
          UnderlineMode::Curled => queue!(self.writer, SetAttribute(CAttribute::Undercurled))?,
        }
        underline_mode = desired_underline_mode;
      }

      if cell.fg != fg || cell.bg != bg {
        queue!(
          self.writer,
          SetColors(Colors::new(
            ratatui_color_to_crossterm(cell.fg),
            ratatui_color_to_crossterm(cell.bg)
          ))
        )?;
        fg = cell.fg;
        bg = cell.bg;
      }

      if cell.underline_color != underline_color {
        queue!(
          self.writer,
          SetUnderlineColor(ratatui_color_to_crossterm(cell.underline_color))
        )?;
        underline_color = cell.underline_color;
      }

      queue!(self.writer, Print(cell.symbol()))?;
    }

    queue!(
      self.writer,
      SetForegroundColor(CColor::Reset),
      SetBackgroundColor(CColor::Reset),
      SetUnderlineColor(CColor::Reset),
      SetAttribute(CAttribute::Reset),
    )
  }

  fn hide_cursor(&mut self) -> io::Result<()> {
    execute!(self.writer, Hide)
  }

  fn show_cursor(&mut self) -> io::Result<()> {
    execute!(self.writer, Show)
  }

  fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
    crossterm::cursor::position()
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))
  }

  fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
    execute!(self.writer, MoveTo(x, y))
  }

  fn clear(&mut self) -> io::Result<()> {
    self.clear_region(ClearType::All)
  }

  fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
    execute!(
      self.writer,
      Clear(match clear_type {
        ClearType::All => crossterm::terminal::ClearType::All,
        ClearType::AfterCursor => crossterm::terminal::ClearType::FromCursorDown,
        ClearType::BeforeCursor => crossterm::terminal::ClearType::FromCursorUp,
        ClearType::CurrentLine => crossterm::terminal::ClearType::CurrentLine,
        ClearType::UntilNewLine => crossterm::terminal::ClearType::UntilNewLine,
      })
    )
  }

  fn append_lines(&mut self, n: u16) -> io::Result<()> {
    for _ in 0..n {
      queue!(self.writer, Print("\n"))?;
    }
    self.writer.flush()
  }

  fn size(&self) -> io::Result<Rect> {
    let (width, height) = terminal::size()?;
    Ok(Rect::new(0, 0, width, height))
  }

  fn window_size(&mut self) -> io::Result<WindowSize> {
    let crossterm::terminal::WindowSize {
      columns,
      rows,
      width,
      height,
    } = terminal::window_size()?;
    Ok(WindowSize {
      columns_rows: Size {
        width:  columns,
        height: rows,
      },
      pixels:       Size { width, height },
    })
  }

  fn flush(&mut self) -> io::Result<()> {
    self.writer.flush()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnderlineMode {
  None,
  Straight,
  Curled,
}

struct ModifierDiff {
  from: Modifier,
  to:   Modifier,
}

impl ModifierDiff {
  fn queue<W: Write>(self, mut writer: W) -> io::Result<()> {
    let removed = self.from - self.to;
    if removed.contains(Modifier::REVERSED) {
      queue!(writer, SetAttribute(CAttribute::NoReverse))?;
    }
    if removed.contains(Modifier::BOLD) {
      queue!(writer, SetAttribute(CAttribute::NormalIntensity))?;
      if self.to.contains(Modifier::DIM) {
        queue!(writer, SetAttribute(CAttribute::Dim))?;
      }
    }
    if removed.contains(Modifier::ITALIC) {
      queue!(writer, SetAttribute(CAttribute::NoItalic))?;
    }
    if removed.contains(Modifier::DIM) {
      queue!(writer, SetAttribute(CAttribute::NormalIntensity))?;
    }
    if removed.contains(Modifier::CROSSED_OUT) {
      queue!(writer, SetAttribute(CAttribute::NotCrossedOut))?;
    }
    if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
      queue!(writer, SetAttribute(CAttribute::NoBlink))?;
    }
    if removed.contains(Modifier::HIDDEN) {
      queue!(writer, SetAttribute(CAttribute::NoHidden))?;
    }

    let added = self.to - self.from;
    if added.contains(Modifier::REVERSED) {
      queue!(writer, SetAttribute(CAttribute::Reverse))?;
    }
    if added.contains(Modifier::BOLD) {
      queue!(writer, SetAttribute(CAttribute::Bold))?;
    }
    if added.contains(Modifier::ITALIC) {
      queue!(writer, SetAttribute(CAttribute::Italic))?;
    }
    if added.contains(Modifier::DIM) {
      queue!(writer, SetAttribute(CAttribute::Dim))?;
    }
    if added.contains(Modifier::CROSSED_OUT) {
      queue!(writer, SetAttribute(CAttribute::CrossedOut))?;
    }
    if added.contains(Modifier::SLOW_BLINK) {
      queue!(writer, SetAttribute(CAttribute::SlowBlink))?;
    }
    if added.contains(Modifier::RAPID_BLINK) {
      queue!(writer, SetAttribute(CAttribute::RapidBlink))?;
    }
    if added.contains(Modifier::HIDDEN) {
      queue!(writer, SetAttribute(CAttribute::Hidden))?;
    }

    Ok(())
  }
}

fn ratatui_color_to_crossterm(color: Color) -> CColor {
  match color {
    Color::Reset => CColor::Reset,
    Color::Black => CColor::Black,
    Color::Red => CColor::DarkRed,
    Color::Green => CColor::DarkGreen,
    Color::Yellow => CColor::DarkYellow,
    Color::Blue => CColor::DarkBlue,
    Color::Magenta => CColor::DarkMagenta,
    Color::Cyan => CColor::DarkCyan,
    Color::Gray => CColor::Grey,
    Color::DarkGray => CColor::DarkGrey,
    Color::LightRed => CColor::Red,
    Color::LightGreen => CColor::Green,
    Color::LightYellow => CColor::Yellow,
    Color::LightBlue => CColor::Blue,
    Color::LightMagenta => CColor::Magenta,
    Color::LightCyan => CColor::Cyan,
    Color::White => CColor::White,
    Color::Rgb(r, g, b) => CColor::Rgb { r, g, b },
    Color::Indexed(value) => CColor::AnsiValue(value),
  }
}
