use std::sync::Mutex;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ropey::Rope;
use the_lib::{
  movement::{
    Direction,
    MotionMode,
    WordKind,
    WordTarget,
  },
  render::Viewport,
};
use the_runtime::{
  DocumentHandle,
  RuntimeSession,
  SelectionTransform,
  SurfaceHandle,
  SurfaceOperation,
};

#[napi(object)]
pub struct JsViewport {
  pub first_row: u32,
  pub row_count: u32,
  pub first_col: u32,
  pub wrap_width: u32,
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
pub struct JsSelectionTransform {
  pub kind: String,
  pub direction: Option<String>,
  pub count: Option<u32>,
  pub mode: Option<String>,
  pub word_kind: Option<String>,
  pub word_target: Option<String>,
}

#[napi(object)]
pub struct JsSurfaceOperation {
  pub kind: String,
  pub transform: Option<JsSelectionTransform>,
  pub text: Option<String>,
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
  pub fn apply_surface_operation(&self, surface_id: u32, operation: JsSurfaceOperation) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .apply_surface_operation(surface_handle(surface_id)?, surface_operation(operation)?)
      .map_err(runtime_error)
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
  DocumentHandle::new(u64::from(id)).ok_or_else(|| Error::from_reason(format!("invalid document id {id}")))
}

fn surface_handle(id: u32) -> Result<SurfaceHandle> {
  SurfaceHandle::new(u64::from(id)).ok_or_else(|| Error::from_reason(format!("invalid surface id {id}")))
}

fn surface_operation(operation: JsSurfaceOperation) -> Result<SurfaceOperation> {
  match operation.kind.as_str() {
    "transformSelection" => Ok(SurfaceOperation::TransformSelection {
      transform: selection_transform(
        operation
          .transform
          .ok_or_else(|| Error::from_reason("missing selection transform"))?,
      )?,
    }),
    "insertText" => Ok(SurfaceOperation::InsertText {
      text: operation.text.unwrap_or_default(),
    }),
    "insertNewline" => Ok(SurfaceOperation::InsertNewline),
    "deleteBackward" => Ok(SurfaceOperation::DeleteBackward),
    "deleteForward" => Ok(SurfaceOperation::DeleteForward),
    kind => Err(Error::from_reason(format!("unknown surface operation {kind}"))),
  }
}

fn selection_transform(transform: JsSelectionTransform) -> Result<SelectionTransform> {
  let count = transform.count.unwrap_or(1) as usize;
  let mode = motion_mode(transform.mode.as_deref().unwrap_or("move"))?;

  match transform.kind.as_str() {
    "horizontal" => Ok(SelectionTransform::Horizontal {
      direction: direction(required(&transform.direction, "direction")?)?,
      count,
      mode,
    }),
    "vertical" => Ok(SelectionTransform::Vertical {
      direction: direction(required(&transform.direction, "direction")?)?,
      count,
      mode,
    }),
    "visualVertical" => Ok(SelectionTransform::VisualVertical {
      direction: direction(required(&transform.direction, "direction")?)?,
      count,
      mode,
    }),
    "word" => Ok(SelectionTransform::Word {
      direction: direction(required(&transform.direction, "direction")?)?,
      kind: word_kind(required(&transform.word_kind, "wordKind")?)?,
      target: word_target(required(&transform.word_target, "wordTarget")?)?,
      count,
      mode,
    }),
    "paragraph" => Ok(SelectionTransform::Paragraph {
      direction: direction(required(&transform.direction, "direction")?)?,
      count,
      mode,
    }),
    "lineStart" => Ok(SelectionTransform::LineStart { mode }),
    "lineEnd" => Ok(SelectionTransform::LineEnd { mode }),
    "firstNonWhitespace" => Ok(SelectionTransform::FirstNonWhitespace { mode }),
    "documentStart" => Ok(SelectionTransform::DocumentStart { mode }),
    "documentEnd" => Ok(SelectionTransform::DocumentEnd { mode }),
    kind => Err(Error::from_reason(format!("unknown selection transform {kind}"))),
  }
}

fn required<'a>(value: &'a Option<String>, name: &str) -> Result<&'a str> {
  value
    .as_deref()
    .ok_or_else(|| Error::from_reason(format!("missing {name}")))
}

fn direction(direction: &str) -> Result<Direction> {
  match direction {
    "forward" => Ok(Direction::Forward),
    "backward" => Ok(Direction::Backward),
    _ => Err(Error::from_reason(format!("unknown direction {direction}"))),
  }
}

fn motion_mode(mode: &str) -> Result<MotionMode> {
  match mode {
    "move" => Ok(MotionMode::Move),
    "extend" => Ok(MotionMode::Extend),
    _ => Err(Error::from_reason(format!("unknown motion mode {mode}"))),
  }
}

fn word_kind(kind: &str) -> Result<WordKind> {
  match kind {
    "word" => Ok(WordKind::Word),
    "longWord" => Ok(WordKind::LongWord),
    "subWord" => Ok(WordKind::SubWord),
    _ => Err(Error::from_reason(format!("unknown word kind {kind}"))),
  }
}

fn word_target(target: &str) -> Result<WordTarget> {
  match target {
    "start" => Ok(WordTarget::Start),
    "end" => Ok(WordTarget::End),
    _ => Err(Error::from_reason(format!("unknown word target {target}"))),
  }
}

fn runtime_error(error: the_runtime::RuntimeError) -> Error {
  Error::from_reason(error.to_string())
}
