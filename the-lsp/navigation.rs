use serde::Deserialize;
use serde_json::{
  Value,
  json,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspPosition {
  pub line:      u32,
  pub character: u32,
}

impl LspPosition {
  fn as_json(self) -> Value {
    json!({
      "line": self.line,
      "character": self.character,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspRange {
  pub start: LspPosition,
  pub end:   LspPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspLocation {
  pub uri:   String,
  pub range: LspRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSymbol {
  pub name:           String,
  pub detail:         Option<String>,
  pub kind:           u32,
  pub container_name: Option<String>,
  pub location:       Option<LspLocation>,
}

#[derive(Debug, Error)]
pub enum NavigationParseError {
  #[error("invalid lsp navigation result shape")]
  InvalidShape,
  #[error("failed to decode lsp navigation payload: {0}")]
  Decode(#[from] serde_json::Error),
}

pub fn goto_definition_params(uri: &str, position: LspPosition) -> Value {
  text_document_position_params(uri, position)
}

pub fn hover_params(uri: &str, position: LspPosition) -> Value {
  text_document_position_params(uri, position)
}

pub fn references_params(uri: &str, position: LspPosition, include_declaration: bool) -> Value {
  json!({
    "textDocument": { "uri": uri },
    "position": position.as_json(),
    "context": { "includeDeclaration": include_declaration },
  })
}

pub fn document_symbols_params(uri: &str) -> Value {
  json!({
    "textDocument": { "uri": uri },
  })
}

pub fn workspace_symbols_params(query: &str) -> Value {
  json!({
    "query": query,
  })
}

pub fn parse_locations_response(
  result: Option<&Value>,
) -> Result<Vec<LspLocation>, NavigationParseError> {
  let Some(result) = result else {
    return Ok(Vec::new());
  };
  if result.is_null() {
    return Ok(Vec::new());
  }

  if let Ok(location) = serde_json::from_value::<LocationPayload>(result.clone()) {
    return Ok(vec![location.into_location()]);
  }

  if let Ok(locations) = serde_json::from_value::<Vec<LocationPayload>>(result.clone()) {
    return Ok(
      locations
        .into_iter()
        .map(LocationPayload::into_location)
        .collect(),
    );
  }

  if let Ok(links) = serde_json::from_value::<Vec<LocationLinkPayload>>(result.clone()) {
    return Ok(
      links
        .into_iter()
        .map(LocationLinkPayload::into_location)
        .collect(),
    );
  }

  Err(NavigationParseError::InvalidShape)
}

pub fn parse_hover_response(
  result: Option<&Value>,
) -> Result<Option<String>, NavigationParseError> {
  let Some(result) = result else {
    return Ok(None);
  };
  if result.is_null() {
    return Ok(None);
  }

  let contents = result
    .get("contents")
    .ok_or(NavigationParseError::InvalidShape)?;
  let text = hover_contents_to_text(contents).ok_or(NavigationParseError::InvalidShape)?;
  Ok(normalize_hover_text(&text))
}

pub fn parse_document_symbols_response(
  uri: &str,
  result: Option<&Value>,
) -> Result<Vec<LspSymbol>, NavigationParseError> {
  let Some(result) = result else {
    return Ok(Vec::new());
  };
  if result.is_null() {
    return Ok(Vec::new());
  }

  if let Ok(symbols) = serde_json::from_value::<Vec<DocumentSymbolPayload>>(result.clone()) {
    let mut out = Vec::new();
    for symbol in symbols {
      flatten_document_symbol(uri, symbol, None, &mut out);
    }
    return Ok(out);
  }

  if let Ok(information) = serde_json::from_value::<Vec<SymbolInformationPayload>>(result.clone()) {
    return Ok(
      information
        .into_iter()
        .map(SymbolInformationPayload::into_symbol)
        .collect(),
    );
  }

  Err(NavigationParseError::InvalidShape)
}

pub fn parse_workspace_symbols_response(
  result: Option<&Value>,
) -> Result<Vec<LspSymbol>, NavigationParseError> {
  let Some(result) = result else {
    return Ok(Vec::new());
  };
  if result.is_null() {
    return Ok(Vec::new());
  }

  if let Ok(symbols) = serde_json::from_value::<Vec<WorkspaceSymbolPayload>>(result.clone()) {
    return Ok(
      symbols
        .into_iter()
        .map(WorkspaceSymbolPayload::into_symbol)
        .collect(),
    );
  }

  if let Ok(information) = serde_json::from_value::<Vec<SymbolInformationPayload>>(result.clone()) {
    return Ok(
      information
        .into_iter()
        .map(SymbolInformationPayload::into_symbol)
        .collect(),
    );
  }

  Err(NavigationParseError::InvalidShape)
}

fn text_document_position_params(uri: &str, position: LspPosition) -> Value {
  json!({
    "textDocument": { "uri": uri },
    "position": position.as_json(),
  })
}

fn hover_contents_to_text(value: &Value) -> Option<String> {
  if let Some(text) = value.as_str() {
    return Some(text.to_string());
  }

  if let Some(array) = value.as_array() {
    let joined = array
      .iter()
      .filter_map(hover_contents_to_text)
      .collect::<Vec<_>>()
      .join("\n\n");
    if joined.is_empty() {
      return None;
    }
    return Some(joined);
  }

  let object = value.as_object()?;
  let value = object.get("value").and_then(Value::as_str)?;
  if let Some(language) = object.get("language").and_then(Value::as_str) {
    // MarkedString with explicit language: preserve language context via fenced
    // block unless content is already markdown.
    if language.eq_ignore_ascii_case("markdown") || language.eq_ignore_ascii_case("md") {
      return Some(value.to_string());
    }
    return Some(format!("```{language}\n{value}\n```"));
  }
  if let Some(kind) = object.get("kind").and_then(Value::as_str) {
    if kind.eq_ignore_ascii_case("markdown") || kind.eq_ignore_ascii_case("md") {
      return Some(value.to_string());
    }
    if kind.eq_ignore_ascii_case("plaintext") || kind.eq_ignore_ascii_case("text") {
      return Some(plaintext_hover_to_markdown(value));
    }
  }
  Some(value.to_string())
}

fn plaintext_hover_to_markdown(value: &str) -> String {
  let trimmed = value.trim();
  if trimmed.is_empty() || trimmed.contains("```") {
    return trimmed.to_string();
  }

  let paragraphs = split_plaintext_hover_paragraphs(trimmed);
  if paragraphs.is_empty() {
    return String::new();
  }

  let mut code_prefix_count = 0usize;
  for paragraph in &paragraphs {
    if hover_paragraph_looks_like_code(paragraph) {
      code_prefix_count = code_prefix_count.saturating_add(1);
    } else {
      break;
    }
  }
  if code_prefix_count == 0 {
    return trimmed.to_string();
  }

  let code_block = paragraphs[..code_prefix_count].join("\n\n");
  let body = if code_prefix_count < paragraphs.len() {
    paragraphs[code_prefix_count..].join("\n\n")
  } else {
    String::new()
  };

  if body.is_empty() {
    format!("```\n{code_block}\n```")
  } else {
    format!("```\n{code_block}\n```\n\n{body}")
  }
}

fn split_plaintext_hover_paragraphs(text: &str) -> Vec<String> {
  let mut paragraphs = Vec::new();
  let mut current = Vec::new();
  for raw_line in text.lines() {
    let line = raw_line.trim_end();
    if line.trim().is_empty() {
      if !current.is_empty() {
        let paragraph = current.join("\n").trim().to_string();
        if !paragraph.is_empty() {
          paragraphs.push(paragraph);
        }
        current.clear();
      }
      continue;
    }
    current.push(line.to_string());
  }
  if !current.is_empty() {
    let paragraph = current.join("\n").trim().to_string();
    if !paragraph.is_empty() {
      paragraphs.push(paragraph);
    }
  }
  paragraphs
}

fn hover_paragraph_looks_like_code(value: &str) -> bool {
  let lower = value.to_ascii_lowercase();
  let first_line = value.lines().next().unwrap_or_default().trim();
  value.contains("::")
    || value.contains("->")
    || value.contains("=>")
    || value
      .chars()
      .any(|ch| matches!(ch, '{' | '}' | '(' | ')' | '[' | ']' | ';' | '<' | '>'))
    || lower.contains("fn ")
    || lower.contains("pub ")
    || lower.contains("struct ")
    || lower.contains("enum ")
    || lower.contains("trait ")
    || lower.contains("impl ")
    || first_line.starts_with("use ")
    || first_line.starts_with("let ")
    || first_line.starts_with("const ")
    || first_line.starts_with("type ")
    || first_line.starts_with("mod ")
}

fn normalize_hover_text(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return None;
  }
  Some(trimmed.to_string())
}

fn flatten_document_symbol(
  uri: &str,
  symbol: DocumentSymbolPayload,
  container_name: Option<String>,
  out: &mut Vec<LspSymbol>,
) {
  let location = Some(LspLocation {
    uri:   uri.to_string(),
    range: symbol.selection_range.into_range(),
  });

  let next_container = Some(symbol.name.clone());
  out.push(LspSymbol {
    name: symbol.name,
    detail: symbol.detail,
    kind: symbol.kind,
    container_name,
    location,
  });

  for child in symbol.children {
    flatten_document_symbol(uri, child, next_container.clone(), out);
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
struct LocationPayload {
  uri:   String,
  range: RangePayload,
}

impl LocationPayload {
  fn into_location(self) -> LspLocation {
    LspLocation {
      uri:   self.uri,
      range: self.range.into_range(),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocationLinkPayload {
  target_uri:             String,
  target_selection_range: Option<RangePayload>,
  target_range:           RangePayload,
}

impl LocationLinkPayload {
  fn into_location(self) -> LspLocation {
    let range = self
      .target_selection_range
      .unwrap_or(self.target_range)
      .into_range();
    LspLocation {
      uri: self.target_uri,
      range,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentSymbolPayload {
  name:            String,
  detail:          Option<String>,
  kind:            u32,
  selection_range: RangePayload,
  #[serde(default)]
  children:        Vec<DocumentSymbolPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymbolInformationPayload {
  name:           String,
  kind:           u32,
  location:       LocationPayload,
  container_name: Option<String>,
}

impl SymbolInformationPayload {
  fn into_symbol(self) -> LspSymbol {
    LspSymbol {
      name:           self.name,
      detail:         None,
      kind:           self.kind,
      container_name: self.container_name,
      location:       Some(self.location.into_location()),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSymbolPayload {
  name:           String,
  kind:           u32,
  location:       Option<WorkspaceSymbolLocationPayload>,
  container_name: Option<String>,
}

impl WorkspaceSymbolPayload {
  fn into_symbol(self) -> LspSymbol {
    LspSymbol {
      name:           self.name,
      detail:         None,
      kind:           self.kind,
      container_name: self.container_name,
      location:       self
        .location
        .and_then(WorkspaceSymbolLocationPayload::into_location),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspaceSymbolLocationPayload {
  Location(LocationPayload),
  Uri { uri: String },
}

impl WorkspaceSymbolLocationPayload {
  fn into_location(self) -> Option<LspLocation> {
    match self {
      Self::Location(location) => Some(location.into_location()),
      Self::Uri { .. } => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn parses_locations_array() {
    let value = json!([
      {
        "uri": "file:///tmp/a.rs",
        "range": {
          "start": { "line": 1, "character": 2 },
          "end": { "line": 1, "character": 4 }
        }
      }
    ]);
    let locations = parse_locations_response(Some(&value)).expect("locations parse");
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].uri, "file:///tmp/a.rs");
  }

  #[test]
  fn parses_hover_contents() {
    let value = json!({
      "contents": {
        "kind": "markdown",
        "value": "```rust\nfn test()\n```"
      }
    });
    let hover = parse_hover_response(Some(&value)).expect("hover parse");
    assert_eq!(hover, Some("```rust\nfn test()\n```".to_string()));
  }

  #[test]
  fn parses_hover_marked_string_array() {
    let value = json!({
      "contents": [
        { "language": "rust", "value": "core::time::Duration" },
        "A duration type."
      ]
    });
    let hover = parse_hover_response(Some(&value)).expect("hover parse");
    assert_eq!(
      hover,
      Some("```rust\ncore::time::Duration\n```\n\nA duration type.".to_string())
    );
  }

  #[test]
  fn parses_hover_plaintext_signature_into_fenced_markdown() {
    let value = json!({
      "contents": {
        "kind": "plaintext",
        "value": "pub struct Duration {\nsecs: u64,\n}\n\nA duration type."
      }
    });
    let hover = parse_hover_response(Some(&value)).expect("hover parse");
    assert_eq!(
      hover,
      Some("```\npub struct Duration {\nsecs: u64,\n}\n```\n\nA duration type.".to_string())
    );
  }

  #[test]
  fn parses_hover_plaintext_namespace_and_signature_into_single_fence() {
    let value = json!({
      "contents": {
        "kind": "plaintext",
        "value": "core::time\n\npub struct Duration {\nsecs: u64,\nnanos: Nanoseconds,\n}\n\nA duration type."
      }
    });
    let hover = parse_hover_response(Some(&value)).expect("hover parse");
    assert_eq!(
      hover,
      Some(
        "```\ncore::time\n\npub struct Duration {\nsecs: u64,\nnanos: Nanoseconds,\n}\n```\n\nA \
         duration type."
          .to_string()
      )
    );
  }

  #[test]
  fn leaves_plaintext_hover_unchanged_when_prose_comes_first() {
    let value = json!({
      "contents": {
        "kind": "plaintext",
        "value": "A duration type.\n\npub struct Duration {}"
      }
    });
    let hover = parse_hover_response(Some(&value)).expect("hover parse");
    assert_eq!(
      hover,
      Some("A duration type.\n\npub struct Duration {}".to_string())
    );
  }
}
