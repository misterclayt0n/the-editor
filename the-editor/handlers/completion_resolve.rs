/// Handler for resolving incomplete completion items
///
/// From the LSP spec:
/// > If computing full completion items is expensive, servers can additionally
/// > provide a handler for the completion item resolve request. A typical use
/// > case is for example: the `textDocument/completion` request doesn't fill
/// > in the `documentation` property for returned completion items since it
/// > is expensive to compute. When the item is selected in the user interface
/// > then a 'completionItem/resolve' request is sent with the selected
/// > completion item as a parameter.
use std::sync::Arc;

use the_editor_event::{AsyncHook, TaskController, cancelable_future, send_blocking};
use the_editor_lsp_types::types as lsp;
use tokio::{
  sync::mpsc::Sender,
  time::{Duration, Instant},
};

use super::completion::{CompletionItem, LspCompletionItem};
use crate::lsp::LanguageServerId;

/// Handler for resolving completion items asynchronously
pub struct ResolveHandler {
  /// The last item we requested resolution for (to avoid duplicates)
  last_request: Option<Arc<LspCompletionItem>>,
  /// Channel to send resolve requests
  resolver: Sender<ResolveRequest>,
}

impl ResolveHandler {
  pub fn new() -> Self {
    Self {
      last_request: None,
      resolver: ResolveTimeout::default().spawn(),
    }
  }

  /// Ensure that a completion item is resolved (has documentation, detail,
  /// etc.) If not resolved, this will trigger an async request to the LSP
  /// server
  pub fn ensure_item_resolved(&mut self, item: &mut LspCompletionItem) {
    if item.resolved {
      return;
    }

    // Check if item actually needs resolution
    // An item is considered fully resolved if it has non-empty documentation,
    // detail, and additional_text_edits
    let is_resolved = item
      .item
      .documentation
      .as_ref()
      .is_some_and(|docs| match docs {
        lsp::Documentation::String(text) => !text.is_empty(),
        lsp::Documentation::MarkupContent(markup) => !markup.value.is_empty(),
      })
      && item
        .item
        .detail
        .as_ref()
        .is_some_and(|detail| !detail.is_empty())
      && item
        .item
        .additional_text_edits
        .as_ref()
        .is_some_and(|edits| !edits.is_empty());

    if is_resolved {
      item.resolved = true;
      return;
    }

    // Don't send duplicate requests
    if self.last_request.as_deref().is_some_and(|it| it == item) {
      return;
    }

    // Get language server - we'll need to pass it along
    // Note: The actual LS lookup will happen in the request execution
    let item_arc = Arc::new(item.clone());
    self.last_request = Some(item_arc.clone());

    send_blocking(
      &self.resolver,
      ResolveRequest {
        item: item_arc,
        provider: item.provider,
      },
    );
  }
}

/// A request to resolve a completion item
struct ResolveRequest {
  item: Arc<LspCompletionItem>,
  provider: LanguageServerId,
}

/// Async hook that debounces resolve requests
#[derive(Default)]
struct ResolveTimeout {
  /// The next pending request (will be sent after debounce)
  next_request: Option<ResolveRequest>,
  /// The currently in-flight request (being processed by LSP)
  in_flight: Option<Arc<LspCompletionItem>>,
  /// Task controller for cancellation
  task_controller: TaskController,
}

impl AsyncHook for ResolveTimeout {
  type Event = ResolveRequest;

  fn handle_event(&mut self, request: Self::Event, timeout: Option<Instant>) -> Option<Instant> {
    // Check if previous in-flight request has completed
    if self.in_flight.is_some() && !self.task_controller.is_running() {
      self.in_flight = None;
    }

    // If we already have a pending request for this same item, keep the current
    // timeout
    if self
      .next_request
      .as_ref()
      .is_some_and(|old_request| old_request.item == request.item)
    {
      return timeout;
    }

    // If this item is already being resolved, don't queue it again
    if self
      .in_flight
      .as_ref()
      .is_some_and(|old_request| old_request.item == request.item.item)
    {
      self.next_request = None;
      return None;
    }

    // Queue this request and set debounce timeout
    self.next_request = Some(request);
    Some(Instant::now() + Duration::from_millis(150))
  }

  fn finish_debounce(&mut self) {
    let Some(request) = self.next_request.take() else {
      return;
    };

    self.in_flight = Some(request.item.clone());
    let handle = self.task_controller.restart();

    // Spawn async task to resolve the item with cancellation support
    tokio::spawn(cancelable_future(request.execute(), handle));
  }
}

impl ResolveRequest {
  async fn execute(self) {
    let item_arc = self.item.clone();
    let provider = self.provider;

    // Create a oneshot channel to get the resolve future from the main thread
    // This is the minimal work we need to do on the main thread
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Use async dispatch to avoid blocking the UI thread
    crate::ui::job::dispatch(move |editor, _compositor| {
      let Some(ls) = editor.language_server_by_id(provider) else {
        log::warn!(
          "Language server {:?} not found for completion resolve",
          provider
        );
        let _ = tx.send(None);
        return;
      };

      // Check if server supports resolve
      if !matches!(
        ls.capabilities().completion_provider,
        Some(lsp::CompletionOptions {
          resolve_provider: Some(true),
          ..
        })
      ) {
        log::debug!("Language server doesn't support completion resolve");
        let _ = tx.send(None);
        return;
      }

      let future = ls.resolve_completion_item(&item_arc.item);
      let _ = tx.send(Some(future));
    })
    .await;

    // Wait for the resolve future from main thread
    let Some(resolve_future) = rx.await.ok().flatten() else {
      return;
    };

    // Await the resolution (this is the potentially slow LSP call)
    let resolved = match resolve_future.await {
      Ok(item) => CompletionItem::Lsp(LspCompletionItem {
        item,
        resolved: true,
        ..*self.item
      }),
      Err(err) => {
        log::error!("Completion resolve request failed: {}", err);
        // Mark as resolved so we don't keep trying
        let mut item = (*self.item).clone();
        item.resolved = true;
        CompletionItem::Lsp(item)
      },
    };

    // Update the completion in the UI using async dispatch
    let old_item = self.item.clone();
    crate::ui::job::dispatch(move |_editor, compositor| {
      if let Some(editor_view) = compositor.find::<crate::ui::EditorView>() {
        if let Some(completion) = &mut editor_view.completion {
          completion.replace_item(&*old_item, resolved);
        }
      }
    })
    .await;
  }
}
