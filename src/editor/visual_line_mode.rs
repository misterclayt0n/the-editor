use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::prelude::SelectionMode;

use super::{Edit, EditorCommand, Mode, ModeType, Normal};

pub struct VisualLineMode;

impl VisualLineMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for VisualLineMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        command_buffer: &mut String,
    ) -> Option<EditorCommand> {
        match event {
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                Some(EditorCommand::DeleteSelection)
            },
            KeyEvent {
                code: KeyCode::Char('s') | KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditAndSwitchMode(
                Edit::ChangeLine,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SwitchMode(ModeType::Normal)),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::Down)),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::Up)),
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::Right)),
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::Left)),
            KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::GoToBottom)),
            KeyEvent {
                code: KeyCode::Char('$'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::EndOfLine)),
            KeyEvent {
                code: KeyCode::Char('0'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::StartOfLine)),
            KeyEvent {
                code: KeyCode::Char('_'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::FirstCharLine)),
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if command_buffer == "g" {
                    command_buffer.clear();
                    Some(EditorCommand::HandleVisualLineMovement(Normal::GoToTop))
                } else {
                    command_buffer.push('g');
                    None
                }
            }
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::PageDown)),
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::HandleVisualLineMovement(Normal::PageUp)),
            _ => None,
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        vec![
            EditorCommand::StartSelection(SelectionMode::VisualLine),
            EditorCommand::SetNeedsRedraw,
        ]
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        vec![EditorCommand::ClearSelection]
    }
}
