use std::rc::Rc;
use std::cell::RefCell;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::prelude::*;

use super::{uicomponents::CommandBar, EditorCommand, Mode};

pub struct InputMode {
    input_type: InputType,
    mode: InputModeType,
}

impl InputMode {
    pub fn new(input_type: InputType, mode: InputModeType) -> Self {
        Self { input_type, mode }
    }
}

impl Mode for InputMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        _command_buffer: &mut String,
        command_bar: Rc<RefCell<CommandBar>>,
    ) -> Option<EditorCommand> {
        match self.mode {
            InputModeType::Insert => {
                match event {
                    KeyEvent {
                        code: KeyCode::Esc,
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        self.mode = InputModeType::Normal;
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Enter,
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => Some(EditorCommand::ExecuteCommand),
                    KeyEvent {
                        code: KeyCode::Char(c),
                        modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                        ..
                    } => {
                        Some(EditorCommand::UpdateCommandBar(Edit::Insert(c)))
                    }
                    KeyEvent {
                        code: KeyCode::Backspace,
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        Some(EditorCommand::UpdateCommandBar(Edit::DeleteBackward))
                    }
                    KeyEvent {
                        code: KeyCode::Delete,
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().handle_edit_command(Edit::Delete);
                        None
                    }
                    _ => None,
                }
            }
            InputModeType::Normal => {
                match event {
                    KeyEvent {
                        code: KeyCode::Char('i'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        self.mode = InputModeType::Insert;
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('h'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_left();
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('l'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_right();
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('0'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_start();
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('$'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_end();
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('w'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_word_forward(WordType::Word);
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('b'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_word_backward(WordType::Word);
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('W'),
                        modifiers: KeyModifiers::SHIFT,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_word_forward(WordType::BigWord);
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('B'),
                        modifiers: KeyModifiers::SHIFT,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_word_backward(WordType::BigWord);
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('A'),
                        modifiers: KeyModifiers::SHIFT,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_end();
                        self.mode = InputModeType::Insert;
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Char('I'),
                        modifiers: KeyModifiers::SHIFT,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_start();
                        self.mode = InputModeType::Insert;
                        None
                    }
                    KeyEvent {
                        code: KeyCode::Esc,
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        Some(EditorCommand::SwitchMode(ModeType::Normal))
                    }
                    KeyEvent {
                        code: KeyCode::Char('a'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        command_bar.borrow_mut().move_cursor_right();
                        self.mode = InputModeType::Insert;
                        None
                    }
                    _ => None,
                }
            }
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        match self.input_type {
            InputType::Search => vec![EditorCommand::EnterSearch],
            _ => vec![],
        }
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        match self.input_type {
            InputType::Search => vec![EditorCommand::ExitSearch],
            _ => vec![],
        }
    }
}
