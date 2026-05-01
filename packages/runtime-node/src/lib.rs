use std::sync::Mutex;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ropey::Rope;
use the_lib::render::{CellGridMetrics, Viewport};
use the_runtime::{
  CursorRangeSpec,
  DocumentHandle,
  ReplacementSpec,
  RuntimeSession,
  SelectionSpec,
  SurfaceHandle,
  TextCommand,
  TransactionSpec,
};

#[napi(object)]
pub struct JsViewport {
  pub first_row: u32,
  pub row_count: u32,
  pub first_col: u32,
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
pub struct JsCursorRange {
  pub id: Option<u32>,
  pub anchor: u32,
  pub head: u32,
}

#[napi(object)]
pub struct JsSelection {
  pub ranges: Vec<JsCursorRange>,
}

#[napi(object)]
pub struct JsReplacement {
  pub start: u32,
  pub end: u32,
  pub text: Option<String>,
}

#[napi(object)]
pub struct JsTransaction {
  pub replacements: Vec<JsReplacement>,
}

#[napi(object)]
pub struct JsTextCommand {
  pub kind: String,
  pub count: Option<u32>,
  pub text: Option<String>,
  pub indent: Option<String>,
  pub indent_unit: Option<String>,
  pub preferred_column: Option<u32>,
}

#[napi(object)]
pub struct JsTextCommandResult {
  pub preferred_column: Option<u32>,
}

#[napi(object)]
pub struct JsSurfaceSnapshot {
  pub text: String,
  pub selection: JsSelection,
  pub active_cursor_id: Option<u32>,
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
  pub fn set_surface_measurements(&self, surface_id: u32, metrics: JsCellGridMetrics) -> Result<()> {
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
  pub fn snapshot_surface_state(&self, surface_id: u32) -> Result<JsSurfaceSnapshot> {
    let runtime = self.lock()?;
    runtime
      .snapshot_surface_state(surface_handle(surface_id)?)
      .map(js_snapshot)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn indent_for_surface_newline(&self, surface_id: u32, pos: u32) -> Result<String> {
    let mut runtime = self.lock()?;
    runtime
      .indent_for_surface_newline(surface_handle(surface_id)?, pos as usize)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn set_surface_selection(&self, surface_id: u32, selection: JsSelection) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .set_surface_selection(surface_handle(surface_id)?, selection_spec(selection))
      .map_err(runtime_error)
  }

  #[napi]
  pub fn execute_surface_text_command(&self, surface_id: u32, command: JsTextCommand) -> Result<JsTextCommandResult> {
    let mut runtime = self.lock()?;
    let preferred_column = runtime
      .execute_surface_text_command(surface_handle(surface_id)?, text_command(command)?)
      .map_err(runtime_error)?;
    Ok(JsTextCommandResult { preferred_column: preferred_column.map(|col| col as u32) })
  }

  #[napi]
  pub fn apply_surface_transaction(
    &self,
    surface_id: u32,
    transaction: JsTransaction,
    selection: JsSelection,
  ) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .apply_surface_transaction(
        surface_handle(surface_id)?,
        transaction_spec(transaction),
        selection_spec(selection),
      )
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

fn selection_spec(selection: JsSelection) -> SelectionSpec {
  SelectionSpec {
    ranges: selection.ranges.into_iter().map(|range| CursorRangeSpec {
      id: range.id.map(u64::from),
      anchor: range.anchor as usize,
      head: range.head as usize,
    }).collect(),
  }
}

fn text_command(command: JsTextCommand) -> Result<TextCommand> {
  let count = command.count.unwrap_or(1) as usize;
  Ok(match command.kind.as_str() {
    "moveCharLeft" => TextCommand::MoveCharLeft { count },
    "moveCharRight" => TextCommand::MoveCharRight { count },
    "moveVisualLineUp" => TextCommand::MoveVisualLineUp {
      count,
      preferred_column: command.preferred_column.map(|col| col as usize),
    },
    "moveVisualLineDown" => TextCommand::MoveVisualLineDown {
      count,
      preferred_column: command.preferred_column.map(|col| col as usize),
    },
    "gotoLineStart" => TextCommand::GotoLineStart,
    "gotoLineEnd" => TextCommand::GotoLineEnd,
    "gotoDocumentStart" => TextCommand::GotoDocumentStart,
    "gotoDocumentEnd" => TextCommand::GotoDocumentEnd,
    "insertText" => TextCommand::InsertText { text: command.text.unwrap_or_default() },
    "insertNewline" => TextCommand::InsertNewline {
      indent: command.indent.unwrap_or_default(),
      indent_unit: command.indent_unit.unwrap_or_else(|| "  ".into()),
    },
    "deleteBackward" => TextCommand::DeleteBackward,
    "deleteForward" => TextCommand::DeleteForward,
    _ => return Err(Error::from_reason(format!("unknown text command {}", command.kind))),
  })
}

fn transaction_spec(transaction: JsTransaction) -> TransactionSpec {
  TransactionSpec {
    replacements: transaction.replacements.into_iter().map(|replacement| ReplacementSpec {
      start: replacement.start as usize,
      end: replacement.end as usize,
      text: replacement.text,
    }).collect(),
  }
}

fn js_snapshot(snapshot: the_runtime::SurfaceSnapshot) -> JsSurfaceSnapshot {
  JsSurfaceSnapshot {
    text: snapshot.text,
    active_cursor_id: snapshot.active_cursor_id.map(|id| id as u32),
    selection: JsSelection {
      ranges: snapshot.selection.ranges.into_iter().map(|range| JsCursorRange {
        id: range.id.map(|id| id as u32),
        anchor: range.anchor as u32,
        head: range.head as u32,
      }).collect(),
    },
  }
}

fn runtime_error(error: the_runtime::RuntimeError) -> Error {
  Error::from_reason(error.to_string())
}
