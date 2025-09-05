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
      NamedKey,
      PhysicalKey,
    },
    window::Window,
  };

  struct RendererApp<A: Application> {
    window:         Option<Arc<Window>>,
    renderer:       Option<Renderer>,
    app:            A,
    title:          String,
    initial_width:  u32,
    initial_height: u32,
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
          event_loop.exit();
        },
        WindowEvent::KeyboardInput {
          event:
            KeyEvent {
              state,
              physical_key: PhysicalKey::Code(code),
              logical_key,
              text,
              ..
            },
          ..
        } => {
          if let Some(renderer) = &mut self.renderer {
            // Track whether the app handled this input
            let mut handled = false;

            // First, handle special keys as keyboard events
            let handled_as_key = match code {
              KeyCode::Escape
              | KeyCode::Enter
              | KeyCode::Backspace
              | KeyCode::ArrowUp
              | KeyCode::ArrowDown
              | KeyCode::ArrowLeft
              | KeyCode::ArrowRight => {
                let input_event = InputEvent::Keyboard(KeyPress {
                  code:    match code {
                    KeyCode::Escape => event::Key::Escape,
                    KeyCode::Enter => event::Key::Enter,
                    KeyCode::Backspace => event::Key::Backspace,
                    KeyCode::ArrowUp => event::Key::Up,
                    KeyCode::ArrowDown => event::Key::Down,
                    KeyCode::ArrowLeft => event::Key::Left,
                    KeyCode::ArrowRight => event::Key::Right,
                    _ => unreachable!(),
                  },
                  pressed: state == ElementState::Pressed,
                  shift:   false,
                  ctrl:    false,
                  alt:     false,
                });
                handled = self.app.handle_event(input_event, renderer);
                if handled && let Some(window) = &self.window {
                  window.request_redraw();
                }
                true
              },
              _ => false,
            };

            // For regular text-producing keys, prefer KeyEvent.text
            if !handled_as_key && state == ElementState::Pressed {
              if let Some(t) = &text {
                if !t.is_empty() {
                  let text_event = InputEvent::Text(t.to_string());
                  handled = self.app.handle_event(text_event, renderer);
                  if handled && let Some(window) = &self.window {
                    window.request_redraw();
                  }
                }
              } else {
                // Fallback: derive text from logical_key when KeyEvent.text is None
                match &logical_key {
                  WinitKey::Character(s) if !s.is_empty() => {
                    let text_event = InputEvent::Text(s.clone().into());
                    handled = self.app.handle_event(text_event, renderer);
                    if handled && let Some(window) = &self.window {
                      window.request_redraw();
                    }
                  },
                  WinitKey::Named(NamedKey::Space) => {
                    let text_event = InputEvent::Text(" ".into());
                    handled = self.app.handle_event(text_event, renderer);
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

            // Exit on Escape only if the app did not handle it
            if matches!(code, KeyCode::Escape) && state == ElementState::Pressed && !handled {
              event_loop.exit();
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
    window: None,
    renderer: None,
    app,
    title: title.to_string(),
    initial_width: width,
    initial_height: height,
  };

  event_loop.run_app(&mut renderer_app)?;
  Ok(())
}
