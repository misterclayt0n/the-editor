use crate::prelude::*;

use super::uicomponents::{UIComponent, View};

pub enum SplitDirection {
    Horizontal,
    Vertical,
}

pub struct Window {
    pub view: View,
    pub origin: Position,
    pub size: Size,
}

impl Window {
    pub fn new(origin: Position, size: Size, view: View) -> Self {
        Self {
            view,
            origin,
            size,
        }
    }

    pub fn resize(&mut self, new_origin: Position, new_size: Size) {
        self.origin = new_origin;
        self.size = new_size;
        self.view.resize(new_size);
    }
}
