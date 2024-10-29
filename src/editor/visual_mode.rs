use std::{cell::RefCell, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::prelude::SelectionMode;

use super::{uicomponents::CommandBar, Edit, EditorCommand, Mode, ModeType, Normal, TextObject};

pub struct VisualMode;

impl VisualMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for VisualMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        command_buffer: &mut String,
        _command_bar: Rc<RefCell<CommandBar>>
    ) -> Option<EditorCommand> {
        match event {
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::DeleteSelection),
            KeyEvent {
                code: KeyCode::Char('s') | KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditAndSwitchMode(
                Edit::SubstitueSelection,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::Left)),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::Down)),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::Up)),
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::Right)),
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::WordForward)),
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::WordBackward)),
            KeyEvent {
                code: KeyCode::Char('W'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::BigWordForward)),
            KeyEvent {
                code: KeyCode::Char('B'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::BigWordBackward)),
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::WordEndForward)),
            KeyEvent {
                code: KeyCode::Char('E'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::HandleVisualMovement(
                Normal::BigWordEndForward,
            )),
            KeyEvent {
                code: KeyCode::Char('$'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::EndOfLine)),
            KeyEvent {
                code: KeyCode::Char('0'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::StartOfLine)),
            KeyEvent {
                code: KeyCode::Char('_'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::FirstCharLine)),
            KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::GoToBottom)),
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if command_buffer == "g" {
                    command_buffer.clear();
                    Some(EditorCommand::HandleVisualMovement(Normal::GoToTop))
                } else {
                    command_buffer.clear();
                    command_buffer.push('g');
                    None
                }
            }
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::PageDown)),
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::HandleVisualMovement(Normal::PageUp)),
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if command_buffer == "v" {
                    command_buffer.push('i');
                    None
                } else {
                    command_buffer.clear();
                    None
                }
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } if matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | '<' | '>') => {
                if command_buffer == "vi" {
                    let text_object = TextObject::Inner(c);
                    command_buffer.clear();
                    Some(EditorCommand::VisualSelectTextObject(text_object))
                } else {
                    command_buffer.clear();
                    None
                }
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                command_buffer.clear();
                Some(EditorCommand::SwitchMode(ModeType::Normal))
            }
            _ => {
                command_buffer.clear();
                None
            }
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        vec![
            EditorCommand::StartSelection(SelectionMode::Visual),
            EditorCommand::SetNeedsRedraw,
        ]
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        vec![EditorCommand::ClearSelection]
    }
}
