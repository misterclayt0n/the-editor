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
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be
/// placed.
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
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be
/// placed.
pub fn constrain_popup_height(
  cursor: CursorPosition,
  popup_height: f32,
  min_popup_height: f32,
  viewport_height: f32,
  min_y: f32,
  bias: Option<PositionBias>,
) -> f32 {
  let (available_above, available_below) =
    calculate_available_space(cursor, viewport_height, min_y);

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
  let font_family = surface.current_font_family().to_owned();
  let per_buffer_enabled = ctx.editor.config().per_buffer_font_size;
  let default_font_size = ctx.editor.config().font_size;
  let editor_font_override = ctx.editor.font_size_override;

  let view_id = ctx.editor.focused_view_id()?;
  let tree = &ctx.editor.tree;
  let animated_area = tree
    .get_animated_area(view_id)
    .or_else(|| tree.try_get(view_id).map(|view| view.area))?;
  let view = tree.get(view_id);
  let doc_id = view.doc()?;
  let doc = &ctx.editor.documents[&doc_id];

  // Get the font size for this view's document (per-buffer aware)
  let view_font_size = if per_buffer_enabled {
    // Per-buffer mode: document override -> editor override -> config default
    doc
      .font_size_override
      .or(editor_font_override)
      .unwrap_or(default_font_size)
  } else {
    // Global mode: editor override -> config default
    editor_font_override.unwrap_or(default_font_size)
  };

  // Layout font size is used for view area calculations (consistent across views)
  let layout_font_size = editor_font_override.unwrap_or(default_font_size);

  // Configure surface with layout font to get layout metrics
  surface.configure_font(&font_family, layout_font_size);
  let layout_cell_w = surface.cell_width().max(1.0);
  let layout_cell_h = surface.cell_height().max(1.0);

  // Configure surface with view font to get view-specific metrics
  surface.configure_font(&font_family, view_font_size);
  let view_cell_w = surface.cell_width().max(1.0);
  let view_cell_h = surface.cell_height().max(1.0);

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

  // Get gutter width - this is in view font cells since gutter uses view font
  let gutter_offset = view.gutter_offset(doc);

  // When per-buffer is enabled, inner_area dimensions are in layout cells,
  // but we need to check visibility using view font cells (effective viewport)
  let (check_width, check_height) = if per_buffer_enabled {
    // Use effective_viewport if available, otherwise calculate from layout area
    if let Some((w, h)) = view.effective_viewport {
      (w as usize, h as usize)
    } else {
      // Fallback: convert layout area to view font cells
      let view_pixel_width = animated_area.width as f32 * layout_cell_w;
      let view_pixel_height = animated_area.height as f32 * layout_cell_h;
      let gutter_pixel_width = gutter_offset as f32 * view_cell_w;
      let content_pixel_width = (view_pixel_width - gutter_pixel_width).max(view_cell_w);
      let content_cols = (content_pixel_width / view_cell_w).floor().max(1.0) as usize;
      let content_rows = (view_pixel_height / view_cell_h).floor().max(1.0) as usize;
      (content_cols, content_rows)
    }
  } else {
    let inner = animated_area.clip_left(gutter_offset);
    (inner.width as usize, inner.height as usize)
  };

  // Check if cursor is visible within the viewport
  if rel_row >= check_height {
    return None;
  }
  if screen_col >= check_width {
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
  let (explorer_px_offset, bufferline_px_offset) = ctx.editor.viewport_pixel_offset;

  // Calculate view position in pixels using LAYOUT metrics for base positioning
  // (since animated_area is in layout cells), then use VIEW metrics for cursor
  // offset within the view.
  //
  // The gutter width in pixels uses view font (gutter renders with view font)
  let gutter_pixel_width = gutter_offset as f32 * view_cell_w;

  let view_x = explorer_px_offset
    + (animated_area.x as f32 * layout_cell_w)
    + gutter_pixel_width
    + shake_offset_x;

  // For Y: animated_area.y is in layout cells (already includes bufferline offset
  // via clip_top(1)). The bufferline has a fixed pixel height that doesn't scale
  // with font size.
  let view_y = if animated_area.y > 0 && bufferline_px_offset > 0.0 {
    // Bufferline takes first row at fixed pixel height, remaining rows use layout
    // cell height for base positioning
    bufferline_px_offset + ((animated_area.y as f32 - 1.0) * layout_cell_h) + shake_offset_y
  } else {
    (animated_area.y as f32 * layout_cell_h) + shake_offset_y
  };

  // Calculate final screen position using VIEW font metrics for cursor offset
  // (since text is rendered with view font)
  let x = view_x + (screen_col as f32 * view_cell_w);
  let line_top = view_y + (rel_row as f32 * view_cell_h);
  let line_bottom = line_top + view_cell_h;

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
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be
/// placed.
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
  let (available_above, available_below) =
    calculate_available_space(cursor, viewport_height, min_y);

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

  // Clamp Y to viewport bounds (min_y is the top boundary, e.g., below
  // bufferline)
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
/// `min_y` is the top boundary (e.g., bufferline height) where popups cannot be
/// placed.
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
  let (available_above, available_below) =
    calculate_available_space(cursor, viewport_height, min_y);

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

  // Clamp Y to viewport bounds (min_y is the top boundary, e.g., below
  // bufferline)
  let popup_y = popup_y.max(min_y).min(viewport_height - anim_height);

  // Center popup horizontally on cursor, accounting for animation scale
  let mut popup_x = cursor.x - (popup_width - anim_width) / 2.0;
  popup_x = popup_x.max(0.0).min(viewport_width - anim_width);

  PopupPosition {
    x: popup_x,
    y: popup_y,
  }
}
