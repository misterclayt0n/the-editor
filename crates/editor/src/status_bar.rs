use renderer::{Color, Component, RenderGUICommand, RenderTUICommand, Renderer};
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
            cursor_position: Position::default(),
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
    fn render_tui(&mut self, renderer: &mut Renderer) {
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
        if status_bar.len() > self.size.width as usize {
            status_bar.truncate(self.size.width as usize);
        } else {
            // Fill with empty spaces to complete the line.
            let padding = " ".repeat(self.size.width as usize - status_bar.len());
            status_bar.push_str(&padding);
        }

        renderer.enqueue_tui_command(RenderTUICommand::MoveCursor(0, self.size.height - 1));
        renderer.enqueue_tui_command(RenderTUICommand::Print(status_bar));

        // TODO: Colors.
        // TODO: Reset colors.
    }

    fn render_gui(&mut self, renderer: &mut Renderer) {
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

        let status = format!(" {} | {} | {}", mode_str, file_name, cursor_pos);

        let font_size = 20;
        let padding = 5;

        let status_bar_height = font_size + padding * 2;
        // Status bar Y position is window height - status bar height
        let status_bar_y_gui = self.size.height.saturating_sub(status_bar_height);

        renderer.enqueue_gui_command(RenderGUICommand::DrawRectangle(
            0,
            status_bar_y_gui,
            self.size.width,
            status_bar_height,
            Color::LIGHTGRAY, // Use a color defined in your renderer::Color enum
        ));

        renderer.enqueue_gui_command(RenderGUICommand::DrawText(
            status,
            padding,
            status_bar_y_gui + padding,
            font_size,
            Color::BLACK, // Use a color for text
        ));
    }
}
