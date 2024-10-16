use crate::prelude::*;
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};

use core::fmt;
use std::{
    env,
    io::Error,
    panic::{set_hook, take_hook},
};

mod color_scheme;
mod documentstatus;
mod terminal;
mod uicomponents;

use documentstatus::DocumentStatus;
use terminal::Terminal;
use uicomponents::{CommandBar, MessageBar, StatusBar, UIComponent, View};

const QUIT_TIMES: u8 = 3;

#[derive(Clone, Copy, PartialEq)]
enum Operator {
    Delete,
    Change,
    Yank,
}

#[derive(Clone, Copy)]
enum TextObject {
    Inner(char), // represents 'i' followed by a delimiter, like '('
}

#[derive(Clone, Copy)]
pub enum Normal {
    PageUp,
    PageDown,
    StartOfLine,
    FirstCharLine,
    EndOfLine,
    Up,
    Left,
    Right,
    Down,
    WordForward,
    WordBackward,
    BigWordForward,
    BigWordBackward,
    WordEndForward,
    BigWordEndForward,
    GoToTop,
    GoToBottom,
    AppendRight,
    InsertAtLineStart,
    InsertAtLineEnd,
}

#[derive(Clone, Copy)]
pub enum Edit {
    Insert(char),
    InsertNewline,
    Delete,
    DeleteBackward,
    SubstituteChar,
    ChangeLine,
    SubstitueSelection,
    InsertNewlineBelow,
    InsertNewlineAbove,
}

#[derive(Eq, PartialEq, Default, Debug, Clone, Copy)]
enum PromptType {
    Search,
    Save,
    #[default]
    None,
}

impl PromptType {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeType {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
    Prompt(PromptType),
}

impl fmt::Display for ModeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeType::Insert => write!(f, "INSERT"),
            ModeType::Normal => write!(f, "NORMAL"),
            ModeType::Visual => write!(f, "VISUAL"),
            ModeType::VisualLine => write!(f, "VISUAL LINE"),
            ModeType::Command => write!(f, "COMMAND"),
            ModeType::Prompt(prompt_type) => match prompt_type {
                PromptType::Search => write!(f, "SEARCH"),
                PromptType::Save => write!(f, "SAVE"),
                _ => write!(f, "PROMPT"),
            },
        }
    }
}

impl Default for ModeType {
    fn default() -> Self {
        ModeType::Normal
    }
}

use std::any::Any;

trait Mode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        command_buffer: &mut String,
    ) -> Option<EditorCommand>;
    fn enter(&mut self) -> Vec<EditorCommand>;
    fn exit(&mut self) -> Vec<EditorCommand>;
    fn as_any(&self) -> &dyn Any;
}

enum EditorCommand {
    MoveCursor(Normal),
    EditCommand(Edit),
    SwitchMode(ModeType),
    SwitchToNormalFromInserion,
    HandleQuitCommand,
    HandleSaveCommand,
    SetPrompt(PromptType),
    ResetQuitTimes,
    UpdateInsertionPoint,
    UpdateMessage(String),
    SaveAs(String),
    Save,
    Quit,
    HandleResizeCommand(Size),
    UpdateCommandBar(Edit),
    PerformSearch(String),
    ClearSelection,
    DeleteSelection,
    UpdateSelection,
    StartSelection(SelectionMode),
    DeleteCharAtCursor,
    DeleteCurrentLine,
    ChangeCurrentLine,
    DeleteCurrentLineAndLeaveEmpty,
    DeleteUntilEndOfLine,
    ChangeUntilEndOfLine,
    UpdateInsertionPointToCursorPosition,
    SetNeedsRedraw,
    HandleVisualMovement(Normal),
    HandleVisualLineMovement(Normal),
    MoveCursorAndSwitchMode(Normal, ModeType),
    EditAndSwitchMode(Edit, ModeType),
    EnterSearch,
    ExitSearch,
    SearchNext,
    SearchPrevious,
    OperatorTextObject(Operator, TextObject),
    VisualSelectTextObject(TextObject),
}

pub struct Editor {
    should_quit: bool,
    view: View,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    terminal_size: Size,
    title: String,
    quit_times: u8,
    current_mode: ModeType,
    current_mode_impl: Box<dyn Mode>,
    prompt_type: PromptType,
    command_buffer: String,
}

impl Editor {
    //
    // Struct lifecycle
    //

    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();

        set_hook(Box::new(move |panic_info| {
            let _ = Terminal::kill();
            current_hook(panic_info);
        }));

        Terminal::init()?;

        let mut editor = Self {
            should_quit: false,
            view: View::default(),
            status_bar: StatusBar::default(),
            message_bar: MessageBar::default(),
            command_bar: CommandBar::default(),
            terminal_size: Size::default(),
            title: String::new(),
            quit_times: 0,
            current_mode: ModeType::Normal,
            current_mode_impl: Box::new(NormalMode::new()),
            prompt_type: PromptType::None,
            command_buffer: String::new(),
        };
        let size = Terminal::size().unwrap_or_default();
        editor.handle_resize_command(size);
        editor.update_message("we gucci");

        let args: Vec<String> = env::args().collect();

        if let Some(file_name) = args.get(1) {
            debug_assert!(!file_name.is_empty());

            if editor.view.load(file_name).is_err() {
                editor.update_message(&format!("ERROR: Could not open file: {file_name}"));
            }
        }

        editor.switch_mode(ModeType::Normal);

        editor.refresh_status();
        Ok(editor)
    }

    //
    // Event Loop
    //

    pub fn run(&mut self) {
        loop {
            self.refresh_screen();
            if self.should_quit {
                break;
            }
            match read() {
                Ok(event) => {
                    if let Event::Key(key_event) = event {
                        if let Some(command) = self
                            .current_mode_impl
                            .handle_event(key_event, &mut self.command_buffer)
                        {
                            self.execute_command(command);
                        }
                    } else if let Event::Resize(width_u16, height_u16) = event {
                        let size = Size {
                            height: height_u16 as usize,
                            width: width_u16 as usize,
                        };
                        self.execute_command(EditorCommand::HandleResizeCommand(size));
                    }
                }
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not read event: {err:?}");
                    }
                }
            }
            self.refresh_status();
        }
    }

    fn execute_command(&mut self, command: EditorCommand) {
        match command {
            EditorCommand::MoveCursor(direction) => {
                self.view.handle_normal_command(direction);
            }
            EditorCommand::VisualSelectTextObject(text_object) => {
                self.switch_mode(ModeType::Visual);
                self.view.select_text_object(text_object);
            }
            EditorCommand::OperatorTextObject(operator, text_object) => {
                self.view.handle_operator_text_object(operator, text_object);

                if operator == Operator::Change {
                    self.switch_mode(ModeType::Insert);
                }
            }
            EditorCommand::MoveCursorAndSwitchMode(direction, mode) => {
                self.view.handle_normal_command(direction);
                self.switch_mode(mode);
            }
            EditorCommand::EditAndSwitchMode(edit, mode) => {
                self.view.handle_edit_command(edit);
                self.switch_mode(mode);
            }
            EditorCommand::EditCommand(edit) => {
                self.view.handle_edit_command(edit);
            }
            EditorCommand::SwitchToNormalFromInserion => {
                self.view.handle_normal_command(Normal::Left);
                self.switch_mode(ModeType::Normal);
            }
            EditorCommand::SwitchMode(mode) => {
                self.switch_mode(mode);
            }
            EditorCommand::HandleQuitCommand => {
                self.handle_quit_command();
            }
            EditorCommand::HandleSaveCommand => {
                self.handle_save_command();
            }
            EditorCommand::SetPrompt(prompt_type) => {
                self.set_prompt(prompt_type);
                self.switch_mode(ModeType::Prompt(prompt_type));
            }
            EditorCommand::ResetQuitTimes => {
                self.reset_quit_times();
            }
            EditorCommand::EnterSearch => {
                self.view.enter_search();
            }
            EditorCommand::ExitSearch => {
                self.view.dismiss_search();
            }
            EditorCommand::SearchNext => {
                self.view.search_next();
            }
            EditorCommand::SearchPrevious => {
                self.view.search_prev();
            }
            EditorCommand::UpdateInsertionPoint => {
                self.update_insertion_point();
            }
            EditorCommand::UpdateMessage(msg) => {
                self.update_message(&msg);
            }
            EditorCommand::SaveAs(file_name) => {
                self.save(Some(&file_name));
            }
            EditorCommand::Save => {
                self.save(None);
            }
            EditorCommand::Quit => {
                self.should_quit = true;
            }
            EditorCommand::HandleResizeCommand(size) => {
                self.handle_resize_command(size);
            }
            EditorCommand::UpdateCommandBar(edit) => {
                self.command_bar.handle_edit_command(edit);
                if let PromptType::Search = self.prompt_type {
                    let query = self.command_bar.value();
                    self.view.search(&query);
                }
            }
            EditorCommand::PerformSearch(_) => {
                let query = self.command_bar.value();
                self.view.search(&query);
            }
            EditorCommand::ClearSelection => {
                self.view.clear_selection();
            }
            EditorCommand::DeleteSelection => {
                self.view.delete_selection();
                self.execute_command(EditorCommand::ClearSelection);
                self.execute_command(EditorCommand::SwitchMode(ModeType::Normal));
            }
            EditorCommand::UpdateSelection => {
                self.view.update_selection();
            }
            EditorCommand::StartSelection(selection_mode) => {
                self.view.start_selection(selection_mode);
            }
            EditorCommand::DeleteCharAtCursor => {
                self.view.delete_char_at_cursor();
            }
            EditorCommand::DeleteCurrentLine => {
                self.view.delete_current_line();
            }
            EditorCommand::ChangeCurrentLine => {
                self.view.delete_current_line_and_leave_empty();
                self.execute_command(EditorCommand::SwitchMode(ModeType::Insert))
            }
            EditorCommand::DeleteCurrentLineAndLeaveEmpty => {
                self.view.delete_current_line_and_leave_empty();
            }
            EditorCommand::DeleteUntilEndOfLine => {
                self.view.delete_until_end_of_line();
            }
            EditorCommand::ChangeUntilEndOfLine => {
                self.view.delete_until_end_of_line();
                self.execute_command(EditorCommand::SwitchMode(ModeType::Insert))
            }
            EditorCommand::UpdateInsertionPointToCursorPosition => {
                self.view.update_insertion_point_to_cursor_position();
            }
            EditorCommand::SetNeedsRedraw => {
                self.view.set_needs_redraw(true);
            }
            EditorCommand::HandleVisualMovement(direction) => {
                self.view.handle_visual_movement(direction);
            }
            EditorCommand::HandleVisualLineMovement(direction) => {
                self.view.handle_visual_line_movement(direction);
            }
        }
    }

    fn refresh_screen(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 {
            return;
        }

        let bottom_bar_row = self.terminal_size.height.saturating_sub(1);
        let _ = Terminal::hide_cursor();

        if self.in_prompt() {
            self.command_bar.render(bottom_bar_row);
        } else {
            self.message_bar.render(bottom_bar_row);
        }

        if self.terminal_size.height > 1 {
            self.status_bar
                .render(self.terminal_size.height.saturating_sub(2));
        }

        if self.terminal_size.height > 2 {
            self.view.render(0);
        }

        let new_cursor_position = if self.in_prompt() {
            Position {
                row: bottom_bar_row,
                col: self.command_bar.cursor_position_col(),
            }
        } else {
            self.view.cursor_position()
        };

        let _ = Terminal::move_cursor_to(new_cursor_position);
        let _ = Terminal::show_cursor();
        let _ = Terminal::execute();
    }

    fn refresh_status(&mut self) {
        let status = self.view.get_status();
        let title = format!("{} - {NAME}", status.file_name);
        self.status_bar.update_status(status, self.current_mode);

        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
            self.title = title;
        }
    }

    //
    // Mode handling
    //

    fn switch_mode(&mut self, mode: ModeType) {
        // Exit the current mode
        let exit_commands = self.current_mode_impl.exit();
        for command in exit_commands {
            self.execute_command(command);
        }

        // Set the current mode type
        self.current_mode = mode;

        // Create a new mode instance
        self.current_mode_impl = match mode {
            ModeType::Normal => Box::new(NormalMode::new()),
            ModeType::Insert => Box::new(InsertMode::new()),
            ModeType::Visual => Box::new(VisualMode::new()),
            ModeType::VisualLine => Box::new(VisualLineMode::new()),
            ModeType::Command => Box::new(CommandMode::new()),
            ModeType::Prompt(prompt_type) => Box::new(PromptMode::new(prompt_type)),
        };

        // Enter the new mode
        let enter_commands = self.current_mode_impl.enter();
        for command in enter_commands {
            self.execute_command(command);
        }
    }

    //
    // Resize command handling
    //

    fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;

        self.view.resize(Size {
            height: size.height.saturating_sub(2),
            width: size.width,
        });

        let bar_size = Size {
            height: 1,
            width: size.width,
        };

        self.message_bar.resize(bar_size);
        self.status_bar.resize(bar_size);
        self.command_bar.resize(bar_size);
    }

    //
    // Quit command handling
    //

    fn handle_quit_command(&mut self) {
        if !self.view.get_status().is_modified || self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
        } else if self.view.get_status().is_modified {
            self.update_message(&format!(
                "WARNING! File has unsaved changes. Press Ctrl-Q {} more times to quit.",
                QUIT_TIMES - self.quit_times - 1
            ));

            self.quit_times += 1;
        }
    }

    fn reset_quit_times(&mut self) {
        if self.quit_times > 0 {
            self.quit_times = 0;
            self.update_message("");
        }
    }

    //
    // Save command & prompt handling
    //

    fn handle_save_command(&mut self) {
        if self.view.is_file_loaded() {
            self.save(None);
        } else {
            self.set_prompt(PromptType::Save);
            self.switch_mode(ModeType::Prompt(PromptType::Save));
        }
    }

    fn save(&mut self, file_name: Option<&str>) {
        let result = if let Some(name) = file_name {
            self.view.save_as(name)
        } else {
            self.view.save()
        };
        if result.is_ok() {
            self.update_message("File saved successfully.");
        } else {
            self.update_message("Error writing file!");
        }
    }

    //
    // Message & command bar
    //

    fn update_message(&mut self, new_message: &str) {
        self.message_bar.update_message(new_message);
    }

    //
    // Prompt handling
    //

    fn in_prompt(&self) -> bool {
        matches!(self.current_mode, ModeType::Prompt(_))
    }

    fn set_prompt(&mut self, prompt_type: PromptType) {
        match prompt_type {
            PromptType::None => self.message_bar.set_needs_redraw(true),
            PromptType::Save => {
                self.command_bar.set_prompt("Save as: ");
                self.command_bar.clear_value();
            }
            PromptType::Search => {
                self.command_bar.set_prompt("/");
                self.command_bar.clear_value();
            }
        }

        self.prompt_type = prompt_type;
    }

    fn update_insertion_point(&mut self) {
        self.view.update_insertion_point_to_cursor_position();
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::kill();
    }
}

//
// Mode Implementations
//

struct NormalMode;

impl NormalMode {
    fn new() -> Self {
        Self
    }
}

impl Mode for NormalMode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        command_buffer: &mut String,
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
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::HandleSaveCommand),
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::SetPrompt(PromptType::Search)),
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
                }
                else {
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
            } => Some(EditorCommand::SetPrompt(PromptType::Search)),
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct InsertMode;

impl InsertMode {
    fn new() -> Self {
        Self
    }
}

impl Mode for InsertMode {
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct VisualMode;

impl VisualMode {
    fn new() -> Self {
        Self
    }
}

impl Mode for VisualMode {
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct VisualLineMode;

impl VisualLineMode {
    fn new() -> Self {
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
            } => Some(EditorCommand::DeleteSelection),
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct CommandMode;

impl CommandMode {
    fn new() -> Self {
        Self
    }
}

impl Mode for CommandMode {
    fn handle_event(
        &mut self,
        _event: KeyEvent,
        _command_buffer: &mut String,
    ) -> Option<EditorCommand> {
        None
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        vec![]
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        vec![]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct PromptMode {
    prompt_type: PromptType,
}

impl PromptMode {
    fn new(prompt_type: PromptType) -> Self {
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
            _ => None,
        }
    }

    fn enter(&mut self) -> Vec<EditorCommand> {
        match self.prompt_type {
            PromptType::Search => vec![EditorCommand::EnterSearch, EditorCommand::SetNeedsRedraw],
            PromptType::Save => vec![EditorCommand::SetNeedsRedraw],
            _ => vec![],
        }
    }

    fn exit(&mut self) -> Vec<EditorCommand> {
        match self.prompt_type {
            PromptType::Search => vec![EditorCommand::ExitSearch],
            _ => vec![],
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
