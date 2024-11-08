use std::cmp::min;

use utils::Cursor;

use crate::buffer::Buffer;

/// Moves the cursor to the left, the Buffer does not quite matter here.
pub fn move_cursor_left(cursor: &mut Cursor) {
    if cursor.position.x > 0 {
        cursor.position.x -= 1;

        // Updates the desired x.
        cursor.desired_x = cursor.position.x;
    }
}

/// Moves the cursor to the right, respecting the boundaries of a Buffer.
pub fn move_cursor_right(cursor: &mut Cursor, buffer: Option<&Buffer>) {
    if let Some(buffer) = buffer {
        let line_length = buffer.get_visible_line_length(cursor.position.y);

        if cursor.position.x < line_length {
            cursor.position.x += 1;

            // Updates the desired x.
            cursor.desired_x = cursor.position.x;
        }
    }
}

/// Moves the cursor up, trying to keep the desired horizontal position.
pub fn move_cursor_up(cursor: &mut Cursor, buffer: Option<&Buffer>) {
    if let Some(buffer) = buffer {
        if cursor.position.y > 0 {
            cursor.position.y -= 1;
            let line_length = buffer.get_visible_line_length(cursor.position.y);

            // Updates the horizontal position to be either the desired x
            // or the line length.
            cursor.position.x = min(cursor.desired_x, line_length);
        }
    }
}

/// Moves the cursor down, trying to keep the desired horizontal position.
pub fn move_cursor_down(cursor: &mut Cursor, buffer: Option<&Buffer>) {
    if let Some(buffer) = buffer {
        if cursor.position.y < buffer.len_nonempty_lines().saturating_sub(1) {
            cursor.position.y += 1;
            let line_length = buffer.get_visible_line_length(cursor.position.y);

            // Updates the horizontal position to be either the desired x
            // or the line length.
            cursor.position.x = min(cursor.desired_x, line_length);
        }
    }
}
