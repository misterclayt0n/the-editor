use command_bar::CommandBar;
use crossterm::event::{read, Event, KeyEvent, KeyEventKind};
use position::Position;
use size::Size;
use status_bar::StatusBar;
use terminal::Terminal;
use ui_component::UIComponent;
use std::{
    env,
    io::Error,
    panic::{set_hook, take_hook},
};
use view::View;
use self::{
    command::{
        Command::{self, Edit, Move, System},
        System::{Quit, Resize, Save, Dismiss, Search},
        Edit::InsertNewline
    },
    message_bar::MessageBar,
};

mod terminal;
mod status_bar;
mod document_status;
mod command;
mod message_bar;
mod line;
mod command_bar;
mod size;
mod position;
mod ui_component;
mod view;

pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";
const QUIT_TIMES: u8 = 3;

#[derive(PartialEq, Eq, Default)]
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

#[derive(Default)]
pub struct Editor {
    should_quit: bool,
    view: View,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    prompt_type: PromptType,
    terminal_size: Size,
    title: String,
    quit_times: u8,
}

impl Editor {
    //
    // struct lifecycle
    //

    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();

        set_hook(Box::new(move |panic_info| {
            let _ = Terminal::terminate();
            current_hook(panic_info);
        }));

        Terminal::init()?;

        let mut editor = Self::default();
        let size = Terminal::size().unwrap_or_default();
        editor.handle_resize_command(size);
        editor.message_bar.update_message("we gucci");

        let args: Vec<String> = env::args().collect();
        if let Some(file_name) = args.get(1) {
            if editor.view.load(file_name).is_err() {
                editor.message_bar.update_message("ERROR: Could not open file: {file_name}");
            }
        }

        editor.refresh_status();
        Ok(editor)
    }

    //
    // event loop
    //

    pub fn run(&mut self) {
        loop {
            self.refresh_screen();

            if self.should_quit {
                break;
            }

            match read() {
                Ok(event) => self.evaluate_event(event),
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not read event: {err:?}");
                    }
                }
            }

            let status = self.view.get_status();
            self.status_bar.update_status(status);
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
            self.status_bar.render(self.terminal_size.height.saturating_sub(2));
        }

        if self.terminal_size.height > 2 {
            self.view.render(0);
        }

        let new_cursor_pos = if self.in_prompt() {
            Position {
                row: bottom_bar_row,
                col: self.command_bar.cursor_position_col()
            }
        } else {
            self.view.cursor_position()
        };

        let _ = Terminal::move_cursor(new_cursor_pos);
        let _ = Terminal::show_cursor();
        let _ = Terminal::execute();
    }

    pub fn refresh_status(&mut self) {
        let status = self.view.get_status();
        let title = format!("{} - {NAME}", status.file_name);
        self.status_bar.update_status(status);

        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
            self.title = title;
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn evaluate_event(&mut self, event: Event) {
        let should_process = match &event {
            Event::Key(KeyEvent { kind, .. }) => kind == &KeyEventKind::Press,
            Event::Resize(_, _) => true,
            _ => false,
        };

        if should_process {
            if let Ok(command) = Command::try_from(event) {
                self.process_command(command);
            }
        }
    }

    //
    // command handling
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
            self.handle_quit();
            return;
        }
        self.reset_quit_times();

        match command {
            System(Quit | Resize(_) | Dismiss) => {} // quit and resize already handled above
            System(Search) => self.set_prompt(PromptType::Search),
            System(Save) => self.handle_save_command(),
            Edit(edit_command) => self.view.handle_edit_command(edit_command),
            Move(move_command) => self.view.handle_move_command(move_command),
        }
    }

    //
    // resize command handling
    //

    fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;
        self.view.resize(Size {
            height: size.height.saturating_sub(2),
            width: size.width
        });

        let bar_size = Size {
            height: 1,
            width: size.width
        };

        self.message_bar.resize(bar_size);
        self.status_bar.resize(bar_size);
        self.command_bar.resize(bar_size);
    }

    //
    // quit command handling
    //

    #[allow(clippy::arithmetic_side_effects)]
    fn handle_quit(&mut self) {
        if !self.view.get_status().is_modified || self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
        } else if self.view.get_status().is_modified {
            self.message_bar.update_message(&format!(
                "WARNING! File has unsaved changes. Press Ctrl-Q {} more times to quit.",
                QUIT_TIMES - self.quit_times - 1
            ));

            self.quit_times += 1;
        }
    }

    fn reset_quit_times(&mut self) {
        if self.quit_times > 0 {
            self.quit_times = 0;
            self.message_bar.update_message("");
        }
    }

    //
    // save command + prompt handling
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
            System(Quit | Resize(_) | Search | Save) | Move(_) => {} // not applicable during save, resize already handled at this stage
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                self.update_message("Save aborted");
            }
            Edit(InsertNewline) => {
                let file_name = self.command_bar.value();
                self.save(Some(&file_name));
                self.set_prompt(PromptType::None);
            }
            Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
        }
    }

    fn save(&mut self, file_name: Option<&str>) {
        let result = if let Some(name) = file_name {
            self.view.save_as(name)
        } else {
            self.view.save()
        };

        if result.is_ok() {
            self.message_bar.update_message("File saved successfully");
        } else {
            self.message_bar.update_message("Error writing file");
        }
    }

    //
    // search command + prompt handling
    //

    fn process_command_during_search(&mut self, command: Command) {
        match command {
            System(Quit | Resize(_) | Search | Save) | Move(_) => {} // not applicable during search, resize already handled at this stage
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                self.view.dismiss_search();
            }
            Edit(InsertNewline) => {
                self.set_prompt(PromptType::None);
                self.view.exit_search();
            }
            Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
                let query = self.command_bar.value();
                self.view.search(&query);
            },
        }
    }

    //
    // message + command bar
    //

    fn update_message(&mut self, new_message: &str) {
        self.message_bar.update_message(new_message);
    }

    //
    // prompt handling
    //

    fn in_prompt(&self) -> bool {
        !self.prompt_type.is_none()
    }

    fn set_prompt(&mut self, prompt_type: PromptType) {
        match prompt_type {
            PromptType::None => self.message_bar.set_needs_redraw(true), // ensures the message bar is properly painted during next redraw cycle
            PromptType::Save => self.command_bar.set_prompt("Save as: "),
            PromptType::Search => {
                self.view.enter_search();
                self.command_bar.set_prompt("Search: ");
            }
        }

        self.command_bar.clear_value();
        self.prompt_type = prompt_type;
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::terminate();
        if self.should_quit {
            let _ = Terminal::print("leaving so soon?\r\n");
        }
    }
}
