use std::sync::Mutex;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ropey::Rope;
use the_lib::render::{
  CellGridMetrics,
  Viewport,
};
use the_runtime::{
  DocumentHandle,
  OperationSpec,
  RuntimeSession,
  SurfaceHandle,
};

#[napi(object)]
pub struct JsViewport {
  pub first_row:  u32,
  pub row_count:  u32,
  pub first_col:  u32,
  pub wrap_width: u32,
}

#[napi(object)]
pub struct JsCellGridMetrics {
  pub cell_width_px: u32,
  pub row_height_px: u32,
}

impl From<JsViewport> for Viewport {
  fn from(value: JsViewport) -> Self {
    Viewport::new(
      value.first_row as usize,
      value.row_count as usize,
      value.first_col as usize,
      value.wrap_width as usize,
    )
  }
}

#[napi(object)]
pub struct JsSurfaceOperation {
  pub name:         String,
  pub count:        Option<u32>,
  pub text:         Option<String>,
  pub values:       Option<Vec<String>>,
  pub after:        Option<bool>,
  pub line_ending:  Option<String>,
  pub indent:       Option<String>,
  pub indent_unit:  Option<String>,
  pub indent_width: Option<u32>,
  pub tab_width:    Option<u32>,
}

#[napi]
pub struct NativeRuntimeSession {
  inner: Mutex<RuntimeSession>,
}

#[napi]
impl NativeRuntimeSession {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {
      inner: Mutex::new(RuntimeSession::default()),
    }
  }

  #[napi]
  pub fn create_scratch(&self, text: String) -> Result<u32> {
    let mut runtime = self.lock()?;
    let document = runtime
      .create_scratch(Rope::from(text))
      .map_err(runtime_error)?;
    Ok(document.get() as u32)
  }

  #[napi]
  pub fn open_file(&self, path: String) -> Result<u32> {
    let mut runtime = self.lock()?;
    let document = runtime.open_file(path.as_ref()).map_err(runtime_error)?;
    Ok(document.get() as u32)
  }

  #[napi]
  pub fn create_surface(&self, document_id: u32, viewport: Option<JsViewport>) -> Result<u32> {
    let mut runtime = self.lock()?;
    let document = document_handle(document_id)?;
    let surface = match viewport {
      Some(viewport) => runtime.create_surface_with_viewport(document, viewport.into()),
      None => runtime.create_surface(document),
    }
    .map_err(runtime_error)?;
    Ok(surface.get() as u32)
  }

  #[napi]
  pub fn set_surface_viewport(&self, surface_id: u32, viewport: JsViewport) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .set_surface_viewport(surface_handle(surface_id)?, viewport.into())
      .map_err(runtime_error)
  }

  #[napi]
  pub fn set_surface_measurements(
    &self,
    surface_id: u32,
    metrics: JsCellGridMetrics,
  ) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .set_surface_measurements(
        surface_handle(surface_id)?,
        CellGridMetrics::new(metrics.cell_width_px, metrics.row_height_px),
      )
      .map_err(runtime_error)
  }

  #[napi]
  pub fn set_surface_soft_wrap(&self, surface_id: u32, soft_wrap: bool) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .set_surface_soft_wrap(surface_handle(surface_id)?, soft_wrap)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn apply_surface_operation(
    &self,
    surface_id: u32,
    operation: JsSurfaceOperation,
  ) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .apply_surface_operation(surface_handle(surface_id)?, operation_spec(operation))
      .map_err(runtime_error)
  }

  #[napi]
  pub fn commit_surface_undo_checkpoint(&self, surface_id: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .commit_surface_undo_checkpoint(surface_handle(surface_id)?)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn undo_surface(&self, surface_id: u32, count: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .undo_surface(surface_handle(surface_id)?, count as usize)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn redo_surface(&self, surface_id: u32, count: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .redo_surface(surface_handle(surface_id)?, count as usize)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn earlier_surface(&self, surface_id: u32, count: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .earlier_surface(surface_handle(surface_id)?, count as usize)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn later_surface(&self, surface_id: u32, count: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .later_surface(surface_handle(surface_id)?, count as usize)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn read_register(&self, name: String) -> Result<Option<Vec<String>>> {
    let runtime = self.lock()?;
    let name = register_name(&name)?;
    Ok(runtime.read_register(name))
  }

  #[napi]
  pub fn write_register(&self, name: String, values: Vec<String>) -> Result<()> {
    let mut runtime = self.lock()?;
    let name = register_name(&name)?;
    runtime.write_register(name, values);
    Ok(())
  }

  #[napi]
  pub fn read_clipboard(&self, kind: String) -> Result<Option<String>> {
    let runtime = self.lock()?;
    Ok(runtime.read_clipboard(clipboard_type(&kind)?).ok())
  }

  #[napi]
  pub fn write_clipboard(&self, value: String, kind: String) -> Result<()> {
    let runtime = self.lock()?;
    let _ = runtime.write_clipboard(&value, clipboard_type(&kind)?);
    Ok(())
  }

  #[napi]
  pub fn render_frame_json(
    &self,
    surface_id: u32,
    viewport: Option<JsViewport>,
    since_generation: Option<u32>,
  ) -> Result<String> {
    let mut runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    if let Some(viewport) = viewport {
      runtime
        .set_surface_viewport(surface, viewport.into())
        .map_err(runtime_error)?;
    }
    let delta = runtime
      .render_surface(surface, since_generation.map(u64::from))
      .map_err(runtime_error)?;
    serde_json::to_string(&delta).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn save_surface_document(&self, surface_id: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    let document = runtime
      .document_for_surface(surface)
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?;
    runtime.save(document).map_err(runtime_error)
  }

  fn lock(&self) -> Result<std::sync::MutexGuard<'_, RuntimeSession>> {
    self
      .inner
      .lock()
      .map_err(|_| Error::from_reason("runtime session lock poisoned"))
  }
}

fn document_handle(id: u32) -> Result<DocumentHandle> {
  DocumentHandle::new(u64::from(id))
    .ok_or_else(|| Error::from_reason(format!("invalid document id {id}")))
}

fn surface_handle(id: u32) -> Result<SurfaceHandle> {
  SurfaceHandle::new(u64::from(id))
    .ok_or_else(|| Error::from_reason(format!("invalid surface id {id}")))
}

fn operation_spec(operation: JsSurfaceOperation) -> OperationSpec {
  OperationSpec {
    name:         operation.name,
    count:        operation.count.map(|count| count as usize),
    text:         operation.text,
    values:       operation.values,
    after:        operation.after,
    line_ending:  operation.line_ending,
    indent:       operation.indent,
    indent_unit:  operation.indent_unit,
    indent_width: operation.indent_width.map(|width| width as usize),
    tab_width:    operation.tab_width.map(|width| width as usize),
  }
}

fn register_name(name: &str) -> Result<char> {
  let mut chars = name.chars();
  match (chars.next(), chars.next()) {
    (Some(ch), None) => Ok(ch),
    _ => {
      Err(Error::from_reason(
        "register name must be a single character",
      ))
    },
  }
}

fn clipboard_type(kind: &str) -> Result<the_lib::clipboard::ClipboardType> {
  match kind {
    "Clipboard" => Ok(the_lib::clipboard::ClipboardType::Clipboard),
    "Selection" => Ok(the_lib::clipboard::ClipboardType::Selection),
    _ => Err(Error::from_reason(format!("unknown clipboard kind {kind}"))),
  }
}

fn runtime_error(error: the_runtime::RuntimeError) -> Error {
  Error::from_reason(error.to_string())
}
