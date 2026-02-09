use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{
  Value,
  json,
};
use thiserror::Error;

use crate::navigation::{
  LspPosition,
  LspRange,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspTextEdit {
  pub range:    LspRange,
  pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDocumentEdit {
  pub uri:     String,
  pub version: Option<i32>,
  pub edits:   Vec<LspTextEdit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LspWorkspaceEdit {
  pub documents: Vec<LspDocumentEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCompletionItem {
  pub label:            String,
  pub primary_edit:     Option<LspTextEdit>,
  pub additional_edits: Vec<LspTextEdit>,
  pub insert_text:      Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSignatureHelp {
  pub label:            String,
  pub active_parameter: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LspCodeAction {
  pub title:        String,
  pub edit:         Option<LspWorkspaceEdit>,
  pub command:      Option<LspExecuteCommand>,
  pub is_preferred: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LspExecuteCommand {
  pub command:   String,
  pub arguments: Option<Vec<Value>>,
}

#[derive(Debug, Error)]
pub enum EditingParseError {
  #[error("invalid lsp editing result shape")]
  InvalidShape,
  #[error("failed to decode lsp editing payload: {0}")]
  Decode(#[from] serde_json::Error),
}

pub fn completion_params(uri: &str, position: LspPosition) -> Value {
  json!({
    "textDocument": { "uri": uri },
    "position": {
      "line": position.line,
      "character": position.character,
    },
    "context": { "triggerKind": 1 },
  })
}

pub fn signature_help_params(uri: &str, position: LspPosition) -> Value {
  json!({
    "textDocument": { "uri": uri },
    "position": {
      "line": position.line,
      "character": position.character,
    },
  })
}

pub fn code_action_params(
  uri: &str,
  range: LspRange,
  diagnostics: Value,
  only: Option<Vec<String>>,
) -> Value {
  let mut value = json!({
    "textDocument": { "uri": uri },
    "range": {
      "start": { "line": range.start.line, "character": range.start.character },
      "end": { "line": range.end.line, "character": range.end.character },
    },
    "context": {
      "diagnostics": diagnostics,
    },
  });
  if let Some(only) = only
    && let Some(context) = value.get_mut("context").and_then(Value::as_object_mut)
  {
    context.insert("only".into(), json!(only));
  }
  value
}

pub fn rename_params(uri: &str, position: LspPosition, new_name: &str) -> Value {
  json!({
    "textDocument": { "uri": uri },
    "position": {
      "line": position.line,
      "character": position.character,
    },
    "newName": new_name,
  })
}

pub fn formatting_params(uri: &str, tab_size: u32, insert_spaces: bool) -> Value {
  json!({
    "textDocument": { "uri": uri },
    "options": {
      "tabSize": tab_size,
      "insertSpaces": insert_spaces,
      "trimTrailingWhitespace": true,
      "insertFinalNewline": true,
      "trimFinalNewlines": true,
    },
  })
}

pub fn execute_command_params(command: &str, arguments: Option<Vec<Value>>) -> Value {
  match arguments {
    Some(arguments) => {
      json!({
        "command": command,
        "arguments": arguments,
      })
    },
    None => {
      json!({
        "command": command,
      })
    },
  }
}

pub fn parse_completion_response(
  result: Option<&Value>,
) -> Result<Vec<LspCompletionItem>, EditingParseError> {
  let Some(result) = result else {
    return Ok(Vec::new());
  };
  if result.is_null() {
    return Ok(Vec::new());
  }

  if let Ok(list) = serde_json::from_value::<CompletionListPayload>(result.clone()) {
    return Ok(
      list
        .items
        .into_iter()
        .map(CompletionItemPayload::into_item)
        .collect(),
    );
  }

  if let Ok(items) = serde_json::from_value::<Vec<CompletionItemPayload>>(result.clone()) {
    return Ok(
      items
        .into_iter()
        .map(CompletionItemPayload::into_item)
        .collect(),
    );
  }

  Err(EditingParseError::InvalidShape)
}

pub fn parse_signature_help_response(
  result: Option<&Value>,
) -> Result<Option<LspSignatureHelp>, EditingParseError> {
  let Some(result) = result else {
    return Ok(None);
  };
  if result.is_null() {
    return Ok(None);
  }

  let payload: SignatureHelpPayload = serde_json::from_value(result.clone())?;
  let mut signatures = payload.signatures;
  if signatures.is_empty() {
    return Ok(None);
  }
  let active_signature = payload
    .active_signature
    .unwrap_or(0)
    .min(signatures.len().saturating_sub(1) as u32) as usize;
  let selected = signatures.swap_remove(active_signature);

  Ok(Some(LspSignatureHelp {
    label:            selected.label,
    active_parameter: payload.active_parameter,
  }))
}

pub fn parse_code_actions_response(
  result: Option<&Value>,
) -> Result<Vec<LspCodeAction>, EditingParseError> {
  let Some(result) = result else {
    return Ok(Vec::new());
  };
  if result.is_null() {
    return Ok(Vec::new());
  }

  let payload: Vec<CodeActionPayload> = serde_json::from_value(result.clone())?;
  Ok(
    payload
      .into_iter()
      .map(CodeActionPayload::into_code_action)
      .collect(),
  )
}

pub fn parse_workspace_edit_response(
  result: Option<&Value>,
) -> Result<Option<LspWorkspaceEdit>, EditingParseError> {
  let Some(result) = result else {
    return Ok(None);
  };
  if result.is_null() {
    return Ok(None);
  }
  Ok(Some(parse_workspace_edit_payload(result.clone())?))
}

pub fn parse_formatting_response(
  result: Option<&Value>,
) -> Result<Vec<LspTextEdit>, EditingParseError> {
  let Some(result) = result else {
    return Ok(Vec::new());
  };
  if result.is_null() {
    return Ok(Vec::new());
  }
  let edits: Vec<TextEditPayload> = serde_json::from_value(result.clone())?;
  Ok(
    edits
      .into_iter()
      .map(TextEditPayload::into_text_edit)
      .collect(),
  )
}

fn parse_workspace_edit_payload(value: Value) -> Result<LspWorkspaceEdit, EditingParseError> {
  let payload: WorkspaceEditPayload = serde_json::from_value(value)?;
  Ok(workspace_edit_from_payload(payload))
}

fn workspace_edit_from_payload(payload: WorkspaceEditPayload) -> LspWorkspaceEdit {
  let mut per_uri: BTreeMap<String, LspDocumentEdit> = BTreeMap::new();

  for (uri, edits) in payload.changes {
    let entry = per_uri.entry(uri.clone()).or_insert_with(|| {
      LspDocumentEdit {
        uri,
        version: None,
        edits: Vec::new(),
      }
    });
    entry
      .edits
      .extend(edits.into_iter().map(TextEditPayload::into_text_edit));
  }

  for change in payload.document_changes {
    let Some(text_document) = change.into_text_document_edit() else {
      continue;
    };

    let entry = per_uri.entry(text_document.uri.clone()).or_insert_with(|| {
      LspDocumentEdit {
        uri:     text_document.uri.clone(),
        version: text_document.version,
        edits:   Vec::new(),
      }
    });
    if entry.version.is_none() {
      entry.version = text_document.version;
    }
    entry.edits.extend(
      text_document
        .edits
        .into_iter()
        .map(TextEditOrAnnotatedPayload::into_text_edit),
    );
  }

  LspWorkspaceEdit {
    documents: per_uri.into_values().collect(),
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompletionListPayload {
  #[serde(default)]
  items: Vec<CompletionItemPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompletionItemPayload {
  label:                 String,
  insert_text:           Option<String>,
  text_edit:             Option<CompletionTextEditPayload>,
  #[serde(default)]
  additional_text_edits: Vec<TextEditPayload>,
}

impl CompletionItemPayload {
  fn into_item(self) -> LspCompletionItem {
    let primary_edit = self
      .text_edit
      .map(CompletionTextEditPayload::into_text_edit);
    LspCompletionItem {
      label: self.label,
      primary_edit,
      additional_edits: self
        .additional_text_edits
        .into_iter()
        .map(TextEditPayload::into_text_edit)
        .collect(),
      insert_text: self.insert_text,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CompletionTextEditPayload {
  Simple(TextEditPayload),
  InsertReplace {
    #[serde(rename = "newText")]
    new_text: String,
    insert:   RangePayload,
    replace:  Option<RangePayload>,
  },
}

impl CompletionTextEditPayload {
  fn into_text_edit(self) -> LspTextEdit {
    match self {
      CompletionTextEditPayload::Simple(edit) => edit.into_text_edit(),
      CompletionTextEditPayload::InsertReplace {
        new_text,
        insert,
        replace: _replace,
      } => {
        LspTextEdit {
          range: insert.into_range(),
          new_text,
        }
      },
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignatureHelpPayload {
  #[serde(default)]
  signatures:       Vec<SignatureInformationPayload>,
  active_signature: Option<u32>,
  active_parameter: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SignatureInformationPayload {
  label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeActionPayload {
  title:        String,
  edit:         Option<WorkspaceEditPayload>,
  command:      Option<CommandPayload>,
  is_preferred: Option<bool>,
}

impl CodeActionPayload {
  fn into_code_action(self) -> LspCodeAction {
    LspCodeAction {
      title:        self.title,
      edit:         self
        .edit
        .map(workspace_edit_from_payload)
        .filter(|edit| !edit.documents.is_empty()),
      command:      self.command.map(CommandPayload::into_command),
      is_preferred: self.is_preferred.unwrap_or(false),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandPayload {
  command:   String,
  arguments: Option<Vec<Value>>,
}

impl CommandPayload {
  fn into_command(self) -> LspExecuteCommand {
    LspExecuteCommand {
      command:   self.command,
      arguments: self.arguments,
    }
  }
}

#[derive(Debug, Deserialize)]
struct WorkspaceEditPayload {
  #[serde(default)]
  changes:          BTreeMap<String, Vec<TextEditPayload>>,
  #[serde(default, rename = "documentChanges")]
  document_changes: Vec<DocumentChangePayload>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DocumentChangePayload {
  TextDocumentEdit {
    #[serde(rename = "textDocument")]
    text_document: VersionedTextDocumentIdentifierPayload,
    edits:         Vec<TextEditOrAnnotatedPayload>,
  },
  ResourceOperation {
    kind: String,
    uri:  Option<String>,
  },
}

impl DocumentChangePayload {
  fn into_text_document_edit(self) -> Option<VersionedTextDocumentEditPayload> {
    match self {
      DocumentChangePayload::TextDocumentEdit {
        text_document,
        edits,
      } => {
        Some(VersionedTextDocumentEditPayload {
          uri: text_document.uri,
          version: text_document.version,
          edits,
        })
      },
      DocumentChangePayload::ResourceOperation { kind, uri } => {
        let _ = (kind, uri);
        None
      },
    }
  }
}

#[derive(Debug)]
struct VersionedTextDocumentEditPayload {
  uri:     String,
  version: Option<i32>,
  edits:   Vec<TextEditOrAnnotatedPayload>,
}

#[derive(Debug, Deserialize)]
struct VersionedTextDocumentIdentifierPayload {
  uri:     String,
  version: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TextEditOrAnnotatedPayload {
  TextEdit(TextEditPayload),
  Annotated {
    range:         RangePayload,
    #[serde(rename = "newText")]
    new_text:      String,
    #[serde(rename = "annotationId")]
    annotation_id: Option<String>,
  },
}

impl TextEditOrAnnotatedPayload {
  fn into_text_edit(self) -> LspTextEdit {
    match self {
      TextEditOrAnnotatedPayload::TextEdit(edit) => edit.into_text_edit(),
      TextEditOrAnnotatedPayload::Annotated {
        range,
        new_text,
        annotation_id,
      } => {
        let _ = annotation_id;
        LspTextEdit {
          range: range.into_range(),
          new_text,
        }
      },
    }
  }
}

#[derive(Debug, Deserialize)]
struct TextEditPayload {
  range:    RangePayload,
  #[serde(rename = "newText")]
  new_text: String,
}

impl TextEditPayload {
  fn into_text_edit(self) -> LspTextEdit {
    LspTextEdit {
      range:    self.range.into_range(),
      new_text: self.new_text,
    }
  }
}

#[derive(Debug, Deserialize)]
struct RangePayload {
  start: PositionPayload,
  end:   PositionPayload,
}

impl RangePayload {
  fn into_range(self) -> LspRange {
    LspRange {
      start: self.start.into_position(),
      end:   self.end.into_position(),
    }
  }
}

#[derive(Debug, Deserialize)]
struct PositionPayload {
  line:      u32,
  character: u32,
}

impl PositionPayload {
  fn into_position(self) -> LspPosition {
    LspPosition {
      line:      self.line,
      character: self.character,
    }
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn parse_workspace_edit_changes_and_document_changes() {
    let value = json!({
      "changes": {
        "file:///tmp/a.rs": [
          {
            "range": {
              "start": { "line": 0, "character": 0 },
              "end": { "line": 0, "character": 1 }
            },
            "newText": "x"
          }
        ]
      },
      "documentChanges": [
        {
          "textDocument": { "uri": "file:///tmp/b.rs", "version": 2 },
          "edits": [
            {
              "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 1 }
              },
              "newText": "y"
            }
          ]
        }
      ]
    });

    let parsed = parse_workspace_edit_response(Some(&value))
      .expect("parse ok")
      .expect("some edit");
    assert_eq!(parsed.documents.len(), 2);
  }
}
