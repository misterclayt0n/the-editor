use the_editor_renderer::{
  Application,
  InputEvent,
  Key,
  Renderer,
};

use crate::{
  core::graphics::Rect,
  editor::Editor,
  input::InputHandler,
  keymap::{
    KeyBinding,
    Keymaps,
  },
  ui::{
    components::button::Button,
    compositor::{
      Component,
      Compositor,
      Context,
      Event,
    },
    editor_view::EditorView,
    job::Jobs,
  },
};

pub struct App {
  pub compositor:    Compositor,
  pub editor:        Editor,
  pub jobs:          Jobs,
  pub input_handler: InputHandler,
}

impl App {
  pub fn new(editor: Editor) -> Self {
    let area = Rect::new(0, 0, 120, 40); // Default size, will be updated on resize.
    let mut compositor = Compositor::new(area);

    let mode = editor.mode;

    let keymaps = Keymaps::default();
    let editor_view = Box::new(EditorView::new(keymaps));
    compositor.push(editor_view);

    // NOTE: This is a test button btw.
    let button = Box::new(
      Button::new("Run")
                .with_rect(Rect::new(100, 1, 8, 2)) // Top-right.
                .color(the_editor_renderer::Color::rgb(0.6, 0.6, 0.8))
                .visible(false)
                .on_click(|| println!("Button clicked!")),
    );
    compositor.push(button);

    Self {
      compositor,
      editor,
      jobs: Jobs::new(),
      input_handler: InputHandler::new(mode),
    }
  }
}

impl Application for App {
  fn init(&mut self, renderer: &mut Renderer) {
    println!("Application initialized!");

    renderer.set_ligature_protection(false);

    // NOTE: We currently allow users to specify a font file path via env var
    if let Ok(path) = std::env::var("THE_EDITOR_FONT_FILE") {
      if let Err(err) = renderer.configure_font_from_path(&path, 22.0) {
        // TODO: Get from editor config.
        log::warn!("failed to load font from THE_EDITOR_FONT_FILE={path}: {err}");
      }
    }

    // Ensure the active view has an initial cursor/selection.
    use crate::core::selection::Selection;
    let (view, doc) = crate::current!(self.editor);
    doc.set_selection(view.id, Selection::point(0));
  }

  fn render(&mut self, renderer: &mut Renderer) {
    the_editor_event::start_frame();

    // The renderer's begin_frame/end_frame are handled by the main loop.
    // We just need to draw our content here.

    // Create context for rendering.
    let mut cx = Context {
      editor: &mut self.editor,
      scroll: None,
      jobs:   &mut self.jobs,
    };

    // Render through the compositor.
    let area = self.compositor.size();
    self.compositor.render(area, renderer, &mut cx);
  }

  fn handle_event(&mut self, event: InputEvent, _renderer: &mut Renderer) -> bool {
    // Check if EditorView has a pending on_next_key callback.
    // This happens for commands like 'r' that wait for the next character.
    let pending_char = self.compositor.layers.iter().any(|layer| {
      // Try to downcast to EditorView to check pending state.
      layer
        .as_any()
        .downcast_ref::<crate::ui::editor_view::EditorView>()
        .map_or(false, |view| view.has_pending_on_next_key())
    });

    if pending_char {
      self.input_handler.set_pending_char();
    }

    // Process the event through our unified input handler.
    let result = self.input_handler.handle_input(event.clone());

    // Update mode in input handler if changed.
    self.input_handler.set_mode(self.editor.mode);

    // Handle cancelled pending operations.
    if result.cancelled {
      // Clear any pending state in the compositor.
      return true;
    }

    // Handle pending character callbacks (e.g., from 'r' command).
    if let Some(ch) = result.pending_char {
      // Convert to KeyBinding for compatibility.
      let binding = KeyBinding::new(if ch == '\n' {
        Key::Enter
      } else {
        Key::Char(ch)
      });
      let event = Event::Key(binding);

      let mut cx = Context {
        editor: &mut self.editor,
        scroll: None,
        jobs:   &mut self.jobs,
      };

      return self.compositor.handle_event(&event, &mut cx);
    }

    // Handle insert mode character insertion.
    if let Some(ch) = result.insert_char {
      // In insert mode, send character as a key event.
      let binding = KeyBinding::new(Key::Char(ch));
      let event = Event::Key(binding);

      let mut cx = Context {
        editor: &mut self.editor,
        scroll: None,
        jobs:   &mut self.jobs,
      };

      return self.compositor.handle_event(&event, &mut cx);
    }

    // Handle command mode keys.
    if let Some(binding) = result.command_key {
      let event = Event::Key(binding);

      let mut cx = Context {
        editor: &mut self.editor,
        scroll: None,
        jobs:   &mut self.jobs,
      };

      return self.compositor.handle_event(&event, &mut cx);
    }

    // Handle scroll events.
    if let Some(_scroll) = result.scroll {
      // TODO: Implement scroll handling.
      return false;
    }

    // Handle mouse events.
    if let Some(mouse) = result.mouse {
      let event = Event::Mouse(mouse);

      let mut cx = Context {
        editor: &mut self.editor,
        scroll: None,
        jobs:   &mut self.jobs,
      };

      return self.compositor.handle_event(&event, &mut cx);
    }

    // Handle keymap lookups (normal key events).
    if let Some(keys) = result.keys {
      // For now, handle the last key in the sequence.
      if let Some(binding) = keys.last() {
        let event = Event::Key(*binding);

        let mut cx = Context {
          editor: &mut self.editor,
          scroll: None,
          jobs:   &mut self.jobs,
        };

        return self.compositor.handle_event(&event, &mut cx);
      }
    }

    // Also handle raw events for compatibility.
    // This ensures Text events are processed.
    match event {
      InputEvent::Text(text) => {
        // Text events should generate characters.
        for ch in text.chars() {
          let binding = KeyBinding::new(Key::Char(ch));
          let event = Event::Key(binding);

          let mut cx = Context {
            editor: &mut self.editor,
            scroll: None,
            jobs:   &mut self.jobs,
          };

          if self.compositor.handle_event(&event, &mut cx) {
            return true;
          }
        }
        false
      },
      _ => result.consumed,
    }
  }

  fn resize(&mut self, width: u32, height: u32, _renderer: &mut Renderer) {
    // Update compositor area.
    let area = Rect::new(0, 0, width as u16, height as u16);
    self.compositor.resize(area);
  }

  fn wants_redraw(&self) -> bool {
    // Check if any component needs updates (e.g., for animations).
    use crate::ui::components::button::Button;

    // First check editor needs_redraw.
    if self.editor.needs_redraw {
      return true;
    }

    // Then check if any component needs updates.
    for layer in self.compositor.layers.iter() {
      // Check if it's a button with active animation.
      if let Some(button) = layer.as_any().downcast_ref::<Button>() {
        if button.should_update() {
          return true;
        }
      }

      // Other components can also request redraws via should_update.
      if layer.should_update() {
        return true;
      }
    }

    false
  }
}
