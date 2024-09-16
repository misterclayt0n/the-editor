use super::{terminal::{Size, Terminal}, ui_component::UIComponent};

#[derive(Default)]
pub struct MessageBar {
    current_message: String,
    needs_redraw: bool,
}

impl MessageBar {
    pub fn update_message(&mut self, new_message: String) {
        if new_message != self.current_message {
            self.current_message = new_message;
            self.mark_redraw(true);
        }
    }
}

impl UIComponent for MessageBar {
    fn mark_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, _: Size) { }

    fn draw(&mut self, origin: usize) -> Result<(), std::io::Error> {
        Terminal::print_row(origin, &self.current_message)
    }
}
