use std::{cell::RefCell, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{uicomponents::CommandBar, window::SplitDirection, Edit, EditorCommand, FocusDirection, InputModeType, InputType, Mode, ModeType, Normal, Operator, TextObject};

pub struct NormalMode;

impl NormalMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for NormalMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        command_buffer: &mut String,
        _command_bar: Rc<RefCell<CommandBar>>
    ) -> Option<EditorCommand> {
        match event {
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::Left)),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::Down)),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::Up)),
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::Right)),
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::HandleQuitCommand),
            KeyEvent {
                code: KeyCode::Char(':'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SwitchMode(ModeType::Input(InputType::Command, InputModeType::Insert))),
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::HandleSaveCommand),
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::SetPrompt(InputType::Search)),
            KeyEvent {
                code: KeyCode::Char('0'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::StartOfLine)),
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::PageDown)),
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::PageUp)),
            KeyEvent {
                code: KeyCode::Char('_'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::FirstCharLine)),
            KeyEvent {
                code: KeyCode::Char('$'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::EndOfLine)),
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::WordForward)),
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::WordBackward)),
            KeyEvent {
                code: KeyCode::Char('W'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::BigWordForward)),
            KeyEvent {
                code: KeyCode::Char('B'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::BigWordBackward)),
            KeyEvent {
            	code: KeyCode::Char('v'),
            	modifiers: KeyModifiers::ALT,
            	..
            } => Some(EditorCommand::Split(SplitDirection::Vertical)),
            KeyEvent {
            	code: KeyCode::Char('h'),
            	modifiers: KeyModifiers::ALT,
            	..
            } => Some(EditorCommand::Split(SplitDirection::Horizontal)),
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::WordEndForward)),
            KeyEvent {
                code: KeyCode::Char('E'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::BigWordEndForward)),
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if command_buffer == "g" {
                    command_buffer.clear();
                    Some(EditorCommand::MoveCursor(Normal::GoToTop))
                } else {
                    command_buffer.clear();
                    command_buffer.push('g');
                    None
                }
            }
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if command_buffer == "d" {
                    command_buffer.clear();
                    Some(EditorCommand::DeleteCurrentLine)
                } else {
                    command_buffer.clear();
                    command_buffer.push('d');
                    None // wait for the next char
                }
            }

            // delimiters
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } if matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | '<' | '>') => {
                if command_buffer == "di" {
                    let operator = Operator::Delete;
                    let text_object = TextObject::Inner(c);
                    command_buffer.clear();
                    Some(EditorCommand::OperatorTextObject(operator, text_object))
                } else if command_buffer == "ci" {
                    let operator = Operator::Change;
                    let text_object = TextObject::Inner(c);
                    command_buffer.clear();
                    Some(EditorCommand::OperatorTextObject(operator, text_object))
                } else {
                    command_buffer.clear();
                    None
                }
            }
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if command_buffer == "v" {
                    command_buffer.push('i');
                    None
                } else if command_buffer == "d" {
                    command_buffer.push('i');
                    None
                } else if command_buffer == "c" {
                    command_buffer.push('i');
                    None
                } else {
                    command_buffer.clear();
                    Some(EditorCommand::SwitchMode(ModeType::Insert))
                }
            }
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if command_buffer == "c" {
                    command_buffer.clear();
                    Some(EditorCommand::ChangeCurrentLine)
                } else {
                    command_buffer.clear();
                    command_buffer.push('c');
                    None // wait for next command
                }
            }
            KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCursor(Normal::GoToBottom)),
            KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                command_buffer.push('v');
                Some(EditorCommand::SwitchMode(ModeType::Visual))
            }
            KeyEvent {
                code: KeyCode::Char('V'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::SwitchMode(ModeType::VisualLine)),
            KeyEvent {
                code: KeyCode::Char('I'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCursorAndSwitchMode(
                Normal::InsertAtLineStart,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::MoveCursorAndSwitchMode(
                Normal::InsertAtLineEnd,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::MoveCursorAndSwitchMode(
                Normal::AppendRight,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditAndSwitchMode(
                Edit::SubstituteChar,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::DeleteCharAtCursor),
            KeyEvent {
                code: KeyCode::Char('O'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::EditAndSwitchMode(
                Edit::InsertNewlineAbove,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Char('o'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::EditAndSwitchMode(
                Edit::InsertNewlineBelow,
                ModeType::Insert,
            )),
            KeyEvent {
                code: KeyCode::Char('D'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::DeleteUntilEndOfLine),
            KeyEvent {
                code: KeyCode::Char('C'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::ChangeUntilEndOfLine),
            KeyEvent {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SetPrompt(InputType::Search)),
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SearchNext),
            KeyEvent {
                code: KeyCode::Char('N'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(EditorCommand::SearchPrevious),
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::ALT,
                ..
            } => Some(EditorCommand::CloseWindow),
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::ALT,
                ..
            } => Some(EditorCommand::AddCursorInCurrentLocation),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::Focus(FocusDirection::Up)),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::Focus(FocusDirection::Down)),
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::Focus(FocusDirection::Right)),
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::Focus(FocusDirection::Left)),
            _ => {
                command_buffer.clear();
                None
            }
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        vec![
            EditorCommand::ResetQuitTimes,
            EditorCommand::UpdateMessage(String::new()),
            EditorCommand::SetNeedsRedraw,
        ]
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        vec![]
    }
}
