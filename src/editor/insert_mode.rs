use std::{cell::RefCell, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{uicomponents::CommandBar, Edit, EditorCommand, Mode};

pub struct InsertMode;

impl InsertMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for InsertMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        _command_buffer: &mut String,
        _command_bar: Rc<RefCell<CommandBar>>
    ) -> Option<EditorCommand> {
        match event {
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SwitchToNormalFromInserion),
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::EditCommand(Edit::Insert(c))),
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditCommand(Edit::DeleteBackward)),
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditCommand(Edit::Delete)),
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditCommand(Edit::InsertNewline)),
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditCommand(Edit::Insert('\t'))),
            _ => None,
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        vec![
            EditorCommand::UpdateInsertionPoint,
            EditorCommand::ClearSelection,
            EditorCommand::SetNeedsRedraw,
        ]
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        vec![]
    }
}
