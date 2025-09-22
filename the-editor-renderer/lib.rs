//! # The-Editor Renderer
//!
//! A high-level GPU-accelerated text rendering library built on wgpu.
//!
//! ## Overview
//!
//! This crate provides a simple API for creating GPU-accelerated applications
//! with text rendering capabilities. It handles window creation, event
//! management, and efficient text rendering using wgpu.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use the_editor_renderer::{
//!   Application,
//!   Color,
//!   InputEvent,
//!   Renderer,
//!   TextSection,
//!   TextSegment,
//!   run,
//! };
//!
//! struct MyApp;
//!
//! impl Application for MyApp {
//!   fn init(&mut self, renderer: &mut Renderer) {
//!     // Initialize your application
//!   }
//!
//!   fn render(&mut self, renderer: &mut Renderer) {
//!     // Draw text
//!     renderer.draw_text(TextSection::simple(
//!       10.0,
//!       10.0,
//!       "Hello, World!",
//!       24.0,
//!       Color::WHITE,
//!     ));
//!   }
//!
//!   fn handle_event(&mut self, event: InputEvent, renderer: &mut Renderer) -> bool {
//!     // Handle input events
//!     false // Return true if the screen needs to be redrawn
//!   }
//!
//!   fn resize(&mut self, width: u32, height: u32, renderer: &mut Renderer) {
//!     // Handle window resize
//!   }
//! }
//!
//! # fn main() {
//! #     let app = MyApp;
//! #     // run("My Application", 800, 600, app).unwrap();
//! # }
//! ```

mod color;
mod error;
pub mod event;
mod renderer;
mod text;

pub use color::Color;
pub use error::{
  RendererError,
  Result,
};
pub use event::{
  InputEvent,
  Key,
  KeyPress,
  MouseButton,
  MouseEvent,
  ScrollDelta,
};
pub use renderer::{
  Renderer,
  RendererConfig,
};
pub use text::{
  Font,
  TextSection,
  TextSegment,
  TextStyle,
};
use winit::window::WindowId;

/// Main trait that applications must implement to use the renderer.
///
/// This trait defines the interface between your application and the rendering
/// system. All methods receive a mutable reference to the [`Renderer`] to
/// perform drawing operations.
pub trait Application {
  /// Called once when the renderer is initialized.
  ///
  /// Use this method to set up initial state, load resources, or configure the
  /// renderer.
  fn init(&mut self, renderer: &mut Renderer);

  /// Called every frame to render the application.
  ///
  /// This is where you should draw all your visual elements using the
  /// renderer's drawing methods. The renderer automatically handles frame
  /// buffering and presentation.
  fn render(&mut self, renderer: &mut Renderer);

  /// Called when an input event occurs.
  ///
  /// Process keyboard, mouse, and text input events here.
  /// Return `true` if the event causes changes that require a redraw.
  fn handle_event(&mut self, event: InputEvent, renderer: &mut Renderer) -> bool;

  /// Called when the window is resized.
  ///
  /// Update your layout or viewport-dependent state here.
  /// The renderer automatically updates its internal viewport.
  fn resize(&mut self, width: u32, height: u32, renderer: &mut Renderer);

  /// Return true if the application wants another immediate redraw.
  /// Default is false. Override for simple animations or transient effects.
  fn wants_redraw(&self) -> bool {
    false
  }
}

/// Run the application with the renderer.
///
/// This is the main entry point for your application. It creates a window,
/// initializes the GPU renderer, and runs the event loop.
///
/// # Arguments
///
/// * `title` - The window title
/// * `width` - Initial window width in pixels
/// * `height` - Initial window height in pixels
/// * `app` - Your application implementing the [`Application`] trait
///
/// # Returns
///
/// Returns `Ok(())` when the application exits normally, or an error if
/// initialization fails.
///
/// # Example
///
/// ```rust,no_run
/// # use the_editor_renderer::{Application, Renderer, InputEvent, run};
/// # struct MyApp;
/// # impl Application for MyApp {
/// #     fn init(&mut self, _: &mut Renderer) {}
/// #     fn render(&mut self, _: &mut Renderer) {}
/// #     fn handle_event(&mut self, _: InputEvent, _: &mut Renderer) -> bool { false }
/// #     fn resize(&mut self, _: u32, _: u32, _: &mut Renderer) {}
/// # }
/// let app = MyApp;
/// run("My Application", 800, 600, app).expect("Failed to run application");
/// ```
pub fn run<A: Application + 'static>(
  title: &str,
  width: u32,
  height: u32,
  app: A,
) -> crate::Result<()> {
  env_logger::init();

  use std::sync::Arc;

  use winit::{
    application::ApplicationHandler,
    event::{
      ElementState,
      KeyEvent,
      MouseScrollDelta,
      WindowEvent,
    },
    event_loop::{
      ActiveEventLoop,
      ControlFlow,
      EventLoop,
    },
    keyboard::{
      Key as WinitKey,
      KeyCode,
      ModifiersState,
      NamedKey,
      PhysicalKey,
    },
    window::Window,
  };

  fn map_winit_key(logical_key: &WinitKey, physical_key: &PhysicalKey) -> event::Key {
    if matches!(logical_key, WinitKey::Dead(_)) {
      return event::Key::Other;
    }

    match logical_key {
      WinitKey::Character(s) if !s.is_empty() => {
        let mut chars = s.chars();
        // Prefer the first scalar value; additional characters (e.g. composed glyphs)
        // will be delivered through the text path when the keymap does not consume
        // them.
        chars
          .next()
          .map(event::Key::Char)
          .unwrap_or(event::Key::Other)
      },
      WinitKey::Named(named) => {
        map_named_key(named).unwrap_or_else(|| map_physical_key(physical_key))
      },
      _ => map_physical_key(physical_key),
    }
  }

  fn map_named_key(named: &NamedKey) -> Option<event::Key> {
    use event::Key;

    Some(match named {
      NamedKey::Enter => Key::Enter,
      NamedKey::Tab => Key::Tab,
      NamedKey::Escape => Key::Escape,
      NamedKey::Backspace => Key::Backspace,
      NamedKey::Delete => Key::Delete,
      NamedKey::Home => Key::Home,
      NamedKey::End => Key::End,
      NamedKey::PageUp => Key::PageUp,
      NamedKey::PageDown => Key::PageDown,
      NamedKey::ArrowUp => Key::Up,
      NamedKey::ArrowDown => Key::Down,
      NamedKey::ArrowLeft => Key::Left,
      NamedKey::ArrowRight => Key::Right,
      NamedKey::Space => Key::Char(' '),
      _ => return None,
    })
  }

  fn map_physical_key(physical_key: &PhysicalKey) -> event::Key {
    use event::Key;

    match physical_key {
      PhysicalKey::Code(KeyCode::ArrowUp) => Key::Up,
      PhysicalKey::Code(KeyCode::ArrowDown) => Key::Down,
      PhysicalKey::Code(KeyCode::ArrowLeft) => Key::Left,
      PhysicalKey::Code(KeyCode::ArrowRight) => Key::Right,
      PhysicalKey::Code(KeyCode::Enter) => Key::Enter,
      PhysicalKey::Code(KeyCode::Tab) => Key::Tab,
      PhysicalKey::Code(KeyCode::Escape) => Key::Escape,
      PhysicalKey::Code(KeyCode::Backspace) => Key::Backspace,
      PhysicalKey::Code(KeyCode::Delete) => Key::Delete,
      PhysicalKey::Code(KeyCode::Home) => Key::Home,
      PhysicalKey::Code(KeyCode::End) => Key::End,
      PhysicalKey::Code(KeyCode::PageUp) => Key::PageUp,
      PhysicalKey::Code(KeyCode::PageDown) => Key::PageDown,
      PhysicalKey::Code(KeyCode::Space) => Key::Char(' '),
      PhysicalKey::Code(KeyCode::Backquote) => Key::Char('`'),
      PhysicalKey::Code(KeyCode::BracketLeft) => Key::Char('['),
      PhysicalKey::Code(KeyCode::BracketRight) => Key::Char(']'),
      PhysicalKey::Code(KeyCode::Comma) => Key::Char(','),
      PhysicalKey::Code(KeyCode::Period) => Key::Char('.'),
      PhysicalKey::Code(KeyCode::Slash) => Key::Char('/'),
      PhysicalKey::Code(KeyCode::Backslash) => Key::Char('\\'),
      PhysicalKey::Code(KeyCode::Minus) => Key::Char('-'),
      PhysicalKey::Code(KeyCode::Equal) => Key::Char('='),
      PhysicalKey::Code(KeyCode::Quote) => Key::Char('\''),
      PhysicalKey::Code(KeyCode::Semicolon) => Key::Char(';'),
      PhysicalKey::Code(KeyCode::Digit0) => Key::Char('0'),
      PhysicalKey::Code(KeyCode::Digit1) => Key::Char('1'),
      PhysicalKey::Code(KeyCode::Digit2) => Key::Char('2'),
      PhysicalKey::Code(KeyCode::Digit3) => Key::Char('3'),
      PhysicalKey::Code(KeyCode::Digit4) => Key::Char('4'),
      PhysicalKey::Code(KeyCode::Digit5) => Key::Char('5'),
      PhysicalKey::Code(KeyCode::Digit6) => Key::Char('6'),
      PhysicalKey::Code(KeyCode::Digit7) => Key::Char('7'),
      PhysicalKey::Code(KeyCode::Digit8) => Key::Char('8'),
      PhysicalKey::Code(KeyCode::Digit9) => Key::Char('9'),
      PhysicalKey::Code(KeyCode::KeyA) => Key::Char('a'),
      PhysicalKey::Code(KeyCode::KeyB) => Key::Char('b'),
      PhysicalKey::Code(KeyCode::KeyC) => Key::Char('c'),
      PhysicalKey::Code(KeyCode::KeyD) => Key::Char('d'),
      PhysicalKey::Code(KeyCode::KeyE) => Key::Char('e'),
      PhysicalKey::Code(KeyCode::KeyF) => Key::Char('f'),
      PhysicalKey::Code(KeyCode::KeyG) => Key::Char('g'),
      PhysicalKey::Code(KeyCode::KeyH) => Key::Char('h'),
      PhysicalKey::Code(KeyCode::KeyI) => Key::Char('i'),
      PhysicalKey::Code(KeyCode::KeyJ) => Key::Char('j'),
      PhysicalKey::Code(KeyCode::KeyK) => Key::Char('k'),
      PhysicalKey::Code(KeyCode::KeyL) => Key::Char('l'),
      PhysicalKey::Code(KeyCode::KeyM) => Key::Char('m'),
      PhysicalKey::Code(KeyCode::KeyN) => Key::Char('n'),
      PhysicalKey::Code(KeyCode::KeyO) => Key::Char('o'),
      PhysicalKey::Code(KeyCode::KeyP) => Key::Char('p'),
      PhysicalKey::Code(KeyCode::KeyQ) => Key::Char('q'),
      PhysicalKey::Code(KeyCode::KeyR) => Key::Char('r'),
      PhysicalKey::Code(KeyCode::KeyS) => Key::Char('s'),
      PhysicalKey::Code(KeyCode::KeyT) => Key::Char('t'),
      PhysicalKey::Code(KeyCode::KeyU) => Key::Char('u'),
      PhysicalKey::Code(KeyCode::KeyV) => Key::Char('v'),
      PhysicalKey::Code(KeyCode::KeyW) => Key::Char('w'),
      PhysicalKey::Code(KeyCode::KeyX) => Key::Char('x'),
      PhysicalKey::Code(KeyCode::KeyY) => Key::Char('y'),
      PhysicalKey::Code(KeyCode::KeyZ) => Key::Char('z'),
      _ => Key::Other,
    }
  }

  fn update_modifier_state(
    mods: &mut ModifiersState,
    physical_key: &PhysicalKey,
    state: ElementState,
  ) {
    use winit::keyboard::KeyCode;

    let flag = match physical_key {
      PhysicalKey::Code(KeyCode::ShiftLeft) | PhysicalKey::Code(KeyCode::ShiftRight) => {
        Some(ModifiersState::SHIFT)
      },
      PhysicalKey::Code(KeyCode::ControlLeft) | PhysicalKey::Code(KeyCode::ControlRight) => {
        Some(ModifiersState::CONTROL)
      },
      PhysicalKey::Code(KeyCode::AltLeft) | PhysicalKey::Code(KeyCode::AltRight) => {
        Some(ModifiersState::ALT)
      },
      PhysicalKey::Code(KeyCode::SuperLeft) | PhysicalKey::Code(KeyCode::SuperRight) => {
        Some(ModifiersState::SUPER)
      },
      _ => None,
    };

    if let Some(flag) = flag {
      match state {
        ElementState::Pressed => mods.insert(flag),
        ElementState::Released => mods.remove(flag),
      }
    }
  }

  struct RendererApp<A: Application> {
    // Important: drop renderer before window to ensure GPU surface is released
    // while the window still exists (avoids driver stalls on shutdown).
    renderer:             Option<Renderer>,
    window:               Option<Arc<Window>>,
    app:                  A,
    title:                String,
    initial_width:        u32,
    initial_height:       u32,
    modifiers_state:      ModifiersState,
    last_cursor_position: Option<(f32, f32)>,
  }

  impl<A: Application> ApplicationHandler for RendererApp<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
      if self.window.is_none() {
        let window_attributes = Window::default_attributes()
          .with_title(&self.title)
          .with_inner_size(winit::dpi::LogicalSize::new(
            self.initial_width,
            self.initial_height,
          ));

        match event_loop.create_window(window_attributes) {
          Ok(window) => {
            let window = Arc::new(window);
            match pollster::block_on(Renderer::new(window.clone())) {
              Ok(mut renderer) => {
                self.app.init(&mut renderer);
                self.renderer = Some(renderer);
                self.window = Some(window);

                // Bridge the the-editor-event redraw requests to winit's redraws.
                // This lets background tasks request UI redraws.
                if let Some(win) = &self.window {
                  let weak_win = Arc::downgrade(win);
                  std::thread::spawn(move || {
                    loop {
                      // Wait for an async redraw request from the event system
                      pollster::block_on(the_editor_event::redraw_requested());
                      if let Some(win) = weak_win.upgrade() {
                        win.request_redraw();
                      } else {
                        // Window is gone; exit the helper thread.
                        break;
                      }
                    }
                  });
                }
              },
              Err(e) => {
                eprintln!("Failed to create renderer: {}", e);
              },
            }
          },
          Err(e) => {
            eprintln!("Failed to create window: {}", e);
          },
        }
      }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
      match event {
        WindowEvent::CloseRequested => {
          // Drop the renderer (and thus the GPU surface) before the window
          // to avoid platform driver stalls or validation errors.
          let _ = self.renderer.take();
          event_loop.exit();
        },
        WindowEvent::KeyboardInput {
          event:
            KeyEvent {
              state,
              physical_key,
              logical_key,
              text,
              ..
            },
          ..
        } => {
          if let Some(renderer) = &mut self.renderer {
            update_modifier_state(&mut self.modifiers_state, &physical_key, state);

            let code = map_winit_key(&logical_key, &physical_key);
            let modifiers = self.modifiers_state;
            let mut key_press = KeyPress {
              code,
              pressed: state == ElementState::Pressed,
              shift: modifiers.shift_key(),
              ctrl: modifiers.control_key(),
              alt: modifiers.alt_key(),
            };
            if matches!(key_press.code, event::Key::Char(_)) {
              key_press.shift = false;
            }
            let handled = self
              .app
              .handle_event(InputEvent::Keyboard(key_press), renderer);
            if handled && let Some(window) = &self.window {
              window.request_redraw();
            }

            // For regular text-producing keys, prefer KeyEvent.text when not already
            // handled. Skip textual insertion when control or alt modifiers are
            // active so chorded keybindings (e.g. Alt+Backspace) do not insert
            // control characters.
            if !handled
              && state == ElementState::Pressed
              && !(modifiers.alt_key() || modifiers.control_key())
            {
              if let Some(t) = &text {
                if !t.is_empty() {
                  let text_event = InputEvent::Text(t.to_string());
                  let handled = self.app.handle_event(text_event, renderer);
                  if handled && let Some(window) = &self.window {
                    window.request_redraw();
                  }
                }
              } else {
                // Fallback: derive text from logical_key when KeyEvent.text is None
                match &logical_key {
                  WinitKey::Character(s) if !s.is_empty() => {
                    let text_event = InputEvent::Text(s.clone().into());
                    let handled = self.app.handle_event(text_event, renderer);
                    if handled && let Some(window) = &self.window {
                      window.request_redraw();
                    }
                  },
                  WinitKey::Named(NamedKey::Space) => {
                    let text_event = InputEvent::Text(" ".into());
                    let handled = self.app.handle_event(text_event, renderer);
                    if handled && let Some(window) = &self.window {
                      window.request_redraw();
                    }
                  },
                  _ => {
                    // Ignore other non-text keys here
                  },
                }
              }
            }
          }
        },
        WindowEvent::ModifiersChanged(modifiers) => {
          self.modifiers_state = modifiers.state();
        },

        // Handle mouse button events
        WindowEvent::MouseInput { state, button, .. } => {
          if let Some(renderer) = &mut self.renderer {
            use winit::event::ElementState;

            let mouse_button = match button {
              winit::event::MouseButton::Left => Some(event::MouseButton::Left),
              winit::event::MouseButton::Right => Some(event::MouseButton::Right),
              winit::event::MouseButton::Middle => Some(event::MouseButton::Middle),
              _ => None,
            };

            if let Some(mouse_button) = mouse_button {
              let mouse_event = event::MouseEvent {
                position: self.last_cursor_position.unwrap_or((0.0, 0.0)),
                button:   Some(mouse_button),
                pressed:  state == ElementState::Pressed,
              };

              if self
                .app
                .handle_event(InputEvent::Mouse(mouse_event), renderer)
                && let Some(window) = &self.window
              {
                window.request_redraw();
              }
            }
          }
        },

        // Handle cursor movement
        WindowEvent::CursorMoved { position, .. } => {
          if let Some(renderer) = &mut self.renderer {
            let cursor_pos = (position.x as f32, position.y as f32);
            self.last_cursor_position = Some(cursor_pos);

            let mouse_event = event::MouseEvent {
              position: cursor_pos,
              button:   None,
              pressed:  false,
            };

            if self
              .app
              .handle_event(InputEvent::Mouse(mouse_event), renderer)
              && let Some(window) = &self.window
            {
              window.request_redraw();
            }
          }
        },

        // NOTE: This sucks ass, but will do it for now.
        WindowEvent::MouseWheel { delta, .. } => {
          if let Some(renderer) = &mut self.renderer {
            let scroll_delta = match delta {
              MouseScrollDelta::LineDelta(x, y) => event::ScrollDelta::Lines { x, y },
              MouseScrollDelta::PixelDelta(pos) => {
                event::ScrollDelta::Pixels {
                  x: pos.x as f32,
                  y: pos.y as f32,
                }
              },
            };
            if self
              .app
              .handle_event(InputEvent::Scroll(scroll_delta), renderer)
              && let Some(window) = &self.window
            {
              window.request_redraw();
            }
          }
        },
        WindowEvent::Ime(ime) => {
          if let Some(renderer) = &mut self.renderer {
            use winit::event::Ime;
            match ime {
              Ime::Commit(text) => {
                let input_event = InputEvent::Text(text);
                if self.app.handle_event(input_event, renderer)
                  && let Some(window) = &self.window
                {
                  window.request_redraw();
                }
              },
              Ime::Preedit(text, _cursor) => {
                // For now, we could show preedit text but don't commit it
                // This is where composed characters appear before being finalized
                if !text.is_empty() {
                  eprintln!("IME Preedit: {:?}", text);
                }
              },
              Ime::Enabled | Ime::Disabled => {
                // IME state changes, we can ignore these for now
              },
            }
          }
        },
        WindowEvent::Resized(physical_size) => {
          if let Some(renderer) = &mut self.renderer {
            renderer.resize(physical_size);
            self
              .app
              .resize(physical_size.width, physical_size.height, renderer);
          }
        },
        WindowEvent::RedrawRequested => {
          if let Some(renderer) = &mut self.renderer {
            match renderer.begin_frame() {
              Ok(()) => {
                self.app.render(renderer);
                if let Err(e) = renderer.end_frame() {
                  eprintln!("Render error: {:?}", e);
                }
                // If the app wants to continue animating, request another frame.
                if self.app.wants_redraw() {
                  if let Some(window) = &self.window {
                    window.request_redraw();
                  }
                }
              },
              Err(e) => eprintln!("Failed to begin frame: {:?}", e),
            }
          }
        },
        _ => {},
      }
    }
  }

  let event_loop =
    EventLoop::new().map_err(|e| crate::RendererError::WindowCreation(e.to_string()))?;
  event_loop.set_control_flow(ControlFlow::Wait);

  let mut renderer_app = RendererApp {
    renderer: None,
    window: None,
    app,
    title: title.to_string(),
    initial_width: width,
    initial_height: height,
    modifiers_state: ModifiersState::empty(),
    last_cursor_position: None,
  };

  event_loop.run_app(&mut renderer_app)?;
  Ok(())
}
