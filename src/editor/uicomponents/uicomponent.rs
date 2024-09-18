use std::io::Error;
use crate::prelude::*;

pub trait UIComponent {
    /// Marks this UI component as in need of redrawing (or not)
    fn set_needs_redraw(&mut self, value: bool);

    /// Determines if a component needs to be redrawn or not
    fn needs_redraw(&self) -> bool;

    /// Updates the size and marks as redraw-needed
    fn resize(&mut self, size: Size) {
        self.set_size(size);
        self.set_needs_redraw(true);
    }

    /// Updates the size. Needs to be implemented by each component.
    fn set_size(&mut self, size: Size);

    /// Draw this component if it's visible and in need of redrawing
    fn render(&mut self, origin_row: RowIndex) {
        if self.needs_redraw() {
            if let Err(err) = self.draw(origin_row) {
                #[cfg(debug_assertions)]
                {
                    panic!("Could not render component: {err:?}");
                }
                #[cfg(not(debug_assertions))]
                {
                    let _ = err;
                }
            } else {
                self.set_needs_redraw(false);
            }
        }
    }

    /// Method to actually draw the component, must be implemented by each component
    fn draw(&mut self, origin_row: RowIndex) -> Result<(), Error>;
}
