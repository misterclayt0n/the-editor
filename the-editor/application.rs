use std::{
  cell::RefCell,
  rc::Rc,
  sync::Arc,
};

use the_editor_renderer::{
  Application,
  InputEvent,
  Key,
  Renderer,
  ScrollDelta,
};

use crate::{
  core::{
    ViewId,
    commands,
    graphics::Rect,
    movement::Direction,
  },
  editor::{
    Editor,
    TerminalPane,
  },
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

  // GlobalConfig pointer for runtime updates
  pub config_ptr: std::sync::Arc<arc_swap::ArcSwap<crate::core::config::Config>>,

  // LocalSet for polling !Send futures (ACP)
  local_set:      Rc<tokio::task::LocalSet>,
  runtime_handle: tokio::runtime::Handle,

  // Smooth scrolling configuration and state
  smooth_scroll_enabled: bool,
  scroll_lerp_factor:    f32, // fraction of remaining distance per frame (0..1)
  scroll_min_step_lines: f32, // minimum line step when animating
  scroll_min_step_cols:  f32, // minimum column step when animating

  // Accumulated pending scroll deltas to animate (lines/cols)
  pending_scroll_lines: f32,
  pending_scroll_cols:  f32,

  // Trackpad fractional scroll accumulation (separate from animation)
  trackpad_scroll_lines: f32,
  trackpad_scroll_cols:  f32,

  // Delta time tracking for time-based animations
  last_frame_time: std::time::Instant,

  // Throttle LocalSet polling to reduce overhead
  // Only poll every N frames to avoid blocking on every single frame
  frame_counter: u32,

  // Track whether the next key press should be treated as a meta prefix for terminal shortcuts
  terminal_meta_pending:          bool,
  // Track whether Ctrl+W was pressed in terminal, waiting for window command
  terminal_window_prefix_pending: bool,
}

impl App {
  pub fn new(
    editor: Editor,
    local_set: Rc<tokio::task::LocalSet>,
    runtime_handle: tokio::runtime::Handle,
    config_ptr: std::sync::Arc<arc_swap::ArcSwap<crate::core::config::Config>>,
  ) -> Self {
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
    // Use layout engine to position button in top-right corner
    use crate::core::layout::{
      Alignment,
      align,
    };
    let button_rect = align(area, 8, 2, Alignment::End);

    let button = Box::new(
      Button::new("Run")
                .with_rect(button_rect) // Layout-calculated position instead of hardcoded
                .color(the_editor_renderer::Color::rgb(0.6, 0.6, 0.8))
                .visible(false)
                .on_click(|| println!("Button clicked!")),
    );
    compositor.push(button);

    let conf = editor.config();
    Self {
      compositor,
      editor,
      config_ptr,
      jobs: Jobs::new(),
      input_handler: InputHandler::new(mode),
      local_set,
      runtime_handle,
      smooth_scroll_enabled: conf.smooth_scroll_enabled,
      scroll_lerp_factor: conf.scroll_lerp_factor,
      scroll_min_step_lines: conf.scroll_min_step_lines,
      scroll_min_step_cols: conf.scroll_min_step_cols,
      pending_scroll_lines: 0.0,
      pending_scroll_cols: 0.0,
      trackpad_scroll_lines: 0.0,
      trackpad_scroll_cols: 0.0,
      last_frame_time: std::time::Instant::now(),
      frame_counter: 0,
      terminal_meta_pending: false,
      terminal_window_prefix_pending: false,
    }
  }

  fn handle_config_events(&mut self, config_event: crate::editor::ConfigEvent) {
    use crate::editor::ConfigEvent;

    match config_event {
      ConfigEvent::Refresh => {
        // Reload configuration from disk
        if let Ok(new_config) = crate::core::config::Config::load_user() {
          // Store old config before updating
          let old_editor_config = self.editor.config().clone();

          // Store the new config in the global config pointer
          self
            .config_ptr
            .store(std::sync::Arc::new(new_config.clone()));

          // Update theme if specified
          if let Some(theme_name) = &new_config.theme {
            if let Ok(new_theme) = self.editor.theme_loader.load(theme_name) {
              self.editor.set_theme(new_theme);
            }
          } else {
            // Use default theme
            let default_theme = self
              .editor
              .theme_loader
              .default_theme(self.editor.config().true_color);
            self.editor.set_theme(default_theme);
          }

          // Refresh editor configuration
          self.editor.refresh_config(&old_editor_config);

          // Re-detect .editorconfig for all documents
          for doc in self.editor.documents.values_mut() {
            doc.detect_editor_config();
          }

          // Reset view positions for scrolloff changes
          let scrolloff = self.editor.config().scrolloff;
          for (view, _) in self.editor.tree.views() {
            if let Some(doc) = self.editor.documents.get_mut(&view.doc) {
              view.ensure_cursor_in_view(doc, scrolloff);
            }
          }

          self.editor.set_status("Configuration reloaded".to_string());
        } else {
          self
            .editor
            .set_status("Failed to reload configuration".to_string());
        }
      },
      ConfigEvent::Update(_new_config) => {
        // Configuration update already applied
        self.editor.set_status("Configuration updated".to_string());
      },
    }
  }

  fn handle_language_server_message(
    &mut self,
    server_id: crate::lsp::LanguageServerId,
    call: crate::lsp::Call,
  ) {
    use crate::lsp::{
      Call,
      MethodCall,
      Notification,
    };

    match call {
      Call::Notification(notification) => {
        // Parse and handle the notification
        match Notification::parse(&notification.method, notification.params) {
          Ok(notification) => {
            match notification {
              crate::lsp::Notification::PublishDiagnostics(params) => {
                let uri = match crate::core::uri::Uri::try_from(&params.uri) {
                  Ok(uri) => uri,
                  Err(err) => {
                    log::error!("Invalid URI in PublishDiagnostics: {:?}", err);
                    return;
                  },
                };

                // Convert LSP diagnostics to internal diagnostics
                use crate::core::diagnostics::DiagnosticProvider;
                let provider = DiagnosticProvider::Lsp {
                  server_id,
                  identifier: None,
                };

                self.editor.handle_lsp_diagnostics(
                  &provider,
                  uri,
                  params.version,
                  params.diagnostics,
                );
              },
              crate::lsp::Notification::ProgressMessage(params) => {
                use crate::lsp::lsp::types::ProgressParamsValue;
                match params.value {
                  ProgressParamsValue::WorkDone(progress) => {
                    use crate::lsp::lsp::types::WorkDoneProgress;
                    match progress {
                      WorkDoneProgress::Begin(work) => {
                        self
                          .editor
                          .lsp_progress
                          .begin(server_id, params.token, work);
                      },
                      WorkDoneProgress::Report(work) => {
                        self
                          .editor
                          .lsp_progress
                          .update(server_id, params.token, work);
                      },
                      WorkDoneProgress::End(_work) => {
                        self
                          .editor
                          .lsp_progress
                          .end_progress(server_id, &params.token);
                      },
                    }
                  },
                }
              },
              _ => {
                log::debug!("Received LSP notification: {:?}", notification);
              },
            }
          },
          Err(crate::lsp::Error::Unhandled) => {
            log::info!("Ignoring unhandled notification from language server");
          },
          Err(err) => {
            log::error!("Failed to parse LSP notification: {:?}", err);
          },
        }
      },
      Call::MethodCall(method_call) => {
        let crate::lsp::jsonrpc::MethodCall {
          method, params, id, ..
        } = method_call;

        // Parse the method call and generate a reply
        let reply = match MethodCall::parse(&method, params) {
          Err(crate::lsp::Error::Unhandled) => {
            log::error!(
              "Language server method not found: {} (id: {:?})",
              method,
              id
            );
            Err(crate::lsp::jsonrpc::Error {
              code:    crate::lsp::jsonrpc::ErrorCode::MethodNotFound,
              message: format!("Method not found: {}", method),
              data:    None,
            })
          },
          Err(err) => {
            log::error!(
              "Failed to parse language server method call {}: {:?}",
              method,
              err
            );
            Err(crate::lsp::jsonrpc::Error {
              code:    crate::lsp::jsonrpc::ErrorCode::ParseError,
              message: format!("Malformed method call: {}", method),
              data:    None,
            })
          },
          Ok(MethodCall::WorkDoneProgressCreate(params)) => {
            // Track the progress creation
            self.editor.lsp_progress.create(server_id, params.token);
            Ok(serde_json::Value::Null)
          },
          Ok(MethodCall::ApplyWorkspaceEdit(params)) => {
            // Get the language server to get its offset encoding
            if let Some(language_server) = self.editor.language_server_by_id(server_id) {
              let offset_encoding = language_server.offset_encoding();
              let result = self
                .editor
                .apply_workspace_edit(offset_encoding, &params.edit);

              use crate::lsp::lsp::types::ApplyWorkspaceEditResponse;
              Ok(
                serde_json::to_value(ApplyWorkspaceEditResponse {
                  applied:        result.is_ok(),
                  failure_reason: result.as_ref().err().map(|err| err.kind.to_string()),
                  failed_change:  result
                    .as_ref()
                    .err()
                    .map(|err| err.failed_change_idx as u32),
                })
                .unwrap(),
              )
            } else {
              Err(crate::lsp::jsonrpc::Error {
                code:    crate::lsp::jsonrpc::ErrorCode::InvalidRequest,
                message: "Language server not found".to_string(),
                data:    None,
              })
            }
          },
          Ok(MethodCall::WorkspaceConfiguration(params)) => {
            if let Some(language_server) = self.editor.language_server_by_id(server_id) {
              let result: Vec<_> = params
                .items
                .iter()
                .map(|item| {
                  let mut config = language_server.config()?;
                  if let Some(section) = item.section.as_ref() {
                    // Some LSPs send an empty string (e.g., vscode-eslint-language-server)
                    if !section.is_empty() {
                      for part in section.split('.') {
                        config = config.get(part)?;
                      }
                    }
                  }
                  Some(config.clone())
                })
                .collect();
              Ok(serde_json::to_value(result).unwrap())
            } else {
              Err(crate::lsp::jsonrpc::Error {
                code:    crate::lsp::jsonrpc::ErrorCode::InvalidRequest,
                message: "Language server not found".to_string(),
                data:    None,
              })
            }
          },
          Ok(MethodCall::RegisterCapability(_params)) => {
            // For now, just acknowledge capability registration
            // TODO: Actually register file watchers, etc.
            log::debug!("Language server registered capability (not yet fully implemented)");
            Ok(serde_json::Value::Null)
          },
          Ok(MethodCall::UnregisterCapability(_params)) => {
            // For now, just acknowledge capability unregistration
            log::debug!("Language server unregistered capability");
            Ok(serde_json::Value::Null)
          },
          Ok(_) => {
            // Other method calls we don't handle yet
            log::warn!("Unimplemented language server method: {}", method);
            Err(crate::lsp::jsonrpc::Error {
              code:    crate::lsp::jsonrpc::ErrorCode::MethodNotFound,
              message: format!("Method not implemented: {}", method),
              data:    None,
            })
          },
        };

        // Send the reply back to the language server
        if let Some(language_server) = self.editor.language_server_by_id(server_id) {
          if let Err(err) = language_server.reply(id, reply) {
            log::error!("Failed to send reply to language server: {:?}", err);
          }
        }
      },
      Call::Invalid { id } => {
        log::error!("Received invalid LSP call with id: {:?}", id);
      },
    }
  }
}

fn terminal_toggle_target(code: Key) -> Option<TerminalPane> {
  match code {
    Key::Char('j') | Key::Char('J') => Some(TerminalPane::Bottom),
    Key::Char('l') | Key::Char('L') => Some(TerminalPane::Right),
    _ => None,
  }
}

/// Convert InputEvent to bytes for terminal PTY
///
/// This handles special keys and VT100 escape sequences.
/// Returns None if the event should not be sent to terminal.
fn input_event_to_terminal_bytes(event: &InputEvent) -> Option<Vec<u8>> {
  match event {
    InputEvent::Keyboard(key_press) => {
      // Only handle key presses, not releases
      if !key_press.pressed {
        return None;
      }

      // DEBUG: Log the raw KeyPress we received
      log::debug!(
        "input_event_to_terminal_bytes: key={:?}, shift={}, ctrl={}, alt={}",
        key_press.code,
        key_press.shift,
        key_press.ctrl,
        key_press.alt
      );

      // Set up terminal modes for encoding
      let modes = crate::key_encode::TerminalModes {
        cursor_key_application: false,
        keypad_application:     false,
        modify_other_keys:      0,
        kitty_flags:            0,
        alt_esc_prefix:         true,
      };

      // Use the comprehensive key encoder
      let bytes = crate::key_encode::encode(key_press, &modes);

      // DEBUG: Log encoded bytes
      log::debug!(
        "input_event_to_terminal_bytes encoded {} bytes: {:?}",
        bytes.len(),
        bytes
      );

      if bytes.is_empty() { None } else { Some(bytes) }
    },
    InputEvent::Text(text) => {
      // Handle composed text from dead keys (e.g., " + space = ")
      // and IME input
      log::debug!("input_event_to_terminal_bytes: text=\"{}\"", text);
      Some(text.as_bytes().to_vec())
    },
    _ => None, // Mouse events, scroll, etc. not handled yet
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
    // Only do this if a view is focused (not a terminal or container)
    use crate::core::selection::Selection;
    if crate::focus_is_view!(self.editor) {
      let (view, doc) = crate::current!(self.editor);
      doc.set_selection(view.id, Selection::point(0));
    }
  }

  fn render(&mut self, renderer: &mut Renderer) {
    the_editor_event::start_frame();

    // The renderer's begin_frame/end_frame are handled by the main loop.
    // We just need to draw our content here.

    // Increment frame counter
    self.frame_counter = self.frame_counter.wrapping_add(1);

    // Process pending terminal responses and check for exits
    // This sends queued responses (e.g., cursor position reports) back to the shell
    let mut exited_terminals = Vec::new();
    for (view_id, terminal) in self.editor.tree.terminals() {
      terminal.session.borrow_mut().process_responses();

      // Check if terminal has exited
      if !terminal.session.borrow().is_alive() {
        exited_terminals.push(view_id);
      }
    }

    // Clean up exited terminals
    for terminal_id in exited_terminals {
      self.handle_terminal_exit(terminal_id);
    }

    // Handle pending actions from the editor (e.g., spawn terminal)
    if let Some(action) = self.editor.pending_action.take() {
      self.handle_pending_action(action);
    }

    // Process any pending config events
    while let Some(config_event) = self.editor.try_poll_config_event() {
      self.handle_config_events(config_event);
    }

    // Process any pending LSP messages
    while let Some((server_id, call)) = self.editor.try_poll_lsp_message() {
      self.handle_language_server_message(server_id, call);
    }

    // Process any pending saves
    while let Some(save_event) = self.editor.try_poll_save() {
      if let Some(doc) = self.editor.documents.get_mut(&save_event.doc_id) {
        doc.set_last_saved_revision(save_event.revision, save_event.save_time);
      }
    }

    // Process any pending job callbacks before rendering
    while let Ok(callback) = self.jobs.callbacks.try_recv() {
      self
        .jobs
        .handle_callback(&mut self.editor, &mut self.compositor, Ok(Some(callback)));
    }

    // Process any pending local job callbacks (for !Send futures like ACP)
    // Drain all callbacks from the Vec
    let local_cbs = self
      .jobs
      .local_callbacks
      .borrow_mut()
      .drain(..)
      .collect::<Vec<_>>();
    for callback in local_cbs {
      self
        .jobs
        .handle_local_callback(&mut self.editor, &mut self.compositor, Ok(Some(callback)));
    }

    // Process any pending ACP session notifications
    self.process_acp_notifications();

    // Process any pending status messages
    while let Ok(status) = self.jobs.status_messages.try_recv() {
      self.editor.set_status(status.message.to_string());
    }

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

    // Update split animations
    let _split_animating = self.editor.tree.update_animations(dt);

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
    // If a terminal is focused, route keyboard input directly to it
    let focus_id = self.editor.tree.focus;
    if let Some(terminal) = self.editor.tree.get_terminal_mut(focus_id) {
      if let InputEvent::Keyboard(key_press) = &event {
        if !key_press.pressed {
          return true;
        }

        let mut prepend_escape = false;

        // Handle Ctrl+W prefix for window navigation (h/j/k/l)
        if self.terminal_window_prefix_pending {
          self.terminal_window_prefix_pending = false;

          // Check for window navigation keys (h/j/k/l or arrow keys)
          use crate::core::tree::Direction;
          let direction = match key_press.code {
            Key::Char('h') | Key::Left => Some(Direction::Left),
            Key::Char('j') | Key::Down => Some(Direction::Down),
            Key::Char('k') | Key::Up => Some(Direction::Up),
            Key::Char('l') | Key::Right => Some(Direction::Right),
            _ => None,
          };

          if let Some(dir) = direction {
            self.editor.focus_direction(dir);
            return true;
          }

          // If not a window navigation key, fall through to send to terminal
        }

        if self.terminal_meta_pending {
          self.terminal_meta_pending = false;
          if let Some(pane) = terminal_toggle_target(key_press.code) {
            self.editor.toggle_terminal_pane(pane);
            return true;
          }
          prepend_escape = true;
        }

        // Intercept Ctrl+W to enable window commands from terminal
        if key_press.code == Key::Char('w') && key_press.ctrl && !key_press.alt && !key_press.shift
        {
          self.terminal_window_prefix_pending = true;
          return true;
        }

        // Use Ctrl+Space as the terminal meta prefix instead of ESC
        // This allows ESC to be sent directly to terminal apps (vim, helix, etc.)
        if key_press.code == Key::Char(' ') && key_press.ctrl && !key_press.alt && !key_press.shift
        {
          self.terminal_meta_pending = true;
          return true;
        }

        // Old ESC-based shortcuts disabled to allow ESC in terminal apps:
        // if key_press.code == Key::Escape && !key_press.alt && !key_press.ctrl &&
        // !key_press.shift {   self.terminal_meta_pending = true;
        //   return true;
        // }

        if key_press.alt {
          if let Some(pane) = terminal_toggle_target(key_press.code) {
            self.editor.toggle_terminal_pane(pane);
            return true;
          }
        }

        if let Some(mut bytes) = input_event_to_terminal_bytes(&event) {
          if prepend_escape {
            let mut with_escape = Vec::with_capacity(bytes.len() + 1);
            with_escape.push(0x1B);
            with_escape.extend(bytes);
            bytes = with_escape;
          }

          let result = {
            let session_ref = terminal.session.borrow();
            session_ref.send_input(bytes)
          };
          match result {
            Ok(()) => {
              terminal.session.borrow().mark_needs_redraw();
              the_editor_event::request_redraw();
            },
            Err(e) => {
              log::error!("Failed to write to terminal PTY: {}", e);
            },
          }
        }

        return true;
      }

      if let Some(bytes) = input_event_to_terminal_bytes(&event) {
        let result = {
          let session_ref = terminal.session.borrow();
          session_ref.send_input(bytes)
        };
        match result {
          Ok(()) => {
            terminal.session.borrow().mark_needs_redraw();
            the_editor_event::request_redraw();
          },
          Err(e) => {
            log::error!("Failed to write to terminal PTY: {}", e);
          },
        }
        return true;
      }
    } else {
      self.terminal_meta_pending = false;
      self.terminal_window_prefix_pending = false;
    }

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

    // Keep redrawing while split animations are active.
    if self.editor.tree.has_active_animations() {
      return true;
    }

    // Keep redrawing while any LSP is loading (for breathing animation).
    if self.editor.lsp_progress.has_active_progress() {
      return true;
    }

    // Keep redrawing while a scroll animation is active.
    // Use same threshold as animate_scroll to prevent micro-animations
    if self.smooth_scroll_enabled
      && (self.pending_scroll_lines.abs() > 0.1 || self.pending_scroll_cols.abs() > 0.1)
    {
      return true;
    }

    // Check if any terminal needs redraw
    // Terminals continuously process PTY output in dedicated threads, so we need
    // to keep redrawing while any terminal is alive
    for (_, terminal) in self.editor.tree.terminals() {
      let session = terminal.session.borrow();
      if session.needs_redraw() {
        return true;
      }
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
  fn handle_pending_action(&mut self, action: crate::editor::Action) {
    use crate::editor::Action;

    match action {
      Action::SpawnTerminal => {
        use the_terminal::TerminalSession;

        // Get next terminal ID
        let id = self.editor.next_terminal_id;
        self.editor.next_terminal_id += 1;

        // Spawn terminal session with default dimensions (80x24)
        match TerminalSession::new(24, 80, None) {
          Ok(mut session) => {
            // Apply configured max FPS from editor config
            if let Some(terminal_config) = &self.editor.config().terminal {
              session.set_max_fps(terminal_config.max_fps);
            }

            session.set_redraw_notifier(Some(Arc::new(|| the_editor_event::request_redraw())));

            let session = Rc::new(RefCell::new(session));
            // Insert terminal into the tree
            self.editor.tree.insert_terminal(session, id);
            self.editor.set_status("Terminal spawned in split");
          },
          Err(e) => {
            self
              .editor
              .set_error(format!("Failed to spawn terminal: {}", e));
          },
        }
      },
      Action::SpawnTerminalInPane { mut anchor, pane } => {
        use the_terminal::TerminalSession;

        if !self.editor.tree.contains(anchor) {
          anchor = match self.editor.focused_view_id() {
            Some(id) => id,
            None => {
              self
                .editor
                .set_error("No view available to host a terminal split");
              return;
            },
          };
        }

        let id = self.editor.next_terminal_id;
        self.editor.next_terminal_id += 1;

        match TerminalSession::new(24, 80, None) {
          Ok(mut session) => {
            // Apply configured max FPS from editor config
            if let Some(terminal_config) = &self.editor.config().terminal {
              session.set_max_fps(terminal_config.max_fps);
            }

            session.set_redraw_notifier(Some(Arc::new(|| the_editor_event::request_redraw())));

            let session = Rc::new(RefCell::new(session));
            match self
              .editor
              .tree
              .insert_terminal_pane(anchor, session, id, pane.layout())
            {
              Some(view_id) => {
                self.editor.register_terminal_pane(pane, view_id);
                self
                  .editor
                  .set_status(format!("Opened {}", pane.display_name()));
              },
              None => {
                self
                  .editor
                  .set_error("Failed to place terminal in requested split");
              },
            }
          },
          Err(e) => {
            self
              .editor
              .set_error(format!("Failed to spawn terminal: {}", e));
          },
        }
      },
      Action::ReplaceViewWithTerminal { view_id } => {
        use the_terminal::TerminalSession;

        // Get the document ID before we replace the view
        let doc_id = if let Some(view) = self.editor.tree.try_get(view_id) {
          Some(view.doc)
        } else {
          self.editor.set_error("View not found");
          return;
        };

        let terminal_id = self.editor.next_terminal_id;
        self.editor.next_terminal_id += 1;

        match TerminalSession::new(24, 80, None) {
          Ok(mut session) => {
            // Apply configured max FPS from editor config
            if let Some(terminal_config) = &self.editor.config().terminal {
              session.set_max_fps(terminal_config.max_fps);
            }

            session.set_redraw_notifier(Some(Arc::new(|| the_editor_event::request_redraw())));

            let session = Rc::new(RefCell::new(session));
            match self
              .editor
              .tree
              .replace_view_with_terminal(view_id, session, terminal_id)
            {
              Some(_terminal_view_id) => {
                // Remove view data from the document
                if let Some(doc_id) = doc_id {
                  if let Some(doc) = self.editor.documents.get_mut(&doc_id) {
                    doc.remove_view(view_id);
                  }
                }
                self.editor.set_status("Opened terminal");
              },
              None => {
                self
                  .editor
                  .set_error("Failed to replace view with terminal");
              },
            }
          },
          Err(e) => {
            self
              .editor
              .set_error(format!("Failed to spawn terminal: {}", e));
          },
        }
      },
      _ => {
        // Other actions not yet implemented
      },
    }
  }

  fn handle_terminal_exit(&mut self, terminal_id: ViewId) {
    // Check if this is a tracked terminal pane
    if let Some(pane) = self.editor.terminal_pane_for_view(terminal_id) {
      // Tracked pane terminal exited
      self.editor.clear_terminal_pane(pane);
      self.editor.tree.remove(terminal_id);

      // Focus another view if available
      if let Some(fallback_view) = self.editor.focused_view_id() {
        self.editor.tree.focus = fallback_view;
      }

      self
        .editor
        .set_status(format!("{} exited", pane.display_name()));
      return;
    }

    // Fullscreen terminal - try to restore the original document
    if let Some(terminal) = self.editor.tree.get_terminal(terminal_id) {
      if let Some(doc_id) = terminal.replaced_doc {
        // Restore the original document
        if self.editor.documents.contains_key(&doc_id) {
          match self
            .editor
            .tree
            .replace_terminal_with_view(terminal_id, doc_id)
          {
            Some(view_id) => {
              // Initialize view data in the document
              if let Some(doc) = self.editor.documents.get_mut(&doc_id) {
                doc.ensure_view_init(view_id);
              }

              self
                .editor
                .set_status("Terminal exited, restored buffer".to_string());
              return;
            },
            None => {
              self
                .editor
                .set_error("Failed to restore buffer after terminal exit".to_string());
            },
          }
        }
      }
    }

    // Fallback: just remove the terminal
    self.editor.tree.remove(terminal_id);

    // Focus another view if available
    if let Some(fallback_view) = self.editor.focused_view_id() {
      self.editor.tree.focus = fallback_view;
    }

    self.editor.set_status("Terminal exited".to_string());
  }

  fn handle_scroll(&mut self, delta: ScrollDelta, renderer: &mut Renderer) {
    match delta {
      // Mouse wheel: discrete line-based scrolling
      // Use smooth scrolling animation for these
      ScrollDelta::Lines { x, y } => {
        let config_lines = self.editor.config().scroll_lines.max(1) as f32;
        let d_cols = -x * 4.0;
        let d_lines = -y * config_lines;

        // Accumulate into pending animation deltas for smooth scrolling
        self.pending_scroll_lines += d_lines;
        self.pending_scroll_cols += d_cols;

        // Nudge a redraw loop
        the_editor_event::request_redraw();
      },

      // Trackpad: continuous pixel-based scrolling
      // Already smooth from OS, accumulate fractional lines and apply when ready
      ScrollDelta::Pixels { x, y } => {
        let line_h = renderer.cell_height().max(1.0);
        let col_w = renderer.cell_width().max(1.0);

        // Apply same multiplier as mouse wheel for consistent scroll speed
        let config_lines = self.editor.config().scroll_lines.max(1) as f32;
        let d_cols = (-x / col_w) * 4.0; // Same horizontal multiplier as mouse wheel
        let d_lines = (-y / line_h) * config_lines;

        // Accumulate fractional scrolling
        self.trackpad_scroll_lines += d_lines;
        self.trackpad_scroll_cols += d_cols;

        // Extract integer part to scroll
        let lines_to_scroll = self.trackpad_scroll_lines.trunc() as i32;
        let cols_to_scroll = self.trackpad_scroll_cols.trunc() as i32;

        // Keep fractional remainder for next event
        self.trackpad_scroll_lines -= lines_to_scroll as f32;
        self.trackpad_scroll_cols -= cols_to_scroll as f32;

        // Apply accumulated scroll immediately if we have at least 1 line/col
        if lines_to_scroll != 0 || cols_to_scroll != 0 {
          self.apply_scroll_immediate(lines_to_scroll, cols_to_scroll);
        }
      },
    }
  }

  /// Apply scroll immediately without animation (for trackpad)
  fn apply_scroll_immediate(&mut self, lines: i32, cols: i32) {
    use crate::core::movement::Direction;

    if lines != 0 {
      let direction = if lines > 0 {
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

      commands::scroll(&mut cmd_cx, lines.unsigned_abs() as usize, direction, false);
    }

    if cols != 0 {
      let focus_view = self.editor.tree.focus;
      // Only scroll if focused on a view, not a terminal
      if let Some(view) = self.editor.tree.try_get(focus_view) {
        let doc_id = view.doc;
        let doc = self.editor.documents.get_mut(&doc_id).unwrap();
        let mut vp = doc.view_offset(focus_view);

        if cols >= 0 {
          vp.horizontal_offset = vp.horizontal_offset.saturating_add(cols as usize);
        } else {
          vp.horizontal_offset = vp.horizontal_offset.saturating_sub((-cols) as usize);
        }

        doc.set_view_offset(focus_view, vp);
      }
    }
  }

  fn animate_scroll(&mut self, _renderer: &mut Renderer) {
    // Vertical: apply a fraction of pending lines via commands::scroll
    let apply_axis = |pending: &mut f32| -> i32 {
      let remaining = *pending;
      // Use higher threshold to stop micro-animations faster
      if remaining.abs() < 0.1 {
        // Close enough to zero, snap to zero and stop animating
        *pending = 0.0;
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
    if remaining_h.abs() >= 0.1 {
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

      // Apply to focused view (only if it's a view, not a terminal)
      let focus_view = self.editor.tree.focus;
      if let Some(view) = self.editor.tree.try_get(focus_view) {
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
      }
      self.pending_scroll_cols -= step_i as f32;
    } else {
      // Below threshold, snap to zero to stop animation
      self.pending_scroll_cols = 0.0;
    }
  }

  fn process_acp_notifications(&mut self) {
    use crate::acp::{
      SessionNotification,
      acp::SessionUpdate,
    };

    // Drain all notifications from the queue
    let notifications = self
      .editor
      .acp_sessions
      .notifications()
      .borrow_mut()
      .drain(..)
      .collect::<Vec<_>>();

    if !notifications.is_empty() {
      log::info!("ACP App: Processing {} notifications", notifications.len());
    }

    // Process each notification
    for notification in notifications {
      let SessionNotification { session_id, update } = notification;
      log::info!(
        "ACP App: Processing notification for session {:?}",
        session_id
      );

      // Convert the update into a message and append to document
      match update {
        SessionUpdate::AgentMessageChunk { content } => {
          let text = Self::extract_content_text(&content);
          log::info!("ACP App: AgentMessageChunk - {} chars", text.len());

          // Update session state to Streaming
          let registry = self.editor.acp_sessions.handle();
          let session_id_clone = session_id.clone();
          self.jobs.callback_local(async move {
            use crate::acp::session::SessionState;
            let _ = registry
              .update_session(&session_id_clone, |session| {
                session.state = SessionState::Streaming;
              })
              .await;
            Ok(None)
          });

          // Append to document for this session (this will also update gutter state)
          self.append_to_session_document(
            &session_id,
            &text,
            crate::acp::session::MessageRole::Agent,
          );
        },
        SessionUpdate::UserMessageChunk { content } => {
          let text = Self::extract_content_text(&content);
          log::info!("ACP App: UserMessageChunk - {} chars", text.len());
          // Could track user messages too, but for now we focus on agent messages
          log::debug!("User message chunk: {}", text);
        },
        SessionUpdate::AgentThoughtChunk { content } => {
          // Update session state to Thinking
          let registry = self.editor.acp_sessions.handle();
          let session_id_clone = session_id.clone();
          self.jobs.callback_local(async move {
            use crate::acp::session::SessionState;
            let _ = registry
              .update_session(&session_id_clone, |session| {
                session.state = SessionState::Thinking;
              })
              .await;
            Ok(None)
          });

          // Append thinking to document (this will also update gutter state)
          let thought = Self::extract_content_text(&content);
          log::info!("ACP App: AgentThoughtChunk - {} chars", thought.len());
          self.append_to_session_document(
            &session_id,
            &format!("[Thinking: {}]\n", thought),
            crate::acp::session::MessageRole::Thinking,
          );
        },
        SessionUpdate::ToolCall(tool_call) => {
          // Update session state to ExecutingTool
          let registry = self.editor.acp_sessions.handle();
          let session_id_clone = session_id.clone();
          let tool_name = tool_call.title.clone();
          self.jobs.callback_local(async move {
            use crate::acp::session::SessionState;
            let _ = registry
              .update_session(&session_id_clone, |session| {
                session.state = SessionState::ExecutingTool;
                session.current_tool_name = Some(tool_name.clone());
              })
              .await;
            Ok(None)
          });

          let text = format!("[Tool Call: {}]\n", tool_call.title);
          log::info!("ACP App: ToolCall - {}", tool_call.title);
          // Append to document (this will also update gutter state with tool name)
          self.append_to_session_document(
            &session_id,
            &text,
            crate::acp::session::MessageRole::Tool,
          );
        },
        SessionUpdate::ToolCallUpdate(_) | SessionUpdate::Plan(_) => {
          // These could be rendered specially in the future
          log::debug!("ACP App: Received session update: {:?}", update);
        },
        SessionUpdate::CurrentModeUpdate { .. } | SessionUpdate::AvailableCommandsUpdate { .. } => {
          // Metadata updates
          log::debug!("ACP App: Received metadata update: {:?}", update);
        },
      }
    }
  }

  fn extract_content_text(content: &crate::acp::acp::ContentBlock) -> String {
    use crate::acp::acp::ContentBlock;
    match content {
      ContentBlock::Text(text_content) => text_content.text.clone(),
      ContentBlock::Image(_) => "<image>".into(),
      ContentBlock::Audio(_) => "<audio>".into(),
      ContentBlock::ResourceLink(resource_link) => format!("<link: {}>", resource_link.uri),
      ContentBlock::Resource(_) => "<resource>".into(),
    }
  }

  fn append_to_session_document(
    &mut self,
    session_id: &crate::acp::acp::SessionId,
    text: &str,
    role: crate::acp::session::MessageRole,
  ) {
    log::info!(
      "ACP App: append_to_session_document - {} chars for session {:?}",
      text.len(),
      session_id
    );

    // We need to look up the doc_id synchronously
    // Since RegistryState is behind an async Mutex, we can't directly access it
    // here Instead, we'll spawn a callback_local job that can do the async
    // lookup and append
    let registry = self.editor.acp_sessions.handle();
    let session_id_clone = session_id.clone();
    let text_owned = text.to_string();

    // Spawn a local callback to append text
    self.jobs.callback_local(async move {
      log::info!("ACP App: Inside append callback_local, looking up doc_id");
      // Get the document ID for this session
      if let Some(doc_id) = registry.get_doc_id_by_session(&session_id_clone).await {
        log::info!(
          "ACP App: Found doc_id {:?}, returning editor callback",
          doc_id
        );

        // Clone registry for the callback
        let registry_for_callback = registry.clone();
        let session_id_for_callback = session_id_clone.clone();

        // Return a callback to append text to the document
        Ok(Some(crate::ui::job::LocalCallback::EditorCompositor(
          Box::new(move |editor, _compositor| {
            use crate::core::{
              selection::Selection,
              transaction::Transaction,
            };

            log::info!(
              "ACP App: Inside editor callback, appending to doc {:?}",
              doc_id
            );

            // Get the document and append text
            if let Some(doc) = editor.documents.get_mut(&doc_id) {
              // Get the current byte position (start of new content)
              let start_byte = doc.text().len_bytes();
              log::info!("ACP App: Starting append at byte {}", start_byte);

              // Get the end of the document in chars
              let len = doc.text().len_chars();
              log::info!(
                "ACP App: Document has {} chars, appending {} chars",
                len,
                text_owned.len()
              );

              // Find a view that's displaying this document
              let view_id = editor
                .tree
                .views()
                .find(|(view, _)| view.doc == doc_id)
                .map(|(view, _)| view.id)
                .unwrap_or_else(|| {
                  // If no view is found, get any view ID from the document's selections
                  doc.selections().keys().next().copied().unwrap_or_default()
                });

              log::info!("ACP App: Using view_id {:?} for append", view_id);

              // Create a selection at the end of the document
              let selection = Selection::single(len, len);
              // Insert text at the end
              let transaction =
                Transaction::insert(doc.text(), &selection, text_owned.clone().into());
              doc.apply(&transaction, view_id);

              // Get the end byte position after appending
              let end_byte = doc.text().len_bytes();
              log::info!(
                "ACP App: Text appended successfully, end byte: {}",
                end_byte
              );

              // Store the message span directly in the document for rendering
              doc.acp_message_spans.push((role, start_byte..end_byte));
              log::debug!(
                "ACP: Added message span {:?} from {} to {} in document",
                role,
                start_byte,
                end_byte
              );

              // Calculate current line number for gutter display
              let current_line = doc.text().char_to_line(doc.text().byte_to_char(start_byte));
              log::debug!("ACP: Current line for gutter: {}", current_line);

              // Update document's gutter state directly based on the role
              // Map role to session state for gutter display
              use crate::acp::session::{
                MessageRole,
                SessionState,
              };
              let session_state = match role {
                MessageRole::Agent => SessionState::Streaming,
                MessageRole::Thinking => SessionState::Thinking,
                MessageRole::Tool => SessionState::ExecutingTool,
                MessageRole::User => SessionState::Idle, // Shouldn't happen but handle it
              };

              doc.acp_gutter_state = Some(crate::core::document::AcpGutterState {
                state:        session_state,
                current_line: Some(current_line),
                tool_name:    None, // We don't have tool name here, will be updated separately
              });

              // Also update the session's message spans and current_line for persistence
              let registry_clone = registry_for_callback.clone();
              let session_id_clone = session_id_for_callback.clone();
              tokio::task::spawn_local(async move {
                let _ = registry_clone
                  .update_session(&session_id_clone, |session| {
                    session
                      .message_spans
                      .push(crate::acp::session::MessageSpan {
                        role,
                        start_byte,
                        end_byte,
                      });
                    session.current_line = Some(current_line);
                  })
                  .await;
              });
            } else {
              log::warn!("ACP App: Document {:?} not found in editor", doc_id);
            }
          }),
        )))
      } else {
        log::warn!(
          "ACP App: No doc_id found for session {:?}",
          session_id_clone
        );
        Ok(None)
      }
    });
  }
}

impl Drop for App {
  fn drop(&mut self) {
    log::debug!("App dropping, cleaning up terminals");

    // Kill all terminal sessions to ensure clean shutdown
    // This prevents hanging on PTY read threads during runtime shutdown
    let terminals: Vec<_> = self.editor.tree.terminals().map(|(id, _)| id).collect();

    if !terminals.is_empty() {
      log::debug!("Killing {} terminal session(s)", terminals.len());

      for terminal_id in terminals {
        if let Some(terminal) = self.editor.tree.get_terminal_mut(terminal_id) {
          // Use the runtime handle to block on the async kill operation
          if let Err(e) = self
            .runtime_handle
            .block_on(terminal.session.borrow_mut().kill())
          {
            log::warn!("Failed to kill terminal {:?}: {}", terminal_id, e);
          }
        }
      }

      log::debug!("Terminal cleanup complete");
    }
  }
}
