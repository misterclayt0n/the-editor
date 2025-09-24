//! # The-Editor Renderer
//!
//! GPUI backed renderer and event loop integration for the-editor.

mod color;
mod error;
pub mod event;
mod renderer;
mod text;

use std::{
  borrow::Cow,
  cell::RefCell,
  sync::{
    Arc,
    Mutex,
    atomic::{
      AtomicBool,
      Ordering,
    },
  },
};

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
use gpui::{
  self,
  App,
  AppContext,
  Application as GpuiApplication,
  Background,
  BorderStyle,
  BoxShadow,
  Context,
  Corners,
  Edges,
  FocusHandle,
  Hsla,
  InteractiveElement,
  KeyDownEvent,
  KeyUpEvent,
  Keystroke,
  Menu,
  Modifiers,
  MouseButton as GpuiMouseButton,
  MouseDownEvent,
  MouseMoveEvent,
  MouseUpEvent,
  ParentElement,
  Rgba,
  ScrollWheelEvent,
  SharedString,
  StatefulInteractiveElement,
  Styled,
  TextRun,
  TitlebarOptions,
  Window,
  WindowBounds,
  WindowHandle,
  WindowOptions,
  bounds,
  canvas,
  div,
  font,
  point,
  px,
  size,
};
use renderer::{
  DrawCommand,
  FrameData,
};
const MONO_FONT_BYTES: &[u8] = include_bytes!("../assets/Iosevka-Regular.ttc");
const MONO_FONT_NAME: &str = "Iosevka";
const DEFAULT_FONT_SIZE: f32 = 22.0;
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
use the_editor_event;

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

  let result = Arc::new(Mutex::new(Ok(())));
  let result_for_app = result.clone();
  let app_cell = RefCell::new(Some(app));

  let window_title = SharedString::from(title.to_owned());

  GpuiApplication::new().run(move |cx| {
    if let Err(err) = cx
      .text_system()
      .add_fonts(vec![Cow::Borrowed(MONO_FONT_BYTES)])
    {
      log::warn!("failed to load editor font: {err}");
    }

    cx.set_menus(vec![Menu {
      name:  SharedString::from("Application"),
      items: vec![],
    }]);

    let window_options = WindowOptions {
      titlebar: Some(TitlebarOptions {
        title: Some(window_title.clone()),
        ..Default::default()
      }),
      window_bounds: Some(WindowBounds::Windowed(bounds(
        point(px(0.0), px(0.0)),
        size(px(width as f32), px(height as f32)),
      ))),
      ..Default::default()
    };

    let window_result = cx.open_window(window_options, |window, cx| {
      let app = app_cell
        .borrow_mut()
        .take()
        .expect("application already initialised");
      let viewport = window.viewport_size();
      let viewport_width = viewport.width.0.max(0.0).round() as u32;
      let viewport_height = viewport.height.0.max(0.0).round() as u32;
      let renderer = Renderer::new(viewport_width, viewport_height);
      let focus_handle = cx.focus_handle();
      let focus_for_window = focus_handle.clone();
      let entity = cx.new(move |_cx| EditorHost::new(app, renderer, focus_handle));
      window.focus(&focus_for_window);
      entity
    });

    match window_result {
      Ok(window) => {
        let title_text = window_title.to_string();
        let _ = window.update(cx, move |_, window, _| {
          window.set_window_title(&title_text);
        });

        let window_alive = Arc::new(AtomicBool::new(true));
        let alive_for_close = window_alive.clone();
        let _ = window.update(cx, move |_, window, cx| {
          window.on_window_should_close(cx, move |_window, _cx| {
            alive_for_close.store(false, Ordering::SeqCst);
            true
          });
        });

        spawn_redraw_listener(&window, cx, window_alive);
      },
      Err(err) => {
        *result_for_app.lock().unwrap() = Err(RendererError::WindowCreation(err.to_string()));
        cx.quit();
      },
    }
  });

  result.lock().unwrap().clone()
}

fn spawn_redraw_listener<A: Application + 'static>(
  window: &WindowHandle<EditorHost<A>>,
  cx: &App,
  alive: Arc<AtomicBool>,
) {
  let window_handle = window.clone();
  cx.spawn(move |async_cx: &mut gpui::AsyncApp| {
    let mut async_app = async_cx.clone();
    async move {
      loop {
        if !alive.load(Ordering::SeqCst) {
          break;
        }
        the_editor_event::redraw_requested().await;
        if !alive.load(Ordering::SeqCst) {
          break;
        }
        if window_handle
          .update(&mut async_app, |_, window, _| {
            window.refresh();
          })
          .is_err()
        {
          break;
        }
      }
    }
  })
  .detach();
}

struct EditorHost<A: Application> {
  app:           A,
  renderer:      Renderer,
  focus_handle:  FocusHandle,
  initial_focus: bool,
  initialised:   bool,
}

impl<A: Application> EditorHost<A> {
  fn new(app: A, renderer: Renderer, focus_handle: FocusHandle) -> Self {
    Self {
      app,
      renderer,
      focus_handle,
      initial_focus: false,
      initialised: false,
    }
  }

  fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window) {
    self.process_key_event(window, &event.keystroke, true);
  }

  fn on_key_up(&mut self, event: &KeyUpEvent, window: &mut Window) {
    self.process_key_event(window, &event.keystroke, false);
  }

  fn process_key_event(&mut self, window: &mut Window, keystroke: &Keystroke, pressed: bool) {
    let KeyPressState {
      key_press,
      should_emit_text,
      text,
    } = map_keypress(keystroke, pressed);

    let mut handled = self
      .app
      .handle_event(InputEvent::Keyboard(key_press), &mut self.renderer);

    if !handled && pressed && should_emit_text {
      if let Some(text) = text {
        handled = self
          .app
          .handle_event(InputEvent::Text(text), &mut self.renderer);
      }
    }

    if handled {
      window.refresh();
    }
  }

  fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window) {
    if handle_mouse_event(
      &mut self.app,
      &mut self.renderer,
      window,
      event.position.x.0,
      event.position.y.0,
      Some(event.button),
      true,
    ) {
      window.refresh();
    }
  }

  fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window) {
    if handle_mouse_event(
      &mut self.app,
      &mut self.renderer,
      window,
      event.position.x.0,
      event.position.y.0,
      Some(event.button),
      false,
    ) {
      window.refresh();
    }
  }

  fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window) {
    if handle_mouse_event(
      &mut self.app,
      &mut self.renderer,
      window,
      event.position.x.0,
      event.position.y.0,
      event.pressed_button,
      event.pressed_button.is_some(),
    ) {
      window.refresh();
    }
  }

  fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window) {
    let delta = match event.delta {
      gpui::ScrollDelta::Pixels(delta) => {
        ScrollDelta::Pixels {
          x: delta.x.0,
          y: delta.y.0,
        }
      },
      gpui::ScrollDelta::Lines(delta) => {
        ScrollDelta::Lines {
          x: delta.x,
          y: delta.y,
        }
      },
    };
    if self
      .app
      .handle_event(InputEvent::Scroll(delta), &mut self.renderer)
    {
      window.refresh();
    }
  }

  fn canvas(&mut self, frame: FrameData) -> gpui::Canvas<FrameData> {
    canvas(
      move |_bounds, _window, _cx| frame,
      move |bounds, frame, window, cx| paint_frame(bounds, frame, window, cx),
    )
    .size_full()
  }
}

impl<A: Application + 'static> gpui::Render for EditorHost<A> {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
    if !self.initialised {
      self.app.init(&mut self.renderer);
      self.initialised = true;
    }

    if !self.initial_focus {
      let focus_handle = self.focus_handle.clone();
      window.focus(&focus_handle);
      self.initial_focus = true;
    }

    let viewport = window.viewport_size();
    let width = viewport.width.0.max(0.0).round() as u32;
    let height = viewport.height.0.max(0.0).round() as u32;
    if self.renderer.update_viewport(width, height) {
      self.app.resize(width, height, &mut self.renderer);
    }

    self.renderer.ensure_font_metrics(window);

    let _ = self.renderer.begin_frame();
    self.app.render(&mut self.renderer);
    let frame = self.renderer.take_frame();
    let canvas = self.canvas(frame);

    if self.app.wants_redraw() {
      window.refresh();
    }

    let mut root = div()
      .id("the-editor-root")
      .size_full()
      .focusable()
      .track_focus(&self.focus_handle)
      .on_key_down(cx.listener(|this, event, window, _| this.on_key_down(event, window)))
      .on_key_up(cx.listener(|this, event, window, _| this.on_key_up(event, window)))
      .on_mouse_move(cx.listener(|this, event, window, _| this.on_mouse_move(event, window)))
      .on_scroll_wheel(cx.listener(|this, event, window, _| this.on_scroll_wheel(event, window)));

    for button in [
      GpuiMouseButton::Left,
      GpuiMouseButton::Right,
      GpuiMouseButton::Middle,
    ] {
      root = root
        .on_mouse_down(
          button,
          cx.listener(|this, event, window, _| this.on_mouse_down(event, window)),
        )
        .on_mouse_up(
          button,
          cx.listener(|this, event, window, _| this.on_mouse_up(event, window)),
        );
    }

    root.child(canvas)
  }
}

struct KeyPressState {
  key_press:        KeyPress,
  should_emit_text: bool,
  text:             Option<String>,
}

fn map_keypress(keystroke: &Keystroke, pressed: bool) -> KeyPressState {
  let KeyPressStateModifiers {
    shift,
    ctrl,
    alt,
    emit_text,
  } = modifier_state(&keystroke.modifiers);

  let key = map_key(&keystroke.key, keystroke.key_char.as_deref());
  let text = keystroke.key_char.clone();

  KeyPressState {
    key_press: KeyPress {
      code: key,
      pressed,
      shift,
      ctrl,
      alt,
    },
    should_emit_text: emit_text && text.is_some(),
    text,
  }
}

struct KeyPressStateModifiers {
  shift:     bool,
  ctrl:      bool,
  alt:       bool,
  emit_text: bool,
}

fn modifier_state(modifiers: &Modifiers) -> KeyPressStateModifiers {
  let shift = modifiers.shift;
  let ctrl = modifiers.control || modifiers.platform;
  let alt = modifiers.alt;
  let emit_text = !(modifiers.control || modifiers.alt || modifiers.platform);
  KeyPressStateModifiers {
    shift,
    ctrl,
    alt,
    emit_text,
  }
}

fn map_key(raw_key: &str, key_char: Option<&str>) -> Key {
  let normalized = raw_key.to_ascii_lowercase();
  match normalized.as_str() {
    "enter" | "return" => Key::Enter,
    "tab" => Key::Tab,
    "escape" | "esc" => Key::Escape,
    "backspace" => Key::Backspace,
    "delete" => Key::Delete,
    "home" => Key::Home,
    "end" => Key::End,
    "pageup" => Key::PageUp,
    "pagedown" => Key::PageDown,
    "up" | "arrowup" => Key::Up,
    "down" | "arrowdown" => Key::Down,
    "left" | "arrowleft" => Key::Left,
    "right" | "arrowright" => Key::Right,
    _ => {
      key_char
        .and_then(|char_text| char_text.chars().next())
        .or_else(|| normalized.chars().next())
        .map(Key::Char)
        .unwrap_or(Key::Other)
    },
  }
}

fn handle_mouse_event<A: Application>(
  app: &mut A,
  renderer: &mut Renderer,
  _window: &mut Window,
  x: f32,
  y: f32,
  button: Option<GpuiMouseButton>,
  pressed: bool,
) -> bool {
  let mapped_button = button.and_then(map_mouse_button);
  let mouse_event = MouseEvent {
    position: (x, y),
    button: mapped_button,
    pressed,
  };
  app.handle_event(InputEvent::Mouse(mouse_event), renderer)
}

fn map_mouse_button(button: GpuiMouseButton) -> Option<MouseButton> {
  match button {
    GpuiMouseButton::Left => Some(MouseButton::Left),
    GpuiMouseButton::Right => Some(MouseButton::Right),
    GpuiMouseButton::Middle => Some(MouseButton::Middle),
    GpuiMouseButton::Navigate(_) => None,
  }
}

fn paint_frame(
  bounds: gpui::Bounds<gpui::Pixels>,
  frame: FrameData,
  window: &mut Window,
  cx: &mut App,
) {
  window.paint_quad(gpui::fill(bounds, to_background(frame.background_color)));

  for command in frame.commands {
    match command {
      DrawCommand::Rect {
        x,
        y,
        width,
        height,
        color,
      } => {
        let rect_bounds = rect_bounds(x, y, width, height);
        window.paint_quad(gpui::fill(rect_bounds, to_background(color)));
      },
      DrawCommand::RoundedRect {
        x,
        y,
        width,
        height,
        corner_radius,
        color,
      } => {
        let rect_bounds = rect_bounds(x, y, width, height);
        let quad = gpui::fill(rect_bounds, to_background(color))
          .corner_radii(Corners::all(px(corner_radius)));
        window.paint_quad(quad);
      },
      DrawCommand::RoundedRectGlow {
        x,
        y,
        width,
        height,
        corner_radius,
        glow_radius,
        color,
        ..
      } => {
        let rect_bounds = rect_bounds(x, y, width, height);
        let layer_bounds = rect_bounds;
        window.paint_layer(layer_bounds, |window| {
          window.paint_shadows(layer_bounds, Corners::all(px(corner_radius)), &[
            BoxShadow {
              color:         to_hsla(color),
              offset:        point(px(0.0), px(0.0)),
              blur_radius:   px(glow_radius),
              spread_radius: px(0.0),
            },
          ]);
        });
      },
      DrawCommand::RoundedRectStroke {
        x,
        y,
        width,
        height,
        corner_radius,
        thickness,
        color,
      } => {
        let rect_bounds = rect_bounds(x, y, width, height);
        let quad = gpui::quad(
          rect_bounds,
          Corners::all(px(corner_radius)),
          Background::from(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
          }),
          Edges::all(px(thickness)),
          to_hsla(color),
          BorderStyle::Solid,
        );
        window.paint_quad(quad);
      },
      DrawCommand::Text(section) => paint_text_section(section, window, cx),
    }
  }
}

fn rect_bounds(x: f32, y: f32, width: f32, height: f32) -> gpui::Bounds<gpui::Pixels> {
  gpui::Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn paint_text_section(section: TextSection, window: &mut Window, cx: &mut App) {
  if section.texts.is_empty() {
    return;
  }

  let mut cursor_x = px(section.position.0);
  let top = px(section.position.1);

  for segment in section.texts {
    if segment.content.is_empty() {
      continue;
    }

    let text = SharedString::from(segment.content);
    let text_run = TextRun {
      len:              text.len(),
      font:             font(MONO_FONT_NAME),
      color:            to_hsla(segment.style.color),
      background_color: None,
      underline:        None,
      strikethrough:    None,
    };

    let font_size = px(segment.style.size);
    let shaped_line = window
      .text_system()
      .shape_line(text, font_size, &[text_run], None);

    let origin = point(cursor_x, top);
    if let Err(err) = shaped_line.paint(origin, font_size, window, cx) {
      log::warn!("failed to paint text: {err}");
    }

    cursor_x += shaped_line.width;
  }
}

fn to_background(color: Color) -> Background {
  Background::from(to_rgba(color))
}

fn to_hsla(color: Color) -> Hsla {
  Hsla::from(to_rgba(color))
}

fn to_rgba(color: Color) -> Rgba {
  Rgba {
    r: color.r,
    g: color.g,
    b: color.b,
    a: color.a,
  }
}
