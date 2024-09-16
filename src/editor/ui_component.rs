use std::io::Error;

use super::terminal::Size;

pub trait UIComponent {
    // marks this UI component as in need of redrawing (or not)
    fn mark_redraw(&mut self, value: bool);

    // determines if a component needs to be redrawn or not
    fn needs_redraw(&self) -> bool;

    // updates the size and marks as redraw-needed
    fn resize(&mut self, size: Size) {
        self.set_size(size);
        self.mark_redraw(true);
    }

    // updates the size. Needs to be implemented by each component.
    fn set_size(&mut self, size: Size);

    // draw this component if it's visible and in need of redrawing
    fn render(&mut self, origin_y: usize) {
        if self.needs_redraw() {
            match self.draw(origin_y) {
                    Ok(()) => self.mark_redraw(false),
                    Err(err) => {
                        #[cfg(debug_assertions)]
                        {
                            panic!("Could not render component: {err:?}")
                        }
                    }
            }
        }
    }

    fn draw(&mut self, origin_y: usize) -> Result<(), Error>;
}
