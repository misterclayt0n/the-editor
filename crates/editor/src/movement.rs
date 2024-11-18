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
///
/// `exceed_line` means if we want the cursor to be able to move beyond the visible part
/// of the line, which means we are counting the '\n' character.
pub fn move_cursor_right(cursor: &mut Cursor, buffer: &Buffer, exceed_line: bool) {
    let line_length = if exceed_line {
        buffer.get_line_length(cursor.position.y)
    } else {
        buffer.get_visible_line_length(cursor.position.y)
    };

    if cursor.position.x < line_length {
        cursor.position.x += 1;

        // Updates the desired x.
        cursor.desired_x = cursor.position.x;
    }
}

/// Moves the cursor up, trying to keep the desired horizontal position.
pub fn move_cursor_up(cursor: &mut Cursor, buffer: &Buffer) {
    if cursor.position.y > 0 {
        cursor.position.y -= 1;
        let line_length = buffer.get_visible_line_length(cursor.position.y);

        // Updates the horizontal position to be either the desired x
        // or the line length.
        cursor.position.x = min(cursor.desired_x, line_length);
    }
}

/// Moves the cursor down, trying to keep the desired horizontal position.
pub fn move_cursor_down(cursor: &mut Cursor, buffer: &Buffer) {
    if cursor.position.y < buffer.len_nonempty_lines().saturating_sub(1) {
        cursor.position.y += 1;
        let line_length = buffer.get_visible_line_length(cursor.position.y);

        // Updates the horizontal position to be either the desired x
        // or the line length.
        cursor.position.x = min(cursor.desired_x, line_length);
    }
}

pub fn move_cursor_end_of_line(cursor: &mut Cursor, buffer: &Buffer) {
    let line_length = buffer.get_visible_line_length(cursor.position.y);
    cursor.position.x = line_length;

    // Updates the desired x.
    cursor.desired_x = cursor.position.x;
}

pub fn move_cursor_start_of_line(cursor: &mut Cursor) {
    cursor.position.x = 0;

    // Updates the desired x.
    cursor.desired_x = cursor.position.x;
}

/// Moves the cursor to the first non-blank character of the current line.
pub fn move_cursor_first_char_of_line(cursor: &mut Cursor, buffer: &Buffer) {
    let line = buffer.get_trimmed_line(cursor.position.y);
    let first_non_blank = line.chars().position(|c| !c.is_whitespace());

    cursor.position.x = first_non_blank.unwrap_or(0);

    cursor.desired_x = cursor.position.x;
}

pub fn move_cursor_word_forward(cursor: &mut Cursor, buffer: &Buffer, big_word: bool) {
    if let Some(new_pos) = buffer.find_next_word_start(cursor.position, big_word) {
        cursor.position = new_pos;
        cursor.desired_x = cursor.position.x;
    }
}

pub fn move_cursor_word_backward(cursor: &mut Cursor, buffer: &Buffer, big_word: bool) {
    if let Some(new_pos) = buffer.find_prev_word_start(cursor.position, big_word) {
        cursor.position = new_pos;
        cursor.desired_x = cursor.position.x;
    }
}

pub fn move_cursor_word_forward_end(cursor: &mut Cursor, buffer: &Buffer, big_word: bool) {
    if let Some(new_pos) = buffer.find_next_word_end(cursor.position, big_word) {
        cursor.position = new_pos;
        cursor.desired_x = cursor.position.x;
    }
}

pub fn move_cursor_after_insert(cursor: &mut Cursor, c: char) {
    if c == '\n' {
        cursor.position.x = 0;
        cursor.position.y += 1;
    } else {
        cursor.position.x += 1;
    }

    cursor.desired_x = cursor.position.x;
}

pub fn move_cursor_before_deleting_backward(cursor: &mut Cursor, buffer: &Buffer) {
    if cursor.position.x > 0 {
        cursor.position.x -= 1;
    } else if cursor.position.y > 0 {
        cursor.position.y -= 1;
        cursor.position.x = buffer.get_visible_line_length(cursor.position.y);
    }

    cursor.desired_x = cursor.position.x;
}
