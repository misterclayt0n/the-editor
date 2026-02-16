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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspInsertTextFormat {
  PlainText,
  Snippet,
}

impl LspInsertTextFormat {
  fn from_lsp(value: u8) -> Option<Self> {
    match value {
      1 => Some(Self::PlainText),
      2 => Some(Self::Snippet),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspCompletionItemKind {
  Text,
  Method,
  Function,
  Constructor,
  Field,
  Variable,
  Class,
  Interface,
  Module,
  Property,
  Unit,
  Value,
  Enum,
  Keyword,
  Snippet,
  Color,
  File,
  Reference,
  Folder,
  EnumMember,
  Constant,
  Struct,
  Event,
  Operator,
  TypeParameter,
}

impl LspCompletionItemKind {
  pub fn from_lsp(value: u8) -> Option<Self> {
    match value {
      1 => Some(Self::Text),
      2 => Some(Self::Method),
      3 => Some(Self::Function),
      4 => Some(Self::Constructor),
      5 => Some(Self::Field),
      6 => Some(Self::Variable),
      7 => Some(Self::Class),
      8 => Some(Self::Interface),
      9 => Some(Self::Module),
      10 => Some(Self::Property),
      11 => Some(Self::Unit),
      12 => Some(Self::Value),
      13 => Some(Self::Enum),
      14 => Some(Self::Keyword),
      15 => Some(Self::Snippet),
      16 => Some(Self::Color),
      17 => Some(Self::File),
      18 => Some(Self::Reference),
      19 => Some(Self::Folder),
      20 => Some(Self::EnumMember),
      21 => Some(Self::Constant),
      22 => Some(Self::Struct),
      23 => Some(Self::Event),
      24 => Some(Self::Operator),
      25 => Some(Self::TypeParameter),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCompletionItem {
  pub label:              String,
  pub filter_text:        Option<String>,
  pub sort_text:          Option<String>,
  pub preselect:          bool,
  pub detail:             Option<String>,
  pub documentation:      Option<String>,
  pub kind:               Option<LspCompletionItemKind>,
  pub primary_edit:       Option<LspTextEdit>,
  pub additional_edits:   Vec<LspTextEdit>,
  pub insert_text:        Option<String>,
  pub insert_text_format: Option<LspInsertTextFormat>,
  pub commit_characters:  Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRenderedSnippet {
  pub text:              String,
  pub cursor_char_range: Option<std::ops::Range<usize>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LspCompletionResponse {
  pub items:     Vec<LspCompletionItem>,
  pub raw_items: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSignatureHelp {
  pub signatures:       Vec<LspSignatureInformation>,
  pub active_signature: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSignatureInformation {
  pub label:                  String,
  pub documentation:          Option<String>,
  pub active_parameter:       Option<u32>,
  pub active_parameter_range: Option<std::ops::Range<usize>>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspCompletionTriggerKind {
  Invoked,
  TriggerCharacter,
  TriggerForIncompleteCompletions,
}

impl LspCompletionTriggerKind {
  fn as_lsp_code(self) -> u8 {
    match self {
      Self::Invoked => 1,
      Self::TriggerCharacter => 2,
      Self::TriggerForIncompleteCompletions => 3,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCompletionContext {
  pub trigger_kind:      LspCompletionTriggerKind,
  pub trigger_character: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspSignatureHelpTriggerKind {
  Invoked,
  TriggerCharacter,
  ContentChange,
}

impl LspSignatureHelpTriggerKind {
  fn as_lsp_code(self) -> u8 {
    match self {
      Self::Invoked => 1,
      Self::TriggerCharacter => 2,
      Self::ContentChange => 3,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSignatureHelpContext {
  pub trigger_kind:      LspSignatureHelpTriggerKind,
  pub trigger_character: Option<String>,
  pub is_retrigger:      bool,
}

impl LspSignatureHelpContext {
  pub fn invoked() -> Self {
    Self {
      trigger_kind:      LspSignatureHelpTriggerKind::Invoked,
      trigger_character: None,
      is_retrigger:      false,
    }
  }

  pub fn trigger_character(ch: char) -> Self {
    Self {
      trigger_kind:      LspSignatureHelpTriggerKind::TriggerCharacter,
      trigger_character: Some(ch.to_string()),
      is_retrigger:      false,
    }
  }

  pub fn content_change_retrigger() -> Self {
    Self {
      trigger_kind:      LspSignatureHelpTriggerKind::ContentChange,
      trigger_character: None,
      is_retrigger:      true,
    }
  }
}

pub fn render_lsp_snippet(source: &str) -> LspRenderedSnippet {
  let chars: Vec<char> = source.chars().collect();
  let mut parser = LspSnippetParser::new(&chars);
  let _ = parser.render_fragment(0, None);
  let cursor_char_range = parser.cursor_char_range();
  let text = parser.output;
  LspRenderedSnippet {
    text,
    cursor_char_range,
  }
}

struct LspSnippetParser<'a> {
  chars:         &'a [char],
  output:        String,
  output_chars:  usize,
  first_tabstop: Option<(u32, std::ops::Range<usize>)>,
  final_tabstop: Option<std::ops::Range<usize>>,
}

impl<'a> LspSnippetParser<'a> {
  fn new(chars: &'a [char]) -> Self {
    Self {
      chars,
      output: String::new(),
      output_chars: 0,
      first_tabstop: None,
      final_tabstop: None,
    }
  }

  fn cursor_char_range(&self) -> Option<std::ops::Range<usize>> {
    self
      .first_tabstop
      .as_ref()
      .map(|(_, range)| range.clone())
      .or_else(|| self.final_tabstop.clone())
  }

  fn push_char(&mut self, ch: char) {
    self.output.push(ch);
    self.output_chars = self.output_chars.saturating_add(1);
  }

  fn push_str(&mut self, text: &str) {
    self.output.push_str(text);
    self.output_chars = self.output_chars.saturating_add(text.chars().count());
  }

  fn render_fragment(&mut self, mut index: usize, terminator: Option<char>) -> usize {
    while index < self.chars.len() {
      let ch = self.chars[index];
      if terminator == Some(ch) {
        return index + 1;
      }
      if ch == '\\' {
        if let Some(next) = self.chars.get(index + 1).copied() {
          self.push_char(next);
          index += 2;
        } else {
          index += 1;
        }
        continue;
      }
      if ch == '$'
        && let Some(next_index) = self.parse_dollar(index)
      {
        index = next_index;
        continue;
      }
      self.push_char(ch);
      index += 1;
    }
    index
  }

  fn parse_dollar(&mut self, index: usize) -> Option<usize> {
    let next = *self.chars.get(index + 1)?;
    if next.is_ascii_digit() {
      let (tabstop, next_index) = parse_tabstop_digits(self.chars, index + 1);
      let start = self.output_chars;
      self.record_tabstop(tabstop, start, start);
      return Some(next_index);
    }
    if next == '{' {
      return Some(self.parse_braced(index + 2));
    }
    if is_snippet_identifier_char(next) {
      let mut cursor = index + 1;
      while self
        .chars
        .get(cursor)
        .copied()
        .is_some_and(is_snippet_identifier_char)
      {
        cursor += 1;
      }
      return Some(cursor);
    }
    None
  }

  fn parse_braced(&mut self, mut index: usize) -> usize {
    let start = index;
    while self
      .chars
      .get(index)
      .copied()
      .is_some_and(is_snippet_identifier_char)
    {
      index += 1;
    }
    if index == start {
      return index;
    }
    let token: String = self.chars[start..index].iter().collect();
    let tabstop = token
      .chars()
      .all(|ch| ch.is_ascii_digit())
      .then(|| token.parse::<u32>().ok())
      .flatten();

    match self.chars.get(index).copied() {
      Some('}') => {
        if let Some(tabstop) = tabstop {
          let start = self.output_chars;
          self.record_tabstop(tabstop, start, start);
        }
        index + 1
      },
      Some(':') => {
        let start = self.output_chars;
        let next = self.render_fragment(index + 1, Some('}'));
        let end = self.output_chars;
        if let Some(tabstop) = tabstop {
          self.record_tabstop(tabstop, start, end);
        }
        next
      },
      Some('|') => {
        let start = self.output_chars;
        let (choice, next) = parse_snippet_choice(self.chars, index + 1);
        self.push_str(&choice);
        let end = self.output_chars;
        if let Some(tabstop) = tabstop {
          self.record_tabstop(tabstop, start, end);
        }
        next
      },
      Some(_) => {
        let mut cursor = index;
        while cursor < self.chars.len() && self.chars[cursor] != '}' {
          cursor += 1;
        }
        if cursor < self.chars.len() {
          cursor + 1
        } else {
          cursor
        }
      },
      None => index,
    }
  }

  fn record_tabstop(&mut self, tabstop: u32, start: usize, end: usize) {
    let range = start..end;
    if tabstop == 0 {
      if self.final_tabstop.is_none() {
        self.final_tabstop = Some(range);
      }
      return;
    }

    let should_replace = self
      .first_tabstop
      .as_ref()
      .is_none_or(|(current, _)| tabstop < *current);
    if should_replace {
      self.first_tabstop = Some((tabstop, range));
    }
  }
}

fn parse_tabstop_digits(chars: &[char], mut index: usize) -> (u32, usize) {
  let mut value: u32 = 0;
  while chars
    .get(index)
    .copied()
    .is_some_and(|ch| ch.is_ascii_digit())
  {
    let digit = chars[index].to_digit(10).unwrap_or(0);
    value = value.saturating_mul(10).saturating_add(digit);
    index += 1;
  }
  (value, index)
}

fn parse_snippet_choice(chars: &[char], mut index: usize) -> (String, usize) {
  let mut first_choice: Option<String> = None;
  let mut current = String::new();
  let mut escaped = false;
  while index < chars.len() {
    let ch = chars[index];
    if escaped {
      current.push(ch);
      escaped = false;
      index += 1;
      continue;
    }
    if ch == '\\' {
      escaped = true;
      index += 1;
      continue;
    }
    if ch == ',' {
      if first_choice.is_none() {
        first_choice = Some(current.clone());
      }
      current.clear();
      index += 1;
      continue;
    }
    if ch == '|' && chars.get(index + 1).copied() == Some('}') {
      if first_choice.is_none() {
        first_choice = Some(current);
      }
      return (first_choice.unwrap_or_default(), index + 2);
    }
    current.push(ch);
    index += 1;
  }
  (first_choice.unwrap_or(current), index)
}

fn is_snippet_identifier_char(ch: char) -> bool {
  ch.is_ascii_alphanumeric() || ch == '_'
}

impl LspCompletionContext {
  pub fn invoked() -> Self {
    Self {
      trigger_kind:      LspCompletionTriggerKind::Invoked,
      trigger_character: None,
    }
  }

  pub fn trigger_character(ch: char) -> Self {
    Self {
      trigger_kind:      LspCompletionTriggerKind::TriggerCharacter,
      trigger_character: Some(ch.to_string()),
    }
  }

  pub fn trigger_for_incomplete() -> Self {
    Self {
      trigger_kind:      LspCompletionTriggerKind::TriggerForIncompleteCompletions,
      trigger_character: None,
    }
  }
}

pub fn completion_params(
  uri: &str,
  position: LspPosition,
  context: &LspCompletionContext,
) -> Value {
  let mut context_json = json!({
    "triggerKind": context.trigger_kind.as_lsp_code(),
  });
  if let Some(ch) = context.trigger_character.as_deref()
    && let Some(object) = context_json.as_object_mut()
  {
    object.insert("triggerCharacter".to_string(), json!(ch));
  }

  json!({
    "textDocument": { "uri": uri },
    "position": {
      "line": position.line,
      "character": position.character,
    },
    "context": context_json,
  })
}

pub fn signature_help_params(
  uri: &str,
  position: LspPosition,
  context: &LspSignatureHelpContext,
) -> Value {
  let mut context_json = json!({
    "triggerKind": context.trigger_kind.as_lsp_code(),
    "isRetrigger": context.is_retrigger,
  });
  if let Some(ch) = context.trigger_character.as_deref()
    && let Some(object) = context_json.as_object_mut()
  {
    object.insert("triggerCharacter".to_string(), json!(ch));
  }

  json!({
    "textDocument": { "uri": uri },
    "position": {
      "line": position.line,
      "character": position.character,
    },
    "context": context_json,
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
  Ok(parse_completion_response_with_raw(result)?.items)
}

pub fn parse_completion_response_with_raw(
  result: Option<&Value>,
) -> Result<LspCompletionResponse, EditingParseError> {
  let Some(result) = result else {
    return Ok(LspCompletionResponse::default());
  };
  if result.is_null() {
    return Ok(LspCompletionResponse::default());
  }

  if let Ok(list) = serde_json::from_value::<CompletionListPayload>(result.clone()) {
    let defaults = list.item_defaults;
    let mut raw_items = result
      .get("items")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default();
    for raw in &mut raw_items {
      apply_completion_defaults_to_raw_item(raw, defaults.as_ref());
    }
    let items = if !raw_items.is_empty() {
      raw_items
        .iter()
        .cloned()
        .map(serde_json::from_value::<CompletionItemPayload>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|item| item.into_item(None))
        .collect()
    } else {
      list
        .items
        .into_iter()
        .map(|item| item.into_item(defaults.as_ref()))
        .collect()
    };
    return Ok(LspCompletionResponse { items, raw_items });
  }

  if let Ok(items) = serde_json::from_value::<Vec<CompletionItemPayload>>(result.clone()) {
    let raw_items = result.as_array().cloned().unwrap_or_default();
    let items = items.into_iter().map(|item| item.into_item(None)).collect();
    return Ok(LspCompletionResponse { items, raw_items });
  }

  Err(EditingParseError::InvalidShape)
}

pub fn parse_completion_item_response(
  result: Option<&Value>,
) -> Result<Option<LspCompletionItem>, EditingParseError> {
  let Some(result) = result else {
    return Ok(None);
  };
  if result.is_null() {
    return Ok(None);
  }
  let payload: CompletionItemPayload = serde_json::from_value(result.clone())?;
  Ok(Some(payload.into_item(None)))
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
  if payload.signatures.is_empty() {
    return Ok(None);
  }

  let active_signature = payload
    .active_signature
    .unwrap_or(0)
    .min(payload.signatures.len().saturating_sub(1) as u32) as usize;

  let signatures = payload
    .signatures
    .into_iter()
    .map(|signature| signature.into_signature_information(payload.active_parameter))
    .collect();

  Ok(Some(LspSignatureHelp {
    signatures,
    active_signature,
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
  items:         Vec<CompletionItemPayload>,
  item_defaults: Option<CompletionItemDefaultsPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompletionItemPayload {
  label:                 String,
  filter_text:           Option<String>,
  sort_text:             Option<String>,
  preselect:             Option<bool>,
  detail:                Option<String>,
  documentation:         Option<DocumentationPayload>,
  kind:                  Option<u8>,
  insert_text:           Option<String>,
  insert_text_format:    Option<u8>,
  #[serde(default)]
  commit_characters:     Option<Vec<String>>,
  text_edit:             Option<CompletionTextEditPayload>,
  #[serde(default)]
  additional_text_edits: Vec<TextEditPayload>,
}

impl CompletionItemPayload {
  fn into_item(self, defaults: Option<&CompletionItemDefaultsPayload>) -> LspCompletionItem {
    let commit_characters = self
      .commit_characters
      .or_else(|| defaults.and_then(|default| default.commit_characters.clone()))
      .unwrap_or_default();
    let insert_text_format = self
      .insert_text_format
      .and_then(LspInsertTextFormat::from_lsp)
      .or_else(|| {
        defaults.and_then(|default| {
          default
            .insert_text_format
            .and_then(LspInsertTextFormat::from_lsp)
        })
      });
    let primary_edit = self
      .text_edit
      .map(CompletionTextEditPayload::into_text_edit);
    LspCompletionItem {
      label: self.label,
      filter_text: self.filter_text,
      sort_text: self.sort_text,
      preselect: self.preselect.unwrap_or(false),
      detail: self.detail,
      documentation: self.documentation.map(DocumentationPayload::into_text),
      kind: self.kind.and_then(LspCompletionItemKind::from_lsp),
      primary_edit,
      additional_edits: self
        .additional_text_edits
        .into_iter()
        .map(TextEditPayload::into_text_edit)
        .collect(),
      insert_text: self.insert_text,
      insert_text_format,
      commit_characters,
    }
  }
}

fn apply_completion_defaults_to_raw_item(
  raw_item: &mut Value,
  defaults: Option<&CompletionItemDefaultsPayload>,
) {
  let Some(defaults) = defaults else {
    return;
  };
  let Some(obj) = raw_item.as_object_mut() else {
    return;
  };

  if !obj.contains_key("commitCharacters")
    && let Some(value) = defaults.commit_characters.clone()
  {
    obj.insert("commitCharacters".to_string(), json!(value));
  }
  if !obj.contains_key("insertTextFormat")
    && let Some(value) = defaults.insert_text_format
  {
    obj.insert("insertTextFormat".to_string(), json!(value));
  }
  if !obj.contains_key("data")
    && let Some(value) = defaults.data.clone()
  {
    obj.insert("data".to_string(), value);
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompletionItemDefaultsPayload {
  commit_characters:  Option<Vec<String>>,
  insert_text_format: Option<u8>,
  data:               Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DocumentationPayload {
  String(String),
  Markup(MarkupContentPayload),
}

impl DocumentationPayload {
  fn into_text(self) -> String {
    match self {
      Self::String(value) => value,
      Self::Markup(markup) => markup.value,
    }
  }
}

#[derive(Debug, Deserialize)]
struct MarkupContentPayload {
  value: String,
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
  label:            String,
  documentation:    Option<DocumentationPayload>,
  parameters:       Option<Vec<SignatureParameterPayload>>,
  active_parameter: Option<u32>,
}

impl SignatureInformationPayload {
  fn into_signature_information(
    self,
    response_active_parameter: Option<u32>,
  ) -> LspSignatureInformation {
    let active_parameter = self.active_parameter.or(response_active_parameter);
    let active_parameter_range = active_parameter.and_then(|param_idx| {
      let param_idx = param_idx as usize;
      let parameters = self.parameters.as_ref()?;
      let parameter = parameters.get(param_idx)?;
      parameter.label.as_label_range(&self.label)
    });

    LspSignatureInformation {
      label: self.label,
      documentation: self.documentation.map(DocumentationPayload::into_text),
      active_parameter,
      active_parameter_range,
    }
  }
}

#[derive(Debug, Deserialize)]
struct SignatureParameterPayload {
  label: SignatureParameterLabelPayload,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SignatureParameterLabelPayload {
  Simple(String),
  Offsets([u32; 2]),
}

impl SignatureParameterLabelPayload {
  fn as_label_range(&self, signature_label: &str) -> Option<std::ops::Range<usize>> {
    match self {
      SignatureParameterLabelPayload::Simple(text) => {
        let start = signature_label.find(text.as_str())?;
        Some(start..start + text.len())
      },
      SignatureParameterLabelPayload::Offsets([start, end]) => {
        let start = utf16_code_units_to_byte_idx(signature_label, *start as usize)?;
        let end = utf16_code_units_to_byte_idx(signature_label, *end as usize)?;
        if start >= end {
          None
        } else {
          Some(start..end)
        }
      },
    }
  }
}

fn utf16_code_units_to_byte_idx(text: &str, utf16_idx: usize) -> Option<usize> {
  let mut consumed = 0usize;
  for (byte_idx, ch) in text.char_indices() {
    if consumed == utf16_idx {
      return Some(byte_idx);
    }
    consumed = consumed.saturating_add(ch.len_utf16());
    if consumed > utf16_idx {
      return None;
    }
  }
  if consumed == utf16_idx {
    Some(text.len())
  } else {
    None
  }
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

  #[test]
  fn parse_completion_response_applies_item_defaults_and_metadata() {
    let value = json!({
      "items": [
        {
          "label": "println!",
          "filterText": "println",
          "sortText": "0001",
          "preselect": true,
          "detail": "macro_rules!",
          "documentation": {
            "kind": "markdown",
            "value": "Prints to stdout."
          },
          "insertText": "println!($1)$0",
          "insertTextFormat": 2,
          "commitCharacters": [";"]
        },
        {
          "label": "dbg!",
          "documentation": "Debug macro",
          "insertText": "dbg!($1)"
        }
      ],
      "itemDefaults": {
        "commitCharacters": ["."],
        "insertTextFormat": 2
      }
    });

    let parsed = parse_completion_response(Some(&value)).expect("completion parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].filter_text.as_deref(), Some("println"));
    assert_eq!(parsed[0].sort_text.as_deref(), Some("0001"));
    assert!(parsed[0].preselect);
    assert_eq!(parsed[0].detail.as_deref(), Some("macro_rules!"));
    assert_eq!(
      parsed[0].documentation.as_deref(),
      Some("Prints to stdout.")
    );
    assert_eq!(
      parsed[0].insert_text_format,
      Some(LspInsertTextFormat::Snippet)
    );
    assert_eq!(parsed[0].commit_characters, vec![";".to_string()]);
    assert_eq!(parsed[1].documentation.as_deref(), Some("Debug macro"));
    assert!(!parsed[1].preselect);
    assert_eq!(
      parsed[1].insert_text_format,
      Some(LspInsertTextFormat::Snippet)
    );
    assert_eq!(parsed[1].commit_characters, vec![".".to_string()]);
  }

  #[test]
  fn parse_completion_response_with_raw_applies_default_data_to_raw_items() {
    let value = json!({
      "items": [
        {
          "label": "alpha"
        },
        {
          "label": "beta",
          "data": { "x": 1 }
        }
      ],
      "itemDefaults": {
        "data": { "kind": "default" }
      }
    });

    let parsed = parse_completion_response_with_raw(Some(&value)).expect("completion parse");
    assert_eq!(parsed.items.len(), 2);
    assert_eq!(parsed.raw_items.len(), 2);
    assert_eq!(
      parsed.raw_items[0].get("data"),
      Some(&json!({ "kind": "default" }))
    );
    assert_eq!(parsed.raw_items[1].get("data"), Some(&json!({ "x": 1 })));
  }

  #[test]
  fn parse_completion_item_response_handles_single_item() {
    let value = json!({
      "label": "abc",
      "detail": "detail",
      "documentation": "docs"
    });
    let parsed = parse_completion_item_response(Some(&value))
      .expect("parse ok")
      .expect("item");
    assert_eq!(parsed.label, "abc");
    assert_eq!(parsed.detail.as_deref(), Some("detail"));
    assert_eq!(parsed.documentation.as_deref(), Some("docs"));
  }

  #[test]
  fn completion_params_sets_invoked_context() {
    let params = completion_params(
      "file:///tmp/main.rs",
      LspPosition {
        line:      3,
        character: 5,
      },
      &LspCompletionContext::invoked(),
    );
    assert_eq!(params["context"]["triggerKind"], json!(1));
    assert!(params["context"].get("triggerCharacter").is_none());
  }

  #[test]
  fn completion_params_sets_trigger_character_context() {
    let params = completion_params(
      "file:///tmp/main.rs",
      LspPosition {
        line:      3,
        character: 5,
      },
      &LspCompletionContext::trigger_character(':'),
    );
    assert_eq!(params["context"]["triggerKind"], json!(2));
    assert_eq!(params["context"]["triggerCharacter"], json!(":"));
  }

  #[test]
  fn signature_help_params_sets_invoked_context() {
    let params = signature_help_params(
      "file:///tmp/main.rs",
      LspPosition {
        line:      3,
        character: 5,
      },
      &LspSignatureHelpContext::invoked(),
    );
    assert_eq!(params["context"]["triggerKind"], json!(1));
    assert_eq!(params["context"]["isRetrigger"], json!(false));
    assert!(params["context"].get("triggerCharacter").is_none());
  }

  #[test]
  fn signature_help_params_sets_trigger_character_context() {
    let params = signature_help_params(
      "file:///tmp/main.rs",
      LspPosition {
        line:      3,
        character: 5,
      },
      &LspSignatureHelpContext::trigger_character('('),
    );
    assert_eq!(params["context"]["triggerKind"], json!(2));
    assert_eq!(params["context"]["isRetrigger"], json!(false));
    assert_eq!(params["context"]["triggerCharacter"], json!("("));
  }

  #[test]
  fn parse_signature_help_response_retains_docs_and_active_parameter_range() {
    let value = json!({
      "signatures": [
        {
          "label": "foo(first: i32, second: u32)",
          "documentation": {
            "kind": "markdown",
            "value": "docs for foo"
          },
          "parameters": [
            { "label": "first: i32" },
            { "label": [16, 27] }
          ]
        }
      ],
      "activeSignature": 0,
      "activeParameter": 1
    });

    let parsed = parse_signature_help_response(Some(&value))
      .expect("parse ok")
      .expect("signature help");
    assert_eq!(parsed.active_signature, 0);
    assert_eq!(parsed.signatures.len(), 1);
    let signature = &parsed.signatures[0];
    assert_eq!(
      signature.documentation.as_deref(),
      Some("docs for foo")
    );
    assert_eq!(signature.active_parameter, Some(1));
    assert_eq!(
      signature.active_parameter_range,
      Some(16..27)
    );
  }

  #[test]
  fn render_lsp_snippet_renders_fallback_text_and_cursor_range() {
    let rendered = render_lsp_snippet("foo($1, ${2:bar}, ${3|x,y|})$0");
    assert_eq!(rendered.text, "foo(, bar, x)");
    assert_eq!(rendered.cursor_char_range, Some(4..4));
  }

  #[test]
  fn render_lsp_snippet_prefers_lowest_nonzero_tabstop() {
    let rendered = render_lsp_snippet("${2:bar}${1:foo}$0");
    assert_eq!(rendered.text, "barfoo");
    assert_eq!(rendered.cursor_char_range, Some(3..6));
  }

  #[test]
  fn render_lsp_snippet_renders_variable_defaults_and_escapes() {
    let rendered = render_lsp_snippet("${TM_FILENAME:main}.rs");
    assert_eq!(rendered.text, "main.rs");
    assert_eq!(rendered.cursor_char_range, None);

    let escaped = render_lsp_snippet("a\\$b\\}");
    assert_eq!(escaped.text, "a$b}");
  }
}
