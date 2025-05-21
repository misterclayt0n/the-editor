use anyhow::Result;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEvent};
use std::time::Duration;
use utils::{error, Command, Mode, Size};

pub enum Event {
    TerminalKeyPress(KeyEvent),
    Resize(usize, usize),
    Mock,
}

/// Specific event handler for the terminal version, that is event driven.
pub struct EventHandler;

impl EventHandler {
    pub fn new() -> Self {
        EventHandler { }
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        self.poll_terminal_events()
    }

    fn poll_terminal_events(&self) -> Vec<Event> {
        let mut events = Vec::new();

        if let Err(e) = event::poll(Duration::from_millis(50)) {
            error!("Failed to poll events: {}", e);
            return events;
        }

        if let Ok(c_event) = event::read() {
            match c_event {
                CEvent::Key(key_event) => events.push(Event::TerminalKeyPress(key_event)),
                CEvent::Resize(width, height) => {
                    events.push(Event::Resize(width as usize, height as usize))
                }
                _ => {}
            }
        }

        events
    }

    pub fn handle_event(&self, event: Event, mode: Mode) -> Result<Vec<Command>> {
        let mut commands = Vec::new();

        if let Event::TerminalKeyPress(key_event) = event {
            commands = self.handle_terminal_key_event(key_event, mode);
        } else if let Event::Resize(width, height) = event {
            commands.push(Command::Resize(Size { width, height }));
        }

        Ok(commands)
    }

    fn handle_terminal_key_event(&self, key_event: KeyEvent, mode: Mode) -> Vec<Command> {
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

        commands
    }
}
