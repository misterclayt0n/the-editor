use std::env;

// TODO: I want to render all errors in some sort of message bar in the future
use anyhow::{Context, Result};
use editor::EditorState;
use events::EventHandler;
use renderer::{terminal::Terminal, Renderer};
use utils::{info, init_logging};

fn main() -> Result<()> {
    init_logging().unwrap();

    info!("we gucci");

    // NOTE: I'm capturing the args in the most raw way possible.
    // Maybe in the future I'll make a pretty CLI using clap or something.
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };

    let event_handler = EventHandler::new();
    let terminal = Terminal::new();
    let renderer = Renderer::new(terminal);
    let mut editor_state = EditorState::new(event_handler, renderer, file_path);

    editor_state.run().context("Running editor")?;

    Ok(())
}
