use serde::Deserialize;
use serde_json::Value;
use the_lib::diagnostics::{
  Diagnostic,
  DiagnosticPosition,
  DiagnosticRange,
  DiagnosticSeverity,
  DocumentDiagnostics,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PublishDiagnosticsError {
  #[error("publishDiagnostics missing params")]
  MissingParams,
  #[error("publishDiagnostics params decode failed: {0}")]
  Decode(#[from] serde_json::Error),
}

pub fn parse_publish_diagnostics(
  params: Option<&Value>,
) -> Result<DocumentDiagnostics, PublishDiagnosticsError> {
  let Some(params) = params else {
    return Err(PublishDiagnosticsError::MissingParams);
  };
  let payload: PublishDiagnosticsPayload = serde_json::from_value(params.clone())?;
  Ok(payload.into_document_diagnostics())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishDiagnosticsPayload {
  uri:         String,
  version:     Option<i32>,
  #[serde(default)]
  diagnostics: Vec<DiagnosticPayload>,
}

impl PublishDiagnosticsPayload {
  fn into_document_diagnostics(self) -> DocumentDiagnostics {
    DocumentDiagnostics {
      uri:         self.uri,
      version:     self.version,
      diagnostics: self
        .diagnostics
        .into_iter()
        .map(DiagnosticPayload::into_diagnostic)
        .collect(),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticPayload {
  range:    RangePayload,
  severity: Option<u8>,
  code:     Option<DiagnosticCodePayload>,
  source:   Option<String>,
  message:  String,
}

impl DiagnosticPayload {
  fn into_diagnostic(self) -> Diagnostic {
    Diagnostic {
      range:    self.range.into_range(),
      severity: self.severity.and_then(DiagnosticSeverity::from_lsp_code),
      code:     self.code.map(DiagnosticCodePayload::into_string),
      source:   self.source,
      message:  self.message,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DiagnosticCodePayload {
  String(String),
  Number(i64),
}

impl DiagnosticCodePayload {
  fn into_string(self) -> String {
    match self {
      Self::String(value) => value,
      Self::Number(value) => value.to_string(),
    }
  }
}

#[derive(Debug, Deserialize)]
struct RangePayload {
  start: PositionPayload,
  end:   PositionPayload,
}

impl RangePayload {
  fn into_range(self) -> DiagnosticRange {
    DiagnosticRange {
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
  fn into_position(self) -> DiagnosticPosition {
    DiagnosticPosition {
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
  fn parse_publish_diagnostics_payload() {
    let params = json!({
      "uri": "file:///tmp/a.rs",
      "version": 7,
      "diagnostics": [
        {
          "range": {
            "start": { "line": 1, "character": 2 },
            "end": { "line": 1, "character": 4 }
          },
          "severity": 1,
          "code": "E0001",
          "source": "rust-analyzer",
          "message": "example error"
        },
        {
          "range": {
            "start": { "line": 3, "character": 0 },
            "end": { "line": 3, "character": 3 }
          },
          "severity": 2,
          "code": 42,
          "message": "warning"
        }
      ]
    });

    let parsed = parse_publish_diagnostics(Some(&params)).expect("valid diagnostics");
    assert_eq!(parsed.uri, "file:///tmp/a.rs");
    assert_eq!(parsed.version, Some(7));
    assert_eq!(parsed.diagnostics.len(), 2);
    assert_eq!(
      parsed.diagnostics[0].severity,
      Some(DiagnosticSeverity::Error)
    );
    assert_eq!(parsed.diagnostics[1].code.as_deref(), Some("42"));
  }
}
