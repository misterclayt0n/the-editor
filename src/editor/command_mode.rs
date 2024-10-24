use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{Edit, EditorCommand, Mode, ModeType};

pub struct CommandMode;

impl CommandMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for CommandMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        _command_buffer: &mut String,
    ) -> Option<EditorCommand> {
        match event {
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                Some(EditorCommand::SwitchMode(ModeType::CommandNormal))
            },
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::ExecuteCommand),
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::UpdateCommandBar(Edit::Insert(c))),
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::UpdateCommandBar(Edit::DeleteBackward)),
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::UpdateCommandBar(Edit::Delete)),
            _ => None,
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        vec![]
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        vec![]
    }
}
