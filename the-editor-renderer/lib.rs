//! # The-Editor Renderer
//!
//! GPU-backed renderer and event loop integration for the-editor.

// NOTE: This renderer is currently simply not fast enough, like, at all.

mod color;
mod error;
pub mod event;
mod renderer;
mod text;
mod text_cache;

use std::sync::Arc;

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
pub trait Application {
  /// Called once when the renderer is initialized.
  fn init(&mut self, renderer: &mut Renderer);

  /// Called every frame to render the application.
  fn render(&mut self, renderer: &mut Renderer);

  /// Called when an input event occurs.
  fn handle_event(&mut self, event: InputEvent, renderer: &mut Renderer) -> bool;

  /// Called when the window is resized.
  fn resize(&mut self, width: u32, height: u32, renderer: &mut Renderer);

  /// Return true if the application wants another immediate redraw.
  fn wants_redraw(&self) -> bool {
    false
  }
}

/// Run the application with the renderer.
pub fn run<A: Application + 'static>(title: &str, width: u32, height: u32, app: A) -> Result<()> {
  let _ = env_logger::try_init();

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
    // Dead keys are handled as text events, so we shouldn't normally get here
    // But if we do, return Other to ignore them
    if matches!(logical_key, WinitKey::Dead(_)) {
      return event::Key::Other;
    }

    match logical_key {
      WinitKey::Character(s) if !s.is_empty() => {
        let mut chars = s.chars();
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
    modifiers: &mut ModifiersState,
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
        ElementState::Pressed => modifiers.insert(flag),
        ElementState::Released => modifiers.remove(flag),
      }
    }
  }

  struct RendererApp<A: Application> {
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
      if self.window.is_some() {
        return;
      }

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
              self.window = Some(window.clone());

              if let Some(win) = &self.window {
                let weak = Arc::downgrade(win);
                std::thread::spawn(move || {
                  loop {
                    pollster::block_on(the_editor_event::redraw_requested());
                    if let Some(win) = weak.upgrade() {
                      win.request_redraw();
                    } else {
                      break;
                    }
                  }
                });
              }
            },
            Err(e) => {
              log::error!("Failed to create renderer: {e}");
            },
          }
        },
        Err(e) => {
          log::error!("Failed to create window: {e}");
        },
      }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
      match event {
        WindowEvent::CloseRequested => {
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

            // Skip keyboard events for dead keys - they'll come through as text events
            let is_dead_key = matches!(logical_key, WinitKey::Dead(_));
            let modifiers = self.modifiers_state;
            let mut handled = false;

            // Check if this key has composed text (from dead key composition)
            // Composed text is when the text doesn't match what the key would normally
            // produce For example, ´ + space produces ' instead of just space
            let has_composed_text = if let Some(t) = &text {
              // Only consider it composed if it's not a special key and the text differs from
              // expected
              match &logical_key {
                WinitKey::Character(expected) => t != expected,
                WinitKey::Named(NamedKey::Space) => t != " ",
                // Special keys like Escape, Enter, etc. should not be treated as composed text
                WinitKey::Named(_) => false,
                _ => false,
              }
            } else {
              false
            };

            if !is_dead_key && !has_composed_text {
              let code = map_winit_key(&logical_key, &physical_key);
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
              handled = self
                .app
                .handle_event(InputEvent::Keyboard(key_press), renderer);
            }
            if handled {
              if let Some(win) = &self.window {
                win.request_redraw();
              }
            }

            // Send text events for composed text, unhandled keys, or dead keys
            if (has_composed_text || !handled || is_dead_key)
              && state == ElementState::Pressed
              && !(modifiers.alt_key() || modifiers.control_key())
            {
              if let Some(t) = &text {
                if !t.is_empty() {
                  let handled = self
                    .app
                    .handle_event(InputEvent::Text(t.to_string()), renderer);
                  if handled {
                    if let Some(win) = &self.window {
                      win.request_redraw();
                    }
                  }
                }
              } else if !is_dead_key {
                match &logical_key {
                  WinitKey::Character(s) if !s.is_empty() => {
                    let handled = self
                      .app
                      .handle_event(InputEvent::Text(s.to_string()), renderer);
                    if handled {
                      if let Some(win) = &self.window {
                        win.request_redraw();
                      }
                    }
                  },
                  WinitKey::Named(NamedKey::Space) => {
                    let handled = self
                      .app
                      .handle_event(InputEvent::Text(" ".into()), renderer);
                    if handled {
                      if let Some(win) = &self.window {
                        win.request_redraw();
                      }
                    }
                  },
                  _ => {},
                }
              }
            }
          }
        },
        WindowEvent::ModifiersChanged(mods) => {
          self.modifiers_state = mods.state();
        },
        WindowEvent::MouseInput { state, button, .. } => {
          if let Some(renderer) = &mut self.renderer {
            let mouse_button = match button {
              winit::event::MouseButton::Left => Some(MouseButton::Left),
              winit::event::MouseButton::Right => Some(MouseButton::Right),
              winit::event::MouseButton::Middle => Some(MouseButton::Middle),
              _ => None,
            };

            if let Some(mouse_button) = mouse_button {
              let mouse_event = MouseEvent {
                position: self.last_cursor_position.unwrap_or((0.0, 0.0)),
                button:   Some(mouse_button),
                pressed:  state == ElementState::Pressed,
              };

              if self
                .app
                .handle_event(InputEvent::Mouse(mouse_event), renderer)
              {
                if let Some(win) = &self.window {
                  win.request_redraw();
                }
              }
            }
          }
        },
        WindowEvent::CursorMoved { position, .. } => {
          if let Some(renderer) = &mut self.renderer {
            let cursor_pos = (position.x as f32, position.y as f32);
            self.last_cursor_position = Some(cursor_pos);

            let mouse_event = MouseEvent {
              position: cursor_pos,
              button:   None,
              pressed:  false,
            };

            if self
              .app
              .handle_event(InputEvent::Mouse(mouse_event), renderer)
            {
              if let Some(win) = &self.window {
                win.request_redraw();
              }
            }
          }
        },
        WindowEvent::MouseWheel { delta, .. } => {
          if let Some(renderer) = &mut self.renderer {
            let scroll_delta = match delta {
              MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines { x, y },
              MouseScrollDelta::PixelDelta(pos) => {
                ScrollDelta::Pixels {
                  x: pos.x as f32,
                  y: pos.y as f32,
                }
              },
            };
            if self
              .app
              .handle_event(InputEvent::Scroll(scroll_delta), renderer)
            {
              if let Some(win) = &self.window {
                win.request_redraw();
              }
            }
          }
        },
        WindowEvent::Ime(ime) => {
          if let Some(renderer) = &mut self.renderer {
            use winit::event::Ime;
            match ime {
              Ime::Commit(text) => {
                if self.app.handle_event(InputEvent::Text(text), renderer) {
                  if let Some(win) = &self.window {
                    win.request_redraw();
                  }
                }
              },
              Ime::Preedit(..) | Ime::Enabled | Ime::Disabled => {},
            }
          }
        },
        WindowEvent::Resized(physical_size) => {
          if let Some(renderer) = &mut self.renderer {
            renderer.resize(physical_size);
            self
              .app
              .resize(physical_size.width, physical_size.height, renderer);
            if let Some(win) = &self.window {
              win.request_redraw();
            }
          }
        },
        WindowEvent::ScaleFactorChanged { .. } => {},
        WindowEvent::RedrawRequested => {
          if let Some(renderer) = &mut self.renderer {
            match renderer.begin_frame() {
              Ok(()) => {
                self.app.render(renderer);
                if let Err(err) = renderer.end_frame() {
                  log::error!("Render error: {err:?}");
                }
                if self.app.wants_redraw() {
                  if let Some(win) = &self.window {
                    win.request_redraw();
                  }
                }
              },
              Err(RendererError::SkipFrame) => {
                // Expected during interactive resize; quietly skip
              },
              Err(e) => log::error!("Failed to begin frame: {e:?}"),
            }
          }
        },
        _ => {},
      }
    }
  }

  let event_loop = EventLoop::new().map_err(|e| RendererError::WindowCreation(e.to_string()))?;
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

  event_loop
    .run_app(&mut renderer_app)
    .map_err(|e| RendererError::Runtime(e.to_string()))?;
  Ok(())
}
