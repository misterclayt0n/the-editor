use crossterm::event::{read, Event, KeyEvent, KeyEventKind};
use editor_command::EditorCommand;
use status_bar::StatusBar;
use std::{
    env,
    io::Error,
    panic::{set_hook, take_hook},
};
mod terminal;
mod status_bar;
mod file_info;
mod document_status;
mod editor_command;
mod view;
use terminal::Terminal;
use view::View;

pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";

pub struct Editor {
    should_quit: bool,
    view: View,
    status_bar: StatusBar,
    title: String
}

impl Editor {
    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();

        set_hook(Box::new(move |panic_info| {
            let _ = Terminal::terminate();
            current_hook(panic_info);
        }));

        Terminal::init()?;
        let mut view = View::new(2);
        let args: Vec<String> = env::args().collect();

        if let Some(file_name) = args.get(1) {
            view.load(file_name);
        }

        let mut editor = Self {
            should_quit: false,
            view,
            status_bar: StatusBar::new(1),
            title: String::new()
        };

        let args: Vec<String> = env::args().collect();
        if let Some(file_name) = args.get(1) {
            editor.view.load(file_name);
        }

        editor.refresh_status();
        Ok(editor)
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
            if let Ok(command) = EditorCommand::try_from(event) {
                if matches!(command, EditorCommand::Quit) {
                    self.should_quit = true;
                } else {
                    self.view.handle_command(command);

                    if let EditorCommand::Resize(size) = command {
                        self.status_bar.resize(size);
                    }
                }
            }
        }
    }

    fn refresh_screen(&mut self) {
        let _ = Terminal::hide_cursor();
        self.view.render();
        self.status_bar.render();
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
