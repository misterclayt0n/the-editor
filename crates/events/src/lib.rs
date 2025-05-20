// REFACTOR: Remove crossterm dependency from this crate, refactor it using TerminalInterface.
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEvent};
use raylib::{ffi::KeyboardKey, RaylibHandle};
use utils::{error, Command, InterfaceType, Mode, Size};

/// Event is any type of event that the editor can compute.
pub enum Event {
    TerminalKeyPress(KeyEvent),
    Resize(usize, usize),
    GuiKeyPress(KeyboardKey),
    Mock, // TODO: more events like mouse clicking, scrolling, and things of the nature.
}

pub struct EventHandler {
    interface: InterfaceType,
}

impl EventHandler {
    pub fn new(interface: InterfaceType) -> Self {
        EventHandler { interface }
    }

    pub fn poll_events(&self, rl: Option<&mut RaylibHandle>) -> Vec<Event> {
        match self.interface {
            InterfaceType::TUI => self.poll_terminal_events(),
            InterfaceType::GUI => {
                if let Some(mut handle) = rl {
                    self.poll_gui_events(&mut handle)
                } else {
                    error!("RaylibHandle is required for GUI event polling but was not provided.");
                    Vec::new()
                }
            }
        }
    }

    /// Capture events from the terminal and return them in a Vector.
    pub fn poll_terminal_events(&self) -> Vec<Event> {
        let mut events = Vec::new();

        // We use event::poll here with a timeout of 0 to make it non-blocking.
        if let Err(e) = event::poll(Duration::from_millis(0)) {
            error!("Failed to poll events: {}", e);
            return events;
        }

        if let Ok(c_event) = event::read() {
            // c_event is a crossterm event.
            match c_event {
                CEvent::Key(key_event) => events.push(Event::TerminalKeyPress(key_event)),
                CEvent::Resize(width, height) => {
                    events.push(Event::Resize(width as usize, height as usize))
                }
                // TODO: Treat other events.
                _ => {}
            }
        }

        return events;
    }

    fn poll_gui_events(&self, rl: &mut RaylibHandle) -> Vec<Event> {
        let mut events = Vec::new();

        if let Some(key) = rl.get_key_pressed() {
            events.push(Event::GuiKeyPress(key));
        }

        // TODO: Check for mouse presses.
        if rl.is_window_resized() {
            events.push(Event::Resize(
                rl.get_screen_width() as usize,
                rl.get_screen_height() as usize,
            ));
        }

        return events;
    }

    /// Maps `Events` from `crossterm` to a `Vec<Command>`
    pub fn handle_event(&self, event: Event, mode: Mode) -> Result<Vec<Command>> {
        let mut commands = Vec::new();

        match event {
            Event::TerminalKeyPress(key_event) => {
                // Reuse the existing logic to `KeyPress`
                commands = self.handle_terminal_key_event(key_event, mode);
            }
            Event::Resize(width, height) => {
                commands.push(Command::Resize(Size { width, height }));
            }
            Event::GuiKeyPress(key) => {
                commands = self.handle_gui_key_event(key, mode);
            }
            Event::Mock => {}
        }

        Ok(commands)
    }

    /// Returns a `Vec<Command>` based on the current `Mode` and `KeyEvent`.
    pub fn handle_terminal_key_event(&self, key_event: KeyEvent, mode: Mode) -> Vec<Command> {
        let mut commands = Vec::new();

        match mode {
            Mode::Normal => match key_event.code {
                KeyCode::Char('q') => commands.push(Command::Quit),
                KeyCode::Char('h') => commands.push(Command::MoveCursorLeft),
                KeyCode::Char('l') => commands.push(Command::MoveCursorRight(false)),
                KeyCode::Char('k') => commands.push(Command::MoveCursorUp),
                KeyCode::Char('j') => commands.push(Command::MoveCursorDown),
                KeyCode::Char('i') => commands.push(Command::SwitchMode(Mode::Insert)),
                KeyCode::Char('$') => commands.push(Command::MoveCursorEndOfLine),
                KeyCode::Char('0') => commands.push(Command::MoveCursorStartOfLine),
                KeyCode::Char('_') => commands.push(Command::MoveCursorFirstCharOfLine),
                KeyCode::Char('w') => commands.push(Command::MoveCursorWordForward(false)),
                KeyCode::Char('W') => commands.push(Command::MoveCursorWordForward(true)),
                KeyCode::Char('b') => commands.push(Command::MoveCursorWordBackward(false)),
                KeyCode::Char('B') => commands.push(Command::MoveCursorWordBackward(true)),
                KeyCode::Char('e') => commands.push(Command::MoveCursorWordForwardEnd(false)),
                KeyCode::Char('E') => commands.push(Command::MoveCursorWordForwardEnd(true)),
                KeyCode::Char('x') => commands.push(Command::DeleteCharForward),
                KeyCode::Char('z') => commands.push(Command::ForceError),
                KeyCode::Char('a') => {
                    commands.push(Command::MoveCursorRight(true));
                    commands.push(Command::SwitchMode(Mode::Insert));
                }
                _ => {}
            },
            Mode::Insert => match key_event.code {
                KeyCode::Esc => {
                    commands.push(Command::MoveCursorLeft);
                    commands.push(Command::SwitchMode(Mode::Normal))
                }
                KeyCode::Char(c) => commands.push(Command::InsertChar(c)),
                KeyCode::Enter => commands.push(Command::InsertChar('\n')),
                KeyCode::Left => commands.push(Command::MoveCursorLeft),
                KeyCode::Right => commands.push(Command::MoveCursorRight(false)),
                KeyCode::Up => commands.push(Command::MoveCursorUp),
                KeyCode::Down => commands.push(Command::MoveCursorDown),
                KeyCode::Backspace => commands.push(Command::DeleteCharBackward),
                KeyCode::Delete => commands.push(Command::DeleteCharForward),
                _ => {}
            },
        }

        return commands;
    }

    pub fn handle_gui_key_event(&self, key: KeyboardKey, mode: Mode) -> Vec<Command> {
        let mut commands = Vec::new();
        match mode {
            Mode::Normal => match key {
                KeyboardKey::KEY_Q => commands.push(Command::Quit),
                KeyboardKey::KEY_H => commands.push(Command::MoveCursorLeft),
                KeyboardKey::KEY_L => commands.push(Command::MoveCursorRight(false)),
                KeyboardKey::KEY_K => commands.push(Command::MoveCursorUp),
                KeyboardKey::KEY_J => commands.push(Command::MoveCursorDown),
                KeyboardKey::KEY_I => commands.push(Command::SwitchMode(Mode::Insert)),
                KeyboardKey::KEY_X => commands.push(Command::DeleteCharForward),
                // NOTE: Raylib doesn't distinguish between '0' and KeyCode::Char('0') easily for $ and 0
                // You might need to check rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) for '$' if using KeyboardKey::KEY_FOUR
                // Or, handle character input more generally in Insert mode and use specific keys for Normal mode.
                // For simplicity, I'm mapping some directly.
                _ => {}
            },
            Mode::Insert => match key {
                KeyboardKey::KEY_ESCAPE => commands.push(Command::SwitchMode(Mode::Normal)),
                KeyboardKey::KEY_ENTER | KeyboardKey::KEY_KP_ENTER => {
                    commands.push(Command::InsertChar('\n'))
                }
                KeyboardKey::KEY_BACKSPACE => commands.push(Command::DeleteCharBackward),
                KeyboardKey::KEY_DELETE => commands.push(Command::DeleteCharForward),
                KeyboardKey::KEY_LEFT => commands.push(Command::MoveCursorLeft),
                KeyboardKey::KEY_RIGHT => commands.push(Command::MoveCursorRight(false)),
                KeyboardKey::KEY_UP => commands.push(Command::MoveCursorUp),
                KeyboardKey::KEY_DOWN => commands.push(Command::MoveCursorDown),
                _ => {
                    // Convert other keys to characters if possible
                    if let Some(c) = key_to_char(key, raylib::consts::KeyboardKey::KEY_CAPS_LOCK) {
                        // Placeholder for actual shift/caps state
                        commands.push(Command::InsertChar(c));
                    }
                }
            },
        }
        commands
    }
}

/// Helper function to convert KeyboardKey to char.
/// This is a simplified version. For full accuracy, you'd need to check shift, caps lock, etc.
/// Raylib's `GetKeyPressed()` gives you the raw key, not the char it would produce.
/// For text input, `GetCharPressed()` is often better if available and suitable.
fn key_to_char(key: KeyboardKey, _is_caps_lock_on: raylib::consts::KeyboardKey) -> Option<char> {
    // A more robust implementation would involve checking rl.is_key_down(KeyboardKey::LEFT_SHIFT) etc.
    // or using a different Raylib function if one exists for direct char input.
    // Since rl is not available here, this is a limitation.
    // This example doesn't handle shift for symbols like '!' from '1'.
    match key {
        KeyboardKey::KEY_A => Some('a'),
        KeyboardKey::KEY_B => Some('b'),
        KeyboardKey::KEY_C => Some('c'),
        KeyboardKey::KEY_D => Some('d'),
        KeyboardKey::KEY_E => Some('e'),
        KeyboardKey::KEY_F => Some('f'),
        KeyboardKey::KEY_G => Some('g'),
        KeyboardKey::KEY_H => Some('h'),
        KeyboardKey::KEY_I => Some('i'),
        KeyboardKey::KEY_J => Some('j'),
        KeyboardKey::KEY_K => Some('k'),
        KeyboardKey::KEY_L => Some('l'),
        KeyboardKey::KEY_M => Some('m'),
        KeyboardKey::KEY_N => Some('n'),
        KeyboardKey::KEY_O => Some('o'),
        KeyboardKey::KEY_P => Some('p'),
        KeyboardKey::KEY_Q => Some('q'),
        KeyboardKey::KEY_R => Some('r'),
        KeyboardKey::KEY_S => Some('s'),
        KeyboardKey::KEY_T => Some('t'),
        KeyboardKey::KEY_U => Some('u'),
        KeyboardKey::KEY_V => Some('v'),
        KeyboardKey::KEY_W => Some('w'),
        KeyboardKey::KEY_X => Some('x'),
        KeyboardKey::KEY_Y => Some('y'),
        KeyboardKey::KEY_Z => Some('z'),
        KeyboardKey::KEY_ZERO | KeyboardKey::KEY_KP_0 => Some('0'),
        KeyboardKey::KEY_ONE | KeyboardKey::KEY_KP_1 => Some('1'),
        KeyboardKey::KEY_TWO | KeyboardKey::KEY_KP_2 => Some('2'),
        KeyboardKey::KEY_THREE | KeyboardKey::KEY_KP_3 => Some('3'),
        KeyboardKey::KEY_FOUR | KeyboardKey::KEY_KP_4 => Some('4'),
        KeyboardKey::KEY_FIVE | KeyboardKey::KEY_KP_5 => Some('5'),
        KeyboardKey::KEY_SIX | KeyboardKey::KEY_KP_6 => Some('6'),
        KeyboardKey::KEY_SEVEN | KeyboardKey::KEY_KP_7 => Some('7'),
        KeyboardKey::KEY_EIGHT | KeyboardKey::KEY_KP_8 => Some('8'),
        KeyboardKey::KEY_NINE | KeyboardKey::KEY_KP_9 => Some('9'),
        KeyboardKey::KEY_SPACE => Some(' '),
        KeyboardKey::KEY_PERIOD | KeyboardKey::KEY_KP_DECIMAL => Some('.'),
        KeyboardKey::KEY_COMMA => Some(','),
        KeyboardKey::KEY_SEMICOLON => Some(';'),
        KeyboardKey::KEY_SLASH | KeyboardKey::KEY_KP_DIVIDE => Some('/'),
        KeyboardKey::KEY_BACKSLASH => Some('\\'),
        KeyboardKey::KEY_EQUAL | KeyboardKey::KEY_KP_EQUAL => Some('='),
        KeyboardKey::KEY_MINUS | KeyboardKey::KEY_KP_SUBTRACT => Some('-'),
        KeyboardKey::KEY_APOSTROPHE => Some('\''),
        KeyboardKey::KEY_GRAVE => Some('`'),
        KeyboardKey::KEY_LEFT_BRACKET => Some('['),
        KeyboardKey::KEY_RIGHT_BRACKET => Some(']'),
        _ => None,
    }
}
