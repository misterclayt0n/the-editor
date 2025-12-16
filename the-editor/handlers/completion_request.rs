/// ! Async completion request handler with debouncing
/// !
/// ! This module handles completion requests from the editor, debounces them
/// ! appropriately, and coordinates LSP completion requests across multiple
/// ! language servers.
use std::time::Duration;

use the_editor_event::{
  AsyncHook,
  TaskController,
  TaskHandle,
  cancelable_future,
  register_hook,
};
use the_editor_lsp_types::types as lsp;
use the_editor_stdx::rope::RopeSliceExt;
use tokio::{
  task::JoinSet,
  time::Instant,
};

use super::{
  Handlers,
  completion::{
    CompletionEvent,
    CompletionItem,
    LspCompletionItem,
  },
  completion_path,
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

/// Timeout for showing first completion result (show fast, update later)
const FIRST_RESULT_TIMEOUT: Duration = Duration::from_millis(100);

/// Maximum timeout for all completion results
const ALL_RESULTS_TIMEOUT: Duration = Duration::from_millis(1000);

/// Pending completion request
#[derive(Debug, Clone, Copy)]
pub struct PendingTrigger {
  pub cursor: usize,
  pub doc:    DocumentId,
  pub view:   ViewId,
  pub kind:   TriggerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
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
  /// Task controller for canceling in-flight requests
  task_controller: TaskController,
}

impl CompletionRequestHook {
  pub fn new() -> Self {
    Self {
      pending_trigger: None,
      in_flight:       None,
      task_controller: TaskController::new(),
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
    // Check if previous in-flight request has completed
    if self.in_flight.is_some() && !self.task_controller.is_running() {
      self.in_flight = None;
    }

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
          Some(Instant::now() + AUTO_DEBOUNCE)
        } else {
          None
        }
      },
      CompletionEvent::TriggerChar { cursor, doc, view } => {
        // Cancel any in-flight request and set new trigger char request
        self.task_controller.cancel();
        self.pending_trigger = Some(PendingTrigger {
          cursor,
          doc,
          view,
          kind: TriggerKind::TriggerChar,
        });
        Some(Instant::now() + TRIGGER_CHAR_DEBOUNCE)
      },
      CompletionEvent::ManualTrigger { cursor, doc, view } => {
        // Manual triggers should fire immediately without debouncing
        self.pending_trigger = Some(PendingTrigger {
          cursor,
          doc,
          view,
          kind: TriggerKind::Manual,
        });
        // Immediately finish debounce for manual triggers
        self.finish_debounce();
        None
      },
      CompletionEvent::DeleteText { cursor } => {
        // If we deleted before the trigger position, cancel everything
        if matches!(self.pending_trigger.or(self.in_flight), Some(PendingTrigger{ cursor: trigger_cursor, .. }) if cursor < trigger_cursor)
        {
          self.pending_trigger = None;
          self.task_controller.cancel();
          self.in_flight = None;
        }
        None
      },
      CompletionEvent::Cancel => {
        // Cancel pending trigger and in-flight requests
        self.pending_trigger = None;
        self.task_controller.cancel();
        self.in_flight = None;
        None
      },
    }
  }

  fn finish_debounce(&mut self) {
    // When debounce timer fires, request completions
    let Some(trigger) = self.pending_trigger.take() else {
      return;
    };

    self.in_flight = Some(trigger);
    let handle = self.task_controller.restart();

    // Use dispatch_blocking to get on main thread, but keep work minimal
    crate::ui::job::dispatch_blocking(move |editor, compositor| {
      request_completions(trigger, handle, editor, compositor);
    });
  }
}

/// Response from a single completion provider
struct CompletionResponse {
  items:    Vec<CompletionItem>,
  provider: LanguageServerId,
  priority: i8,
}

/// Entry point for requesting completions (called from dispatch_blocking)
/// Keeps main thread work minimal - just validates state and spawns async task
fn request_completions(
  trigger: PendingTrigger,
  handle: TaskHandle,
  editor: &mut Editor,
  compositor: &mut crate::ui::compositor::Compositor,
) {
  // Check if we're still in insert mode and no completion is already showing
  if editor.mode != Mode::Insert {
    return;
  }

  if let Some(editor_view) = compositor.find::<ui::EditorView>() {
    if editor_view.completion.is_some() {
      return;
    }
  }

  // Get document and view
  let Some(doc) = editor.documents.get(&trigger.doc) else {
    return;
  };
  let Some(view) = editor.tree.try_get(trigger.view) else {
    return;
  };

  // Verify cursor hasn't moved backwards
  let text = doc.text();
  let cursor = doc.selection(view.id).primary().cursor(text.slice(..));
  if cursor < trigger.cursor {
    return;
  }

  // Calculate trigger offset for path completions
  let mut trigger_offset = cursor;
  let text_slice_for_offset = text.slice(..cursor);
  if let Some(path_suffix) = the_editor_stdx::path::get_path_suffix(text_slice_for_offset, false) {
    trigger_offset = cursor - path_suffix.len_chars();
  }

  // Determine trigger kind and character for LSP
  let (lsp_trigger_kind, lsp_trigger_char) = match trigger.kind {
    TriggerKind::Manual => (lsp::CompletionTriggerKind::INVOKED, None),
    TriggerKind::TriggerChar => {
      let trigger_char = doc.language_servers().find_map(|ls| {
        ls.capabilities()
          .completion_provider
          .as_ref()
          .and_then(|cap| cap.trigger_characters.as_ref())
          .and_then(|chars| {
            let trigger_text = text.slice(..cursor);
            chars
              .iter()
              .find(|ch| trigger_text.ends_with(ch.as_str()))
              .cloned()
          })
      });
      (lsp::CompletionTriggerKind::TRIGGER_CHARACTER, trigger_char)
    },
    TriggerKind::Auto => (lsp::CompletionTriggerKind::INVOKED, None),
  };

  let doc_id = trigger.doc;
  let _view_id = trigger.view;

  // Generate path completions synchronously (fast, local operation)
  let text_slice = text.slice(..cursor);
  let has_path_suffix = the_editor_stdx::path::get_path_suffix(text_slice, false).is_some();
  let path_completion_items = if has_path_suffix {
    let text_slice = text.slice(..);
    let doc_path = doc.path().map(|p| p.as_path());
    completion_path::path_completion(text_slice, cursor, doc_path)
  } else {
    Vec::new()
  };

  // Get document URI (may be None for scratch files)
  let uri_str = doc.uri().map(|u| u.to_string());

  // If no URI (scratch file), show path completions if available and return
  if uri_str.is_none() {
    if !path_completion_items.is_empty() {
      show_completion(editor, compositor, path_completion_items, trigger_offset, doc_id);
    }
    return;
  }

  let Some(uri_str) = uri_str else {
    return;
  };

  // Convert path to file:// URL
  let lsp_uri = if uri_str.starts_with("file://") {
    url::Url::parse(&uri_str).ok()
  } else {
    url::Url::from_file_path(&uri_str).ok()
  };

  let Some(lsp_uri) = lsp_uri else {
    // No valid LSP URI but we have path completions, show them
    if !path_completion_items.is_empty() {
      show_completion(editor, compositor, path_completion_items, trigger_offset, doc_id);
    }
    return;
  };

  // Convert cursor position to LSP position
  let pos = crate::lsp::util::pos_to_lsp_pos(text, cursor, OffsetEncoding::Utf16);

  // Spawn JoinSet for parallel LSP requests
  let mut requests: JoinSet<CompletionResponse> = JoinSet::new();

  for (priority_index, client) in doc.language_servers().enumerate() {
    let server_id = client.id();
    let context = lsp::CompletionContext {
      trigger_kind:      lsp_trigger_kind,
      trigger_character: lsp_trigger_char.clone(),
    };

    // Get the completion future - it's important this happens synchronously
    // before any edits so the LSP position is correct
    if let Some(completion_future) = client.completion(
      lsp::TextDocumentIdentifier {
        uri: lsp_uri.clone(),
      },
      pos,
      None,
      context,
    ) {
      let priority = -(priority_index as i8);
      requests.spawn(async move {
        let response = completion_future.await;
        let items = match response {
          Ok(Some(lsp::CompletionResponse::Array(items))) => items,
          Ok(Some(lsp::CompletionResponse::List(list))) => list.items,
          Ok(None) => Vec::new(),
          Err(err) => {
            log::warn!("Completion request failed for {:?}: {}", server_id, err);
            Vec::new()
          },
        };

        // Sort by sort_text
        let mut sorted_items: Vec<_> = items
          .into_iter()
          .map(|item| {
            CompletionItem::Lsp(LspCompletionItem {
              item,
              provider: server_id,
              resolved: false,
              provider_priority: priority,
            })
          })
          .collect();
        sorted_items.sort_by(|a, b| {
          let sort_a = match a {
            CompletionItem::Lsp(lsp) => {
              lsp.item.sort_text.as_deref().unwrap_or(&lsp.item.label)
            },
            _ => "",
          };
          let sort_b = match b {
            CompletionItem::Lsp(lsp) => {
              lsp.item.sort_text.as_deref().unwrap_or(&lsp.item.label)
            },
            _ => "",
          };
          sort_a.cmp(sort_b)
        });

        CompletionResponse {
          items: sorted_items,
          provider: server_id,
          priority,
        }
      });
    }
  }

  // If we have no LSP completions but have path completions, show them immediately
  if requests.is_empty() {
    if !path_completion_items.is_empty() {
      show_completion(editor, compositor, path_completion_items, trigger_offset, doc_id);
    }
    return;
  }

  // Spawn async task to collect results progressively
  let path_items = path_completion_items;
  let request_completions_task = async move {
    let mut all_items = path_items;
    let mut shown_first = false;

    // Wait for the first result with a short timeout
    let first_deadline = Instant::now() + FIRST_RESULT_TIMEOUT;

    loop {
      let result = if !shown_first {
        // For the first result, use a short timeout
        tokio::time::timeout_at(first_deadline, requests.join_next()).await
      } else {
        // For subsequent results, use a longer timeout
        let deadline = Instant::now() + ALL_RESULTS_TIMEOUT;
        tokio::time::timeout_at(deadline, requests.join_next()).await
      };

      match result {
        Ok(Some(Ok(response))) => {
          if response.items.is_empty() {
            continue;
          }

          all_items.extend(response.items);

          if !shown_first {
            // Show first result immediately via async dispatch
            shown_first = true;
            let items_to_show = all_items.clone();
            crate::ui::job::dispatch(move |editor, compositor| {
              show_completion(editor, compositor, items_to_show, trigger_offset, doc_id);
            })
            .await;
          } else {
            // Update existing completion with new items
            let items_to_update = all_items.clone();
            crate::ui::job::dispatch(move |editor, compositor| {
              update_completion(editor, compositor, items_to_update, trigger_offset, doc_id);
            })
            .await;
          }
        },
        Ok(Some(Err(join_err))) => {
          log::warn!("Completion task panicked: {:?}", join_err);
        },
        Ok(None) => {
          // All requests completed
          break;
        },
        Err(_timeout) => {
          if !shown_first && !all_items.is_empty() {
            // Timed out waiting for first result, show what we have (path completions)
            let items_to_show = all_items.clone();
            crate::ui::job::dispatch(move |editor, compositor| {
              show_completion(editor, compositor, items_to_show, trigger_offset, doc_id);
            })
            .await;
            shown_first = true;
          }

          if shown_first {
            // Already showing results, continue collecting more
            continue;
          } else {
            // Timed out with nothing to show, abort
            break;
          }
        },
      }
    }

    // Final update if we collected more items after the initial show
    if shown_first && !requests.is_empty() {
      // There might be more results, wait a bit longer
      while let Ok(Some(Ok(response))) =
        tokio::time::timeout(Duration::from_millis(50), requests.join_next()).await
      {
        if !response.items.is_empty() {
          all_items.extend(response.items);
        }
      }

      // Final update
      let final_items = all_items;
      crate::ui::job::dispatch(move |editor, compositor| {
        update_completion(editor, compositor, final_items, trigger_offset, doc_id);
      })
      .await;
    } else if !shown_first && !all_items.is_empty() {
      // Never showed anything but have items, show now
      crate::ui::job::dispatch(move |editor, compositor| {
        show_completion(editor, compositor, all_items, trigger_offset, doc_id);
      })
      .await;
    }
  };

  // Spawn the task with cancellation support
  tokio::spawn(cancelable_future(request_completions_task, handle));
}

/// Show completion popup in the editor view (creates new popup)
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

  // If completion already exists, update instead
  if editor_view.completion.is_some() {
    update_completion(editor, compositor, items, trigger_offset, doc_id);
    return;
  }

  // Mark completion as triggered
  editor.last_completion = Some(crate::editor::CompleteAction::Triggered);

  // Set the completion
  editor_view.set_completion(editor, items, trigger_offset);
}

/// Update existing completion popup with new items (for progressive loading)
fn update_completion(
  editor: &mut Editor,
  compositor: &mut crate::ui::compositor::Compositor,
  items: Vec<CompletionItem>,
  _trigger_offset: usize,
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

  // Get editor view from compositor
  let Some(editor_view) = compositor.find::<ui::EditorView>() else {
    return;
  };

  // Update the completion items if popup exists
  if let Some(completion) = &mut editor_view.completion {
    completion.update_items(items);
  }
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

    // If completion is already active, update the filter
    if event.cx.editor.last_completion.is_some() {
      // Check if we should clear last_completion immediately to prevent race
      // conditions
      if !crate::core::chars::char_is_word(c) {
        event.cx.editor.last_completion = None;
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
          },
          "delete_word_forward" | "delete_char_forward" | "completion" => {
            // These commands don't close completion
          },
          _ => {
            // Any other command closes completion
            clear_completions(event.cx);
          },
        }
      } else {
        // No completion active, send events for delete operations
        match event.command {
          "delete_char_backward" | "delete_word_forward" | "delete_char_forward" => {
            // Skip if focused view is not a document (e.g., terminal)
            let Some((view, doc)) = crate::try_current!(event.cx.editor) else {
              return Ok(());
            };
            let primary_cursor = doc
              .selection(view.id)
              .primary()
              .cursor(doc.text().slice(..));
            completions_command.event(CompletionEvent::DeleteText {
              cursor: primary_cursor,
            });
          },
          // Don't cancel for these commands
          "completion" | "insert_mode" | "append_mode" => {},
          _ => {
            completions_command.event(CompletionEvent::Cancel);
          },
        }
      }
    }
    Ok(())
  });
}
