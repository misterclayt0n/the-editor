use std::{
  collections::HashMap,
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
    commands,
    graphics::Rect,
    movement::Direction,
  },
  editor::Editor,
  input::InputHandler,
  keymap::{
    KeyBinding,
    KeyTrie,
    Keymaps,
    Mode,
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
  pub config_ptr: Arc<arc_swap::ArcSwap<crate::core::config::Config>>,

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
}

impl App {
  pub fn new(
    editor: Editor,
    runtime_handle: tokio::runtime::Handle,
    config_ptr: Arc<arc_swap::ArcSwap<crate::core::config::Config>>,
  ) -> Self {
    let area = Rect::new(0, 0, 120, 40); // Default size, will be updated on resize.
    let mut compositor = Compositor::new(area);

    let mode = editor.mode;

    let keymaps = Keymaps::new(editor.keymaps.map.clone());
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
    }
  }

  fn handle_config_events(&mut self, config_event: crate::editor::ConfigEvent) {
    use crate::editor::ConfigEvent;

    match config_event {
      ConfigEvent::Refresh => {
        // Reload configuration from disk
        match crate::core::config::Config::load_user() {
          Ok(new_config) => {
            // Store old config before updating
            let old_editor_config = self.editor.config().clone();

            // Store the new config in the global config pointer
            self.config_ptr.store(Arc::new(new_config.clone()));

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
            self.apply_keymaps(&new_config.keys);

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
          },
          Err(err) => {
            println!("Failed to reload configuration: {err:?}");
            self
              .editor
              .set_status("Failed to reload configuration".to_string());
          },
        }
      },
      ConfigEvent::Update(_new_config) => {
        // Configuration update already applied
        self.editor.set_status("Configuration updated".to_string());
      },
    }
  }

  fn apply_keymaps(&mut self, keys: &HashMap<Mode, KeyTrie>) {
    self.editor.set_keymaps(keys);
    for layer in self.compositor.layers.iter_mut() {
      if let Some(editor_view) = layer.as_any_mut().downcast_mut::<EditorView>() {
        editor_view.set_keymaps(keys);
      }
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
    // Only do this if a view is focused.
    use crate::core::selection::Selection;
    if crate::focus_is_view!(self.editor) {
      let (view, doc) = crate::current!(self.editor);
      doc.set_selection(view.id, Selection::point(0));
    }
  }

  fn render(&mut self, renderer: &mut Renderer) {
    the_editor_event::start_frame();

    // Clear needs_redraw flag at the start of each frame.
    // It will be set again if something needs redrawing during this frame.
    self.editor.needs_redraw = false;

    // The renderer's begin_frame/end_frame are handled by the main loop.
    // We just need to draw our content here.

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

    // Process any pending status messages
    while let Ok(status) = self.jobs.status_messages.try_recv() {
      self.editor.set_status(status.message.to_string());
    }

    // Process any pending ACP events
    self.handle_acp_events();

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
        // handle_scroll returns whether immediate redraw is needed
        let needs_immediate_redraw = self.handle_scroll(scroll, _renderer);
        return needs_immediate_redraw;
      }
      // Compositor handled it, request redraw
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
  /// Process pending ACP events (streaming responses and permission requests).
  fn handle_acp_events(&mut self) {
    // Collect events first to avoid borrow issues
    let mut events = Vec::new();
    let mut permissions = Vec::new();

    if let Some(ref mut handle) = self.editor.acp {
      while let Some(event) = handle.try_recv_event() {
        events.push(event);
      }
      while let Some(permission) = handle.try_recv_permission() {
        permissions.push(permission);
      }
    }

    // Now process the collected events
    for event in events {
      match event {
        crate::acp::StreamEvent::TextChunk(text) => {
          // Append text to the ACP response state
          if let Some(ref mut state) = self.editor.acp_response {
            state.response_text.push_str(&text);
          }
          log::debug!("[ACP] Text chunk: {} chars", text.len());
        },
        crate::acp::StreamEvent::ToolCall { name, status } => {
          let status_msg = match status {
            crate::acp::ToolCallStatus::Started => format!("[ACP] Tool: {} starting...", name),
            crate::acp::ToolCallStatus::InProgress(msg) => {
              format!("[ACP] Tool: {} - {}", name, msg)
            },
            crate::acp::ToolCallStatus::Completed => format!("[ACP] Tool: {} completed", name),
            crate::acp::ToolCallStatus::Failed(err) => {
              format!("[ACP] Tool: {} failed: {}", name, err)
            },
          };
          self.editor.set_status(status_msg);
        },
        crate::acp::StreamEvent::Done => {
          // Mark streaming as complete
          if let Some(ref mut state) = self.editor.acp_response {
            state.is_streaming = false;
          }
          self
            .editor
            .set_status("[ACP] Response complete".to_string());
        },
        crate::acp::StreamEvent::Error(err) => {
          // Mark streaming as complete and append error to response
          if let Some(ref mut state) = self.editor.acp_response {
            state.is_streaming = false;
            state
              .response_text
              .push_str(&format!("\n\n**Error:** {}", err));
          }
          self.editor.set_error(format!("[ACP] Error: {}", err));
        },
        crate::acp::StreamEvent::ModelChanged(model_id) => {
          // Update stored model state and response state
          if let Some(ref mut handle) = self.editor.acp {
            handle.update_current_model(&model_id);
          }
          if let Some(ref mut state) = self.editor.acp_response {
            state.model_name = model_id.to_string();
          }
          self
            .editor
            .set_status(format!("[ACP] Model changed to: {}", model_id));
        },
      }
    }

    // Add collected permissions to the manager
    for permission in permissions {
      self.editor.acp_permissions.push(permission);
    }

    // If there are pending permissions, show a status message
    if let Some(msg) = self.editor.acp_permissions.status_message() {
      self.editor.set_status(format!("{} (y/n)", msg));
    }

    // Check for pending model selection from the picker
    if let Some(ref rx) = self.editor.pending_model_selection {
      if let Ok(model_id) = rx.try_recv() {
        if let Some(ref handle) = self.editor.acp {
          if let Err(e) = handle.set_session_model(model_id.clone()) {
            self.editor.set_error(format!("Failed to set model: {}", e));
          } else {
            self
              .editor
              .set_status(format!("Switching to model: {}...", model_id));
          }
        }
        // Clear the receiver after processing
        self.editor.pending_model_selection = None;
      }
    }
  }

  fn handle_scroll(&mut self, delta: ScrollDelta, renderer: &mut Renderer) -> bool {
    match delta {
      // Mouse wheel: discrete line-based scrolling with animation
      ScrollDelta::Lines { x, y } => {
        if Self::is_precision_line_scroll(x, y) {
          // Some backends (notably X11) report high-resolution touchpad input
          // as line deltas. Treat those like pixel scrolling to avoid
          // oscillation from the smooth-scroll animator.
          self.handle_precise_line_scroll(x, y);
          return false;
        }

        let config_lines = self.editor.config().scroll_lines.max(1) as f32;
        let d_cols = -x * 4.0;
        let d_lines = -y * config_lines;

        // Accumulate into pending animation deltas for smooth scrolling
        self.pending_scroll_lines += d_lines;
        self.pending_scroll_cols += d_cols;

        // Nudge a redraw loop
        the_editor_event::request_redraw();
        true // Request immediate redraw for smooth animation
      },

      // Trackpad: continuous pixel-based scrolling handled immediately
      ScrollDelta::Pixels { x, y } => {
        self.handle_precise_pixel_scroll(x, y, renderer);
        false // Don't request immediate redraw - let normal loop handle it
      },
    }
  }

  fn handle_precise_pixel_scroll(&mut self, x: f32, y: f32, renderer: &Renderer) {
    let line_h = renderer.cell_height().max(1.0);
    let col_w = renderer.cell_width().max(1.0);

    // Apply same multiplier as mouse wheel for consistent scroll speed
    let config_lines = self.editor.config().scroll_lines.max(1) as f32;
    let d_cols = (-x / col_w) * 4.0; // Same horizontal multiplier as mouse wheel
    let d_lines = (-y / line_h) * config_lines;

    self.accumulate_precise_scroll(d_lines, d_cols);
  }

  fn handle_precise_line_scroll(&mut self, x: f32, y: f32) {
    let config_lines = self.editor.config().scroll_lines.max(1) as f32;
    let d_cols = -x * 4.0;
    let d_lines = -y * config_lines;
    self.accumulate_precise_scroll(d_lines, d_cols);
  }

  fn accumulate_precise_scroll(&mut self, d_lines: f32, d_cols: f32) {
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
      // Mark editor as needing redraw, but don't request immediate redraw
      // to avoid flickering in X11 where scroll events come very frequently.
      // The normal redraw loop will pick this up.
      self.editor.needs_redraw = true;
    }
  }

  fn is_precision_line_scroll(x: f32, y: f32) -> bool {
    const EPSILON: f32 = 1e-3;
    let is_fractional = |value: f32| {
      if value == 0.0 {
        return false;
      }
      (value - value.round()).abs() > EPSILON
    };

    is_fractional(x) || is_fractional(y)
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
      // Only scroll if focused on a view
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

      // Apply to focused view (only if the focus is a view)
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
}
