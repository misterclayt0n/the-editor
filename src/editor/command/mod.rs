use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::convert::TryFrom;
mod movecommand;
pub use movecommand::Normal;
mod system;
pub use system::System;
mod edit;
pub use edit::Edit;

use super::{Size, VimMode};

#[derive(Clone, Copy)]
pub enum Command {
    Move(Normal),
    Edit(Edit),
    System(System),
    Vim(VimCommand),
}

#[derive(Clone, Copy)]
pub enum VimCommand {
    ChangeMode(VimMode),
    DeleteLine,
    DeleteSelection,
    SubstituteSelection,
}

impl TryFrom<KeyEvent> for VimCommand {
    type Error = String;

    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        match (event.code, event.modifiers) {
            (KeyCode::Char('i'), KeyModifiers::NONE) => Ok(Self::ChangeMode(VimMode::Insert)),
            (KeyCode::Esc, KeyModifiers::NONE) => Ok(Self::ChangeMode(VimMode::Normal)),
            (KeyCode::Char('v'), KeyModifiers::NONE) => Ok(Self::ChangeMode(VimMode::Visual)),
            (KeyCode::Char(':'), KeyModifiers::NONE) => Ok(Self::ChangeMode(VimMode::CommandMode)),
            (KeyCode::Char('d'), KeyModifiers::NONE) => Ok(Self::DeleteLine),
            // TODO: maybe some more keybindings
            _ => Err(format!("Not a Vim command: {:?}", event)),
        }
    }
}

// clippy::as_conversions: Will run into problems for rare edge case systems where usize < u16
#[allow(clippy::as_conversions)]
impl TryFrom<Event> for Command {
    type Error = String;
    fn try_from(event: Event) -> Result<Self, Self::Error> {
        match event {
            Event::Key(key_event) => {
                // try to convert to vim mode
                VimCommand::try_from(key_event)
                    .map(Command::Vim)
                    .or_else(|_| Edit::try_from(key_event).map(Command::Edit))
                    .or_else(|_| Normal::try_from(key_event).map(Command::Move))
                    .or_else(|_| System::try_from(key_event).map(Command::System))
                    .map_err(|_err| format!("Event not supported: {key_event:?}"))
            }
            Event::Resize(width_u16, height_u16) => Ok(Self::System(System::Resize(Size {
                height: height_u16 as usize,
                width: width_u16 as usize,
            }))),
            _ => Err(format!("Event not supported: {event:?}")),
        }
    }
}

impl Command {
    pub fn from_event(event: Event, current_mode: VimMode) -> Result<Self, String> {
        match event {
            Event::Key(key_event) => match current_mode {
                VimMode::Normal => VimCommand::try_from(key_event)
                    .map(Command::Vim)
                    .or_else(|_| Normal::try_from(key_event).map(Command::Move))
                    .or_else(|_| System::try_from(key_event).map(Command::System)),
                VimMode::Insert => {
                    if key_event.code == KeyCode::Esc {
                        Ok(Command::Vim(VimCommand::ChangeMode(VimMode::Normal)))
                    } else {
                        Edit::try_from(key_event).map(Command::Edit)
                    }
                }
                VimMode::Visual => match key_event.code {
                    KeyCode::Char('d') if key_event.modifiers == KeyModifiers::NONE => {
                        Ok(Command::Vim(VimCommand::DeleteSelection))
                    }
                    KeyCode::Char('s') | KeyCode::Char('c') if key_event.modifiers == KeyModifiers::NONE => {
                        Ok(Command::Vim(VimCommand::SubstituteSelection))
                    }
                    _ => VimCommand::try_from(key_event)
                        .map(Command::Vim)
                        .or_else(|_| Normal::try_from(key_event).map(Command::Move))
                        .or_else(|_| System::try_from(key_event).map(Command::System)),
                },
                VimMode::CommandMode => {
                    if key_event.code == KeyCode::Esc {
                        Ok(Command::Vim(VimCommand::ChangeMode(VimMode::Normal)))
                    // exit command mode on esc
                    } else {
                        Edit::try_from(key_event).map(Command::Edit) // allow text editing on command mode
                    }
                }
            }
            .map_err(|_err| format!("Event not supported: {key_event:?}")),
            Event::Resize(width_u16, height_u16) => Ok(Self::System(System::Resize(Size {
                height: height_u16 as usize,
                width: width_u16 as usize,
            }))),
            _ => Err(format!("Event not supported: {event:?}")),
        }
    }
}
