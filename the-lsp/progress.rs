use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::{
  LspProgress,
  LspProgressKind,
};

#[derive(Debug, Error)]
pub enum ProgressParseError {
  #[error("progress notification missing params")]
  MissingParams,
  #[error("failed to decode progress payload: {0}")]
  Decode(#[from] serde_json::Error),
}

pub fn parse_progress_notification(
  params: Option<&Value>,
) -> Result<LspProgress, ProgressParseError> {
  let Some(params) = params else {
    return Err(ProgressParseError::MissingParams);
  };
  let payload: ProgressParamsPayload = serde_json::from_value(params.clone())?;

  Ok(LspProgress {
    token:      payload.token.to_string_token(),
    kind:       payload.value.kind.into_kind(),
    title:      payload.value.title,
    message:    payload.value.message,
    percentage: payload.value.percentage,
  })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressParamsPayload {
  token: ProgressTokenPayload,
  value: ProgressValuePayload,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ProgressTokenPayload {
  String(String),
  Number(i64),
}

impl ProgressTokenPayload {
  fn to_string_token(&self) -> String {
    match self {
      Self::String(token) => token.clone(),
      Self::Number(token) => token.to_string(),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressValuePayload {
  kind:       ProgressKindPayload,
  title:      Option<String>,
  message:    Option<String>,
  percentage: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ProgressKindPayload {
  Begin,
  Report,
  End,
}

impl ProgressKindPayload {
  fn into_kind(self) -> LspProgressKind {
    match self {
      Self::Begin => LspProgressKind::Begin,
      Self::Report => LspProgressKind::Report,
      Self::End => LspProgressKind::End,
    }
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn parse_progress_notification_begin() {
    let params = json!({
      "token": "abc",
      "value": {
        "kind": "begin",
        "title": "Indexing",
        "message": "Loading workspace",
        "percentage": 12
      }
    });
    let parsed = parse_progress_notification(Some(&params)).expect("progress parse");
    assert_eq!(parsed.token, "abc");
    assert_eq!(parsed.kind, LspProgressKind::Begin);
    assert_eq!(parsed.percentage, Some(12));
  }
}
