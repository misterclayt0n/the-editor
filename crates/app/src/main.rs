// TODO: I want to render all errors in some sort of message bar in the future
use anyhow::{Context, Result};
use editor::EditorState;
use events::EventHandler;
use renderer::{terminal::Terminal, Renderer};

fn main() -> Result<()> {
    let event_handler = EventHandler::new();
    let terminal = Terminal::new();
    let renderer = Renderer::new(terminal);
    let mut editor_state = EditorState::new(event_handler, renderer).context("Could not initialize editor state")?;

    editor_state.run().context("Running editor")?;

    Ok(())
}
