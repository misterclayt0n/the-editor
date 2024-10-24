use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{EditorCommand, Mode, ModeType, WordType};

pub struct CommandNormalMode;

impl CommandNormalMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for CommandNormalMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        _command_buffer: &mut String,
    ) -> Option<EditorCommand> {
        match event {
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCommandBarCursorLeft),
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCommandBarCursorRight),
            KeyEvent {
                code: KeyCode::Char('0'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCommandBarCursorStart),
            KeyEvent {
                code: KeyCode::Char('$'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCommandBarCursorEnd),
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SwitchMode(ModeType::Command)),
            KeyEvent {
                code: KeyCode::Char('I'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::AppendStartCommandBar),
            KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::AppendEndCommandBar),
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::AppendRightCommandBar),
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SwitchMode(ModeType::Normal)),
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCommandBarWordForward(WordType::Word)),
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCommandBarWordBackward(WordType::Word)),
            KeyEvent {
                code: KeyCode::Char('W'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCommandBarWordForward(WordType::BigWord)),
            KeyEvent {
                code: KeyCode::Char('B'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCommandBarWordBackward(WordType::BigWord)),
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCommandBarWordEnd(WordType::Word)),
            KeyEvent {
                code: KeyCode::Char('E'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCommandBarWordEnd(WordType::BigWord)),
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
