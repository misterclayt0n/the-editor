use std::env;

// TODO: I want to render all errors in some sort of message bar in the future
use anyhow::{Context, Result};
use editor::EditorState;
use events::EventHandler;
use renderer::{terminal::Terminal, Renderer};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };

    let event_handler = EventHandler::new();
    let terminal = Terminal::new();
    let renderer = Renderer::new(terminal);
    let mut editor_state = EditorState::new(event_handler, renderer, file_path).context("Could not initialize editor state")?;

    editor_state.run().context("Running editor")?;

    Ok(())
}
