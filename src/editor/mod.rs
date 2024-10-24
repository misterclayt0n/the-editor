use crate::prelude::*;
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};
use window::Window;

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
mod window;

use documentstatus::DocumentStatus;
use terminal::Terminal;
use uicomponents::{CommandBar, MessageBar, StatusBar, UIComponent, View};

const QUIT_TIMES: u8 = 3;

#[derive(Clone, Copy, PartialEq)]
enum Operator {
    Delete,
    Change,
    Yank, // TODO
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
    LeftAfterDeletion,
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
    Command,
    #[default]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeType {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
    CommandNormal, // Novo modo
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
            ModeType::CommandNormal => write!(f, "COMMAND NORMAL"),
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

trait Mode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        command_buffer: &mut String,
    ) -> Option<EditorCommand>;
    fn enter(&mut self) -> Vec<EditorCommand>;
    fn exit(&mut self) -> Vec<EditorCommand>;
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
    ExecuteCommand,
    MoveCommandBarCursorLeft,
    MoveCommandBarCursorRight,
    MoveCommandBarCursorStart,
    MoveCommandBarCursorEnd,
    AppendRightCommandBar,
    AppendStartCommandBar,
    AppendEndCommandBar,
    MoveCommandBarWordForward(WordType),
    MoveCommandBarWordEnd(WordType),
    MoveCommandBarWordBackward(WordType),
    SplitHorizontal,
    CloseWindow,
    FocusUp,
    FocusDown,
}

pub struct Editor {
    should_quit: bool,
    windows: Vec<Window>,
    active_window: usize,
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
        let size = Terminal::size().unwrap_or_default();
        let initial_window = Window::new(Position { row: 0, col: 0 }, size, View::default());

        let mut editor = Self {
            should_quit: false,
            windows: vec![initial_window],
            active_window: 0,
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
        editor.handle_resize_command(size);
        editor.update_message("we gucci");

        let args: Vec<String> = env::args().collect();

        if let Some(file_name) = args.get(1) {
            debug_assert!(!file_name.is_empty());

            if editor.active_view_mut().load(file_name).is_err() {
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
                self.active_view_mut().handle_normal_command(direction);
            }
            EditorCommand::ExecuteCommand => {
                let command_text = self.command_bar.value();
                self.handle_command_input(&command_text);
            }
            EditorCommand::VisualSelectTextObject(text_object) => {
                self.switch_mode(ModeType::Visual);
                self.active_view_mut().select_text_object(text_object);
            }
            EditorCommand::OperatorTextObject(operator, text_object) => {
                self.active_view_mut()
                    .handle_operator_text_object(operator, text_object);

                if operator == Operator::Change {
                    self.switch_mode(ModeType::Insert);
                }
            }
            EditorCommand::MoveCursorAndSwitchMode(direction, mode) => {
                self.active_view_mut().handle_normal_command(direction);
                self.switch_mode(mode);
            }
            EditorCommand::EditAndSwitchMode(edit, mode) => {
                self.active_view_mut().handle_edit_command(edit);
                self.switch_mode(mode);
            }
            EditorCommand::EditCommand(edit) => {
                self.active_view_mut().handle_edit_command(edit);
            }
            EditorCommand::SwitchToNormalFromInserion => {
                self.active_view_mut().handle_normal_command(Normal::Left);
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
                self.active_view_mut().enter_search();
            }
            EditorCommand::ExitSearch => {
                self.active_view_mut().dismiss_search();
            }
            EditorCommand::SearchNext => {
                self.active_view_mut().search_next();
            }
            EditorCommand::SearchPrevious => {
                self.active_view_mut().search_prev();
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
                    self.active_view_mut().search(&query);
                }
            }
            EditorCommand::PerformSearch(_) => {
                let query = self.command_bar.value();
                self.active_view_mut().search(&query);
            }
            EditorCommand::ClearSelection => {
                self.active_view_mut().clear_selection();
            }
            EditorCommand::DeleteSelection => {
                self.active_view_mut().delete_selection();
                self.execute_command(EditorCommand::ClearSelection);
                self.execute_command(EditorCommand::SwitchMode(ModeType::Normal));
            }
            EditorCommand::UpdateSelection => {
                self.active_view_mut().update_selection();
            }
            EditorCommand::StartSelection(selection_mode) => {
                self.active_view_mut().start_selection(selection_mode);
            }
            EditorCommand::DeleteCharAtCursor => {
                self.active_view_mut().delete_char_at_cursor();
            }
            EditorCommand::DeleteCurrentLine => {
                self.active_view_mut().delete_current_line();
            }
            EditorCommand::ChangeCurrentLine => {
                let line_index = self.active_view_mut().movement.text_location.line_index;
                self.active_view_mut().replace_line_with_empty(line_index);
                self.execute_command(EditorCommand::SwitchMode(ModeType::Insert))
            }
            EditorCommand::DeleteCurrentLineAndLeaveEmpty => {
                self.active_view_mut().delete_current_line_and_leave_empty();
            }
            EditorCommand::DeleteUntilEndOfLine => {
                self.active_view_mut().delete_until_end_of_line();
            }
            EditorCommand::ChangeUntilEndOfLine => {
                self.active_view_mut().delete_until_end_of_line();
                self.execute_command(EditorCommand::SwitchMode(ModeType::Insert))
            }
            EditorCommand::UpdateInsertionPointToCursorPosition => {
                self.active_view_mut()
                    .update_insertion_point_to_cursor_position();
            }
            EditorCommand::SetNeedsRedraw => {
                self.active_view_mut().set_needs_redraw(true);
            }
            EditorCommand::HandleVisualMovement(direction) => {
                self.active_view_mut().handle_visual_movement(direction);
            }
            EditorCommand::MoveCommandBarCursorLeft => {
                self.command_bar.move_cursor_left();
            }
            EditorCommand::MoveCommandBarCursorRight => {
                self.command_bar.move_cursor_right();
            }
            EditorCommand::MoveCommandBarCursorStart => {
                self.command_bar.move_cursor_start();
            }
            EditorCommand::MoveCommandBarCursorEnd => {
                self.command_bar.move_cursor_end();
            }
            EditorCommand::AppendRightCommandBar => {
                self.command_bar.move_cursor_right();
                self.switch_mode(ModeType::Command)
            }
            EditorCommand::AppendStartCommandBar => {
                self.command_bar.move_cursor_start();
                self.switch_mode(ModeType::Command);
            }
            EditorCommand::AppendEndCommandBar => {
                self.command_bar.move_cursor_end();
                self.switch_mode(ModeType::Command);
            }
            EditorCommand::HandleVisualLineMovement(direction) => {
                self.active_view_mut()
                    .handle_visual_line_movement(direction);
            }
            EditorCommand::MoveCommandBarWordForward(word_type) => {
                self.command_bar.move_cursor_word_forward(word_type);
            }
            EditorCommand::MoveCommandBarWordBackward(word_type) => {
                self.command_bar.move_cursor_word_backward(word_type);
            }
            EditorCommand::MoveCommandBarWordEnd(word_type) => {
                self.command_bar.move_cursor_word_end_forward(word_type);
            }
            EditorCommand::SplitHorizontal => {
                self.split_current_window();
            }
            EditorCommand::CloseWindow => {
                self.close_current_window();
            }
            EditorCommand::FocusUp => {
                self.focus_up();
            }
            EditorCommand::FocusDown => {
                self.focus_down();
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

        // determine if there is a split and calculate the position of the separator
        let has_split = self.windows.len() > 1;
        let separator_row = if has_split {
            self.windows[0].origin.row + self.windows[0].size.height
        } else {
            0
        };

        // render all windows
        for window in self.windows.iter_mut() {
            window.view.render(window.origin.row);
        }

        // draw the horizontal separator, if there is a split
        if has_split {
            self.draw_horizontal_separator(separator_row);
        }

        // define new cursor position
        let new_cursor_position = if self.in_prompt() {
            Position {
                row: bottom_bar_row,
                col: self.command_bar.cursor_position_col(),
            }
        } else {
            let active_window = &self.windows[self.active_window];
            Position {
                row: active_window.origin.row + active_window.view.cursor_position().row,
                col: active_window.origin.col + active_window.view.cursor_position().col,
            }
        };

        let _ = Terminal::move_cursor_to(new_cursor_position);
        let _ = Terminal::show_cursor();
        let _ = Terminal::execute();
    }

    fn refresh_status(&mut self) {
        let status = self.active_view_mut().get_status();
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
        // exit the current mode
        let exit_commands = self.current_mode_impl.exit();
        for command in exit_commands {
            self.execute_command(command);
        }

        // update prompt when change mode
        match mode {
            ModeType::Command => {
                self.set_prompt(PromptType::Command);
            }
            ModeType::Prompt(prompt_type) => {
                self.set_prompt(prompt_type);
            }
            _ => {
                self.set_prompt(PromptType::None);
            }
        }

        // set the current mode type
        self.current_mode = mode;

        // clear the mf
        self.command_buffer.clear();

        // create a new mode instance
        self.current_mode_impl = match mode {
            ModeType::Normal => Box::new(NormalMode::new()),
            ModeType::Insert => Box::new(InsertMode::new()),
            ModeType::Visual => Box::new(VisualMode::new()),
            ModeType::VisualLine => Box::new(VisualLineMode::new()),
            ModeType::Command => Box::new(CommandMode::new()),
            ModeType::CommandNormal => Box::new(CommandNormalMode::new()),
            ModeType::Prompt(prompt_type) => Box::new(PromptMode::new(prompt_type)),
        };

        // enter the new mode
        let enter_commands = self.current_mode_impl.enter();
        for command in enter_commands {
            self.execute_command(command);
        }
    }

    fn handle_command_input(&mut self, input: &str) {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();

        if parts.is_empty() {
            self.update_message("Empty command.");
            self.switch_mode(ModeType::Normal);
            return;
        }

        match input.trim() {
            "w" => {
                self.execute_command(EditorCommand::Save);
                self.switch_mode(ModeType::Normal);
                self.update_message("File saved");
            }
            "q" => {
                self.execute_command(EditorCommand::Quit);
            }
            "wq" | "qw" => {
                self.execute_command(EditorCommand::Save);
                self.execute_command(EditorCommand::Quit);
            }
            "split" => {
                self.execute_command(EditorCommand::SplitHorizontal);
                self.switch_mode(ModeType::Normal);
                self.update_message("screen splitted motherfucker");
            }
            "close" => {
                self.execute_command(EditorCommand::CloseWindow);
                self.switch_mode(ModeType::Normal);
            }
            _ => {
                self.update_message(&format!("Unknown command ma man: {}", input));
                self.switch_mode(ModeType::Normal);
            }
        }

        self.command_bar.clear_value();
    }

    fn focus_up(&mut self) {
        if self.windows.len() < 2 {
            self.update_message("only one window open.");
            return;
        }

        // get current active window
        let active_window = &self.windows[self.active_window];
        let active_top = active_window.origin.row;

        // find the window above the current one
        let mut target_window: Option<usize> = None;
        let mut min_distance = usize::MAX;

        for (i, window) in self.windows.iter().enumerate() {
            if window.origin.row + window.size.height <= active_top {
                let distance = active_top - (window.origin.row + window.size.height);
                if distance < min_distance {
                    min_distance = distance;
                    target_window = Some(i);
                }
            }
        }

        if let Some(new_active) = target_window {
            self.active_window = new_active;
            self.update_message(&format!("switched to window {}", self.active_window + 1));
            self.set_needs_redraw(true);
        } else {
            self.update_message("no window above.");
        }
    }

    fn focus_down(&mut self) {
        if self.windows.len() < 2 {
            self.update_message("only one window open.");
            return;
        }

        let active_window = &self.windows[self.active_window];
        let active_bottom = active_window.origin.row + active_window.size.height;

        let mut target_window: Option<usize> = None;
        let mut min_distance = usize::MAX;

        for (i, window) in self.windows.iter().enumerate() {
            if window.origin.row >= active_bottom {
                let distance = window.origin.row - active_bottom;
                if distance < min_distance {
                    min_distance = distance;
                    target_window = Some(i);
                }
            }
        }

        if let Some(new_active) = target_window {
            self.active_window = new_active;
            self.update_message(&format!("switched to window {}", self.active_window + 1));
            self.set_needs_redraw(true);
        } else {
            self.update_message("no window below.");
        }
    }

    //
    // Resize command handling
    //

    fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;
        let num_windows = self.windows.len();
        if num_windows == 0 {
            return;
        }

        let num_separator_lines = if num_windows > 1 { num_windows - 1 } else { 0 };
        let bottom_bars_height = 2; // status bar and message bar
        let available_height = size.height.saturating_sub(bottom_bars_height + num_separator_lines);

        let window_height = available_height / num_windows;
        let extra_lines = available_height % num_windows;

        let mut current_row = 0;
        for (i, window) in self.windows.iter_mut().enumerate() {
            let additional_line = if i < extra_lines { 1 } else { 0 };
            let height = window_height + additional_line;

            window.resize(
                Position { row: current_row, col: 0 },
                Size { height, width: size.width },
            );

            current_row += height;
            if i < num_windows - 1 {
                current_row += 1; // increment a line because of the separator
            }
        }

        // resize inferior bars
        let bar_size = Size { height: 1, width: size.width };
        self.message_bar.resize(bar_size);
        self.status_bar.resize(bar_size);
        self.command_bar.resize(bar_size);

        self.set_needs_redraw(true);
    }

    fn set_needs_redraw(&mut self, needs_redraw: bool) {
        if needs_redraw {
            self.refresh_screen();
        }
    }

    //
    // Quit command handling
    //

    fn handle_quit_command(&mut self) {
        if !self.active_view_mut().get_status().is_modified || self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
        } else if self.active_view_mut().get_status().is_modified {
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
        if self.active_view_mut().is_file_loaded() {
            self.save(None);
        } else {
            self.set_prompt(PromptType::Save);
            self.switch_mode(ModeType::Prompt(PromptType::Save));
        }
    }

    fn save(&mut self, file_name: Option<&str>) {
        let result = if let Some(name) = file_name {
            self.active_view_mut().save_as(name)
        } else {
            self.active_view_mut().save()
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
        matches!(
            self.current_mode,
            ModeType::Prompt(_) | ModeType::Command | ModeType::CommandNormal
        )
    }

    fn set_prompt(&mut self, prompt_type: PromptType) {
        if self.prompt_type != prompt_type {
            match prompt_type {
                PromptType::None => self.message_bar.set_needs_redraw(true),
                PromptType::Save => {
                    self.command_bar.set_prompt("Save as: ");
                    self.command_bar.clear_value();
                }
                PromptType::Search => {
                    self.command_bar.set_prompt("Search: ");
                    self.command_bar.clear_value();
                }
                PromptType::Command => {
                    self.command_bar.set_prompt(":");
                    // self.command_bar.clear_value();
                }
            }
        }

        self.prompt_type = prompt_type;
    }

    //
    // Splitting
    //

    fn split_current_window(&mut self) {
        if self.windows.len() >= 2 {
            self.update_message("Already split into two windows.");
            return;
        }

        let new_view = self.active_view().clone();
        let new_window = Window::new(Position { row: 0, col: 0 }, Size { height: 0, width: 0 }, new_view);
        self.windows.push(new_window);
        self.active_window = self.windows.len() - 1; // focus on new window

        // call handle_resize_command to adjust the size of windows
        self.handle_resize_command(self.terminal_size);
    }


    fn draw_horizontal_separator(&self, row: usize) {
        Terminal::move_cursor_to(Position { row, col: 0 }).unwrap_or(());
        Terminal::print(&"─".repeat(self.terminal_size.width)).unwrap_or(());
    }

    fn close_current_window(&mut self) {
        if self.windows.len() > 1 {
            self.windows.remove(self.active_window);
            // adjust index of active window

            if self.active_window >= self.windows.len() {
                self.active_window = self.windows.len() - 1;
            }

            // resize remaining window to fill the entire screen
            self.handle_resize_command(self.terminal_size);
            self.update_message("Window closed.");
            self.set_needs_redraw(true);
        } else {
            self.update_message("Cannot close the only window.");
        }
    }

    //
    // Helpers
    //

    fn update_insertion_point(&mut self) {
        self.active_view_mut()
            .update_insertion_point_to_cursor_position();
    }

    /// immutable reference to view of active window
    fn active_view(&self) -> &View {
        &self.windows[self.active_window].view
    }

    /// mutable reference to view of active window
    fn active_view_mut(&mut self) -> &mut View {
        &mut self.windows[self.active_window].view
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
                code: KeyCode::Char(':'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SwitchMode(ModeType::Command)),
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
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::CloseWindow),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::FocusUp),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(EditorCommand::FocusDown),
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
        event: KeyEvent,
        _command_buffer: &mut String,
    ) -> Option<EditorCommand> {
        match event {
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(EditorCommand::SwitchMode(ModeType::CommandNormal)),
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

struct CommandNormalMode;

impl CommandNormalMode {
    fn new() -> Self {
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
