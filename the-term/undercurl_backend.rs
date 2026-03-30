//! Crossterm backend variant that renders underlines as undercurls.

use std::{
  io::{
    self,
    Write,
  },
  time::Instant,
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TerminalIoPerfStats {
  pub bytes:                   u64,
  pub write_calls:             u64,
  pub flush_calls:             u64,
  pub flush_ms:                f64,
  pub diff_cells:              u64,
  pub move_ops:                u64,
  pub modifier_attr_ops:       u64,
  pub color_changes:           u64,
  pub underline_color_changes: u64,
  pub underline_mode_changes:  u64,
  pub glyphs:                  u64,
  pub clear_ops:               u64,
}

#[derive(Debug)]
struct CountingWriter<W: Write> {
  inner: W,
  stats: TerminalIoPerfStats,
}

impl<W: Write> CountingWriter<W> {
  const fn new(inner: W) -> Self {
    Self {
      inner,
      stats: TerminalIoPerfStats {
        bytes:                   0,
        write_calls:             0,
        flush_calls:             0,
        flush_ms:                0.0,
        diff_cells:              0,
        move_ops:                0,
        modifier_attr_ops:       0,
        color_changes:           0,
        underline_color_changes: 0,
        underline_mode_changes:  0,
        glyphs:                  0,
        clear_ops:               0,
      },
    }
  }

  fn reset_perf_stats(&mut self) {
    self.stats = TerminalIoPerfStats::default();
  }

  fn take_perf_stats(&mut self) -> TerminalIoPerfStats {
    std::mem::take(&mut self.stats)
  }
}

impl<W: Write> Write for CountingWriter<W> {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    let written = self.inner.write(buf)?;
    self.stats.bytes = self.stats.bytes.saturating_add(written as u64);
    self.stats.write_calls = self.stats.write_calls.saturating_add(1);
    Ok(written)
  }

  fn flush(&mut self) -> io::Result<()> {
    let start = Instant::now();
    let result = self.inner.flush();
    self.stats.flush_calls = self.stats.flush_calls.saturating_add(1);
    self.stats.flush_ms += start.elapsed().as_secs_f64() * 1000.0;
    result
  }
}

#[derive(Debug)]
pub struct UndercurlCrosstermBackend<W: Write> {
  writer: CountingWriter<W>,
}

impl<W: Write> UndercurlCrosstermBackend<W> {
  pub const fn new(writer: W) -> Self {
    Self {
      writer: CountingWriter::new(writer),
    }
  }

  pub fn reset_perf_stats(&mut self) {
    self.writer.reset_perf_stats();
  }

  pub fn take_perf_stats(&mut self) -> TerminalIoPerfStats {
    self.writer.take_perf_stats()
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
      self.writer.stats.diff_cells = self.writer.stats.diff_cells.saturating_add(1);
      if !matches!(last_pos, Some((px, py)) if x == px + 1 && y == py) {
        self.writer.stats.move_ops = self.writer.stats.move_ops.saturating_add(1);
        queue!(self.writer, MoveTo(x, y))?;
      }
      last_pos = Some((x, y));

      if cell.modifier != modifier {
        self.writer.stats.modifier_attr_ops = self.writer.stats.modifier_attr_ops.saturating_add(
          ModifierDiff {
            from: modifier,
            to:   cell.modifier,
          }
          .queue(&mut self.writer)? as u64,
        );
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
        self.writer.stats.underline_mode_changes =
          self.writer.stats.underline_mode_changes.saturating_add(1);
        match desired_underline_mode {
          UnderlineMode::None => queue!(self.writer, SetAttribute(CAttribute::NoUnderline))?,
          UnderlineMode::Straight => queue!(self.writer, SetAttribute(CAttribute::Underlined))?,
          UnderlineMode::Curled => queue!(self.writer, SetAttribute(CAttribute::Undercurled))?,
        }
        underline_mode = desired_underline_mode;
      }

      if cell.fg != fg || cell.bg != bg {
        self.writer.stats.color_changes = self.writer.stats.color_changes.saturating_add(1);
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
        self.writer.stats.underline_color_changes =
          self.writer.stats.underline_color_changes.saturating_add(1);
        queue!(
          self.writer,
          SetUnderlineColor(ratatui_color_to_crossterm(cell.underline_color))
        )?;
        underline_color = cell.underline_color;
      }

      self.writer.stats.glyphs = self.writer.stats.glyphs.saturating_add(1);
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
    self.writer.stats.clear_ops = self.writer.stats.clear_ops.saturating_add(1);
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
  fn queue<W: Write>(self, mut writer: W) -> io::Result<usize> {
    let mut ops = 0usize;
    let removed = self.from - self.to;
    if removed.contains(Modifier::REVERSED) {
      queue!(writer, SetAttribute(CAttribute::NoReverse))?;
      ops += 1;
    }
    if removed.contains(Modifier::BOLD) {
      queue!(writer, SetAttribute(CAttribute::NormalIntensity))?;
      ops += 1;
      if self.to.contains(Modifier::DIM) {
        queue!(writer, SetAttribute(CAttribute::Dim))?;
        ops += 1;
      }
    }
    if removed.contains(Modifier::ITALIC) {
      queue!(writer, SetAttribute(CAttribute::NoItalic))?;
      ops += 1;
    }
    if removed.contains(Modifier::DIM) {
      queue!(writer, SetAttribute(CAttribute::NormalIntensity))?;
      ops += 1;
    }
    if removed.contains(Modifier::CROSSED_OUT) {
      queue!(writer, SetAttribute(CAttribute::NotCrossedOut))?;
      ops += 1;
    }
    if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
      queue!(writer, SetAttribute(CAttribute::NoBlink))?;
      ops += 1;
    }
    if removed.contains(Modifier::HIDDEN) {
      queue!(writer, SetAttribute(CAttribute::NoHidden))?;
      ops += 1;
    }

    let added = self.to - self.from;
    if added.contains(Modifier::REVERSED) {
      queue!(writer, SetAttribute(CAttribute::Reverse))?;
      ops += 1;
    }
    if added.contains(Modifier::BOLD) {
      queue!(writer, SetAttribute(CAttribute::Bold))?;
      ops += 1;
    }
    if added.contains(Modifier::ITALIC) {
      queue!(writer, SetAttribute(CAttribute::Italic))?;
      ops += 1;
    }
    if added.contains(Modifier::DIM) {
      queue!(writer, SetAttribute(CAttribute::Dim))?;
      ops += 1;
    }
    if added.contains(Modifier::CROSSED_OUT) {
      queue!(writer, SetAttribute(CAttribute::CrossedOut))?;
      ops += 1;
    }
    if added.contains(Modifier::SLOW_BLINK) {
      queue!(writer, SetAttribute(CAttribute::SlowBlink))?;
      ops += 1;
    }
    if added.contains(Modifier::RAPID_BLINK) {
      queue!(writer, SetAttribute(CAttribute::RapidBlink))?;
      ops += 1;
    }
    if added.contains(Modifier::HIDDEN) {
      queue!(writer, SetAttribute(CAttribute::Hidden))?;
      ops += 1;
    }

    Ok(ops)
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

#[cfg(test)]
mod tests {
  use ratatui::{
    backend::Backend,
    buffer::Cell,
    style::{
      Color,
      Modifier,
      Style,
    },
  };

  use super::UndercurlCrosstermBackend;

  #[test]
  fn draw_collects_diff_and_io_counters() {
    let mut backend = UndercurlCrosstermBackend::new(Vec::new());
    let mut first = Cell::default();
    first
      .set_symbol("a")
      .set_fg(Color::Blue)
      .set_bg(Color::Black)
      .set_style(
        Style::default()
          .fg(Color::Blue)
          .bg(Color::Black)
          .underline_color(Color::LightRed)
          .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
      );
    let mut second = Cell::default();
    second
      .set_symbol("b")
      .set_fg(Color::Blue)
      .set_bg(Color::Black);

    backend.reset_perf_stats();
    backend
      .draw(vec![(0, 0, &first), (1, 0, &second)].into_iter())
      .expect("draw");
    backend.flush().expect("flush");
    let stats = backend.take_perf_stats();

    assert_eq!(stats.diff_cells, 2);
    assert_eq!(stats.glyphs, 2);
    assert_eq!(stats.move_ops, 1);
    assert!(stats.modifier_attr_ops >= 1);
    assert_eq!(stats.color_changes, 1);
    assert!(stats.underline_color_changes >= 1);
    assert!(stats.underline_mode_changes >= 1);
    assert!(stats.bytes > 0);
    assert!(stats.write_calls > 0);
    assert!(stats.flush_calls >= 1);
  }
}
