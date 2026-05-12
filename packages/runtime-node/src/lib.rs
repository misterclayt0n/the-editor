use std::{
  num::NonZeroU64,
  sync::Mutex,
};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ropey::Rope;
use smallvec::SmallVec;
use the_lib::{
  operation::Edit,
  render::{
    CellGridMetrics,
    CursorSemantics,
    Viewport,
  },
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
  },
  transaction::{
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
  },
};
use the_runtime::{
  DocumentHandle,
  LspServerConfig,
  LspServerHandle,
  OperationSpec,
  RuntimeSession,
  SearchIndexHandle,
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

#[napi(object)]
pub struct JsSurfaceSelectionRange {
  pub cursor_id: u32,
  pub anchor:    u32,
  pub head:      u32,
}

#[napi(object)]
pub struct JsSurfaceChange {
  pub start: u32,
  pub end:   u32,
  pub text:  Option<String>,
}

#[napi(object)]
pub struct JsSurfaceEdit {
  pub changes:   Option<Vec<JsSurfaceChange>>,
  pub selection: Option<Vec<JsSurfaceSelectionRange>>,
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
  pub fn lsp_register_server(&self, config_json: String) -> Result<()> {
    let config: LspServerConfig =
      serde_json::from_str(&config_json).map_err(|err| Error::from_reason(err.to_string()))?;
    let mut runtime = self.lock()?;
    runtime.lsp_register_server(config);
    Ok(())
  }

  #[napi]
  pub fn lsp_configured_servers_json(&self) -> Result<String> {
    let runtime = self.lock()?;
    serde_json::to_string(&runtime.lsp_configured_servers())
      .map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn lsp_start_server(&self, id: String, root_uri: Option<String>) -> Result<u32> {
    let mut runtime = self.lock()?;
    let handle = runtime
      .lsp_start_server(&id, root_uri)
      .map_err(runtime_error)?;
    Ok(handle.get() as u32)
  }

  #[napi]
  pub fn lsp_shutdown_server(&self, server_id: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .lsp_shutdown_server(lsp_server_handle(server_id)?)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn lsp_status_json(&self) -> Result<String> {
    let mut runtime = self.lock()?;
    serde_json::to_string(&runtime.lsp_status()).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn lsp_diagnostics_json(&self) -> Result<String> {
    let mut runtime = self.lock()?;
    serde_json::to_string(&runtime.lsp_diagnostics())
      .map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn lsp_logs_json(&self, server_id: u32) -> Result<String> {
    let mut runtime = self.lock()?;
    let logs = runtime
      .lsp_logs(lsp_server_handle(server_id)?)
      .map_err(runtime_error)?;
    serde_json::to_string(&logs).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn lsp_request_json(
    &self,
    server_id: u32,
    method: String,
    params_json: String,
    timeout_ms: Option<u32>,
  ) -> Result<String> {
    let params =
      serde_json::from_str(&params_json).map_err(|err| Error::from_reason(err.to_string()))?;
    let mut runtime = self.lock()?;
    let result = runtime
      .lsp_request(
        lsp_server_handle(server_id)?,
        &method,
        params,
        timeout_ms.unwrap_or(10_000) as u64,
      )
      .map_err(runtime_error)?;
    serde_json::to_string(&result).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn lsp_notify_json(&self, server_id: u32, method: String, params_json: String) -> Result<()> {
    let params =
      serde_json::from_str(&params_json).map_err(|err| Error::from_reason(err.to_string()))?;
    let mut runtime = self.lock()?;
    runtime
      .lsp_notify(lsp_server_handle(server_id)?, &method, params)
      .map_err(runtime_error)
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
  pub fn destroy_surface(&self, surface_id: u32) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .destroy_surface(surface_handle(surface_id)?)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn load_surface_file(&self, surface_id: u32, path: String) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .load_surface_file(surface_handle(surface_id)?, path.as_ref())
      .map_err(runtime_error)
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
  pub fn set_surface_cursor_semantics(&self, surface_id: u32, semantics: String) -> Result<()> {
    let mut runtime = self.lock()?;
    let semantics = match semantics.as_str() {
      "InsertionPoint" => CursorSemantics::InsertionPoint,
      "Cell" => CursorSemantics::Cell,
      other => {
        return Err(Error::from_reason(format!(
          "unknown cursor semantics: {other}"
        )));
      },
    };
    runtime
      .set_surface_cursor_semantics(surface_handle(surface_id)?, semantics)
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
  pub fn read_surface_text(&self, surface_id: u32) -> Result<String> {
    let runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    let document = runtime
      .document_for_surface(surface)
      .and_then(|document| runtime.document(document))
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?;
    Ok(document.text().to_string())
  }

  #[napi]
  pub fn read_surface_revision(&self, surface_id: u32) -> Result<u32> {
    let runtime = self.lock()?;
    let document = runtime
      .document_for_surface(surface_handle(surface_id)?)
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?;
    Ok(
      runtime
        .document(document)
        .ok_or_else(|| Error::from_reason(format!("unknown document {}", document.get())))?
        .revision() as u32,
    )
  }

  #[napi]
  pub fn read_surface_line_ending(&self, surface_id: u32) -> Result<String> {
    let runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    let document = runtime
      .document_for_surface(surface)
      .and_then(|document| runtime.document(document))
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?;
    Ok(document.line_ending().as_str().to_string())
  }

  #[napi]
  pub fn read_surface_line_count(&self, surface_id: u32) -> Result<u32> {
    let runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    let document = runtime
      .document_for_surface(surface)
      .and_then(|document| runtime.document(document))
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?;
    Ok(document.text().len_lines() as u32)
  }

  #[napi]
  pub fn read_surface_selection(&self, surface_id: u32) -> Result<Vec<JsSurfaceSelectionRange>> {
    let runtime = self.lock()?;
    let selection = runtime
      .surface(surface_handle(surface_id)?)
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?
      .selection();
    Ok(
      selection
        .iter()
        .map(|cursor| {
          JsSurfaceSelectionRange {
            cursor_id: cursor.id.get() as u32,
            anchor:    cursor.range.anchor as u32,
            head:      cursor.range.head as u32,
          }
        })
        .collect(),
    )
  }

  #[napi]
  pub fn complete_surface_words_json(
    &self,
    surface_id: u32,
    cursor_offset: u32,
    query: String,
    limit: Option<u32>,
  ) -> Result<String> {
    let runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    let document = runtime
      .document_for_surface(surface)
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?;
    let text = runtime
      .document(document)
      .ok_or_else(|| Error::from_reason(format!("unknown document {}", document.get())))?
      .text()
      .to_string();
    let cursor = cursor_offset as usize;
    let query_lower = query.to_lowercase();
    let mut seen = std::collections::BTreeSet::new();
    let mut words = Vec::<serde_json::Value>::new();
    for (start, word) in indexed_words(&text) {
      if start <= cursor && cursor <= start + word.len() {
        continue;
      }
      if !query_lower.is_empty() && !word.to_lowercase().contains(&query_lower) {
        continue;
      }
      if !seen.insert(word.to_string()) {
        continue;
      }
      words.push(serde_json::json!({ "label": word }));
      if words.len() >= limit.unwrap_or(100) as usize {
        break;
      }
    }
    serde_json::to_string(&words).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn read_surface_active_cursor(&self, surface_id: u32) -> Result<Option<u32>> {
    let runtime = self.lock()?;
    Ok(
      runtime
        .surface(surface_handle(surface_id)?)
        .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?
        .active_cursor()
        .map(|cursor| cursor.get() as u32),
    )
  }

  #[napi]
  pub fn read_surface_cursor_semantics(&self, surface_id: u32) -> Result<String> {
    let runtime = self.lock()?;
    let semantics = runtime
      .surface(surface_handle(surface_id)?)
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?
      .cursor_semantics();
    Ok(
      match semantics {
        CursorSemantics::InsertionPoint => "InsertionPoint",
        CursorSemantics::Cell => "Cell",
      }
      .to_string(),
    )
  }

  #[napi]
  pub fn read_surface_viewport(&self, surface_id: u32) -> Result<JsViewport> {
    let runtime = self.lock()?;
    let viewport = runtime
      .surface(surface_handle(surface_id)?)
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?
      .viewport();
    Ok(JsViewport {
      first_row:  viewport.first_row as u32,
      row_count:  viewport.row_count as u32,
      first_col:  viewport.first_col as u32,
      wrap_width: viewport.wrap_width as u32,
    })
  }

  #[napi]
  pub fn set_surface_active_cursor(&self, surface_id: u32, cursor_id: Option<u32>) -> Result<()> {
    let mut runtime = self.lock()?;
    runtime
      .set_surface_active_cursor(surface_handle(surface_id)?, cursor_id.map(|id| id as u64))
      .map_err(runtime_error)
  }

  #[napi]
  pub fn apply_surface_edit(&self, surface_id: u32, edit: JsSurfaceEdit) -> Result<()> {
    let mut runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    let document_len = runtime
      .document_for_surface(surface)
      .and_then(|document| runtime.document(document))
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?
      .text()
      .len_chars();
    let current_selection = runtime
      .surface(surface)
      .ok_or_else(|| Error::from_reason(format!("unknown surface {surface_id}")))?
      .selection()
      .clone();
    runtime
      .apply_surface_edit(
        surface,
        js_surface_edit(edit, document_len, &current_selection)?,
      )
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
  pub fn read_syntax_highlight_legend_json(&self) -> Result<String> {
    let runtime = self.lock()?;
    let legend = runtime
      .syntax()
      .registry()
      .highlight_names()
      .into_iter()
      .enumerate()
      .map(|(kind, name)| (kind, name))
      .collect::<std::collections::BTreeMap<usize, String>>();
    serde_json::to_string(&legend).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn render_frame_json(&self, surface_id: u32, viewport: Option<JsViewport>) -> Result<String> {
    let mut runtime = self.lock()?;
    let surface = surface_handle(surface_id)?;
    if let Some(viewport) = viewport {
      runtime
        .set_surface_viewport(surface, viewport.into())
        .map_err(runtime_error)?;
    }
    let frame = runtime.render_surface(surface).map_err(runtime_error)?;
    serde_json::to_string(&frame).map_err(|err| Error::from_reason(err.to_string()))
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

  #[napi]
  pub fn ensure_search_index_json(&self, root_path: String) -> Result<String> {
    let mut runtime = self.lock()?;
    let ready = runtime
      .ensure_search_index(root_path.as_ref())
      .map_err(runtime_error)?;
    serde_json::to_string(&ready).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn read_search_index_status_json(&self, index_id: u32) -> Result<String> {
    let runtime = self.lock()?;
    let status = runtime
      .search_index_status(search_handle(index_id)?)
      .map_err(runtime_error)?;
    serde_json::to_string(&status).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn refresh_search_index(&self, index_id: u32) -> Result<()> {
    let runtime = self.lock()?;
    runtime
      .refresh_search_index(search_handle(index_id)?)
      .map_err(runtime_error)
  }

  #[napi]
  pub fn query_search_files_json(
    &self,
    index_id: u32,
    query: String,
    offset: Option<u32>,
    limit: Option<u32>,
    current_path: Option<String>,
  ) -> Result<String> {
    let runtime = self.lock()?;
    let result = runtime
      .query_search_files(
        search_handle(index_id)?,
        &query,
        offset.unwrap_or(0) as usize,
        limit.unwrap_or(100) as usize,
        current_path.as_deref(),
      )
      .map_err(runtime_error)?;
    serde_json::to_string(&result).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn query_search_dirs_json(
    &self,
    index_id: u32,
    query: String,
    offset: Option<u32>,
    limit: Option<u32>,
    current_path: Option<String>,
  ) -> Result<String> {
    let runtime = self.lock()?;
    let result = runtime
      .query_search_dirs(
        search_handle(index_id)?,
        &query,
        offset.unwrap_or(0) as usize,
        limit.unwrap_or(100) as usize,
        current_path.as_deref(),
      )
      .map_err(runtime_error)?;
    serde_json::to_string(&result).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn query_search_mixed_json(
    &self,
    index_id: u32,
    query: String,
    offset: Option<u32>,
    limit: Option<u32>,
    current_path: Option<String>,
  ) -> Result<String> {
    let runtime = self.lock()?;
    let result = runtime
      .query_search_mixed(
        search_handle(index_id)?,
        &query,
        offset.unwrap_or(0) as usize,
        limit.unwrap_or(100) as usize,
        current_path.as_deref(),
      )
      .map_err(runtime_error)?;
    serde_json::to_string(&result).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn query_search_grep_json(
    &self,
    index_id: u32,
    query: String,
    offset: Option<u32>,
    limit: Option<u32>,
    current_path: Option<String>,
    before_context: Option<u32>,
    after_context: Option<u32>,
  ) -> Result<String> {
    let runtime = self.lock()?;
    let result = runtime
      .query_search_grep(
        search_handle(index_id)?,
        &query,
        offset.unwrap_or(0) as usize,
        limit.unwrap_or(100) as usize,
        current_path.as_deref(),
        before_context.unwrap_or(0) as usize,
        after_context.unwrap_or(0) as usize,
      )
      .map_err(runtime_error)?;
    serde_json::to_string(&result).map_err(|err| Error::from_reason(err.to_string()))
  }

  #[napi]
  pub fn record_search_selection(&self, index_id: u32, query: String, path: String) -> Result<()> {
    let runtime = self.lock()?;
    runtime
      .record_search_selection(search_handle(index_id)?, &query, path.as_ref())
      .map_err(runtime_error)
  }

  #[napi]
  pub fn preview_path_cache_key(&self, path: String) -> Result<String> {
    let runtime = self.lock()?;
    runtime
      .preview_path_cache_key(path.as_ref())
      .map_err(runtime_error)
  }

  #[napi]
  pub fn read_text_preview_json(
    &self,
    path: String,
    max_bytes: Option<u32>,
    max_lines: Option<u32>,
    around_line: Option<u32>,
    context_lines: Option<u32>,
  ) -> Result<String> {
    let runtime = self.lock()?;
    let preview = runtime
      .read_text_preview(
        path.as_ref(),
        max_bytes.unwrap_or(8192) as usize,
        max_lines.unwrap_or(80) as usize,
        around_line.map(|line| line as usize),
        context_lines.unwrap_or(6) as usize,
      )
      .map_err(runtime_error)?;
    serde_json::to_string(&preview).map_err(|err| Error::from_reason(err.to_string()))
  }

  fn lock(&self) -> Result<std::sync::MutexGuard<'_, RuntimeSession>> {
    self
      .inner
      .lock()
      .map_err(|_| Error::from_reason("runtime session lock poisoned"))
  }
}

fn indexed_words(text: &str) -> Vec<(usize, &str)> {
  let mut words = Vec::new();
  let mut start = None;
  for (index, ch) in text.char_indices() {
    let is_word = ch == '_' || ch == '$' || ch.is_ascii_alphanumeric();
    match (start, is_word) {
      (None, true) => start = Some(index),
      (Some(word_start), false) => {
        if index > word_start + 1 {
          words.push((word_start, &text[word_start..index]));
        }
        start = None;
      },
      _ => {},
    }
  }
  if let Some(word_start) = start
    && text.len() > word_start + 1
  {
    words.push((word_start, &text[word_start..]));
  }
  words
}

fn document_handle(id: u32) -> Result<DocumentHandle> {
  DocumentHandle::new(u64::from(id))
    .ok_or_else(|| Error::from_reason(format!("invalid document id {id}")))
}

fn surface_handle(id: u32) -> Result<SurfaceHandle> {
  SurfaceHandle::new(u64::from(id))
    .ok_or_else(|| Error::from_reason(format!("invalid surface id {id}")))
}

fn search_handle(id: u32) -> Result<SearchIndexHandle> {
  SearchIndexHandle::new(u64::from(id))
    .ok_or_else(|| Error::from_reason(format!("invalid search index id {id}")))
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

fn js_surface_edit(edit: JsSurfaceEdit, len: usize, current_selection: &Selection) -> Result<Edit> {
  let had_selection = edit.selection.is_some();
  let selection = match edit.selection {
    Some(selection) => {
      Selection::from_cursors(
        selection
          .into_iter()
          .map(|range| {
            let cursor_id = CursorId::new(
              NonZeroU64::new(u64::from(range.cursor_id))
                .ok_or_else(|| Error::from_reason("cursor id must be non-zero"))?,
            );
            Ok(CursorRange {
              id:    cursor_id,
              range: Range::new(range.anchor as usize, range.head as usize),
            })
          })
          .collect::<Result<SmallVec<[CursorRange; 1]>>>()?,
      )
      .map_err(|err| Error::from_reason(err.to_string()))?
    },
    None => current_selection.clone(),
  };

  let transaction = match edit.changes {
    Some(changes) if !changes.is_empty() => {
      Transaction::from_replacements(
        len,
        changes.into_iter().map(|change| {
          Replacement {
            span: Span {
              start: change.start as usize,
              end:   change.end as usize,
            },
            text: change.text.map(Into::into),
          }
        }),
        ReplacementOverlapPolicy::Reject,
      )
      .map_err(|err| Error::from_reason(err.to_string()))?
    },
    _ => Transaction::empty(len),
  };

  let selection = if had_selection {
    selection
  } else {
    selection
      .map(transaction.changes())
      .map_err(|err| Error::from_reason(err.to_string()))?
  };

  Ok(Edit {
    transaction,
    selection,
  })
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

fn lsp_server_handle(id: u32) -> Result<LspServerHandle> {
  if id == 0 {
    return Err(Error::from_reason("lsp server handle must be non-zero"));
  }
  Ok(LspServerHandle::from_raw(id as u64))
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
