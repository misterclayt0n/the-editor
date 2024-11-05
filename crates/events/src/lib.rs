use std::time::Duration;

use crossterm::event::{self, Event as CEvent, KeyCode, KeyEvent};
use thiserror::Error;
use utils::{Command, Mode};

/// Represents all possible errors that can occur in `events`.
#[derive(Error, Debug)]
pub enum EventsError {
    /// Error in capturing terminal events.
    #[error("Error in capturing event: {0}")]
    EventCapture(#[from] std::io::Error),

    /// Error in handling key event.
    #[error("Error in handling key event: {0}")]
    KeyEventError(String),

    #[error("Generic error: {0}")]
    GenericError(String),
}

/// Event is any type of event that the editor can compute.
pub enum Event {
    KeyPress(KeyEvent),
    Mock, // TODO: more events like mouse clicking, scrolling, and things of the nature.
}

pub struct EventHandler;

impl EventHandler {
    pub fn new() -> Self {
        EventHandler
    }

    /// Capture events from the terminal and return them in a Vector.
    pub fn poll_events(&self) -> Result<Vec<Event>, EventsError> {
        let mut events = Vec::new();

        // We use event::poll here with a timeout of 0 to make it non-blocking.
        if event::poll(Duration::from_millis(0))? {
            if let Ok(c_event) = event::read() {
                // c_event is a crossterm event.
                match c_event {
                    CEvent::Key(key_event) => events.push(Event::KeyPress(key_event)),
                    // TODO: Treat other events.
                    _ => {}
                }
            }
        }

        Ok(events)
    }

    pub fn handle_key_event(
        &self,
        key_event: KeyEvent,
        mode: Mode,
    ) -> Result<Vec<Command>, EventsError> {
        let mut commands = Vec::new();

        match mode {
            Mode::Normal => match key_event.code {
                KeyCode::Char('q') => commands.push(Command::Quit),
                KeyCode::Char('h') => commands.push(Command::MoveCursorLeft),
                KeyCode::Char('l') => commands.push(Command::MoveCursorRight),
                KeyCode::Char('k') => commands.push(Command::MoveCursorUp),
                KeyCode::Char('j') => commands.push(Command::MoveCursorDown),
                KeyCode::Char('i') => commands.push(Command::SwitchMode(Mode::Insert)),
                KeyCode::Char('a') => {
                    return Err(EventsError::KeyEventError(
                        "Key 'a' is not allowed in this context".to_string(),
                    ));
                }
                KeyCode::Char(c) => commands.push(Command::Print(format!("Key pressed: {c}"))),
                _ => {}
            },
            Mode::Insert => match key_event.code {
                KeyCode::Esc => commands.push(Command::SwitchMode(Mode::Normal)),
                _ => {}
            }
        }

        Ok(commands)
    }
}
