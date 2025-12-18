use std::time::Instant;

use crate::ui::{
  components::popup::PositionBias,
  compositor::{
    Context,
    Surface,
  },
};

/// Pixel gap between cursor and popup
const CURSOR_POPUP_MARGIN: f32 = 4.0;

/// Calculate available space above and below cursor for popup placement.
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be placed.
pub fn calculate_available_space(
  cursor: CursorPosition,
  viewport_height: f32,
  min_y: f32,
) -> (f32, f32) {
  // Available above is from cursor to the top boundary (min_y), not screen top
  let available_above = (cursor.line_top - min_y - CURSOR_POPUP_MARGIN).max(0.0);
  let available_below = (viewport_height - cursor.line_bottom - CURSOR_POPUP_MARGIN).max(0.0);
  (available_above, available_below)
}

/// Constrain popup height to fit within available space.
/// Returns the maximum height the popup can be without overflowing.
/// Respects bias preference: tries the preferred side first, but uses the other
/// side if the preferred side doesn't have enough space. If no bias is
/// provided, prefers the side with more space (below by default if equal).
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be placed.
pub fn constrain_popup_height(
  cursor: CursorPosition,
  popup_height: f32,
  min_popup_height: f32,
  viewport_height: f32,
  min_y: f32,
  bias: Option<PositionBias>,
) -> f32 {
  let (available_above, available_below) = calculate_available_space(cursor, viewport_height, min_y);

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
  pub x:           f32,
  /// Y coordinate of top of cursor line
  pub line_top:    f32,
  /// Y coordinate of bottom of cursor line (baseline)
  pub line_bottom: f32,
}

/// Calculate cursor position in screen coordinates using document font metrics.
/// Returns None if cursor is not visible.
pub fn calculate_cursor_position(ctx: &Context, surface: &mut Surface) -> Option<CursorPosition> {
  // Configure the document font to get correct metrics
  // This is important because the surface may have UI font configured (e.g.,
  // after explorer render)
  let font_family = surface.current_font_family().to_owned();
  let font_size = ctx
    .editor
    .font_size_override
    .unwrap_or(ctx.editor.config().font_size);
  surface.configure_font(&font_family, font_size);

  let doc_cell_w = surface.cell_width().max(1.0);
  let doc_cell_h = surface.cell_height().max(1.0);

  let view_id = ctx.editor.focused_view_id()?;
  let tree = &ctx.editor.tree;
  let animated_area = tree
    .get_animated_area(view_id)
    .or_else(|| tree.try_get(view_id).map(|view| view.area))?;
  let view = tree.get(view_id);
  let doc_id = view.doc()?;
  let doc = &ctx.editor.documents[&doc_id];
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

  let inner_area = animated_area.clip_left(view.gutter_offset(doc));

  // Check if cursor is visible within the animated viewport
  if rel_row >= usize::from(inner_area.height) {
    return None;
  }
  if screen_col >= usize::from(inner_area.width) {
    return None;
  }

  // Account for active screen shake effects
  let now = Instant::now();
  let (shake_offset_x, shake_offset_y) = doc
    .screen_shake(view.id)
    .and_then(|shake| shake.sample(now))
    .unwrap_or((0.0, 0.0));

  // Get viewport pixel offsets (explorer width, bufferline height)
  // These are set by EditorView during render
  let (explorer_px_offset, _bufferline_px_offset) = ctx.editor.viewport_pixel_offset;

  // Calculate view position in pixels - must match how editor_view.rs renders
  // text Tree coordinates are 0-based (not offset by explorer), so we add
  // explorer_px_offset inner_area.x contains gutter offset in cells
  let view_x = explorer_px_offset + (inner_area.x as f32 * doc_cell_w) + shake_offset_x;

  // For Y: inner_area.y is in cells (already includes bufferline offset via clip_top(1))
  // Convert to pixels - DO NOT add bufferline_px_offset as that would double-count
  let view_y = (inner_area.y as f32 * doc_cell_h) + shake_offset_y;

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
/// Tries the preferred side first (based on bias), but falls back to the other
/// side if there's not enough space. If no bias is provided, chooses the side
/// with more space (below by default if equal). Accounts for animation
/// slide_offset and scale.
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be placed.
pub fn position_popup_near_cursor(
  cursor: CursorPosition,
  popup_width: f32,
  popup_height: f32,
  viewport_width: f32,
  viewport_height: f32,
  min_y: f32,
  slide_offset: f32,
  scale: f32,
  bias: Option<PositionBias>,
) -> PopupPosition {
  // Apply animation transforms
  let anim_width = popup_width * scale;
  let anim_height = popup_height * scale;

  // Calculate available space to determine best placement
  let (available_above, available_below) = calculate_available_space(cursor, viewport_height, min_y);

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

  // Clamp Y to viewport bounds (min_y is the top boundary, e.g., below bufferline)
  let popup_y = popup_y.max(min_y).min(viewport_height - anim_height);

  // Align popup with cursor column and clamp to viewport
  let mut popup_x = cursor.x;
  popup_x = popup_x.max(0.0).min(viewport_width - anim_width);

  PopupPosition {
    x: popup_x,
    y: popup_y,
  }
}

/// Position a popup relative to the cursor, centered horizontally.
/// Tries the preferred side first (based on bias), but falls back to the other
/// side if there's not enough space. If no bias is provided, chooses the side
/// with more space (below by default if equal). Useful for signature help which
/// should be centered on the cursor.
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be placed.
pub fn position_popup_centered_on_cursor(
  cursor: CursorPosition,
  popup_width: f32,
  popup_height: f32,
  viewport_width: f32,
  viewport_height: f32,
  min_y: f32,
  slide_offset: f32,
  scale: f32,
  bias: Option<PositionBias>,
) -> PopupPosition {
  // Apply animation transforms
  let anim_width = popup_width * scale;
  let anim_height = popup_height * scale;

  // Calculate available space to determine best placement
  let (available_above, available_below) = calculate_available_space(cursor, viewport_height, min_y);

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

  // Clamp Y to viewport bounds (min_y is the top boundary, e.g., below bufferline)
  let popup_y = popup_y.max(min_y).min(viewport_height - anim_height);

  // Center popup horizontally on cursor, accounting for animation scale
  let mut popup_x = cursor.x - (popup_width - anim_width) / 2.0;
  popup_x = popup_x.max(0.0).min(viewport_width - anim_width);

  PopupPosition {
    x: popup_x,
    y: popup_y,
  }
}
