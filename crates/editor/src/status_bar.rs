use renderer::{terminal::TerminalInterface, Component, Renderer, TerminalCommand};
use utils::{Mode, Position, Size};

pub struct StatusBar {
    current_mode: Mode,
    file_name: Option<String>,
    cursor_position: Position,
    pub size: Size,
}

impl StatusBar {
    pub fn new(size: Size) -> Self {
        Self {
            current_mode: Mode::Normal, // EditorState starts with Normal mode.
            file_name: None,
            cursor_position: Position::new(),
            size,
        }
    }

    pub fn update(&mut self, mode: Mode, file_name: Option<String>, cursor_position: Position) {
        self.current_mode = mode;
        self.file_name = file_name;
        self.cursor_position = cursor_position;
    }
}

impl Component for StatusBar {
    fn render<T>(&mut self, renderer: &mut Renderer<T>)
    where
        T: TerminalInterface,
    {
        // Build the string for the `StatusBar`.
        let mode_str = match self.current_mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
        };

        let file_name = self.file_name.as_deref().unwrap_or("[No Name]");
        let cursor_pos = format!(
            "{}:{}",
            self.cursor_position.y + 1,
            self.cursor_position.x + 1
        );

        // Format `StatusBar`.
        let status = format!(" {} | {} | {}", mode_str, file_name, cursor_pos);

        // Make sure it fits the screen.
        let mut status_bar = status;
        if status_bar.len() > self.size.width {
            status_bar.truncate(self.size.width);
        } else {
            // Fill with empty spaces to complete the line.
            let padding = " ".repeat(self.size.width - status_bar.len());
            status_bar.push_str(&padding);
        }

        renderer.enqueue_command(TerminalCommand::MoveCursor(0, self.size.height - 1));
        renderer.enqueue_command(TerminalCommand::Print(status_bar));

        // TODO: Colors.
        // TODO: Reset colors.
    }
}
