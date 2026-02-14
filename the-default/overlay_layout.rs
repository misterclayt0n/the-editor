#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OverlayRect {
  pub x:      u16,
  pub y:      u16,
  pub width:  u16,
  pub height: u16,
}

impl OverlayRect {
  pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
    Self {
      x,
      y,
      width,
      height,
    }
  }
}

pub fn completion_panel_rect(
  area: OverlayRect,
  panel_width: u16,
  panel_height: u16,
  editor_cursor: Option<(u16, u16)>,
) -> OverlayRect {
  let width = panel_width.min(area.width).max(1);
  let desired_height = panel_height.min(area.height).max(1);
  let center_x = area.x + (area.width.saturating_sub(width)) / 2;
  let center_y = area.y + (area.height.saturating_sub(desired_height)) / 2;
  let Some((cursor_x, cursor_y)) = editor_cursor else {
    return OverlayRect::new(center_x, center_y, width, desired_height);
  };
  if area.width == 0 || area.height == 0 {
    return OverlayRect::new(center_x, center_y, width, desired_height);
  }

  let max_x = area.x + area.width.saturating_sub(width);
  let cursor_x = cursor_x.clamp(area.x, area.x + area.width.saturating_sub(1));
  let cursor_y = cursor_y.clamp(area.y, area.y + area.height.saturating_sub(1));

  let mut x = cursor_x.saturating_sub(1);
  x = x.clamp(area.x, max_x);

  let below_start = cursor_y.saturating_add(1).max(area.y);
  let below_space = area
    .y
    .saturating_add(area.height)
    .saturating_sub(below_start);
  let above_space = cursor_y.saturating_sub(area.y);
  let place_below = if below_space >= desired_height {
    true
  } else if above_space >= desired_height {
    false
  } else {
    below_space >= above_space
  };

  let (height, y) = if place_below {
    let height = desired_height.min(below_space.max(1));
    let y = below_start;
    (height, y)
  } else {
    let height = desired_height.min(above_space.max(1));
    let y = cursor_y.saturating_sub(height).max(area.y);
    (height, y)
  };

  OverlayRect::new(x, y, width, height)
}

pub fn completion_docs_panel_rect(
  area: OverlayRect,
  panel_width: u16,
  panel_height: u16,
  completion_rect: OverlayRect,
) -> Option<OverlayRect> {
  if area.width == 0 || area.height == 0 {
    return None;
  }
  let desired_width = panel_width.min(area.width).max(1);
  let desired_height = panel_height.min(area.height).max(1);
  let gap = 1u16;
  let min_side_width = 24u16;

  let area_right = area.x.saturating_add(area.width);
  let area_bottom = area.y.saturating_add(area.height);
  let right_x = completion_rect
    .x
    .saturating_add(completion_rect.width)
    .saturating_add(gap);
  let right_available_width = area_right.saturating_sub(right_x);
  let left_end = completion_rect.x.saturating_sub(gap);
  let left_available_width = left_end.saturating_sub(area.x);

  if right_available_width >= min_side_width || left_available_width >= min_side_width {
    let place_right = right_available_width >= min_side_width
      && (right_available_width >= left_available_width || left_available_width < min_side_width);
    let available_width = if place_right {
      right_available_width
    } else {
      left_available_width
    };
    let width = desired_width.min(available_width).max(1);
    // Keep docs top-aligned with the completion panel whenever possible.
    // If there is not enough vertical space, shrink height instead of shifting `y`.
    let y = completion_rect
      .y
      .max(area.y)
      .min(area_bottom.saturating_sub(1));
    let available_height = area_bottom.saturating_sub(y);
    if available_height == 0 {
      return None;
    }
    let height = desired_height.min(available_height).max(1);
    let x = if place_right {
      right_x
    } else {
      left_end.saturating_sub(width)
    };
    return Some(OverlayRect::new(x, y, width, height));
  }

  None
}

#[cfg(test)]
mod tests {
  use super::{
    OverlayRect,
    completion_docs_panel_rect,
    completion_panel_rect,
  };

  #[test]
  fn completion_panel_rect_places_below_cursor_when_space_exists() {
    let area = OverlayRect::new(0, 0, 100, 30);
    let rect = completion_panel_rect(area, 32, 8, Some((40, 10)));
    assert_eq!(rect.y, 11);
    assert_eq!(rect.width, 32);
    assert_eq!(rect.height, 8);
  }

  #[test]
  fn completion_panel_rect_flips_above_when_below_is_tight() {
    let area = OverlayRect::new(0, 0, 80, 12);
    let rect = completion_panel_rect(area, 30, 8, Some((20, 10)));
    assert!(rect.y < 10);
    assert_eq!(rect.height, 8);
  }

  #[test]
  fn completion_panel_rect_stays_adjacent_and_shrinks_when_neither_side_fits() {
    let area = OverlayRect::new(0, 0, 40, 10);
    let rect = completion_panel_rect(area, 20, 9, Some((10, 2)));
    // Cursor at row 2 means below starts at row 3.
    assert_eq!(rect.y, 3);
    // Below has only 7 rows available, so panel should shrink.
    assert_eq!(rect.height, 7);
  }

  #[test]
  fn completion_panel_rect_clamps_to_viewport_bounds() {
    let area = OverlayRect::new(5, 3, 20, 10);
    let rect = completion_panel_rect(area, 18, 9, Some((500, 500)));
    assert!(rect.x >= area.x);
    assert!(rect.y >= area.y);
    assert!(rect.x + rect.width <= area.x + area.width);
    assert!(rect.y + rect.height <= area.y + area.height);
  }

  #[test]
  fn completion_docs_panel_rect_prefers_right_side() {
    let area = OverlayRect::new(0, 0, 100, 30);
    let completion_rect = OverlayRect::new(20, 9, 30, 8);
    let docs_rect = completion_docs_panel_rect(area, 24, 10, completion_rect).expect("docs rect");
    assert_eq!(docs_rect.x, 51);
    assert_eq!(docs_rect.y, completion_rect.y);
  }

  #[test]
  fn completion_docs_panel_rect_flips_left_when_right_is_tight() {
    let area = OverlayRect::new(0, 0, 70, 20);
    let completion_rect = OverlayRect::new(45, 4, 24, 8);
    let docs_rect = completion_docs_panel_rect(area, 20, 8, completion_rect).expect("docs rect");
    assert_eq!(docs_rect.x, 24);
    assert_eq!(docs_rect.y, completion_rect.y);
  }

  #[test]
  fn completion_docs_panel_rect_keeps_top_aligned_and_shrinks_height() {
    let area = OverlayRect::new(0, 0, 80, 20);
    let completion_rect = OverlayRect::new(10, 15, 30, 4);
    let docs_rect = completion_docs_panel_rect(area, 24, 10, completion_rect).expect("docs rect");
    assert_eq!(docs_rect.y, completion_rect.y);
    assert_eq!(docs_rect.height, 5);
  }

  #[test]
  fn completion_docs_panel_rect_hides_when_viewport_is_narrow() {
    let area = OverlayRect::new(0, 0, 72, 22);
    let completion_rect = OverlayRect::new(4, 5, 46, 10);
    let docs_rect = completion_docs_panel_rect(area, 40, 9, completion_rect);
    assert!(docs_rect.is_none());
  }

  #[test]
  fn completion_docs_panel_rect_hides_when_side_space_is_unavailable() {
    let area = OverlayRect::new(0, 0, 80, 24);
    let completion_rect = OverlayRect::new(2, 6, 76, 10);
    let docs_rect = completion_docs_panel_rect(area, 36, 9, completion_rect);
    assert!(docs_rect.is_none());
  }
}
