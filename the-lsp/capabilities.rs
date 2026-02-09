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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerCapabilitiesSnapshot {
  raw:       Value,
  supported: HashSet<LspCapability>,
}

impl ServerCapabilitiesSnapshot {
  pub fn from_raw(raw: Value) -> Self {
    let mut supported = HashSet::new();

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

    Self { raw, supported }
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
