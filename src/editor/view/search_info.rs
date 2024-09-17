use crate::editor::{line::Line, position::Position};

use super::Location;

pub struct SearchInfo {
    pub prev_location: Location,
    pub prev_scroll_offset: Position,
    pub query: Line,
}
