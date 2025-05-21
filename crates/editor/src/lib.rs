use anyhow::Result;
use events::EventHandler;
use movement::{
    move_cursor_after_insert, move_cursor_before_deleting_backward, move_cursor_down,
    move_cursor_end_of_line, move_cursor_first_char_of_line, move_cursor_left, move_cursor_right,
    move_cursor_start_of_line, move_cursor_up, move_cursor_word_backward, move_cursor_word_forward,
    move_cursor_word_forward_end,
};
use raylib::{ffi::KeyboardKey, RaylibHandle};
use renderer::{Color, Component, RenderGUICommand, Renderer};
use status_bar::StatusBar;
use utils::{error, info, Command, InterfaceType, Mode, Size};
use window::Window;
mod buffer;
pub mod events;
mod movement;
mod status_bar;
mod window;

/// Structure that maintains the global state of the editor.
pub struct EditorState {
    should_quit: bool,
    event_handler: EventHandler,
    window: Window, // NOTE: I should probably implement some sort of window manager.
    mode: Mode,
    status_bar: StatusBar,
    renderer: Renderer,
}

impl EditorState {
    pub fn new(event_handler: EventHandler, renderer: Renderer, file_path: Option<String>) -> Self {
        // Init functions.
        match renderer.interface {
            InterfaceType::TUI => renderer.terminal.as_ref().unwrap().init(),
            InterfaceType::GUI => {} // Raylib initializes itself, so we don't have setup here.
        }

        let (width, height) = match renderer.interface {
            InterfaceType::TUI => renderer.terminal.as_ref().unwrap().size(),
            InterfaceType::GUI => renderer.gui.as_ref().unwrap().size(),
        };

        let window = Window::from_file(file_path, width, height);
        let viewport_size = Size { width, height };
        let status_bar = StatusBar::new(viewport_size);

        let mut editor_state = EditorState {
            should_quit: false,
            event_handler,
            window,
            mode: Mode::Normal, // Start with Normal mode.
            status_bar,
            renderer,
        };

        // Initial render.
        editor_state.render();

        editor_state
    }

    /// Main entrypoint of the application.
    pub fn run(&mut self) -> Result<()> {
        match self.renderer.interface {
            InterfaceType::TUI => self.run_tui(),
            InterfaceType::GUI => self.run_gui(),
        }
    }

    fn run_tui(&mut self) -> Result<()> {
        loop {
            // Poll events.
            let events = self.event_handler.poll_events();
            for event in events {
                let command_result = self.event_handler.handle_event(event, self.mode);
                match command_result {
                    Ok(commands) => {
                        for command in commands {
                            self.apply_command(command)?;
                        }
                    }
                    Err(e) => error!("Error handling events: {}", e),
                }
            }

            self.render();

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn run_gui(&mut self) -> Result<()> {
        while !self.should_quit {
            self.handle_gui_inputs()?;
            self.render();
        }

        Ok(())
    }

    fn handle_gui_inputs(&mut self) -> Result<()> {
        // Collect commands to apply after borrowing rl.
        let mut commands = Vec::new();

        // Scope the mutable borrow of rl.
        {
            let rl = &mut self.renderer.gui.as_mut().unwrap().rl;

            if rl.window_should_close() {
                self.should_quit = true;
                return Ok(());
            }

            match self.mode {
                Mode::Normal => {
                    if press_and_repeat(KeyboardKey::KEY_Q, rl) {
                        commands.push(Command::Quit);
                    }
                    if press_and_repeat(KeyboardKey::KEY_I, rl) {
                        commands.push(Command::SwitchMode(Mode::Insert));
                    }
                    if press_and_repeat(KeyboardKey::KEY_A, rl) {
                        commands.push(Command::MoveCursorRight(true));
                        commands.push(Command::SwitchMode(Mode::Insert));
                    }
                    if press_and_repeat(KeyboardKey::KEY_H, rl) {
                        commands.push(Command::MoveCursorLeft);
                    }
                    if press_and_repeat(KeyboardKey::KEY_L, rl) {
                        commands.push(Command::MoveCursorRight(false));
                    }
                    if press_and_repeat(KeyboardKey::KEY_K, rl) {
                        commands.push(Command::MoveCursorUp);
                    }
                    if press_and_repeat(KeyboardKey::KEY_J, rl) {
                        commands.push(Command::MoveCursorDown);
                    }
                    if press_and_repeat(KeyboardKey::KEY_X, rl) {
                        commands.push(Command::DeleteCharForward);
                    }
                    if press_and_repeat(KeyboardKey::KEY_ZERO, rl) {
                        commands.push(Command::MoveCursorStartOfLine);
                    }
                    if press_and_repeat(KeyboardKey::KEY_FOUR, rl) {
                        commands.push(Command::MoveCursorEndOfLine);
                    }
                    if press_and_repeat(KeyboardKey::KEY_W, rl) {
                        commands.push(Command::MoveCursorWordForward(false));
                    }
                    if press_and_repeat(KeyboardKey::KEY_B, rl) {
                        commands.push(Command::MoveCursorWordBackward(false));
                    }
                    if press_and_repeat(KeyboardKey::KEY_E, rl) {
                        commands.push(Command::MoveCursorWordForwardEnd(false));
                    }
                    if press_and_repeat(KeyboardKey::KEY_ESCAPE, rl) {
                        info!("ESC pressed in GUI Insert mode");
                        commands.push(Command::SwitchMode(Mode::Normal));
                    }
                }
                Mode::Insert => {
                    while let Some(c) = rl.get_char_pressed() {
                        commands.push(Command::InsertChar(c as char));
                    }
                    if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
                        info!("ESC pressed in GUI Insert mode");
                        commands.push(Command::MoveCursorLeft);
                        commands.push(Command::SwitchMode(Mode::Normal));
                    }
                    if press_and_repeat(KeyboardKey::KEY_LEFT, rl) {
                        commands.push(Command::MoveCursorLeft);
                    }
                    if press_and_repeat(KeyboardKey::KEY_RIGHT, rl) {
                        commands.push(Command::MoveCursorRight(false));
                    }
                    if press_and_repeat(KeyboardKey::KEY_UP, rl) {
                        commands.push(Command::MoveCursorUp);
                    }
                    if press_and_repeat(KeyboardKey::KEY_DOWN, rl) {
                        commands.push(Command::MoveCursorDown);
                    }
                    if press_and_repeat(KeyboardKey::KEY_BACKSPACE, rl) {
                        commands.push(Command::DeleteCharBackward);
                    }
                    if press_and_repeat(KeyboardKey::KEY_DELETE, rl) {
                        commands.push(Command::DeleteCharForward);
                    }
                    if press_and_repeat(KeyboardKey::KEY_ENTER, rl) {
                        commands.push(Command::InsertChar('\n'));
                    }
                }
            }

            if rl.is_window_resized() {
                let width = rl.get_screen_width() as usize;
                let height = rl.get_screen_height() as usize;
                commands.push(Command::Resize(Size { width, height }));
            }
        } // rl borrow ends here.

        // Apply commands after rl is no longer borrowed.
        for command in commands {
            self.apply_command(command)?;
        }

        Ok(())
    }

    /// Proccess a command and apply it to the editor state.
    pub fn apply_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::ForceError => {
                error!("This is forced error designed for testing");
                anyhow::bail!("Test error");
            }
            Command::Quit => self.should_quit = true,
            Command::MoveCursorLeft => move_cursor_left(&mut self.window.cursor),
            Command::MoveCursorRight(exceed) => {
                move_cursor_right(&mut self.window.cursor, &self.window.buffer, exceed)
            }
            Command::MoveCursorUp => move_cursor_up(&mut self.window.cursor, &self.window.buffer),
            Command::MoveCursorDown => {
                move_cursor_down(&mut self.window.cursor, &self.window.buffer)
            }
            Command::MoveCursorEndOfLine => {
                move_cursor_end_of_line(&mut self.window.cursor, &self.window.buffer)
            }
            Command::MoveCursorStartOfLine => move_cursor_start_of_line(&mut self.window.cursor),
            Command::MoveCursorFirstCharOfLine => {
                move_cursor_first_char_of_line(&mut self.window.cursor, &self.window.buffer)
            }
            Command::MoveCursorWordForward(big_word) => {
                move_cursor_word_forward(&mut self.window.cursor, &self.window.buffer, big_word)
            }
            Command::MoveCursorWordBackward(big_word) => {
                move_cursor_word_backward(&mut self.window.cursor, &self.window.buffer, big_word)
            }
            Command::MoveCursorWordForwardEnd(big_word) => {
                move_cursor_word_forward_end(&mut self.window.cursor, &self.window.buffer, big_word)
            }
            Command::None => {}
            Command::SwitchMode(mode) => self.switch_mode(mode),
            Command::Resize(new_size) => self.handle_resize(new_size),
            Command::InsertChar(c) => {
                self.window
                    .buffer
                    .insert_char(self.window.cursor.position, c);
                move_cursor_after_insert(&mut self.window.cursor, c)
            }
            Command::DeleteCharBackward => {
                self.window
                    .buffer
                    .delete_char_backward(self.window.cursor.position);
                move_cursor_before_deleting_backward(&mut self.window.cursor, &self.window.buffer);
            }
            Command::DeleteCharForward => {
                self.window
                    .buffer
                    .delete_char_forward(self.window.cursor.position);
            }
        }

        self.window.scroll_to_cursor(&self.renderer);

        Ok(())
    }

    /// Updates the viewport size, scroll if necessary and mark the window for a
    /// redraw.
    fn handle_resize(&mut self, new_size: Size) {
        self.window.viewport_size = new_size;
        self.window.scroll_to_cursor(&self.renderer);
        self.status_bar.size = new_size;
    }

    fn switch_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    fn render(&mut self) {
        let file_name = self.window.buffer.file_path.clone();
        let cursor_position = self.window.cursor.position.clone();

        self.status_bar
            .update(self.mode, file_name, cursor_position);

        self.status_bar.render(&mut self.renderer);
        self.window.render(&mut self.renderer);
        self.renderer
            .enqueue_gui_command(RenderGUICommand::ClearBackground(Color::LIGHTGRAY));
        self.renderer.render();
    }
}

impl Drop for EditorState {
    fn drop(&mut self) {
        match self.renderer.interface {
            InterfaceType::TUI => self.renderer.terminal.as_ref().unwrap().kill(),
            InterfaceType::GUI => {} // Raylib kills itself.
        }
    }
}

fn press_and_repeat(key: KeyboardKey, rl: &mut RaylibHandle) -> bool {
    rl.is_key_pressed(key) || rl.is_key_pressed_repeat(key)
}
