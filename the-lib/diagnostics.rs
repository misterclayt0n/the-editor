use std::{
  collections::{
    BTreeMap,
    VecDeque,
  },
  mem,
};

use serde::{
  Deserialize,
  Serialize,
};

pub const DEFAULT_EVENT_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
  Error,
  Warning,
  Information,
  Hint,
}

impl DiagnosticSeverity {
  pub fn from_lsp_code(code: u8) -> Option<Self> {
    match code {
      1 => Some(Self::Error),
      2 => Some(Self::Warning),
      3 => Some(Self::Information),
      4 => Some(Self::Hint),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticPosition {
  pub line:      u32,
  pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRange {
  pub start: DiagnosticPosition,
  pub end:   DiagnosticPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
  pub range:    DiagnosticRange,
  pub severity: Option<DiagnosticSeverity>,
  pub code:     Option<String>,
  pub source:   Option<String>,
  pub message:  String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCounts {
  pub total:       usize,
  pub errors:      usize,
  pub warnings:    usize,
  pub information: usize,
  pub hints:       usize,
}

impl DiagnosticCounts {
  pub fn from_diagnostics(diagnostics: &[Diagnostic]) -> Self {
    let mut counts = Self::default();
    for diagnostic in diagnostics {
      counts.total = counts.total.saturating_add(1);
      match diagnostic.severity {
        Some(DiagnosticSeverity::Error) => counts.errors = counts.errors.saturating_add(1),
        Some(DiagnosticSeverity::Warning) => counts.warnings = counts.warnings.saturating_add(1),
        Some(DiagnosticSeverity::Information) => {
          counts.information = counts.information.saturating_add(1)
        },
        Some(DiagnosticSeverity::Hint) => counts.hints = counts.hints.saturating_add(1),
        None => {},
      }
    }
    counts
  }

  pub fn is_empty(&self) -> bool {
    self.total == 0
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDiagnostics {
  pub uri:         String,
  pub version:     Option<i32>,
  pub diagnostics: Vec<Diagnostic>,
}

impl DocumentDiagnostics {
  pub fn counts(&self) -> DiagnosticCounts {
    DiagnosticCounts::from_diagnostics(&self.diagnostics)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosticsEventKind {
  Published {
    uri:     String,
    version: Option<i32>,
    counts:  DiagnosticCounts,
  },
  Cleared {
    uri: String,
  },
  ClearedAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsEvent {
  pub seq:  u64,
  #[serde(flatten)]
  pub kind: DiagnosticsEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsSnapshot {
  pub document_count: usize,
  pub total_count:    usize,
  pub oldest_seq:     u64,
  pub latest_seq:     u64,
}

#[derive(Debug, Clone)]
pub struct DiagnosticsState {
  documents:      BTreeMap<String, DocumentDiagnostics>,
  events:         VecDeque<DiagnosticsEvent>,
  next_event_seq: u64,
  event_limit:    usize,
}

impl Default for DiagnosticsState {
  fn default() -> Self {
    Self::with_event_limit(DEFAULT_EVENT_LIMIT)
  }
}

impl DiagnosticsState {
  pub fn with_event_limit(event_limit: usize) -> Self {
    Self {
      documents:      BTreeMap::new(),
      events:         VecDeque::new(),
      next_event_seq: 1,
      event_limit:    event_limit.max(1),
    }
  }

  pub fn document(&self, uri: &str) -> Option<&DocumentDiagnostics> {
    self.documents.get(uri)
  }

  pub fn documents(&self) -> impl Iterator<Item = &DocumentDiagnostics> {
    self.documents.values()
  }

  pub fn total_counts(&self) -> DiagnosticCounts {
    let mut total = DiagnosticCounts::default();
    for document in self.documents.values() {
      let counts = document.counts();
      total.total = total.total.saturating_add(counts.total);
      total.errors = total.errors.saturating_add(counts.errors);
      total.warnings = total.warnings.saturating_add(counts.warnings);
      total.information = total.information.saturating_add(counts.information);
      total.hints = total.hints.saturating_add(counts.hints);
    }
    total
  }

  pub fn oldest_seq(&self) -> u64 {
    self
      .events
      .front()
      .map(|event| event.seq)
      .unwrap_or(self.next_event_seq)
  }

  pub fn latest_seq(&self) -> u64 {
    self.next_event_seq.saturating_sub(1)
  }

  pub fn snapshot(&self) -> DiagnosticsSnapshot {
    DiagnosticsSnapshot {
      document_count: self.documents.len(),
      total_count:    self.total_counts().total,
      oldest_seq:     self.oldest_seq(),
      latest_seq:     self.latest_seq(),
    }
  }

  pub fn events_since(&self, seq: u64) -> Vec<DiagnosticsEvent> {
    self
      .events
      .iter()
      .filter(|event| event.seq > seq)
      .cloned()
      .collect()
  }

  pub fn apply_document(&mut self, document: DocumentDiagnostics) -> DiagnosticCounts {
    let uri = document.uri.clone();
    let counts = document.counts();
    if counts.is_empty() {
      self.documents.remove(&uri);
      self.push_event(DiagnosticsEventKind::Cleared { uri });
      return counts;
    }

    let version = document.version;
    self.documents.insert(uri.clone(), document);
    self.push_event(DiagnosticsEventKind::Published {
      uri,
      version,
      counts,
    });
    counts
  }

  pub fn remove_document(&mut self, uri: &str) -> bool {
    if self.documents.remove(uri).is_some() {
      self.push_event(DiagnosticsEventKind::Cleared {
        uri: uri.to_string(),
      });
      return true;
    }
    false
  }

  pub fn clear(&mut self) {
    if self.documents.is_empty() && self.events.is_empty() {
      return;
    }
    self.documents.clear();
    self.events.clear();
    self.next_event_seq = 1;
    self.push_event(DiagnosticsEventKind::ClearedAll);
  }

  fn push_event(&mut self, kind: DiagnosticsEventKind) {
    let event = DiagnosticsEvent {
      seq: self.next_event_seq,
      kind,
    };
    self.next_event_seq = self.next_event_seq.saturating_add(1);
    self.events.push_back(event);
    while self.events.len() > self.event_limit {
      self.events.pop_front();
    }
  }
}

impl From<Vec<DocumentDiagnostics>> for DiagnosticsState {
  fn from(documents: Vec<DocumentDiagnostics>) -> Self {
    let mut state = DiagnosticsState::default();
    for document in documents {
      state.apply_document(document);
    }
    state
  }
}

impl From<DiagnosticsState> for Vec<DocumentDiagnostics> {
  fn from(mut value: DiagnosticsState) -> Self {
    let mut out = Vec::with_capacity(value.documents.len());
    for (_, document) in mem::take(&mut value.documents) {
      out.push(document);
    }
    out
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn diagnostic(severity: Option<DiagnosticSeverity>) -> Diagnostic {
    Diagnostic {
      range: DiagnosticRange {
        start: DiagnosticPosition {
          line:      0,
          character: 0,
        },
        end:   DiagnosticPosition {
          line:      0,
          character: 1,
        },
      },
      severity,
      code: None,
      source: None,
      message: "x".into(),
    }
  }

  #[test]
  fn apply_document_updates_counts() {
    let mut state = DiagnosticsState::default();
    let document = DocumentDiagnostics {
      uri:         "file:///tmp/a.rs".into(),
      version:     Some(2),
      diagnostics: vec![
        diagnostic(Some(DiagnosticSeverity::Error)),
        diagnostic(Some(DiagnosticSeverity::Warning)),
      ],
    };
    let counts = state.apply_document(document);
    assert_eq!(counts.total, 2);
    assert_eq!(counts.errors, 1);
    assert_eq!(counts.warnings, 1);
    assert_eq!(state.snapshot().document_count, 1);
  }

  #[test]
  fn empty_document_payload_clears_uri() {
    let mut state = DiagnosticsState::default();
    let uri = "file:///tmp/a.rs";
    let document = DocumentDiagnostics {
      uri:         uri.into(),
      version:     None,
      diagnostics: vec![diagnostic(Some(DiagnosticSeverity::Error))],
    };
    state.apply_document(document);
    let clear_payload = DocumentDiagnostics {
      uri:         uri.into(),
      version:     Some(3),
      diagnostics: Vec::new(),
    };
    let counts = state.apply_document(clear_payload);
    assert!(counts.is_empty());
    assert!(state.document(uri).is_none());
    let events = state.events_since(0);
    assert!(matches!(
      events.last().map(|event| &event.kind),
      Some(DiagnosticsEventKind::Cleared { .. })
    ));
  }
}
