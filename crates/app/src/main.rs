// TODO: Remove raylib, it does not make any sense.
// TODO: Iterate through grapheme clusters instead of per bytes.
// TODO: Support for rendering more than ASCII in GUI version.

use std::env;

// TODO: I want to render all errors in some sort of message bar in the future
use anyhow::{Context, Result};
use editor::{events::EventHandler, EditorState};
use renderer::Renderer;
use utils::{info, init_logging, InterfaceType};

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

    let interface = InterfaceType::GUI; // TODO: Turn this into cli args.
    let event_handler = EventHandler;
    let renderer = Renderer::new(interface, "fonts/GeistMono-VariableFont_wght.ttf");
    let mut editor_state = EditorState::new(event_handler, renderer, file_path);

    editor_state.run().context("Running editor")?;

    Ok(())
}
