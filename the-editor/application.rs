use the_editor_renderer::{
  Application,
  InputEvent,
  Key,
  Renderer,
  ScrollDelta,
};

use crate::{
  core::{
    commands,
    graphics::Rect,
    movement::Direction,
  },
  editor::Editor,
  input::InputHandler,
  keymap::{
    KeyBinding,
    Keymaps,
  },
  ui::{
    components::{
      button::Button,
      statusline::StatusLine,
    },
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

  // Smooth scrolling configuration and state
  smooth_scroll_enabled: bool,
  scroll_lerp_factor:    f32, // fraction of remaining distance per frame (0..1)
  scroll_min_step_lines: f32, // minimum line step when animating
  scroll_min_step_cols:  f32, // minimum column step when animating

  // Accumulated pending scroll deltas to animate (lines/cols)
  pending_scroll_lines: f32,
  pending_scroll_cols:  f32,

  // Delta time tracking for time-based animations
  last_frame_time: std::time::Instant,
}

impl App {
  pub fn new(editor: Editor) -> Self {
    let area = Rect::new(0, 0, 120, 40); // Default size, will be updated on resize.
    let mut compositor = Compositor::new(area);

    let mode = editor.mode;

    let keymaps = Keymaps::default();
    let editor_view = Box::new(EditorView::new(keymaps));
    compositor.push(editor_view);

    // Add statusline
    let statusline = Box::new(StatusLine::new());
    compositor.push(statusline);

    // NOTE: This is a test button btw.
    let button = Box::new(
      Button::new("Run")
                .with_rect(Rect::new(100, 1, 8, 2)) // Top-right.
                .color(the_editor_renderer::Color::rgb(0.6, 0.6, 0.8))
                .visible(false)
                .on_click(|| println!("Button clicked!")),
    );
    compositor.push(button);

    let conf = editor.config();
    Self {
      compositor,
      editor,
      jobs: Jobs::new(),
      input_handler: InputHandler::new(mode),
      smooth_scroll_enabled: conf.smooth_scroll_enabled,
      scroll_lerp_factor: conf.scroll_lerp_factor,
      scroll_min_step_lines: conf.scroll_min_step_lines,
      scroll_min_step_cols: conf.scroll_min_step_cols,
      pending_scroll_lines: 0.0,
      pending_scroll_cols: 0.0,
      last_frame_time: std::time::Instant::now(),
    }
  }
}

impl Application for App {
  fn init(&mut self, renderer: &mut Renderer) {
    println!("Application initialized!");

    renderer.set_ligature_protection(false);

    // NOTE: We currently allow users to specify a font file path via env var
    if let Ok(path) = std::env::var("THE_EDITOR_FONT_FILE")
      && let Err(err) = renderer.configure_font_from_path(&path, 22.0)
    {
      // TODO: Get from editor config.
      log::warn!("failed to load font from THE_EDITOR_FONT_FILE={path}: {err}");
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

    // Calculate delta time for time-based animations
    let now = std::time::Instant::now();
    let dt = now.duration_since(self.last_frame_time).as_secs_f32();
    self.last_frame_time = now;

    // Apply smooth scrolling animation prior to rendering this frame.
    if self.smooth_scroll_enabled {
      self.animate_scroll(renderer);
    }

    // Update theme transition animation
    let _theme_animating = self.editor.update_theme_transition(dt);

    // Create context for rendering.
    let mut cx = Context {
      editor: &mut self.editor,
      scroll: None,
      jobs: &mut self.jobs,
      dt,
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
        .is_some_and(|view| view.has_pending_on_next_key())
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
        dt:     0.0, // Events don't use delta time
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
        dt:     0.0,
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
        dt:     0.0,
      };

      return self.compositor.handle_event(&event, &mut cx);
    }

    // Handle scroll events.
    if let Some(scroll) = result.scroll {
      // Try to pass scroll to compositor first (for pickers, etc.)
      let event = Event::Scroll(scroll);
      let mut cx = Context {
        editor: &mut self.editor,
        scroll: None,
        jobs:   &mut self.jobs,
        dt:     0.0,
      };
      let handled = self.compositor.handle_event(&event, &mut cx);

      // If not handled by compositor, use default scroll behavior
      if !handled {
        self.handle_scroll(scroll, _renderer);
      }
      return true;
    }

    // Handle mouse events.
    if let Some(mouse) = result.mouse {
      let event = Event::Mouse(mouse);

      let mut cx = Context {
        editor: &mut self.editor,
        scroll: None,
        jobs:   &mut self.jobs,
        dt:     0.0,
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
          dt:     0.0,
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
            dt:     0.0,
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

    // Keep redrawing while a theme transition is active.
    if self.editor.is_theme_transitioning() {
      return true;
    }

    // Keep redrawing while a scroll animation is active.
    if self.smooth_scroll_enabled
      && (self.pending_scroll_lines.abs() > 0.01 || self.pending_scroll_cols.abs() > 0.01)
    {
      return true;
    }

    // Then check if any component needs updates.
    for layer in self.compositor.layers.iter() {
      // Check if it's a button with active animation.
      if let Some(button) = layer.as_any().downcast_ref::<Button>()
        && button.should_update()
      {
        return true;
      }

      // Other components can also request redraws via should_update.
      if layer.should_update() {
        return true;
      }
    }

    false
  }
}

impl App {
  fn handle_scroll(&mut self, delta: ScrollDelta, renderer: &mut Renderer) {
    // Convert incoming delta to logical lines/columns
    // Positive wheel y in winit is typically scroll up; map to negative lines
    // (toward file top)
    let (d_cols, d_lines) = match delta {
      ScrollDelta::Lines { x, y } => {
        let config_lines = self.editor.config().scroll_lines.max(1) as f32;
        (-x * 4.0, -y * config_lines)
      },
      ScrollDelta::Pixels { x, y } => {
        let line_h = renderer.cell_height().max(1.0);
        let col_w = renderer.cell_width().max(1.0);
        (-x / col_w, -y / line_h)
      },
    };

    // Accumulate into pending animation deltas
    self.pending_scroll_lines += d_lines;
    self.pending_scroll_cols += d_cols;

    // Nudge a redraw loop
    the_editor_event::request_redraw();
  }

  fn animate_scroll(&mut self, _renderer: &mut Renderer) {
    // Vertical: apply a fraction of pending lines via commands::scroll
    let apply_axis = |pending: &mut f32| -> i32 {
      let remaining = *pending;
      if remaining.abs() < 0.01 {
        return 0;
      }
      let step_f = remaining * self.scroll_lerp_factor;
      // Ensure a minimum perceptible step in the right direction
      let min_step = self.scroll_min_step_lines.copysign(remaining);
      let mut step = if step_f.abs() < self.scroll_min_step_lines.abs() {
        min_step
      } else {
        step_f
      };
      // Clamp step to remaining so we don't overshoot wildly
      if step.abs() > remaining.abs() {
        step = remaining;
      }
      // Convert to integral lines
      let step_i = if step >= 0.0 {
        step.floor() as i32
      } else {
        step.ceil() as i32
      };
      if step_i == 0 {
        // If fractional but significant remaining, force a single-line step
        let forced = if remaining > 0.0 { 1 } else { -1 };
        *pending -= forced as f32;
        return forced;
      }
      *pending -= step_i as f32;
      step_i
    };

    // Apply vertical scroll
    let v_lines = apply_axis(&mut self.pending_scroll_lines);
    if v_lines != 0 {
      let direction = if v_lines > 0 {
        Direction::Forward
      } else {
        Direction::Backward
      };
      let mut cmd_cx = commands::Context {
        register:             self.editor.selected_register,
        count:                self.editor.count,
        editor:               &mut self.editor,
        on_next_key_callback: None,
        callback:             Vec::new(),
        jobs:                 &mut self.jobs,
      };
      commands::scroll(
        &mut cmd_cx,
        v_lines.unsigned_abs() as usize,
        direction,
        false,
      );
    }

    // Horizontal: adjust view_offset.horizontal_offset directly
    // We use a separate min step for columns as columns tend to be smaller
    let remaining_h = self.pending_scroll_cols;
    if remaining_h.abs() >= 0.01 {
      let step_f = remaining_h * self.scroll_lerp_factor;
      let min_step = self.scroll_min_step_cols.copysign(remaining_h);
      let mut step = if step_f.abs() < self.scroll_min_step_cols.abs() {
        min_step
      } else {
        step_f
      };
      if step.abs() > remaining_h.abs() {
        step = remaining_h;
      }
      let step_i = if step >= 0.0 {
        step.floor() as i32
      } else {
        step.ceil() as i32
      };
      let step_i = if step_i == 0 {
        if remaining_h > 0.0 { 1 } else { -1 }
      } else {
        step_i
      };

      // Apply to focused view
      let focus_view = self.editor.tree.focus;
      let view = self.editor.tree.get(focus_view);
      let doc_id = view.doc;
      let doc = self.editor.documents.get_mut(&doc_id).unwrap();
      let mut vp = doc.view_offset(focus_view);
      let new_h = if step_i >= 0 {
        vp.horizontal_offset.saturating_add(step_i as usize)
      } else {
        vp.horizontal_offset.saturating_sub((-step_i) as usize)
      };
      vp.horizontal_offset = new_h;
      doc.set_view_offset(focus_view, vp);
      self.pending_scroll_cols -= step_i as f32;
    }
  }
}
