///! Async completion request handler with debouncing
///!
///! This module handles completion requests from the editor, debounces them
///! appropriately, and coordinates LSP completion requests across multiple
///! language servers.

use std::time::Duration;

use the_editor_event::{
  AsyncHook,
  register_hook,
};
use the_editor_lsp_types::types as lsp;
use the_editor_stdx::rope::RopeSliceExt;
use tokio::time::Instant;

use super::{
  Handlers,
  completion::{
    CompletionEvent,
    CompletionItem,
    LspCompletionItem,
  },
};
use crate::{
  core::{
    DocumentId,
    ViewId,
  },
  editor::Editor,
  event::{
    OnModeSwitch,
    PostCommand,
    PostInsertChar,
  },
  keymap::Mode,
  lsp::{
    LanguageServerId,
    OffsetEncoding,
  },
  ui,
};

/// Debounce duration for auto-triggered completions
const AUTO_DEBOUNCE: Duration = Duration::from_millis(120);

/// Debounce duration for trigger character completions (much shorter)
const TRIGGER_CHAR_DEBOUNCE: Duration = Duration::from_millis(5);

/// Helper to assert a future is 'static
fn assert_static<F>(f: F) -> F
where
  F: std::future::Future + Send + 'static,
{
  f
}

/// Pending completion request
#[derive(Debug, Clone, Copy)]
struct PendingTrigger {
  cursor: usize,
  doc:    DocumentId,
  view:   ViewId,
  kind:   TriggerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerKind {
  Auto,
  TriggerChar,
  Manual,
}

/// Async hook for debouncing completion requests
pub struct CompletionRequestHook {
  /// The pending trigger (if any)
  pending_trigger: Option<PendingTrigger>,
  /// The currently in-flight trigger (being processed)
  in_flight:       Option<PendingTrigger>,
}

impl CompletionRequestHook {
  pub fn new() -> Self {
    Self {
      pending_trigger: None,
      in_flight:       None,
    }
  }

  /// Spawn the async hook and return its sender
  pub fn spawn(self) -> tokio::sync::mpsc::Sender<CompletionEvent> {
    the_editor_event::AsyncHook::spawn(self)
  }
}

impl AsyncHook for CompletionRequestHook {
  type Event = CompletionEvent;

  fn handle_event(&mut self, event: Self::Event, _timeout: Option<Instant>) -> Option<Instant> {
    log::info!("CompletionRequestHook received event: {:?}", event);
    match event {
      CompletionEvent::AutoTrigger { cursor, doc, view } => {
        // Only set trigger if doc/view match in-flight OR we have no in-flight
        // This prevents switching documents/views from triggering completions
        if self
          .pending_trigger
          .or(self.in_flight)
          .map_or(true, |trigger| trigger.doc == doc && trigger.view == view)
        {
          self.pending_trigger = Some(PendingTrigger {
            cursor,
            doc,
            view,
            kind: TriggerKind::Auto,
          });
          log::info!("Set auto-trigger, will fire in 120ms");
          Some(Instant::now() + AUTO_DEBOUNCE)
        } else {
          log::info!("Ignoring auto-trigger for different doc/view");
          None
        }
      }
      CompletionEvent::TriggerChar { cursor, doc, view } => {
        // Cancel any pending trigger and set new trigger char request
        self.pending_trigger = Some(PendingTrigger {
          cursor,
          doc,
          view,
          kind: TriggerKind::TriggerChar,
        });
        log::info!("Set trigger-char, will fire in 5ms");
        Some(Instant::now() + TRIGGER_CHAR_DEBOUNCE)
      }
      CompletionEvent::ManualTrigger { cursor, doc, view } => {
        // No debounce for manual triggers
        self.pending_trigger = Some(PendingTrigger {
          cursor,
          doc,
          view,
          kind: TriggerKind::Manual,
        });
        log::info!("Manual trigger, firing immediately");
        None // Fire immediately
      }
      CompletionEvent::DeleteText { cursor } => {
        // If we deleted before the trigger position, cancel
        if matches!(self.pending_trigger.or(self.in_flight), Some(PendingTrigger{ cursor: trigger_cursor, .. }) if cursor < trigger_cursor)
        {
          log::info!("Deleted before trigger position, cancelling");
          self.pending_trigger = None;
          // TODO: Cancel in-flight request via TaskController
        }
        None
      }
      CompletionEvent::Cancel => {
        // Cancel pending trigger
        log::info!("Cancelling completion");
        self.pending_trigger = None;
        // TODO: Cancel in-flight request via TaskController
        None
      }
    }
  }

  fn finish_debounce(&mut self) {
    // When debounce timer fires, request completions
    log::info!("finish_debounce called, pending_trigger: {:?}", self.pending_trigger);
    if let Some(trigger) = self.pending_trigger.take() {
      log::info!("Dispatching completion request for cursor {}", trigger.cursor);
      self.in_flight = Some(trigger.clone());
      crate::ui::job::dispatch_blocking(move |editor, compositor| {
        request_completions_sync(trigger, editor, compositor);
      });
    }
  }
}

/// Synchronous entry point for requesting completions (called from dispatch_blocking)
fn request_completions_sync(
  trigger: PendingTrigger,
  editor: &mut Editor,
  compositor: &mut crate::ui::compositor::Compositor,
) {

  log::info!("request_completions_sync called for cursor {}, kind: {:?}", trigger.cursor, trigger.kind);

  // Check if we're still in insert mode and no completion is already showing
  if editor.mode != Mode::Insert {
    log::info!("Not in insert mode, skipping completion");
    return;
  }

  if let Some(editor_view) = compositor.find::<ui::EditorView>() {
    if editor_view.completion.is_some() {
      return;
    }
  }

  // Get document and view
  let Some(doc) = editor.documents.get(&trigger.doc) else {
    log::info!("Document {:?} not found, skipping completion", trigger.doc);
    return;
  };
  let Some(view) = editor.tree.try_get(trigger.view) else {
    log::info!("View {:?} not found, skipping completion", trigger.view);
    return;
  };

  // Verify cursor hasn't moved backwards
  let text = doc.text();
  let cursor = doc.selection(view.id).primary().cursor(text.slice(..));
  log::info!("Current cursor: {}, trigger cursor: {}", cursor, trigger.cursor);
  if cursor < trigger.cursor {
    log::info!("Cursor moved backwards, skipping completion");
    return;
  }

  // IMPORTANT: Update trigger position to current cursor
  // This matches Helix's behavior - send the CURRENT position, not the old trigger position
  // Language servers need this for incomplete completion lists and proper filtering
  let trigger_offset = cursor;
  log::info!("Updated trigger offset from {} to {}", trigger.cursor, trigger_offset);

  // Determine trigger kind and character for LSP
  let (lsp_trigger_kind, lsp_trigger_char) = match trigger.kind {
    TriggerKind::Manual => (lsp::CompletionTriggerKind::INVOKED, None),
    TriggerKind::TriggerChar => {
      // Find the trigger character
      let trigger_char = doc
        .language_servers()
        .find_map(|ls| {
          ls.capabilities()
            .completion_provider
            .as_ref()
            .and_then(|cap| cap.trigger_characters.as_ref())
            .and_then(|chars| {
              let trigger_text = text.slice(..cursor);
              chars.iter().find(|ch| trigger_text.ends_with(ch.as_str())).cloned()
            })
        });
      (lsp::CompletionTriggerKind::TRIGGER_CHARACTER, trigger_char)
    },
    TriggerKind::Auto => (lsp::CompletionTriggerKind::INVOKED, None),
  };

  let doc_id = trigger.doc;
  let mut completion_futures = Vec::new();

  {

    // Get document URI and LSP position
    let uri_str = doc.uri().map(|u| u.to_string());
    let Some(uri_str) = uri_str else {
      log::info!("Document has no URI, skipping completion");
      return;
    };

    // Convert path to file:// URL if needed
    let lsp_uri = if uri_str.starts_with("file://") {
      url::Url::parse(&uri_str).ok()
    } else {
      url::Url::from_file_path(&uri_str).ok()
    };

    let Some(lsp_uri) = lsp_uri else {
      log::info!("Failed to parse URI: {:?}", uri_str);
      return;
    };
    log::info!("Document URI: {}", lsp_uri);

    // Convert cursor position to LSP position
    let pos = crate::lsp::util::pos_to_lsp_pos(text, cursor, OffsetEncoding::Utf16);

    // Collect completion futures from each language server (now 'static thanks to BoxFuture)
    let server_count = doc.language_servers().count();
    log::info!("Found {} language servers", server_count);

    for client in doc.language_servers() {
      let server_id = client.id();
      log::info!("Requesting completion from server {:?}", server_id);
      let context = lsp::CompletionContext {
        trigger_kind: lsp_trigger_kind,
        trigger_character: lsp_trigger_char.clone(),
      };

      // Get the boxed 'static future
      if let Some(completion_future) = client.completion(
        lsp::TextDocumentIdentifier { uri: lsp_uri.clone() },
        pos,
        None,
        context,
      ) {
        log::info!("Got completion future from server {:?}", server_id);
        completion_futures.push((server_id, completion_future));
      } else {
        log::info!("Server {:?} returned None for completion request", server_id);
      }
    }
  } // Drop editor borrow here

  log::info!("Collected {} completion futures", completion_futures.len());
  if completion_futures.is_empty() {
    log::info!("No completion futures collected, skipping");
    return;
  }

  // Now spawn with only owned data
  tokio::spawn(async move {
    let mut items = Vec::new();
    let timeout = tokio::time::sleep(Duration::from_millis(1000));
    tokio::pin!(timeout);

    for (server_id, completion_future) in completion_futures {
      tokio::select! {
        result = completion_future => {
          match result {
            Ok(Some(response)) => {
              let lsp_items: Vec<lsp::CompletionItem> = match response {
                lsp::CompletionResponse::Array(items) => items,
                lsp::CompletionResponse::List(list) => list.items,
              };
              for lsp_item in lsp_items {
                items.push(CompletionItem::Lsp(LspCompletionItem {
                  item: lsp_item,
                  provider: server_id,
                  resolved: false,
                  provider_priority: 0,
                }));
              }
            },
            Ok(None) => {},
            Err(err) => {
              log::warn!("Completion request failed for {:?}: {}", server_id, err);
            },
          }
        }
        _ = &mut timeout => {
          log::warn!("Completion request timed out");
          break;
        }
      }
    }

    // Dispatch back to main thread to show completion
    crate::ui::job::dispatch(move |editor, compositor| {
      show_completion(editor, compositor, items, trigger_offset, doc_id);
    }).await;
  });
}

/// Async function to request completions from all LSP servers
async fn request_completions_async(
  language_servers: Vec<(LanguageServerId, crate::lsp::Client)>,
  lsp_uri: url::Url,
  pos: lsp::Position,
  trigger_kind: lsp::CompletionTriggerKind,
  trigger_character: Option<String>,
) -> Vec<CompletionItem> {
  let mut items = Vec::new();

  // Request completions from each language server in parallel
  let mut tasks = Vec::new();

  for (server_id, client) in language_servers {
    let lsp_uri = lsp_uri.clone();
    let context = lsp::CompletionContext {
      trigger_kind,
      trigger_character: trigger_character.clone(),
    };

    let future = async move {
      let completion_future = client.completion(
        lsp::TextDocumentIdentifier { uri: lsp_uri },
        pos,
        None, // work_done_token
        context,
      )?;

      match completion_future.await {
        Ok(Some(response)) => {
          let items: Vec<lsp::CompletionItem> = match response {
            lsp::CompletionResponse::Array(items) => items,
            lsp::CompletionResponse::List(list) => list.items,
          };
          Some((server_id, items))
        }
        Ok(None) => None,
        Err(err) => {
          log::warn!("Completion request failed for {:?}: {}", server_id, err);
          None
        }
      }
    };

    tasks.push(future);
  }

  // Wait for all requests to complete (with timeout)
  let timeout = tokio::time::sleep(Duration::from_millis(1000));
  tokio::pin!(timeout);

  for future in tasks {
    tokio::select! {
      result = future => {
        if let Some((server_id, lsp_items)) = result {
          // Convert LSP items to our CompletionItem type
          for lsp_item in lsp_items {
            items.push(CompletionItem::Lsp(LspCompletionItem {
              item: lsp_item,
              provider: server_id,
              resolved: false,
              provider_priority: 0, // TODO: assign based on server order
            }));
          }
        }
      }
      _ = &mut timeout => {
        log::warn!("Completion request timed out");
        break;
      }
    }
  }

  items
}

/// Show completion popup in the editor view
fn show_completion(
  editor: &mut Editor,
  compositor: &mut crate::ui::compositor::Compositor,
  items: Vec<CompletionItem>,
  trigger_offset: usize,
  doc_id: DocumentId,
) {
  // Verify we're still in insert mode
  if editor.mode != Mode::Insert {
    return;
  }

  // Verify document still exists
  if !editor.documents.contains_key(&doc_id) {
    return;
  }

  // Don't show if empty
  if items.is_empty() {
    return;
  }

  // Get editor view from compositor
  let Some(editor_view) = compositor.find::<ui::EditorView>() else {
    return;
  };

  // Mark completion as triggered
  editor.last_completion = Some(crate::editor::CompleteAction::Triggered);

  // Set the completion
  editor_view.set_completion(editor, items, trigger_offset);
}

/// Register hooks for automatic completion triggering
pub fn register_completion_hooks(handlers: &Handlers) {
  let completions_mode = handlers.completions.clone();

  // Hook: Trigger completion on character insertion
  register_hook!(move |event: &mut PostInsertChar<'_, '_>| {
    use crate::handlers::completion_request_helpers::{
      trigger_auto_completion,
      update_completion_filter,
    };

    let c = event.c;
    log::info!("PostInsertChar hook fired for char: '{}'", c);

    // If completion is already active, update the filter
    if event.cx.editor.last_completion.is_some() {
      log::info!("Completion is active, updating filter");

      // Check if we should clear last_completion immediately to prevent race conditions
      if !crate::core::chars::char_is_word(c) {
        event.cx.editor.last_completion = None;
        log::info!("Clearing last_completion immediately for non-word char '{}'", c);
      }

      update_completion_filter(event.cx, Some(c));
    } else {
      // No completion active, try to trigger one
      trigger_auto_completion(event.cx.editor, false);
    }

    Ok(())
  });

  // Hook: Cancel completion when leaving insert mode
  register_hook!(move |event: &mut OnModeSwitch<'_, '_>| {
    if event.old_mode == Mode::Insert && event.new_mode != Mode::Insert {
      completions_mode.event(CompletionEvent::Cancel);
      event.cx.editor.last_completion = None;
    }
    Ok(())
  });

  // Hook: Handle commands that affect completion
  let completions_command = handlers.completions.clone();
  register_hook!(move |event: &mut PostCommand<'_, '_>| {
    use crate::handlers::completion_request_helpers::{
      clear_completions,
      update_completion_filter,
    };

    if event.cx.editor.mode == Mode::Insert {
      if event.cx.editor.last_completion.is_some() {
        // Completion is active, handle specific commands
        match event.command {
          "delete_char_backward" => {
            // Update filter with None to simulate backspace
            update_completion_filter(event.cx, None);
          }
          "delete_word_forward" | "delete_char_forward" | "completion" => {
            // These commands don't close completion
          }
          _ => {
            // Any other command closes completion
            clear_completions(event.cx);
          }
        }
      } else {
        // No completion active, send events for delete operations
        match event.command {
          "delete_char_backward" | "delete_word_forward" | "delete_char_forward" => {
            let (view, doc) = crate::current!(event.cx.editor);
            let primary_cursor = doc.selection(view.id).primary().cursor(doc.text().slice(..));
            completions_command.event(CompletionEvent::DeleteText {
              cursor: primary_cursor,
            });
          }
          // Don't cancel for these commands
          "completion" | "insert_mode" | "append_mode" => {}
          _ => {
            completions_command.event(CompletionEvent::Cancel);
          }
        }
      }
    }
    Ok(())
  });
}
