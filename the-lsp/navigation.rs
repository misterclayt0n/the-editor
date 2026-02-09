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
      .join("\n");
    if joined.is_empty() {
      return None;
    }
    return Some(joined);
  }

  let object = value.as_object()?;
  if let Some(markup) = object.get("value").and_then(Value::as_str) {
    return Some(markup.to_string());
  }
  object
    .get("language")
    .zip(object.get("value"))
    .and_then(|(_, value)| value.as_str().map(ToOwned::to_owned))
}

fn normalize_hover_text(value: &str) -> Option<String> {
  let first_non_empty = value
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or_default();
  if first_non_empty.is_empty() {
    return None;
  }
  let mut out = first_non_empty.to_string();
  if out.len() > 240 {
    out.truncate(240);
    out.push('…');
  }
  Some(out)
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
        "value": "```rust\\nfn test()\\n```"
      }
    });
    let hover = parse_hover_response(Some(&value)).expect("hover parse");
    assert_eq!(hover, Some("```rust".to_string()));
  }
}
