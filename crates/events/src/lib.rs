use std::time::Duration;
use thiserror::Error;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEvent};

/// Represents all possible errors that can occur in `events`.
#[derive(Error, Debug)]
pub enum EventsError {
    /// Error in capturing terminal events
    #[error("Error in capturing event: {0}")]
    EventCapture(#[from] std::io::Error),

    /// Error in handling key event
    #[error("Error in handling key event: {0}")]
    KeyEventError(String),

    #[error("Generic error: {0}")]
    GenericError(String)
}

/// Event is any type of event that the editor can compute
pub enum Event {
    KeyPress(KeyEvent),
    Mock
    // TODO: more events like mouse clicking, scrolling, and things of the nature
}

pub enum Command {
    Quit,
    None,
    Print(String) // Just for now
}

pub struct EventHandler;

impl EventHandler {
    pub fn new() -> Self {
        EventHandler
    }

    /// Capture events from the terminal and return them in a Vector
    pub fn poll_events(&self) -> Result<Vec<Event>, EventsError> {
        let mut events = Vec::new();

        // Small timeouts to avoid blocks
        while event::poll(Duration::from_millis(100))? {
            if let Ok(c_event) = event::read() { // c_event is a crossterm event
                match c_event {
                    CEvent::Key(key_event) => events.push(Event::KeyPress(key_event)),

                    // TODO: Treat other events
                    _ => {}
                }
            }
        }

        Ok(events)
    }

    pub fn handle_key_event(&self, key_event: KeyEvent) -> Result<Vec<Command>, EventsError> {
        let mut commands = Vec::new();

        match key_event.code {
            KeyCode::Char('q') => commands.push(Command::Quit),
            KeyCode::Char('a') => {
                return Err(EventsError::KeyEventError(
                    "Key 'a' is not allowed in this context".to_string(),
                ));
            }
            KeyCode::Char(c) => commands.push(Command::Print(format!("Key pressed: {c}"))),
            _ => {}
        }

        Ok(commands)
    }
}
