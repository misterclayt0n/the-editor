use std::io::{self, BufRead, Write};

use ropey::Rope;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use the_lib::render::{RenderFrameDelta, Viewport};

use crate::{DocumentHandle, RuntimeError, RuntimeSession, SurfaceHandle};

#[derive(Debug, Deserialize)]
struct RpcRequest {
  id:     Option<Value>,
  method: String,
  #[serde(default)]
  params: Value,
}

#[derive(Debug, Serialize)]
struct RpcResponse<'a> {
  id:     Value,
  #[serde(skip_serializing_if = "Option::is_none")]
  result: Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error:  Option<RpcError<'a>>,
}

#[derive(Debug, Serialize)]
struct RpcError<'a> {
  code:    &'a str,
  message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  data:    Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentOpened {
  document_id: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SurfaceCreated {
  surface_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenFileParams {
  path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateScratchParams {
  #[serde(default)]
  text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSurfaceParams {
  document_id: u64,
  viewport:    Option<Viewport>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SurfaceParams {
  surface_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetViewportParams {
  surface_id: u64,
  viewport:   Viewport,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderFrameParams {
  surface_id:        u64,
  viewport:          Option<Viewport>,
  since_generation:  Option<u64>,
}

pub fn serve_stdio() -> io::Result<()> {
  let stdin = io::stdin();
  let mut stdout = io::stdout().lock();
  let mut session = RuntimeSession::default();

  for line in stdin.lock().lines() {
    let line = line?;
    if line.trim().is_empty() {
      continue;
    }

    let response = handle_line(&mut session, &line);
    serde_json::to_writer(&mut stdout, &response)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
  }

  Ok(())
}

fn handle_line(session: &mut RuntimeSession, line: &str) -> RpcResponse<'static> {
  let request: RpcRequest = match serde_json::from_str(line) {
    Ok(request) => request,
    Err(err) => {
      return RpcResponse {
        id:     Value::Null,
        result: None,
        error:  Some(RpcError {
          code:    "parse.error",
          message: err.to_string(),
          data:    None,
        }),
      };
    },
  };

  let id = request.id.clone().unwrap_or(Value::Null);
  match handle_request(session, request) {
    Ok(result) => RpcResponse {
      id,
      result: Some(result),
      error: None,
    },
    Err(error) => RpcResponse {
      id,
      result: None,
      error: Some(error),
    },
  }
}

fn handle_request(session: &mut RuntimeSession, request: RpcRequest) -> Result<Value, RpcError<'static>> {
  match request.method.as_str() {
    "server.ping" => Ok(json!({ "ok": true })),
    "documents.openFile" => {
      let params: OpenFileParams = parse_params(request.params)?;
      let document = session.open_file(params.path.as_ref()).map_err(runtime_error)?;
      to_value(DocumentOpened { document_id: document.get() })
    },
    "documents.createScratch" => {
      let params: CreateScratchParams = parse_params(request.params)?;
      let document = session
        .create_scratch(Rope::from(params.text))
        .map_err(runtime_error)?;
      to_value(DocumentOpened { document_id: document.get() })
    },
    "surfaces.create" => {
      let params: CreateSurfaceParams = parse_params(request.params)?;
      let document = handle_from_u64(params.document_id, "document")?;
      let surface = match params.viewport {
        Some(viewport) => session.create_surface_with_viewport(document, viewport),
        None => session.create_surface(document),
      }
      .map_err(runtime_error)?;
      to_value(SurfaceCreated { surface_id: surface.get() })
    },
    "surfaces.setViewport" => {
      let params: SetViewportParams = parse_params(request.params)?;
      let surface = handle_from_u64(params.surface_id, "surface")?;
      session
        .set_surface_viewport(surface, params.viewport)
        .map_err(runtime_error)?;
      Ok(json!({ "ok": true }))
    },
    "surfaces.renderFrame" => {
      let params: RenderFrameParams = parse_params(request.params)?;
      let surface = handle_from_u64(params.surface_id, "surface")?;
      if let Some(viewport) = params.viewport {
        session
          .set_surface_viewport(surface, viewport)
          .map_err(runtime_error)?;
      }
      let delta: RenderFrameDelta = session
        .render_surface(surface, params.since_generation)
        .map_err(runtime_error)?;
      to_value(delta)
    },
    "resources.save" => {
      let params: SurfaceParams = parse_params(request.params)?;
      let document = session
        .document_for_surface(handle_from_u64(params.surface_id, "surface")?)
        .ok_or_else(|| RpcError {
          code:    "runtime.unknownSurface",
          message: format!("unknown surface {}", params.surface_id),
          data:    Some(json!({ "surfaceId": params.surface_id })),
        })?;
      session.save(document).map_err(runtime_error)?;
      Ok(json!({ "ok": true }))
    },
    method => Err(RpcError {
      code:    "method.notFound",
      message: format!("unknown method {method}"),
      data:    Some(json!({ "method": method })),
    }),
  }
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, RpcError<'static>> {
  serde_json::from_value(params).map_err(|err| RpcError {
    code:    "params.invalid",
    message: err.to_string(),
    data:    None,
  })
}

fn to_value(value: impl Serialize) -> Result<Value, RpcError<'static>> {
  serde_json::to_value(value).map_err(|err| RpcError {
    code:    "result.serializeFailed",
    message: err.to_string(),
    data:    None,
  })
}

fn handle_from_u64<T>(id: u64, kind: &'static str) -> Result<T, RpcError<'static>>
where
  T: HandleFromU64,
{
  T::from_u64(id).ok_or_else(|| RpcError {
    code:    "handle.invalid",
    message: format!("invalid {kind} handle {id}"),
    data:    Some(json!({ "kind": kind, "id": id })),
  })
}

trait HandleFromU64: Sized {
  fn from_u64(id: u64) -> Option<Self>;
}

impl HandleFromU64 for DocumentHandle {
  fn from_u64(id: u64) -> Option<Self> {
    Self::new(id)
  }
}

impl HandleFromU64 for SurfaceHandle {
  fn from_u64(id: u64) -> Option<Self> {
    Self::new(id)
  }
}

fn runtime_error(error: RuntimeError) -> RpcError<'static> {
  let code = match &error {
    RuntimeError::Io(_) => "runtime.io",
    RuntimeError::Surface(_) => "runtime.surface",
    RuntimeError::Syntax(_) => "runtime.syntax",
    RuntimeError::UnknownDocument(_) => "runtime.unknownDocument",
    RuntimeError::UnknownSurface(_) => "runtime.unknownSurface",
    RuntimeError::UnknownResource(_) => "runtime.unknownResource",
    RuntimeError::DocumentHasNoResource { .. } => "runtime.documentHasNoResource",
    RuntimeError::ResourceAlreadyOpen { .. } => "runtime.resourceAlreadyOpen",
    RuntimeError::ReadonlyDocument { .. } => "runtime.readonlyDocument",
  };

  RpcError {
    code,
    message: error.to_string(),
    data: None,
  }
}
