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
  doc,
  editor::Editor,
  event::{
    OnModeSwitch,
    PostInsertChar,
  },
  keymap::Mode,
  lsp::{
    LanguageServerId,
    OffsetEncoding,
  },
  ui,
  view,
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
#[derive(Debug, Clone)]
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
}

impl CompletionRequestHook {
  pub fn new() -> Self {
    Self {
      pending_trigger: None,
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
        // Debounce auto-triggered completions
        self.pending_trigger = Some(PendingTrigger {
          cursor,
          doc,
          view,
          kind: TriggerKind::Auto,
        });
        log::info!("Set auto-trigger, will fire in 120ms");
        Some(Instant::now() + AUTO_DEBOUNCE)
      }
      CompletionEvent::TriggerChar { cursor, doc, view } => {
        // Short debounce for trigger characters
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
      CompletionEvent::DeleteText { .. } => {
        // Cancel pending trigger on delete
        self.pending_trigger = None;
        None
      }
      CompletionEvent::Cancel => {
        // Cancel pending trigger
        self.pending_trigger = None;
        None
      }
    }
  }

  fn finish_debounce(&mut self) {
    // When debounce timer fires, request completions
    log::info!("finish_debounce called, pending_trigger: {:?}", self.pending_trigger);
    if let Some(trigger) = self.pending_trigger.take() {
      log::info!("Dispatching completion request for cursor {}", trigger.cursor);
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
  use crate::ui::compositor::Component;

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

  // Collect completion futures
  let mut completion_futures = Vec::new();
  let doc_id;
  let trigger_offset;

  {
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

    doc_id = trigger.doc;
    trigger_offset = trigger.cursor;

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
  use crate::ui::compositor::Component;

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

  // Set the completion
  editor_view.set_completion(editor, items, trigger_offset);
}

/// Register hooks for automatic completion triggering
pub fn register_completion_hooks(handlers: &Handlers) {
  let completions_insert = handlers.completions.clone();
  let completions_mode = handlers.completions.clone();

  // Hook: Trigger completion on character insertion
  register_hook!(move |event: &mut PostInsertChar<'_, '_>| {
    let c = event.c;
    log::info!("PostInsertChar hook fired for char: '{}'", c);
    let doc = doc!(event.cx.editor);
    let view = view!(event.cx.editor);
    let cursor = doc.selection(view.id).primary().cursor(doc.text().slice(..));

    // Check if this is a trigger character
    let is_trigger_char = doc
      .language_servers()
      .any(|ls| {
        ls.capabilities()
          .completion_provider
          .as_ref()
          .and_then(|cap| cap.trigger_characters.as_ref())
          .map_or(false, |chars| chars.iter().any(|s| s.as_str() == c.to_string()))
      });

    if is_trigger_char {
      log::info!("Trigger character detected, sending TriggerChar event");
      completions_insert.event(CompletionEvent::TriggerChar {
        cursor,
        doc: doc.id,
        view: view.id,
      });
    } else if crate::core::chars::char_is_word(c) {
      // Auto-trigger on word characters
      log::info!("Word character detected, sending AutoTrigger event");
      completions_insert.event(CompletionEvent::AutoTrigger {
        cursor,
        doc: doc.id,
        view: view.id,
      });
    } else {
      log::info!("Character '{}' is neither trigger nor word char", c);
    }

    Ok(())
  });

  // Hook: Cancel completion when leaving insert mode
  register_hook!(move |event: &mut OnModeSwitch<'_, '_>| {
    if event.old_mode == Mode::Insert && event.new_mode != Mode::Insert {
      completions_mode.event(CompletionEvent::Cancel);
    }
    Ok(())
  });
}
