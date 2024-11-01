// TODO: Renderer won't appear here, I want something like `editor_state.run()`, and
// the editor state will contain the rendering logic by window
use std::io::stdout;

use anyhow::{Context, Result};
use crossterm::{execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use editor::EditorState;
use events::{Event, EventHandler};
use renderer::{terminal::Terminal, TerminalCommand, Renderer};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen).context("Entering alternating screen")?;

    let mut editor_state = EditorState::new();
    let event_handler = EventHandler::new();
    let terminal = Terminal{};
    let mut renderer = Renderer::new(terminal);
    renderer.welcome_screen();

    // Main loop (game loop as some would say kkk)
    loop {
        let events = event_handler.poll_events().context("Capturing events")?;

        renderer.render().context("Initial rendering")?;

        for event in events {
            if let Event::KeyPress(key_event) = event {
                match event_handler.handle_key_event(key_event) {
                    Ok(commands) => {
                        for command in commands {
                            if let Err(e) = editor_state.apply_command(command) {
                                renderer.enqueue_command(TerminalCommand::Print(format!("ERROR: {}", e)))
                            }
                        }
                    },
                    Err(e) => {
                        renderer.enqueue_command(TerminalCommand::Print(format!("ERROR: {}", e)))
                    }
                }
            }
        }

        // Get out by pressing 'q'
        if editor_state.should_quit {
            break;
        }
    }

    disable_raw_mode().context("Disabling raw mode")?;
    execute!(stdout, LeaveAlternateScreen).context("Leaving alternate screen")?;

    Ok(())
}
