use serde::{
  Deserialize,
  Serialize,
};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Version {
  #[serde(rename = "2.0")]
  V2,
}

impl Default for Version {
  fn default() -> Self {
    Self::V2
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
  Null,
  Number(u64),
  String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
  #[serde(default)]
  pub jsonrpc: Version,
  pub id:      Id,
  pub method:  String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub params:  Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
  #[serde(default)]
  pub jsonrpc: Version,
  pub method:  String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub params:  Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseError {
  pub code:    i64,
  pub message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data:    Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
  #[serde(default)]
  pub jsonrpc: Version,
  pub id:      Id,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub result:  Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error:   Option<ResponseError>,
}

impl Response {
  pub fn is_error(&self) -> bool {
    self.error.is_some()
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
  Request(Request),
  Notification(Notification),
  Response(Response),
}

impl Message {
  pub fn request(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
    Self::Request(Request {
      jsonrpc: Version::V2,
      id: Id::Number(id),
      method: method.into(),
      params,
    })
  }

  pub fn notification(method: impl Into<String>, params: Option<Value>) -> Self {
    Self::Notification(Notification {
      jsonrpc: Version::V2,
      method: method.into(),
      params,
    })
  }

  pub fn response_ok(id: Id, result: Option<Value>) -> Self {
    Self::Response(Response {
      jsonrpc: Version::V2,
      id,
      result,
      error: None,
    })
  }

  pub fn response_err(id: Id, code: i64, message: impl Into<String>, data: Option<Value>) -> Self {
    Self::Response(Response {
      jsonrpc: Version::V2,
      id,
      result: None,
      error: Some(ResponseError {
        code,
        message: message.into(),
        data,
      }),
    })
  }

  pub fn id(&self) -> Option<&Id> {
    match self {
      Self::Request(request) => Some(&request.id),
      Self::Response(response) => Some(&response.id),
      Self::Notification(_) => None,
    }
  }
}
