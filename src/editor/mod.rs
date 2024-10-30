use crate::prelude::*;
use buffer_manager::BufferManager;
use crossterm::event::{read, Event, KeyEvent};
use dirs::home_dir;
use input_mode::InputMode;
use insert_mode::InsertMode;
use normal_mode::NormalMode;
use visual_line_mode::VisualLineMode;
use visual_mode::VisualMode;
use window::{SplitDirection, Window};

use std::{
    cell::RefCell, env, io::Error, panic::{set_hook, take_hook}, path::PathBuf, rc::Rc
};

mod color_scheme;
mod documentstatus;
mod input_mode;
mod insert_mode;
mod normal_mode;
mod terminal;
mod uicomponents;
mod visual_line_mode;
mod visual_mode;
mod window;
mod buffer_manager;

use documentstatus::DocumentStatus;
use terminal::Terminal;
use uicomponents::{CommandBar, MessageBar, StatusBar, UIComponent, View};

trait Mode {
    fn handle_event(
        &mut self,
        event: KeyEvent,
        command_buffer: &mut String,
        command_bar: Rc<RefCell<CommandBar>>,
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
    SetPrompt(InputType),
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
    Split(SplitDirection),
    CloseWindow,
    Focus(FocusDirection),
    Replace(String, String)
}

pub struct Editor {
    should_quit: bool,
    windows: Vec<Window>,
    active_window: usize,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: Rc<RefCell<CommandBar>>,
    terminal_size: Size,
    title: String,
    quit_times: u8,
    current_mode: ModeType,
    current_mode_impl: Box<dyn Mode>,
    command_buffer: String,
    buffer_manager: BufferManager,
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
        let command_bar = Rc::new(RefCell::new(CommandBar::default()));

        let mut editor = Self {
            should_quit: false,
            windows: vec![initial_window],
            active_window: 0,
            status_bar: StatusBar::default(),
            message_bar: MessageBar::default(),
            command_bar: command_bar.clone(),
            terminal_size: Size::default(),
            title: String::new(),
            quit_times: 0,
            current_mode: ModeType::Normal,
            current_mode_impl: Box::new(NormalMode::new()),
            command_buffer: String::new(),
            buffer_manager: BufferManager::new(),
        };
        editor.handle_resize_command(size);

        let args: Vec<String> = env::args().collect();

        if let Some(file_name) = args.get(1) {
            debug_assert!(!file_name.is_empty());

            let expanded_path = shellexpand::tilde(file_name).into_owned();
            match editor.buffer_manager.get_buffer(&expanded_path) {
                Ok(buffer_rc) => {
                    editor.active_view_mut().buffer = buffer_rc.clone();
                    if editor.active_view_mut().load(&expanded_path).is_err() {
                        editor.update_message(&format!("ERROR: Could not open file: {}", expanded_path));
                    } else {
                        editor.update_message(&format!("File opened: {}", expanded_path));
                    }
                }
                Err(err) => {
                    editor.update_message(&format!("ERROR: Loading buffer: {}", err));
                }
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
                            .handle_event(key_event, &mut self.command_buffer, self.command_bar.clone())
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
                if let ModeType::Input(ref input_type, _) = self.current_mode {
                    match input_type {
                        InputType::Search => {
                            let query = self.command_bar.borrow().value();
                            self.active_view_mut().search(&query);
                            self.switch_mode(ModeType::Normal);
                        },
                        InputType::FindFile => {
                            let file_path = self.command_bar.borrow().value();
                            self.open_file_in_current_window(&file_path);
                            self.switch_mode(ModeType::Normal);
                        },
                        InputType::Save => {
                            let file_name = self.command_bar.borrow().value();
                            self.save(Some(&file_name));
                            self.switch_mode(ModeType::Normal);
                        },
                        InputType::Command => {
                            let command_text = self.command_bar.borrow().value();
                            self.handle_command_input(&command_text);
                        },
                        InputType::Replace => {
                            let query = self.command_bar.borrow().value();
                            self.active_view_mut().search(&query);
                            self.switch_mode(ModeType::Input(InputType::ReplaceFor(query), InputModeType::Insert));
                        }
                        InputType::ReplaceFor(target) => {
                            let replacement = self.command_bar.borrow().value();
                            self.execute_command(EditorCommand::Replace(target.to_owned(), replacement));
                            self.switch_mode(ModeType::Normal);
                        }
                    }
                } else {
                    let command_text = self.command_bar.borrow().value();
                    self.handle_command_input(&command_text);
                }
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
            EditorCommand::SetPrompt(input_type) => {
                self.set_prompt(Some(input_type.clone()));
                self.switch_mode(ModeType::Input(input_type, InputModeType::Insert));
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
                self.command_bar.borrow_mut().handle_edit_command(edit);
                if let ModeType::Input(InputType::Search | InputType::Replace, _) = self.current_mode {
                    let query = self.command_bar.borrow().value();
                    self.active_view_mut().search(&query);
                }
            }
            EditorCommand::PerformSearch(_) => {
                let query = self.command_bar.borrow().value();
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
            EditorCommand::HandleVisualLineMovement(direction) => {
                self.active_view_mut()
                    .handle_visual_line_movement(direction);
                self.mark_all_views_needing_redraw();
            }
            EditorCommand::Split(direction) => {
                self.split_current_window(direction);
            }
            EditorCommand::CloseWindow => {
                self.close_current_window();
            }
            EditorCommand::Focus(direction) => self.focus(direction),
            EditorCommand::Replace(target, replacement) => {
                self.active_view_mut().replace(&target, &replacement);
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
            self.command_bar.borrow_mut().render(Position {
                col: 0,
                row: bottom_bar_row,
            });
        } else {
            self.message_bar.render(Position {
                col: 0,
                row: bottom_bar_row,
            });
        }

        if self.terminal_size.height > 1 {
            self.status_bar.render(Position {
                row: self.terminal_size.height.saturating_sub(2),
                col: 0,
            });
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
                col: self.command_bar.borrow().cursor_position_col(),
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
        self.status_bar.update_status(status, self.current_mode.clone());

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

        if let ModeType::Input(ref input_type, _) = mode {
            self.set_prompt(Some(input_type.clone()));
        } else {
            self.set_prompt(None);
        }

        // set the current mode type
        self.current_mode = mode.clone();

        // clear the mf
        self.command_buffer.clear();

        // create a new mode instance
        self.current_mode_impl = match mode {
            ModeType::Normal => Box::new(NormalMode::new()),
            ModeType::Insert => Box::new(InsertMode::new()),
            ModeType::Visual => Box::new(VisualMode::new()),
            ModeType::VisualLine => Box::new(VisualLineMode::new()),
            ModeType::Input(input_type, input_mode) => {
                Box::new(InputMode::new(input_type.clone(), input_mode))
            }
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
                self.command_bar.borrow_mut().clear_value();
            }
            "q" | "Q" => {
                self.execute_command(EditorCommand::Quit);
                self.command_bar.borrow_mut().clear_value();
            }
            "wq" | "qw" | "Wq" | "Qw" | "WQ" | "QW" => {
                self.execute_command(EditorCommand::Save);
                self.execute_command(EditorCommand::Quit);
                self.command_bar.borrow_mut().clear_value();
            }
            "split" => {
                self.execute_command(EditorCommand::Split(SplitDirection::Horizontal));
                self.switch_mode(ModeType::Normal);
                self.command_bar.borrow_mut().clear_value();
            }
            "vsplit" => {
                self.execute_command(EditorCommand::Split(SplitDirection::Vertical));
                self.switch_mode(ModeType::Normal);
                self.command_bar.borrow_mut().clear_value();
            }
            "find" => {
                self.switch_mode(ModeType::Input(InputType::FindFile, InputModeType::Insert));
            }
            "close" => {
                self.execute_command(EditorCommand::CloseWindow);
                self.switch_mode(ModeType::Normal);
                self.command_bar.borrow_mut().clear_value();
            }
            "replace" => {
                self.switch_mode(ModeType::Input(InputType::Replace, InputModeType::Insert));
            }
            "search" => {
                self.switch_mode(ModeType::Input(InputType::Search, InputModeType::Insert));
            }
            _ => {
                self.update_message(&format!("Unknown command ma man: {}", input));
                self.switch_mode(ModeType::Normal);
                self.command_bar.borrow_mut().clear_value();
            }
        }
    }

    fn focus(&mut self, direction: FocusDirection) {
        if self.windows.len() < 2 {
            self.update_message("Only one window open");
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
                        let distance =
                            active_window.origin.row - (window.origin.row + window.size.height);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                }
                FocusDirection::Down => {
                    if window.origin.row >= active_window.origin.row + active_window.size.height {
                        let distance = window.origin.row
                            - (active_window.origin.row + active_window.size.height);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                }
                FocusDirection::Left => {
                    if window.origin.col + window.size.width <= active_window.origin.col {
                        let distance =
                            active_window.origin.col - (window.origin.col + window.size.width);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                }
                FocusDirection::Right => {
                    if window.origin.col >= active_window.origin.col + active_window.size.width {
                        let distance = window.origin.col
                            - (active_window.origin.col + active_window.size.width);
                        if distance < min_distance {
                            min_distance = distance;
                            target_window = Some(i);
                        }
                    }
                }
            }
        }

        if let Some(new_active) = target_window {
            self.active_window = new_active;
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
        let bar_size = Size {
            height: 1,
            width: size.width,
        };
        self.message_bar.resize(bar_size);
        self.status_bar.resize(bar_size);
        self.command_bar.borrow_mut().resize(bar_size);

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
            self.set_prompt(Some(InputType::Save));
            self.switch_mode(ModeType::Input(InputType::Save, InputModeType::Normal));
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
        matches!(self.current_mode, ModeType::Input(_, _))
    }

    fn set_prompt(&mut self, input_type: Option<InputType>) {
        if let Some(input_type) = input_type {
            match input_type {
                InputType::Save => {
                    self.command_bar.borrow_mut().set_prompt("Save as: ");
                    self.command_bar.borrow_mut().clear_value();
                }
                InputType::Search => {
                    self.command_bar.borrow_mut().set_prompt("Search: ");
                    self.command_bar.borrow_mut().clear_value();
                }
                InputType::Command => {
                    self.command_bar.borrow_mut().set_prompt(":");
                    self.command_bar.borrow_mut().clear_value();
                }
                InputType::Replace => {
                    self.command_bar.borrow_mut().set_prompt("Find to replace: ");
                    self.command_bar.borrow_mut().clear_value();
                }
                InputType::ReplaceFor(_) => {
                    self.command_bar.borrow_mut().set_prompt("Replace for: ");
                    self.command_bar.borrow_mut().clear_value();
                }
                InputType::FindFile => {
                    self.command_bar.borrow_mut().set_prompt("Find File: ");

                    // let initial_value = self.get_current_file_directory();

                    let initial_value =
                        self.get_current_file_directory().unwrap_or_else(
                            || match env::current_dir() {
                                Ok(current_dir) => path_relative_to_home(&current_dir),
                                Err(_) => String::from("~/"),
                            },
                        );
                    self.command_bar.borrow_mut().set_value(initial_value);
                }
            }
        } else {
            self.message_bar.set_needs_redraw(true);
        }
    }

    //
    // Splitting
    //

    fn split_current_window(&mut self, direction: SplitDirection) {
        if self.windows.len() >= 2 {
            // TODO: allow for infinite splits
            return;
        }

        let active_window = &mut self.windows[self.active_window];
        let new_view = active_window.view.clone_shared(); // clone_independent to have a separate buffer
        let mut new_window = Window::new(
            Position { row: 0, col: 0 },
            Size {
                height: 0,
                width: 0,
            },
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

    fn open_file_in_current_window(&mut self, file_path: &str) {
        let expanded_path = shellexpand::tilde(file_path).into_owned();
        match self.buffer_manager.get_buffer(&expanded_path) {
            Ok(buffer_rc) => {
                self.active_view_mut().buffer = buffer_rc.clone();
                if self.active_view_mut().load(&expanded_path).is_err() {
                    self.update_message(&format!("ERROR: Could not open file: {}", expanded_path));
                } else {
                    self.update_message(&format!("File open: {}", expanded_path));
                }
            }
            Err(err) => {
                self.update_message(&format!("ERROR: Loading buffer: {}", err));
            }
        }
        self.command_bar.borrow_mut().clear_value();
        self.mark_all_views_needing_redraw();
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

    fn get_current_file_directory(&self) -> Option<String> {
        if let Some(current_file_path) = self.active_view().buffer.borrow().file_info.get_path() {
            if let Some(parent_dir) = current_file_path.parent() {
                let dir = path_relative_to_home(&PathBuf::from(parent_dir));
                Some(dir)
            } else {
                // if the file has no "father" directory, return Home
                let home = path_relative_to_home(&home_dir().unwrap_or(PathBuf::from("~/")));
                Some(home)
            }
        } else {
            None
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::kill();
    }
}

fn path_relative_to_home(path: &PathBuf) -> String {
    if let Some(home_dir) = home_dir() {
        if path.starts_with(&home_dir) {
            let relative_path = path.strip_prefix(&home_dir).unwrap_or(path);
            let relative_str = relative_path.to_str().unwrap_or("");
            if relative_str.is_empty() {
                String::from("~")
            } else {
                format!("~/{}", relative_str)
            }
        } else {
            path.to_str().unwrap_or("").to_string()
        }
    } else {
        path.to_str().unwrap_or("").to_string()
    }
}
