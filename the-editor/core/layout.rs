use super::graphics::Rect;

/// Constraint for layout sizing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
  /// Fixed size in cells
  Length(u16),
  /// Percentage of available space (0-100)
  Percentage(u16),
  /// Take up remaining space with a weight ratio
  Ratio(u32, u32),
  /// Minimum size, will grow if space available
  Min(u16),
  /// Maximum size, will shrink if needed
  Max(u16),
  /// Fill remaining space equally
  Fill(u16), // weight for distribution
}

/// Direction for layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Horizontal,
  Vertical,
}

/// Alignment within a container
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
  Start,
  Center,
  End,
}

/// A layout engine for arranging UI elements
#[derive(Debug, Clone)]
pub struct Layout {
  direction:   Direction,
  constraints: Vec<Constraint>,
  spacing:     u16,
  alignment:   Alignment,
}

impl Layout {
  /// Create a new horizontal layout
  pub fn horizontal() -> Self {
    Self {
      direction:   Direction::Horizontal,
      constraints: Vec::new(),
      spacing:     0,
      alignment:   Alignment::Start,
    }
  }

  /// Create a new vertical layout
  pub fn vertical() -> Self {
    Self {
      direction:   Direction::Vertical,
      constraints: Vec::new(),
      spacing:     0,
      alignment:   Alignment::Start,
    }
  }

  /// Set the constraints for this layout
  pub fn constraints(mut self, constraints: Vec<Constraint>) -> Self {
    self.constraints = constraints;
    self
  }

  /// Set spacing between elements
  pub fn spacing(mut self, spacing: u16) -> Self {
    self.spacing = spacing;
    self
  }

  /// Set alignment of elements
  pub fn alignment(mut self, alignment: Alignment) -> Self {
    self.alignment = alignment;
    self
  }

  /// Split the area into chunks according to constraints
  pub fn split(&self, area: Rect) -> Vec<Rect> {
    if self.constraints.is_empty() {
      return vec![area];
    }

    match self.direction {
      Direction::Horizontal => self.split_horizontal(area),
      Direction::Vertical => self.split_vertical(area),
    }
  }

  fn split_horizontal(&self, area: Rect) -> Vec<Rect> {
    let mut chunks = Vec::with_capacity(self.constraints.len());

    // Calculate total spacing
    let total_spacing = self.spacing * (self.constraints.len().saturating_sub(1)) as u16;
    let available_width = area.width.saturating_sub(total_spacing);

    // First pass: calculate fixed sizes and remaining space
    let mut remaining_width = available_width;
    let mut sizes = vec![0u16; self.constraints.len()];
    let mut fill_weights = Vec::new();

    for (i, constraint) in self.constraints.iter().enumerate() {
      match constraint {
        Constraint::Length(len) => {
          let size = (*len).min(remaining_width);
          sizes[i] = size;
          remaining_width = remaining_width.saturating_sub(size);
        },
        Constraint::Percentage(pct) => {
          let size = (available_width as u32 * (*pct as u32) / 100) as u16;
          let size = size.min(remaining_width);
          sizes[i] = size;
          remaining_width = remaining_width.saturating_sub(size);
        },
        Constraint::Min(min) => {
          let size = (*min).min(remaining_width);
          sizes[i] = size;
          remaining_width = remaining_width.saturating_sub(size);
        },
        Constraint::Max(max) => {
          let size = (*max).min(remaining_width);
          sizes[i] = size;
          remaining_width = remaining_width.saturating_sub(size);
        },
        Constraint::Ratio(num, denom) => {
          let size = if *denom > 0 {
            (available_width as u32 * num / denom) as u16
          } else {
            0
          };
          let size = size.min(remaining_width);
          sizes[i] = size;
          remaining_width = remaining_width.saturating_sub(size);
        },
        Constraint::Fill(weight) => {
          fill_weights.push((i, *weight));
        },
      }
    }

    // Second pass: distribute remaining space among Fill constraints
    if !fill_weights.is_empty() {
      let total_weight: u32 = fill_weights.iter().map(|(_, w)| *w as u32).sum();
      if total_weight > 0 {
        for &(i, weight) in &fill_weights {
          let size = (remaining_width as u32 * weight as u32 / total_weight) as u16;
          sizes[i] = size;
        }
      }
    }

    // Reclaim leftover columns caused by integer rounding so columns fill
    // the entire available width.
    let mut assigned_width: u16 = sizes.iter().sum();
    let leftover = available_width.saturating_sub(assigned_width);
    if leftover > 0 {
      if let Some(&(fill_index, _)) = fill_weights.last() {
        sizes[fill_index] = sizes[fill_index].saturating_add(leftover);
        assigned_width = sizes.iter().sum();
      }
    }

    let slack = available_width.saturating_sub(assigned_width);

    // Create rects
    let mut x = area.x;
    let alignment_offset = match self.alignment {
      Alignment::Start => 0,
      Alignment::Center => slack / 2,
      Alignment::End => slack,
    };
    x = x.saturating_add(alignment_offset);

    for size in sizes {
      chunks.push(Rect {
        x,
        y: area.y,
        width: size,
        height: area.height,
      });
      x = x.saturating_add(size).saturating_add(self.spacing);
    }

    chunks
  }

  fn split_vertical(&self, area: Rect) -> Vec<Rect> {
    let mut chunks = Vec::with_capacity(self.constraints.len());

    // Calculate total spacing
    let total_spacing = self.spacing * (self.constraints.len().saturating_sub(1)) as u16;
    let available_height = area.height.saturating_sub(total_spacing);

    // First pass: calculate fixed sizes and remaining space
    let mut remaining_height = available_height;
    let mut sizes = vec![0u16; self.constraints.len()];
    let mut fill_weights = Vec::new();

    for (i, constraint) in self.constraints.iter().enumerate() {
      match constraint {
        Constraint::Length(len) => {
          let size = (*len).min(remaining_height);
          sizes[i] = size;
          remaining_height = remaining_height.saturating_sub(size);
        },
        Constraint::Percentage(pct) => {
          let size = (available_height as u32 * (*pct as u32) / 100) as u16;
          let size = size.min(remaining_height);
          sizes[i] = size;
          remaining_height = remaining_height.saturating_sub(size);
        },
        Constraint::Min(min) => {
          let size = (*min).min(remaining_height);
          sizes[i] = size;
          remaining_height = remaining_height.saturating_sub(size);
        },
        Constraint::Max(max) => {
          let size = (*max).min(remaining_height);
          sizes[i] = size;
          remaining_height = remaining_height.saturating_sub(size);
        },
        Constraint::Ratio(num, denom) => {
          let size = if *denom > 0 {
            (available_height as u32 * num / denom) as u16
          } else {
            0
          };
          let size = size.min(remaining_height);
          sizes[i] = size;
          remaining_height = remaining_height.saturating_sub(size);
        },
        Constraint::Fill(weight) => {
          fill_weights.push((i, *weight));
        },
      }
    }

    // Second pass: distribute remaining space among Fill constraints
    if !fill_weights.is_empty() {
      let total_weight: u32 = fill_weights.iter().map(|(_, w)| *w as u32).sum();
      if total_weight > 0 {
        for &(i, weight) in &fill_weights {
          let size = (remaining_height as u32 * weight as u32 / total_weight) as u16;
          sizes[i] = size;
        }
      }
    }

    // Ensure leftover rows from rounding are handed back to Fill constraints.
    let mut assigned_height: u16 = sizes.iter().sum();
    let leftover = available_height.saturating_sub(assigned_height);
    if leftover > 0 {
      if let Some(&(fill_index, _)) = fill_weights.last() {
        sizes[fill_index] = sizes[fill_index].saturating_add(leftover);
        assigned_height = sizes.iter().sum();
      }
    }

    let slack = available_height.saturating_sub(assigned_height);

    // Create rects
    let mut y = area.y;
    let alignment_offset = match self.alignment {
      Alignment::Start => 0,
      Alignment::Center => slack / 2,
      Alignment::End => slack,
    };
    y = y.saturating_add(alignment_offset);

    for size in sizes {
      chunks.push(Rect {
        x: area.x,
        y,
        width: area.width,
        height: size,
      });
      y = y.saturating_add(size).saturating_add(self.spacing);
    }

    chunks
  }
}

/// Helper function to center a rect within a container
pub fn center(container: Rect, width: u16, height: u16) -> Rect {
  let x = container
    .x
    .saturating_add((container.width.saturating_sub(width)) / 2);
  let y = container
    .y
    .saturating_add((container.height.saturating_sub(height)) / 2);
  Rect {
    x,
    y,
    width,
    height,
  }
}

/// Helper function to align a rect within a container
pub fn align(container: Rect, width: u16, height: u16, alignment: Alignment) -> Rect {
  let (x, y) = match alignment {
    Alignment::Start => (container.x, container.y),
    Alignment::Center => {
      (
        container
          .x
          .saturating_add((container.width.saturating_sub(width)) / 2),
        container
          .y
          .saturating_add((container.height.saturating_sub(height)) / 2),
      )
    },
    Alignment::End => {
      (
        container
          .x
          .saturating_add(container.width.saturating_sub(width)),
        container
          .y
          .saturating_add(container.height.saturating_sub(height)),
      )
    },
  };

  Rect {
    x,
    y,
    width,
    height,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_horizontal_layout_fixed() {
    let area = Rect::new(0, 0, 100, 20);
    let layout = Layout::horizontal().constraints(vec![
      Constraint::Length(30),
      Constraint::Length(40),
      Constraint::Length(30),
    ]);

    let chunks = layout.split(area);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0], Rect::new(0, 0, 30, 20));
    assert_eq!(chunks[1], Rect::new(30, 0, 40, 20));
    assert_eq!(chunks[2], Rect::new(70, 0, 30, 20));
  }

  #[test]
  fn test_horizontal_layout_with_spacing() {
    let area = Rect::new(0, 0, 100, 20);
    let layout = Layout::horizontal()
      .constraints(vec![Constraint::Length(30), Constraint::Length(30)])
      .spacing(10);

    let chunks = layout.split(area);
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], Rect::new(0, 0, 30, 20));
    assert_eq!(chunks[1], Rect::new(40, 0, 30, 20)); // 30 + 10 spacing
  }

  #[test]
  fn test_vertical_layout_fixed() {
    let area = Rect::new(0, 0, 100, 60);
    let layout = Layout::vertical().constraints(vec![
      Constraint::Length(20),
      Constraint::Length(20),
      Constraint::Length(20),
    ]);

    let chunks = layout.split(area);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0], Rect::new(0, 0, 100, 20));
    assert_eq!(chunks[1], Rect::new(0, 20, 100, 20));
    assert_eq!(chunks[2], Rect::new(0, 40, 100, 20));
  }

  #[test]
  fn test_percentage_constraint() {
    let area = Rect::new(0, 0, 100, 20);
    let layout = Layout::horizontal()
      .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)]);

    let chunks = layout.split(area);
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].width, 50);
    assert_eq!(chunks[1].width, 50);
  }

  #[test]
  fn test_fill_constraint() {
    let area = Rect::new(0, 0, 100, 20);
    let layout = Layout::horizontal().constraints(vec![
      Constraint::Length(30),
      Constraint::Fill(1),
      Constraint::Length(20),
    ]);

    let chunks = layout.split(area);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].width, 30);
    assert_eq!(chunks[1].width, 50); // 100 - 30 - 20 = 50
    assert_eq!(chunks[2].width, 20);
  }

  #[test]
  fn test_fill_with_weights() {
    let area = Rect::new(0, 0, 100, 20);
    let layout = Layout::horizontal().constraints(vec![
      Constraint::Fill(1),
      Constraint::Fill(2),
      Constraint::Fill(1),
    ]);

    let chunks = layout.split(area);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].width, 25); // 1/4 of 100
    assert_eq!(chunks[1].width, 50); // 2/4 of 100
    assert_eq!(chunks[2].width, 25); // 1/4 of 100
  }

  #[test]
  fn test_center_helper() {
    let container = Rect::new(0, 0, 100, 50);
    let centered = center(container, 40, 20);

    assert_eq!(centered.x, 30); // (100 - 40) / 2
    assert_eq!(centered.y, 15); // (50 - 20) / 2
    assert_eq!(centered.width, 40);
    assert_eq!(centered.height, 20);
  }

  #[test]
  fn test_align_start() {
    let container = Rect::new(10, 10, 100, 50);
    let aligned = align(container, 40, 20, Alignment::Start);

    assert_eq!(aligned.x, 10);
    assert_eq!(aligned.y, 10);
  }

  #[test]
  fn test_align_end() {
    let container = Rect::new(10, 10, 100, 50);
    let aligned = align(container, 40, 20, Alignment::End);

    assert_eq!(aligned.x, 70); // 10 + (100 - 40)
    assert_eq!(aligned.y, 40); // 10 + (50 - 20)
  }
}
