use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{Edit, EditorCommand, Mode, ModeType, PromptType};

pub struct PromptMode {
    prompt_type: PromptType,
}

impl PromptMode {
    pub fn new(prompt_type: PromptType) -> Self {
        Self { prompt_type }
    }
}

impl Mode for PromptMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        _command_buffer: &mut String,
    ) -> Option<EditorCommand> {
        match self.prompt_type {
            PromptType::Search => match event {
                KeyEvent {
                    code: KeyCode::Esc,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    Some(EditorCommand::SwitchMode(ModeType::Normal))
                },
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
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => Some(EditorCommand::SwitchMode(ModeType::Normal)),
                _ => None,
            },
            PromptType::Save => match event {
                KeyEvent {
                    code: KeyCode::Esc,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => Some(EditorCommand::SwitchMode(ModeType::Normal)),
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
                // KeyEvent {
                //     code: KeyCode::Enter,
                //     modifiers: KeyModifiers::NONE,
                //     ..
                // } => Some(EditorCommand::SaveAs(self.command_bar_value.clone())),
                _ => None,
            },
            PromptType::Command => match event {
                KeyEvent {
                    code: KeyCode::Esc,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => Some(EditorCommand::SwitchMode(ModeType::Normal)),
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
            },
            _ => None,
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        match self.prompt_type {
            PromptType::Search => vec![EditorCommand::EnterSearch, EditorCommand::SetNeedsRedraw],
            PromptType::Save => vec![EditorCommand::SetNeedsRedraw],
            PromptType::Command => vec![EditorCommand::SetNeedsRedraw],
            _ => vec![],
        }
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        match self.prompt_type {
            PromptType::Search => vec![EditorCommand::ExitSearch],
            _ => vec![],
        }
    }
}
