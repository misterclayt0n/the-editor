use std::collections::{
  HashMap,
  HashSet,
};

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LspCapability {
  Format,
  GotoDeclaration,
  GotoDefinition,
  GotoTypeDefinition,
  GotoReference,
  GotoImplementation,
  SignatureHelp,
  Hover,
  DocumentHighlight,
  Completion,
  CodeAction,
  WorkspaceCommand,
  DocumentSymbols,
  WorkspaceSymbols,
  Diagnostics,
  RenameSymbol,
  InlayHints,
  DocumentColors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDocumentSyncKind {
  None,
  Full,
  Incremental,
}

impl TextDocumentSyncKind {
  fn from_lsp_code(code: i64) -> Self {
    match code {
      1 => Self::Full,
      2 => Self::Incremental,
      _ => Self::None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDocumentSyncOptions {
  pub open_close:        bool,
  pub kind:              TextDocumentSyncKind,
  pub save_include_text: bool,
}

impl Default for TextDocumentSyncOptions {
  fn default() -> Self {
    Self {
      open_close:        false,
      kind:              TextDocumentSyncKind::None,
      save_include_text: false,
    }
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerCapabilitiesSnapshot {
  raw:                Value,
  supported:          HashSet<LspCapability>,
  text_document_sync: TextDocumentSyncOptions,
}

impl ServerCapabilitiesSnapshot {
  pub fn from_raw(raw: Value) -> Self {
    let mut supported = HashSet::new();
    let text_document_sync = parse_text_document_sync(&raw);

    if capability_present(&raw, "documentFormattingProvider") {
      supported.insert(LspCapability::Format);
    }
    if capability_present(&raw, "declarationProvider") {
      supported.insert(LspCapability::GotoDeclaration);
    }
    if capability_present(&raw, "definitionProvider") {
      supported.insert(LspCapability::GotoDefinition);
    }
    if capability_present(&raw, "typeDefinitionProvider") {
      supported.insert(LspCapability::GotoTypeDefinition);
    }
    if capability_present(&raw, "referencesProvider") {
      supported.insert(LspCapability::GotoReference);
    }
    if capability_present(&raw, "implementationProvider") {
      supported.insert(LspCapability::GotoImplementation);
    }
    if capability_present(&raw, "signatureHelpProvider") {
      supported.insert(LspCapability::SignatureHelp);
    }
    if capability_present(&raw, "hoverProvider") {
      supported.insert(LspCapability::Hover);
    }
    if capability_present(&raw, "documentHighlightProvider") {
      supported.insert(LspCapability::DocumentHighlight);
    }
    if capability_present(&raw, "completionProvider") {
      supported.insert(LspCapability::Completion);
    }
    if capability_present(&raw, "codeActionProvider") {
      supported.insert(LspCapability::CodeAction);
    }
    if capability_present(&raw, "executeCommandProvider") {
      supported.insert(LspCapability::WorkspaceCommand);
    }
    if capability_present(&raw, "documentSymbolProvider") {
      supported.insert(LspCapability::DocumentSymbols);
    }
    if capability_present(&raw, "workspaceSymbolProvider") {
      supported.insert(LspCapability::WorkspaceSymbols);
    }
    if capability_present(&raw, "diagnosticProvider") {
      supported.insert(LspCapability::Diagnostics);
    }
    if capability_present(&raw, "renameProvider") {
      supported.insert(LspCapability::RenameSymbol);
    }
    if capability_present(&raw, "inlayHintProvider") {
      supported.insert(LspCapability::InlayHints);
    }
    if capability_present(&raw, "colorProvider") {
      supported.insert(LspCapability::DocumentColors);
    }

    Self {
      raw,
      supported,
      text_document_sync,
    }
  }

  pub fn raw(&self) -> &Value {
    &self.raw
  }

  pub fn supports(&self, capability: LspCapability) -> bool {
    self.supported.contains(&capability)
  }

  pub fn supported(&self) -> &HashSet<LspCapability> {
    &self.supported
  }

  pub fn text_document_sync(&self) -> TextDocumentSyncOptions {
    self.text_document_sync
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CapabilityRegistry {
  servers: HashMap<String, ServerCapabilitiesSnapshot>,
}

impl CapabilityRegistry {
  pub fn register(&mut self, server_name: impl Into<String>, raw_capabilities: Value) {
    self.servers.insert(
      server_name.into(),
      ServerCapabilitiesSnapshot::from_raw(raw_capabilities),
    );
  }

  pub fn get(&self, server_name: &str) -> Option<&ServerCapabilitiesSnapshot> {
    self.servers.get(server_name)
  }

  pub fn supports(&self, server_name: &str, capability: LspCapability) -> bool {
    self
      .get(server_name)
      .is_some_and(|snapshot| snapshot.supports(capability))
  }

  pub fn remove(&mut self, server_name: &str) -> Option<ServerCapabilitiesSnapshot> {
    self.servers.remove(server_name)
  }

  pub fn clear(&mut self) {
    self.servers.clear();
  }

  pub fn is_empty(&self) -> bool {
    self.servers.is_empty()
  }

  pub fn server_names(&self) -> impl Iterator<Item = &str> {
    self.servers.keys().map(String::as_str)
  }
}

fn capability_present(raw: &Value, key: &str) -> bool {
  match raw.get(key) {
    Some(Value::Bool(enabled)) => *enabled,
    Some(Value::Null) | None => false,
    Some(_) => true,
  }
}

fn parse_text_document_sync(raw: &Value) -> TextDocumentSyncOptions {
  let Some(sync) = raw.get("textDocumentSync") else {
    return TextDocumentSyncOptions::default();
  };

  match sync {
    Value::Number(number) => {
      TextDocumentSyncOptions {
        kind: number
          .as_i64()
          .map(TextDocumentSyncKind::from_lsp_code)
          .unwrap_or(TextDocumentSyncKind::None),
        ..TextDocumentSyncOptions::default()
      }
    },
    Value::Object(object) => {
      let kind = object
        .get("change")
        .and_then(Value::as_i64)
        .map(TextDocumentSyncKind::from_lsp_code)
        .unwrap_or(TextDocumentSyncKind::None);
      let open_close = object
        .get("openClose")
        .and_then(Value::as_bool)
        .unwrap_or(false);
      let save_include_text = match object.get("save") {
        Some(Value::Object(save)) => {
          save
            .get("includeText")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        },
        _ => false,
      };

      TextDocumentSyncOptions {
        open_close,
        kind,
        save_include_text,
      }
    },
    _ => TextDocumentSyncOptions::default(),
  }
}
