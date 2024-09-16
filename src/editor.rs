use crossterm::event::{read, Event, KeyEvent, KeyEventKind};
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
        System::{Quit, Resize, Save},
    },
    message_bar::MessageBar,
    terminal::Size,
};

mod terminal;
mod status_bar;
mod file_info;
mod document_status;
mod command;
mod message_bar;
mod ui_component;
mod view;

pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";
const QUIT_TIMES: u8 = 3;

#[derive(Default)]
pub struct Editor {
    should_quit: bool,
    view: View,
    status_bar: StatusBar,
    message_bar: MessageBar,
    terminal_size: Size,
    title: String,
    quit_times: u8,
}

impl Editor {
    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();

        set_hook(Box::new(move |panic_info| {
            let _ = Terminal::terminate();
            current_hook(panic_info);
        }));

        Terminal::init()?;

        let mut editor = Self::default();
        let size = Terminal::size().unwrap_or_default();
        editor.resize(size);
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

    fn resize(&mut self, size: Size) {
        self.terminal_size = size;

        self.view.resize(Size {
            height: size.height.saturating_sub(2),
            width: size.width,
        });

        self.message_bar.resize(Size {
            height: 1,
            width: size.width,
        });

        self.status_bar.resize(Size {
            height: 1,
            width: size.width,
        });
    }

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

    fn process_command(&mut self, command: Command) {
        match command {
            System(Quit) => self.handle_quit(),
            System(Resize(size)) => self.resize(size),
            _ => self.reset_quit_times(), // Reset quit times for all other commands
        }

        match command {
            System(Quit | Resize(_)) => {} // already handled above
            System(Save) => self.handle_save(),
            Edit(edit_command) => self.view.handle_edit_command(edit_command),
            Move(move_command) => self.view.handle_move_command(move_command),
        }
    }

    fn handle_save(&mut self) {
        if self.view.save().is_ok() {
            self.message_bar.update_message("File saved successfully.");
        } else {
            self.message_bar.update_message("Error writing file!");
        }
    }

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

    fn refresh_screen(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 {
            return;
        }

        let _ = Terminal::hide_cursor();
        self.message_bar.render(self.terminal_size.height.saturating_sub(1));

        if self.terminal_size.height > 1 {
            self.status_bar.render(self.terminal_size.height.saturating_sub(2));
        }

        if self.terminal_size.height > 2 {
            self.view.render(0);
        }

        let _ = Terminal::move_cursor(self.view.cursor_position());
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
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::terminate();
        if self.should_quit {
            let _ = Terminal::print("leaving so soon?\r\n");
        }
    }
}
