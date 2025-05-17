// REFACTOR: Remove crossterm dependency from this crate, refactor it using TerminalInterface.
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEvent};
use utils::{error, Command, Mode, Size};

/// Event is any type of event that the editor can compute.
pub enum Event {
    KeyPress(KeyEvent),
    Resize(usize, usize),
    Mock, // TODO: more events like mouse clicking, scrolling, and things of the nature.
}

pub struct EventHandler;

impl EventHandler {
    pub fn new() -> Self {
        EventHandler
    }

    /// Capture events from the terminal and return them in a Vector.
    pub fn poll_events(&self) -> Vec<Event> {
        let mut events = Vec::new();

        // We use event::poll here with a timeout of 0 to make it non-blocking.
        if let Err(e) = event::poll(Duration::from_millis(0)) {
            error!("Failed to poll events: {}", e);
            return events;
        }

        if let Ok(c_event) = event::read() {
            // c_event is a crossterm event.
            match c_event {
                CEvent::Key(key_event) => events.push(Event::KeyPress(key_event)),
                CEvent::Resize(width, height) => {
                    events.push(Event::Resize(width as usize, height as usize))
                }
                // TODO: Treat other events.
                _ => {}
            }
        }

        return events;
    }

    /// Maps `Events` from `crossterm` to a `Vec<Command>`
    pub fn handle_event(&self, event: Event, mode: Mode) -> Result<Vec<Command>> {
        let mut commands = Vec::new();

        match event {
            Event::KeyPress(key_event) => {
                // Reuse the existing logic to `KeyPress`
                commands = self.handle_key_event(key_event, mode);
            }
            Event::Resize(width, height) => {
                commands.push(Command::Resize(Size { width, height }));
            }
            Event::Mock => {}
        }

        Ok(commands)
    }

    /// Returns a `Vec<Command>` based on the current `Mode` and `KeyEvent`.
    pub fn handle_key_event(&self, key_event: KeyEvent, mode: Mode) -> Vec<Command> {
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
}
