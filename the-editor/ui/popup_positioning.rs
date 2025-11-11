use crate::{
  ui::{
    compositor::{
      Context,
      Surface,
    },
    components::popup::PositionBias,
  },
};

/// Pixel gap between cursor and popup
const CURSOR_POPUP_MARGIN: f32 = 4.0;

/// Calculate available space above and below cursor for popup placement.
pub fn calculate_available_space(
  cursor: CursorPosition,
  viewport_height: f32,
) -> (f32, f32) {
  let available_above = (cursor.line_top - CURSOR_POPUP_MARGIN).max(0.0);
  let available_below = (viewport_height - cursor.line_bottom - CURSOR_POPUP_MARGIN).max(0.0);
  (available_above, available_below)
}

/// Constrain popup height to fit within available space.
/// Returns the maximum height the popup can be without overflowing.
/// Respects bias preference: tries the preferred side first, but uses the other side
/// if the preferred side doesn't have enough space. If no bias is provided, prefers
/// the side with more space (below by default if equal).
pub fn constrain_popup_height(
  cursor: CursorPosition,
  popup_height: f32,
  min_popup_height: f32,
  viewport_height: f32,
  bias: Option<PositionBias>,
) -> f32 {
  let (available_above, available_below) = calculate_available_space(cursor, viewport_height);
  
  // Determine maximum popup height based on bias and available space
  let max_popup_height = match bias {
    Some(PositionBias::Below) => {
      // Try below first, but use above if below doesn't have enough space
      if available_below >= min_popup_height {
        available_below
      } else if available_above > available_below {
        available_above
      } else {
        available_below
      }
    },
    Some(PositionBias::Above) => {
      // Try above first, but use below if above doesn't have enough space
      if available_above >= min_popup_height {
        available_above
      } else if available_below > available_above {
        available_below
      } else {
        available_above
      }
    },
    None => {
      // No bias: prefer side with more space (below if equal)
      if available_below >= available_above {
        available_below
      } else {
        available_above
      }
    },
  };
  
  if max_popup_height <= 0.0 {
    return min_popup_height;
  }
  
  if max_popup_height < min_popup_height {
    // Not enough room to display even minimum content without covering text
    return min_popup_height;
  }
  
  // Constrain to available space
  popup_height.min(max_popup_height).max(min_popup_height)
}

/// Cursor position in screen coordinates (pixels)
#[derive(Clone, Copy, Debug)]
pub struct CursorPosition {
  /// X coordinate of cursor (left edge of character)
  pub x: f32,
  /// Y coordinate of top of cursor line
  pub line_top: f32,
  /// Y coordinate of bottom of cursor line (baseline)
  pub line_bottom: f32,
}

/// Calculate cursor position in screen coordinates using document font metrics.
/// Returns None if cursor is not visible.
pub fn calculate_cursor_position(
  ctx: &Context,
  surface: &Surface,
) -> Option<CursorPosition> {
  let font_state = surface.save_font_state();
  let doc_cell_w = font_state.cell_width.max(1.0);
  let doc_cell_h = font_state.cell_height.max(1.0);

  let (view, doc) = crate::current_ref!(ctx.editor);
  let text = doc.text();
  let cursor_pos = doc.selection(view.id).primary().cursor(text.slice(..));

  // Convert char position to line/column
  let line = text.char_to_line(cursor_pos);
  let line_start = text.line_to_char(line);
  let col = cursor_pos - line_start;

  // Get view scroll offset
  let view_offset = doc.view_offset(view.id);
  let anchor_line = text.char_to_line(view_offset.anchor.min(text.len_chars()));

  // Calculate screen row/col accounting for scroll
  let rel_row = line.saturating_sub(anchor_line);
  let screen_col = col.saturating_sub(view_offset.horizontal_offset);

  // Check if cursor is visible
  if rel_row >= view.inner_height() {
    return None;
  }

  // Get view's screen offset (handles splits correctly)
  let inner = view.inner_area(doc);
  let view_x = inner.x as f32 * doc_cell_w;
  let view_y = inner.y as f32 * doc_cell_h;

  // Calculate final screen position
  let x = view_x + (screen_col as f32 * doc_cell_w);
  let line_top = view_y + (rel_row as f32 * doc_cell_h);
  let line_bottom = line_top + doc_cell_h;

  Some(CursorPosition {
    x,
    line_top,
    line_bottom,
  })
}

/// Popup positioning result
#[derive(Clone, Copy, Debug)]
pub struct PopupPosition {
  /// X coordinate of popup (left edge)
  pub x: f32,
  /// Y coordinate of popup (top edge)
  pub y: f32,
}

/// Position a popup relative to the cursor.
/// Tries the preferred side first (based on bias), but falls back to the other side
/// if there's not enough space. If no bias is provided, chooses the side with more space
/// (below by default if equal). Accounts for animation slide_offset and scale.
pub fn position_popup_near_cursor(
  cursor: CursorPosition,
  popup_width: f32,
  popup_height: f32,
  viewport_width: f32,
  viewport_height: f32,
  slide_offset: f32,
  scale: f32,
  bias: Option<PositionBias>,
) -> PopupPosition {
  // Apply animation transforms
  let anim_width = popup_width * scale;
  let anim_height = popup_height * scale;

  // Calculate available space to determine best placement
  let (available_above, available_below) = calculate_available_space(cursor, viewport_height);

  // Determine which side to use based on bias and available space
  let use_below = match bias {
    Some(PositionBias::Below) => {
      // Try below first if it fits, otherwise use whichever side fits better
      if available_below >= anim_height {
        true
      } else if available_above >= anim_height {
        false
      } else {
        // Neither fits, use whichever has more space
        available_below >= available_above
      }
    },
    Some(PositionBias::Above) => {
      // Try above first if it fits, otherwise use whichever side fits better
      if available_above >= anim_height {
        false
      } else if available_below >= anim_height {
        true
      } else {
        // Neither fits, use whichever has more space
        available_above >= available_below
      }
    },
    None => {
      // No bias: choose side with more space (prefer below if equal)
      available_below >= available_above
    },
  };

  let popup_y = if use_below {
    // Position below cursor
    cursor.line_bottom + CURSOR_POPUP_MARGIN + slide_offset
  } else {
    // Position above cursor
    cursor.line_top - CURSOR_POPUP_MARGIN - anim_height - slide_offset
  };

  // Clamp Y to viewport bounds
  let popup_y = popup_y.max(0.0).min(viewport_height - anim_height);

  // Align popup with cursor column and clamp to viewport
  let mut popup_x = cursor.x;
  popup_x = popup_x.max(0.0).min(viewport_width - anim_width);

  PopupPosition {
    x: popup_x,
    y: popup_y,
  }
}

/// Position a popup relative to the cursor, centered horizontally.
/// Tries the preferred side first (based on bias), but falls back to the other side
/// if there's not enough space. If no bias is provided, chooses the side with more space
/// (below by default if equal). Useful for signature help which should be centered on the cursor.
pub fn position_popup_centered_on_cursor(
  cursor: CursorPosition,
  popup_width: f32,
  popup_height: f32,
  viewport_width: f32,
  viewport_height: f32,
  slide_offset: f32,
  scale: f32,
  bias: Option<PositionBias>,
) -> PopupPosition {
  // Apply animation transforms
  let anim_width = popup_width * scale;
  let anim_height = popup_height * scale;

  // Calculate available space to determine best placement
  let (available_above, available_below) = calculate_available_space(cursor, viewport_height);

  // Determine which side to use based on bias and available space
  let use_below = match bias {
    Some(PositionBias::Below) => {
      // Try below first if it fits, otherwise use whichever side fits better
      if available_below >= anim_height {
        true
      } else if available_above >= anim_height {
        false
      } else {
        // Neither fits, use whichever has more space
        available_below >= available_above
      }
    },
    Some(PositionBias::Above) => {
      // Try above first if it fits, otherwise use whichever side fits better
      if available_above >= anim_height {
        false
      } else if available_below >= anim_height {
        true
      } else {
        // Neither fits, use whichever has more space
        available_above >= available_below
      }
    },
    None => {
      // No bias: choose side with more space (prefer below if equal)
      available_below >= available_above
    },
  };

  let popup_y = if use_below {
    // Position below cursor
    cursor.line_bottom + CURSOR_POPUP_MARGIN + slide_offset
  } else {
    // Position above cursor
    cursor.line_top - CURSOR_POPUP_MARGIN - anim_height - slide_offset
  };

  // Clamp Y to viewport bounds
  let popup_y = popup_y.max(0.0).min(viewport_height - anim_height);

  // Center popup horizontally on cursor, accounting for animation scale
  let mut popup_x = cursor.x - (popup_width - anim_width) / 2.0;
  popup_x = popup_x.max(0.0).min(viewport_width - anim_width);

  PopupPosition {
    x: popup_x,
    y: popup_y,
  }
}

