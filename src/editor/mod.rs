use crate::prelude::*;
use command::{Normal, VimCommand};
use crossterm::event::read;

use core::fmt;
use std::{
    env,
    io::Error,
    panic::{set_hook, take_hook},
};

mod annotatedstring;
mod command;
mod documentstatus;
mod line;
mod terminal;
mod uicomponents;

use annotatedstring::{AnnotatedString, AnnotationType};
use documentstatus::DocumentStatus;
use line::Line;
use terminal::Terminal;
use uicomponents::{CommandBar, MessageBar, StatusBar, UIComponent, View};

use self::command::{
    Command::{self, Edit, Move, System},
    Edit::InsertNewline,
    Normal::{Down, Left, Right, Up},
    System::{Dismiss, Quit, Resize, Save, Search},
};

const QUIT_TIMES: u8 = 3;

#[derive(Eq, PartialEq, Default)]
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
pub enum VimMode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    CommandMode,
}

impl fmt::Display for VimMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VimMode::Insert => write!(f, "INSERT"),
            VimMode::Normal => write!(f, "NORMAL"),
            VimMode::Visual => write!(f, "VISUAL"),
            VimMode::VisualLine => write!(f, "VISUAL LINE"),
            VimMode::CommandMode => write!(f, "COMMAND"),
        }
    }
}

impl Default for VimMode {
    fn default() -> Self {
        VimMode::Normal
    }
}

#[derive(Default)]
pub struct Editor {
    should_quit: bool,
    view: View,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    command_buffer: Option<char>,
    prompt_type: PromptType,
    terminal_size: Size,
    title: String,
    quit_times: u8,
    vim_mode: VimMode,
}

impl Editor {
    //
    // Struct lifecycle
    //

    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();

        set_hook(Box::new(move |panic_info| {
            let _ = Terminal::terminate();
            current_hook(panic_info);
        }));

        Terminal::initialize()?;

        let mut editor = Self::default();
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
                    if let Ok(command) = Command::from_event(event, self.vim_mode) {
                        self.process_command(command);
                    }
                }
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not read event: {err:?}");
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        let _ = err;
                    }
                }
            }
            self.refresh_status();
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
        self.status_bar.update_status(status, self.vim_mode);

        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
            self.title = title;
        }
    }

    //
    // Command handling
    //

    fn process_command(&mut self, command: Command) {
        if let System(Resize(size)) = command {
            self.handle_resize_command(size);
            return;
        }

        match self.prompt_type {
            PromptType::Search => self.process_command_during_search(command),
            PromptType::Save => self.process_command_during_save(command),
            PromptType::None => self.process_command_no_prompt(command),
        }
    }

    fn process_command_no_prompt(&mut self, command: Command) {
        if matches!(command, System(Quit)) {
            self.handle_quit_command();
            return;
        }
        self.reset_quit_times(); // reset quit times for all other commands

        match command {
            System(Quit | Resize(_) | Dismiss) => {} // quit and Resize already handled above, others not applicable
            System(Search) => self.set_prompt(PromptType::Search),
            System(Save) => self.handle_save_command(),
            Edit(edit_command) => {
                if self.vim_mode == VimMode::Insert {
                    // handle edit commands in insert mode
                    self.view.handle_edit_command(edit_command);
                }
            }
            Move(move_command) => {
                match self.vim_mode {
                    VimMode::Normal => {
                        match move_command {
                            Normal::AppendRight => {
                                self.view.handle_normal_command(Normal::Right);
                                self.vim_mode = VimMode::Insert;
                            }
                            Normal::InsertAtLineEnd => {
                                self.view.handle_normal_command(Normal::EndOfLine);
                                self.vim_mode = VimMode::Insert;
                            }
                            Normal::InsertAtLineStart => {
                                self.view.handle_normal_command(Normal::InsertAtLineStart);
                                self.vim_mode = VimMode::Insert;
                            }
                            Normal::Wait => {
                                if let Some('g') = self.command_buffer {
                                    // gg detected
                                    self.view.handle_normal_command(Normal::GoToTop);
                                    self.command_buffer = None;
                                } else {
                                    self.command_buffer = Some('g');
                                }
                            }
                            Normal::VisualLine => {
                                self.view.start_selection(SelectionMode::VisualLine);
                                self.vim_mode = VimMode::VisualLine;
                            }
                            _ => {
                                self.view.handle_normal_command(move_command);
                                self.command_buffer = None;
                            }
                        }
                    }
                    VimMode::Visual => {
                        match move_command {
                            Normal::Wait => {
                                if let Some('g') = self.command_buffer {
                                    // gg detected
                                    self.view.handle_normal_command(Normal::GoToTop);
                                    self.command_buffer = None;
                                } else {
                                    self.command_buffer = Some('g');
                                }
                            }
                            Normal::Up | Normal::Down => {
                                // Move the cursor vertically and expand the selection in Visual mode
                                self.view.handle_visual_movement(move_command);
                            }
                            _ => {
                                self.view.handle_normal_command(move_command);
                                self.command_buffer = None; // Reset buffer if another command is pressed
                            }
                        }
                        // After handling movement in Visual mode, update the selection
                        self.view.update_selection();
                        self.view.set_needs_redraw(true);
                    }
                    VimMode::VisualLine => {
                        match move_command {
                            Normal::Wait => {
                                if let Some('g') = self.command_buffer {
                                    // gg detected
                                    self.view.handle_normal_command(Normal::GoToTop);
                                    self.command_buffer = None;
                                } else {
                                    self.command_buffer = Some('g');
                                }
                            }
                            Normal::Up | Normal::Down => {
                                // Move the cursor vertically and expand the selection in Visual mode
                                self.view.handle_visual_line_movement(move_command);
                            }
                            _ => {
                                self.view.handle_normal_command(move_command);
                                self.command_buffer = None; // Reset buffer if another command is pressed
                            }
                        }
                        // After handling movement in Visual mode, update the selection
                        self.view.update_selection();
                        self.view.set_needs_redraw(true);
                    }
                    _ => {
                        // Other modes (e.g., Insert, CommandMode) do not handle movement commands here
                    }
                }
            }
            Command::Vim(vim_command) => self.handle_vim_command(vim_command),
        }

        if let Command::Vim(vim_command) = command {
            match vim_command {
                VimCommand::DeleteLine => {
                    if let Some('d') = self.command_buffer {
                        self.view.delete_current_line();
                        self.command_buffer = None;
                    } else {
                        self.command_buffer = Some('d');
                    }
                }
                _ => self.command_buffer = None,
            }
        } else {
            self.reset_quit_times();
        }
    }

    fn handle_vim_command(&mut self, vim_command: VimCommand) {
        match vim_command {
            VimCommand::ChangeMode(new_mode) => {
                let old_mode = self.vim_mode;

                match new_mode {
                    VimMode::Insert => {
                        self.update_insertion_point();
                        self.view.clear_selection();
                    }
                    VimMode::Visual => {
                        if old_mode != VimMode::Visual {
                            self.view.start_selection(SelectionMode::Visual);
                        }
                    }
                    VimMode::VisualLine => {
                        if old_mode != VimMode::VisualLine {
                            self.view.start_selection(SelectionMode::VisualLine);
                        }
                    }
                    VimMode::Normal => {
                        if old_mode == VimMode::Insert {
                            self.view.handle_normal_command(Normal::Left);
                        }

                        if old_mode == VimMode::Visual || old_mode == VimMode::VisualLine {
                            self.view.clear_selection();
                        }
                    }
                    VimMode::CommandMode => {}
                }

                self.vim_mode = new_mode;
                self.view.set_needs_redraw(true);
            }
            VimCommand::DeleteSelection => {
                self.view.delete_selection();
                self.view.clear_selection();
                self.vim_mode = VimMode::Normal; // back to normal mode after deleting
                self.view.set_needs_redraw(true);
            }
            VimCommand::SubstituteSelection => {
                self.view.delete_selection();
                self.view.clear_selection();
                self.vim_mode = VimMode::Insert; // back to normal mode after deleting
                self.view.update_insertion_point_to_cursor_position(); // update point of insertion
                self.view.set_needs_redraw(true);
            }
            VimCommand::ChangeSelection => {
                if let Some((start, end)) = self.view.get_selection_range() {
                    let start_line = start.line_index;
                    let end_line = end.line_index;

                    if start_line == end_line {
                        // only one line selected
                        self.view.replace_line_with_empty(start_line);
                    } else {
                        // multiple lines selected
                        // determine the index of initial and final char to line interval
                        let start_idx = self.view.buffer.rope.line_to_char(start_line);
                        let end_idx = self.view.buffer.rope.line_to_char(end_line + 1);

                        // remove all lines all at once
                        self.view.buffer.rope.remove(start_idx..end_idx);

                        // insert only one empty line in place of the first selected line 
                        self.view.buffer.rope.insert(start_idx, "\n");

                        // update cursor postiion
                        self.view.movement.text_location = Location {
                            line_index: start_line,
                            grapheme_index: 0,
                        };

                        self.view.buffer.dirty = true;
                    }

                    // clear selection and enter insert mode
                    self.view.clear_selection();
                    self.vim_mode = VimMode::Insert;
                    self.view.set_needs_redraw(true);
                }
            }
            VimCommand::OpenNewLineBelow => {
                self.view.insert_newline_below();
                self.vim_mode = VimMode::Insert;
                self.view.set_needs_redraw(true);
            }
            VimCommand::OpenNewLineAbove => {
                self.view.insert_newline_above();
                self.vim_mode = VimMode::Insert;
                self.view.set_needs_redraw(true);
            }
            _ => {}
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

    // clippy::arithmetic_side_effects: quit_times is guaranteed to be between 0 and QUIT_TIMES
    #[allow(clippy::arithmetic_side_effects)]
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
        }
    }

    fn process_command_during_save(&mut self, command: Command) {
        match command {
            System(Quit | Resize(_) | Search | Save) | Move(_) => {} // not applicable during save, Resize already handled at this stage
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                self.update_message("Save aborted.");
            }
            Edit(InsertNewline) => {
                let file_name = self.command_bar.value();
                self.save(Some(&file_name));
                self.set_prompt(PromptType::None);
            }
            Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
            Command::Vim(vim_command) => self.handle_vim_command(vim_command),
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
    // Search command & prompt handling
    //

    fn process_command_during_search(&mut self, command: Command) {
        match command {
            System(Dismiss) | Command::Vim(VimCommand::ChangeMode(VimMode::Normal)) => {
                // Handle ESC to exit search and switch back to normal mode
                self.set_prompt(PromptType::None);
                self.view.dismiss_search();
                self.vim_mode = VimMode::Normal; // set mode back to normal after dismissing search
            }
            Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
                let query = self.command_bar.value();
                self.view.search(&query);
            }
            Move(Right | Down) => self.view.search_next(),
            Move(Up | Left) => self.view.search_prev(),
            System(Quit | Resize(_) | Search | Save) | Move(_) => {}
            Command::Vim(vim_command) => self.handle_vim_command(vim_command),
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
        !self.prompt_type.is_none()
    }

    fn set_prompt(&mut self, prompt_type: PromptType) {
        match prompt_type {
            PromptType::None => self.message_bar.set_needs_redraw(true),
            PromptType::Save => {
                self.command_bar.set_prompt("Save as: ");
                self.vim_mode = VimMode::CommandMode; // Enter command mode when saving
            }
            PromptType::Search => {
                self.view.enter_search();
                self.command_bar.set_prompt("Search: ");
                self.vim_mode = VimMode::CommandMode; // Enter command mode when searching
            }
        }

        self.command_bar.clear_value();
        self.prompt_type = prompt_type;
    }

    fn update_insertion_point(&mut self) {
        self.view.update_insertion_point_to_cursor_position();
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::terminate();
    }
}
