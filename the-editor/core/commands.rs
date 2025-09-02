use ropey::RopeSlice;

use crate::{core::{movement::{move_horizontally, Direction, Movement}, selection::Range}, editor::Editor};

pub struct Context<'a> {
    pub editor: &'a Editor,
}

type MoveFn = fn(RopeSlice, Range, Direction, usize, Movement) -> Range;

fn move_impl(cx: &mut Context, move_fn: MoveFn, dir: Direction, behavior: Movement) {
   
}

fn move_char_left(cx: &mut Context) {
    move_impl(cx, move_horizontally, Direction::Backward, Movement::Move)
}
