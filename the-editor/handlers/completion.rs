use std::{
  collections::HashMap,
  sync::{
    Arc,
    Mutex,
  },
};

use the_editor_event::{
  TaskController,
  send_blocking,
};
use the_editor_lsp_types::types as lsp;
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

/// LSP completion item with lazy resolution support
#[derive(Debug, Clone)]
pub struct LspCompletionItem {
  /// The raw LSP completion item
  pub item:              lsp::CompletionItem,
  /// The LSP server that provided this completion
  pub provider:          LanguageServerId,
  /// Whether this item has been resolved (documentation fetched)
  pub resolved:          bool,
  /// Provider priority for sorting
  pub provider_priority: i8,
}

impl LspCompletionItem {
  /// Get the text to use for fuzzy filtering
  pub fn filter_text(&self) -> &str {
    self
      .item
      .filter_text
      .as_ref()
      .unwrap_or(&self.item.label)
      .as_str()
  }

  /// Check if this item is preselected
  pub fn preselect(&self) -> bool {
    self.item.preselect.unwrap_or(false)
  }
}

impl PartialEq for LspCompletionItem {
  fn eq(&self, other: &Self) -> bool {
    self.item.label == other.item.label && self.provider == other.provider
  }
}

/// Non-LSP completion item (e.g., from word or path completion)
#[derive(Debug, PartialEq, Clone)]
pub struct OtherCompletionItem {
  pub transaction:   Transaction,
  pub label:         String,
  pub kind:          Option<String>,
  pub documentation: Option<String>,
  pub provider:      CompletionProvider,
}

impl OtherCompletionItem {
  pub fn filter_text(&self) -> &str {
    &self.label
  }
}

/// Completion item that can come from LSP or other sources
#[derive(Debug, Clone)]
pub enum CompletionItem {
  Lsp(LspCompletionItem),
  Other(OtherCompletionItem),
}

impl CompletionItem {
  /// Get the text to use for fuzzy filtering
  pub fn filter_text(&self) -> &str {
    match self {
      CompletionItem::Lsp(item) => item.filter_text(),
      CompletionItem::Other(item) => item.filter_text(),
    }
  }

  /// Check if this item is preselected
  pub fn preselect(&self) -> bool {
    match self {
      CompletionItem::Lsp(item) => item.preselect(),
      CompletionItem::Other(_) => false,
    }
  }

  /// Get the provider for this completion
  pub fn provider(&self) -> CompletionProvider {
    match self {
      CompletionItem::Lsp(item) => CompletionProvider::Lsp(item.provider),
      CompletionItem::Other(item) => item.provider,
    }
  }

  /// Get provider priority for sorting
  pub fn provider_priority(&self) -> i8 {
    match self {
      CompletionItem::Lsp(item) => item.provider_priority,
      CompletionItem::Other(item) => {
        // Path completions have lower priority (higher number) than LSP
        match item.provider {
          CompletionProvider::Path => 10,
          CompletionProvider::Word => 20,
          CompletionProvider::Lsp(_) => unreachable!(),
        }
      },
    }
  }
}

impl PartialEq for CompletionItem {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (CompletionItem::Lsp(a), CompletionItem::Lsp(b)) => a == b,
      (CompletionItem::Other(a), CompletionItem::Other(b)) => a == b,
      _ => false,
    }
  }
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

#[derive(Clone)]
pub struct CompletionHandler {
  event_tx:               Sender<CompletionEvent>,
  pub active_completions: Arc<Mutex<HashMap<CompletionProvider, ResponseContext>>>,
  pub request_controller: Arc<Mutex<TaskController>>,
}

impl CompletionHandler {
  pub fn new(event_tx: Sender<CompletionEvent>) -> Self {
    Self {
      event_tx,
      active_completions: Arc::new(Mutex::new(HashMap::new())),
      request_controller: Arc::new(Mutex::new(TaskController::new())),
    }
  }

  pub fn event(&self, event: CompletionEvent) {
    send_blocking(&self.event_tx, event);
  }
}

#[derive(Clone)]
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

#[derive(Debug)]
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
