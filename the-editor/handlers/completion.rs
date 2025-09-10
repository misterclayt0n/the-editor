use std::{
  borrow::Cow,
  collections::HashMap,
  sync::Arc,
};

use the_editor_event::{
  TaskController,
  send_blocking,
};
use tokio::sync::mpsc::Sender;

use crate::{
  core::{
    DocumentId,
    ViewId,
    document::SavePoint,
    transaction::Transaction,
  },
  lsp::LanguageServerId,
};

#[derive(Debug, PartialEq, Clone)]
pub struct CompletionItem {
  pub transaction:   Transaction,
  pub label:         Cow<'static, str>,
  pub kind:          Cow<'static, str>,
  /// Containing Markdown
  pub documentation: Option<String>,
  pub provider:      CompletionProvider,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum CompletionProvider {
  Lsp(LanguageServerId),
  Path,
  Word,
}

impl From<LanguageServerId> for CompletionProvider {
  fn from(id: LanguageServerId) -> Self {
    CompletionProvider::Lsp(id)
  }
}

pub struct CompletionHandler {
  event_tx:               Sender<CompletionEvent>,
  pub active_completions: HashMap<CompletionProvider, ResponseContext>,
  pub request_controller: TaskController,
}

impl CompletionHandler {
  pub fn new(event_tx: Sender<CompletionEvent>) -> Self {
    Self {
      event_tx,
      active_completions: HashMap::new(),
      request_controller: TaskController::new(),
    }
  }

  pub fn event(&self, event: CompletionEvent) {
    send_blocking(&self.event_tx, event);
  }
}

pub struct ResponseContext {
  /// Whether the completion response is marked as "incomplete."
  ///
  /// This is used by LSP. When completions are "incomplete" and you continue
  /// typing, the completions should be recomputed by the server instead of
  /// filtered.
  pub is_incomplete: bool,
  pub priority:      i8,
  pub savepoint:     Arc<SavePoint>,
}

pub enum CompletionEvent {
  /// Auto completion was triggered by typing a word char
  AutoTrigger {
    cursor: usize,
    doc:    DocumentId,
    view:   ViewId,
  },
  /// Auto completion was triggered by typing a trigger char
  /// specified by the LSP
  TriggerChar {
    cursor: usize,
    doc:    DocumentId,
    view:   ViewId,
  },
  /// A completion was manually requested (c-x)
  ManualTrigger {
    cursor: usize,
    doc:    DocumentId,
    view:   ViewId,
  },
  /// Some text was deleted and the cursor is now at `pos`
  DeleteText { cursor: usize },
  /// Invalidate the current auto completion trigger
  Cancel,
}
