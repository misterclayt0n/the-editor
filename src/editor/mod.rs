use crate::prelude::*;
use command_mode::CommandMode;
use command_normal_mode::CommandNormalMode;
use crossterm::event::{read, Event, KeyEvent};
use insert_mode::InsertMode;
use normal_mode::NormalMode;
use prompt_mode::PromptMode;
use visual_line_mode::VisualLineMode;
use visual_mode::VisualMode;
use window::{SplitDirection, Window};

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
mod normal_mode;
mod insert_mode;
mod visual_mode;
mod visual_line_mode;
mod command_mode;
mod prompt_mode;
mod command_normal_mode;

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
    CommandNormal,
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
    Split(SplitDirection),
    CloseWindow,
    Focus(FocusDirection),
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

                self.mark_all_views_needing_redraw();
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
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::EditCommand(edit) => {
                self.active_view_mut().handle_edit_command(edit);
                self.mark_all_views_needing_redraw();
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
                self.mark_all_views_needing_redraw();
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
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::UpdateSelection => {
                self.active_view_mut().update_selection();
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::StartSelection(selection_mode) => {
                self.active_view_mut().start_selection(selection_mode);
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::DeleteCharAtCursor => {
                self.active_view_mut().delete_char_at_cursor();
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::DeleteCurrentLine => {
                self.active_view_mut().delete_current_line();
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::ChangeCurrentLine => {
                let line_index = self.active_view_mut().movement.text_location.line_index;
                self.active_view_mut().replace_line_with_empty(line_index);
                self.execute_command(EditorCommand::SwitchMode(ModeType::Insert));
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::DeleteCurrentLineAndLeaveEmpty => {
                self.active_view_mut().delete_current_line_and_leave_empty();
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::DeleteUntilEndOfLine => {
                self.active_view_mut().delete_until_end_of_line();
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::ChangeUntilEndOfLine => {
                self.active_view_mut().delete_until_end_of_line();
                self.execute_command(EditorCommand::SwitchMode(ModeType::Insert));
                self.mark_all_views_needing_redraw();
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
                self.mark_all_views_needing_redraw();
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
                self.mark_all_views_needing_redraw();
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
            EditorCommand::Split(direction) => {
                self.split_current_window(direction);
            }
            EditorCommand::CloseWindow => {
                self.close_current_window();
            }
            EditorCommand::Focus(direction) => {
                self.focus(direction)
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
            self.command_bar.render(Position { col: 0, row: bottom_bar_row });
        } else {
            self.message_bar.render(Position { col: 0, row: bottom_bar_row });
        }

        if self.terminal_size.height > 1 {
            self.status_bar.render(Position { row: self.terminal_size.height.saturating_sub(2), col: 0 });
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
            window.view.render(window.origin);
        }

        // draw the horizontal separator, if there is a split
        if has_split {
            // check if it's a horizontal or vertical split
            if self.windows.len() == 2 {
                let window1 = &self.windows[0];
                let window2 = &self.windows[1];
                if window1.origin.col == window2.origin.col {
                    // horizontal split
                    self.draw_horizontal_separator(separator_row);
                } else if window1.origin.row == window2.origin.row {
                    // vertical split
                    let separator_col = window1.size.width;
                    self.draw_vertical_separator(separator_col);
                }
            }
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
            "w" | "W" => {
                self.execute_command(EditorCommand::Save);
                self.switch_mode(ModeType::Normal);
                self.update_message("File saved");
            }
            "q" | "Q"=> {
                self.execute_command(EditorCommand::Quit);
            }
            "wq" | "qw" | "Wq" | "Qw" | "WQ" | "QW" => {
                self.execute_command(EditorCommand::Save);
                self.execute_command(EditorCommand::Quit);
            }
            "split" => {
                self.execute_command(EditorCommand::Split(SplitDirection::Horizontal));
                self.switch_mode(ModeType::Normal);
                self.update_message("Horizontal split motherfucker");
            }
            "vsplit" => {
                self.execute_command(EditorCommand::Split(SplitDirection::Vertical));
                self.switch_mode(ModeType::Normal);
                self.update_message("Vertical split motherfucker");
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

    fn focus(&mut self, direction: FocusDirection) {
        if self.windows.len() < 2 {
            self.update_message("Apenas uma janela está aberta.");
            return;
        }

        let active_window = &self.windows[self.active_window];
        let mut target_window: Option<usize> = None;
        let mut min_distance = usize::MAX;

        for (i, window) in self.windows.iter().enumerate() {
            if i == self.active_window {
                continue;
            }

            match direction {
                FocusDirection::Up => {
                    if window.origin.row + window.size.height <= active_window.origin.row {
                        let distance = active_window.origin.row - (window.origin.row + window.size.height);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                },
                FocusDirection::Down => {
                    if window.origin.row >= active_window.origin.row + active_window.size.height {
                        let distance = window.origin.row - (active_window.origin.row + active_window.size.height);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                },
                FocusDirection::Left => {
                    if window.origin.col + window.size.width <= active_window.origin.col {
                        let distance = active_window.origin.col - (window.origin.col + window.size.width);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                },
                FocusDirection::Right => {
                    if window.origin.col >= active_window.origin.col + active_window.size.width {
                        let distance = window.origin.col - (active_window.origin.col + active_window.size.width);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                },
            }
        }

        if let Some(new_active) = target_window {
            self.active_window = new_active;
            self.update_message(&format!("focus on window {}", self.active_window + 1));
            self.set_needs_redraw(true);
        }
    }

    //
    // Resize command handling
    //

    fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;
        let bottom_bars_height = 2; // status bar e message bar
        let num_windows = self.windows.len();
        if num_windows == 0 {
            return;
        }

        if num_windows == 1 {
            // single window that fits the entire space
            let window = &mut self.windows[0];
            window.resize(
                Position { row: 0, col: 0 },
                Size {
                    height: size.height - bottom_bars_height,
                    width: size.width,
                },
            );
        } else if num_windows == 2 {
            // horizontal or vertical split
            let (left, right) = self.windows.split_at_mut(1);
            let window1 = &mut left[0];
            let window2 = &mut right[0];

            if window1.origin.row == window2.origin.row {
                // vertical split
                let half_width = size.width / 2;

                window1.resize(
                    Position { row: 0, col: 0 },
                    Size {
                        height: size.height - bottom_bars_height,
                        width: half_width,
                    },
                );
                window2.resize(
                    Position {
                        row: 0,
                        col: half_width + 1, // +1 for separating col
                    },
                    Size {
                        height: size.height - bottom_bars_height,
                        width: size.width - half_width - 1, // -1 for separating col
                    },
                );
            } else {
                // horizontal split
                let half_height = (size.height - bottom_bars_height) / 2;

                window1.resize(
                    Position { row: 0, col: 0 },
                    Size {
                        height: half_height,
                        width: size.width,
                    },
                );
                window2.resize(
                    Position {
                        row: half_height + 1, // +1 for separating row
                        col: 0,
                    },
                    Size {
                        height: size.height - bottom_bars_height - half_height - 1, // -1 for separating row
                        width: size.width,
                    },
                );
            }
        }

        // resize inferior bars
        let bar_size = Size { height: 1, width: size.width };
        self.message_bar.resize(bar_size);
        self.status_bar.resize(bar_size);
        self.command_bar.resize(bar_size);

        self.set_needs_redraw(true);
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

    fn split_current_window(&mut self, direction: SplitDirection) {
        if self.windows.len() >= 2 {
            self.update_message("Already split into two windows.");
            return;
        }

        let active_window = &mut self.windows[self.active_window];
        let new_view = active_window.view.clone();
        let mut new_window = Window::new(
            Position { row: 0, col: 0 },
            Size { height: 0, width: 0 },
            new_view,
        );

        match direction {
            SplitDirection::Horizontal => {
                let total_height = active_window.size.height;
                let half_height = total_height / 2;

                active_window.size.height = half_height;
                new_window.origin.row = active_window.origin.row + half_height + 1;
                new_window.origin.col = active_window.origin.col;
                new_window.size.height = total_height - half_height - 1;
                new_window.size.width = active_window.size.width;
            }
            SplitDirection::Vertical => {
                let total_width = active_window.size.width;
                let half_width = total_width / 2;

                active_window.size.width = half_width;
                new_window.origin.row = active_window.origin.row;
                new_window.origin.col = active_window.origin.col + half_width + 1;
                new_window.size.height = active_window.size.height;
                new_window.size.width = total_width - half_width - 1;
            }
        }

        self.windows.push(new_window);
        self.active_window = self.windows.len() - 1;

        self.handle_resize_command(self.terminal_size);
    }

    fn draw_horizontal_separator(&self, row: usize) {
        Terminal::move_cursor_to(Position { row, col: 0 }).unwrap_or(());
        Terminal::print(&"─".repeat(self.terminal_size.width)).unwrap_or(());
    }

    fn draw_vertical_separator(&self, col: usize) {
        for row in 0..self.terminal_size.height - 2 {
            Terminal::move_cursor_to(Position { row, col }).unwrap_or(());
            Terminal::print("│").unwrap_or(());
        }
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

    fn set_needs_redraw(&mut self, needs_redraw: bool) {
        if needs_redraw {
            self.refresh_screen();
        }
    }

    fn mark_all_views_needing_redraw(&mut self) {
        for window in &mut self.windows {
            window.view.set_needs_redraw(true);
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::kill();
    }
}
