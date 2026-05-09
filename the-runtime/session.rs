use std::{
  collections::BTreeMap,
  num::NonZeroU64,
  ops::Range,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
  time::Duration,
};

use ropey::Rope;
use the_lib::{
  clipboard::{
    ClipboardProvider,
    ClipboardType,
  },
  document::Document,
  operation::{
    OperationAtom,
    OperationSpec,
    execute_operation,
  },
  registers::Registers,
  render::{
    CellGridMetrics,
    CellGridRenderSurface,
    RenderFrame,
    SurfaceId,
    TextFormat,
    Viewport,
  },
  surface::{
    Surface,
    SurfaceSyntaxInterest,
  },
};
use thiserror::Error;

use crate::{
  clipboard::SystemClipboard,
  io::{
    read_text_file,
    write_text_file,
  },
  resource::{
    Resource,
    ResourceId,
    normalize_file_path,
  },
  syntax::{
    RuntimeSyntax,
    RuntimeSyntaxError,
  },
};

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct DocumentHandle(NonZeroU64);

impl DocumentHandle {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }

  fn next(counter: &mut u64) -> Self {
    *counter = counter.wrapping_add(1).max(1);
    Self::new(*counter).expect("document handle must be non-zero")
  }
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct SurfaceHandle(NonZeroU64);

impl SurfaceHandle {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }

  fn next(counter: &mut u64) -> Self {
    *counter = counter.wrapping_add(1).max(1);
    Self::new(*counter).expect("surface handle must be non-zero")
  }
}

#[derive(Debug)]
pub struct RuntimeDocument {
  pub document:       Document,
  pub resource:       Option<ResourceId>,
  pub saved_revision: Option<u64>,
  pub readonly:       bool,
}

impl RuntimeDocument {
  pub fn is_dirty(&self) -> bool {
    self.saved_revision != Some(self.document.revision())
  }
}

#[derive(Debug)]
pub struct RuntimeSurface {
  pub surface:  Surface,
  pub document: DocumentHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SyntaxWorkPriority {
  Demanded,
  Visible,
  Background,
}

#[derive(Debug, Clone)]
struct SurfaceDemandRecord {
  touch:    u64,
  interest: SurfaceSyntaxInterest,
}

#[derive(Debug, Default)]
struct SyntaxScheduler {
  next_touch:       u64,
  document_touches: BTreeMap<DocumentHandle, u64>,
  surface_demands:  BTreeMap<SurfaceHandle, SurfaceDemandRecord>,
}

impl SyntaxScheduler {
  fn touch_document(&mut self, document: DocumentHandle) {
    self.next_touch = self.next_touch.saturating_add(1);
    self.document_touches.insert(document, self.next_touch);
  }

  fn record_surface_interest(&mut self, surface: SurfaceHandle, interest: SurfaceSyntaxInterest) {
    self.next_touch = self.next_touch.saturating_add(1);
    self.surface_demands.insert(surface, SurfaceDemandRecord {
      touch: self.next_touch,
      interest,
    });
  }

  fn last_touch(&self, document: DocumentHandle) -> u64 {
    self.document_touches.get(&document).copied().unwrap_or(0)
  }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
  #[error(transparent)]
  Io(#[from] std::io::Error),

  #[error(transparent)]
  Surface(#[from] the_lib::surface::SurfaceError),

  #[error(transparent)]
  Selection(#[from] the_lib::selection::SelectionError),

  #[error(transparent)]
  Transaction(#[from] the_lib::transaction::TransactionError),

  #[error(transparent)]
  Operation(#[from] the_lib::operation::OperationError),

  #[error(transparent)]
  Syntax(#[from] RuntimeSyntaxError),

  #[error("unknown document handle {0:?}")]
  UnknownDocument(DocumentHandle),

  #[error("unknown surface handle {0:?}")]
  UnknownSurface(SurfaceHandle),

  #[error("unknown resource id {0:?}")]
  UnknownResource(ResourceId),

  #[error("document {document:?} is not attached to a resource")]
  DocumentHasNoResource { document: DocumentHandle },

  #[error("resource path is already attached to document {document:?}")]
  ResourceAlreadyOpen { document: DocumentHandle },

  #[error("document {document:?} is readonly")]
  ReadonlyDocument { document: DocumentHandle },
}

pub type RuntimeResult<T> = std::result::Result<T, RuntimeError>;

pub struct RuntimeSession {
  syntax:             RuntimeSyntax,
  clipboard:          Arc<dyn ClipboardProvider>,
  registers:          Registers,
  syntax_scheduler:   SyntaxScheduler,
  next_resource_id:   u64,
  next_document_id:   u64,
  next_surface_id:    u64,
  resources:          BTreeMap<ResourceId, Resource>,
  resource_paths:     BTreeMap<PathBuf, ResourceId>,
  resource_documents: BTreeMap<ResourceId, DocumentHandle>,
  documents:          BTreeMap<DocumentHandle, RuntimeDocument>,
  surfaces:           BTreeMap<SurfaceHandle, RuntimeSurface>,
}

impl std::fmt::Debug for RuntimeSession {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("RuntimeSession")
      .field("syntax", &self.syntax)
      .field("clipboard", &self.clipboard.name())
      .field("registers", &self.registers)
      .field("resources", &self.resources)
      .field("documents", &self.documents.keys().collect::<Vec<_>>())
      .field("surfaces", &self.surfaces.keys().collect::<Vec<_>>())
      .finish()
  }
}

impl Default for RuntimeSession {
  fn default() -> Self {
    let syntax = RuntimeSyntax::builtin().unwrap_or_else(|err| {
      tracing::warn!("failed to initialize builtin syntax runtime: {err}");
      RuntimeSyntax::disabled()
    });
    Self::new(syntax, SystemClipboard::provider())
  }
}

impl RuntimeSession {
  pub fn new(syntax: RuntimeSyntax, clipboard: Arc<dyn ClipboardProvider>) -> Self {
    Self {
      syntax,
      clipboard,
      registers: Registers::new(),
      syntax_scheduler: SyntaxScheduler::default(),
      next_resource_id: 0,
      next_document_id: 0,
      next_surface_id: 0,
      resources: BTreeMap::new(),
      resource_paths: BTreeMap::new(),
      resource_documents: BTreeMap::new(),
      documents: BTreeMap::new(),
      surfaces: BTreeMap::new(),
    }
  }

  pub fn syntax(&self) -> &RuntimeSyntax {
    &self.syntax
  }

  pub fn clipboard(&self) -> &Arc<dyn ClipboardProvider> {
    &self.clipboard
  }

  pub fn clipboard_name(&self) -> std::borrow::Cow<'_, str> {
    self.clipboard.name()
  }

  pub fn read_clipboard(
    &self,
    clipboard_type: ClipboardType,
  ) -> the_lib::clipboard::Result<String> {
    self.clipboard.get_contents(clipboard_type)
  }

  pub fn write_clipboard(
    &self,
    content: &str,
    clipboard_type: ClipboardType,
  ) -> the_lib::clipboard::Result<()> {
    self.clipboard.set_contents(content, clipboard_type)
  }

  pub fn registers(&self) -> &Registers {
    &self.registers
  }

  pub fn registers_mut(&mut self) -> &mut Registers {
    &mut self.registers
  }

  pub fn read_register(&self, name: char) -> Option<Vec<String>> {
    self
      .registers
      .read(name)
      .map(|values| values.iter().map(|value| value.to_string()).collect())
  }

  pub fn write_register(&mut self, name: char, values: Vec<String>) {
    self.registers.write(
      name,
      values.into_iter().map(the_lib::Tendril::from).collect(),
    );
  }

  pub fn resource(&self, resource: ResourceId) -> Option<&Resource> {
    self.resources.get(&resource)
  }

  pub fn document_entry(&self, document: DocumentHandle) -> Option<&RuntimeDocument> {
    self.documents.get(&document)
  }

  pub fn document_entry_mut(&mut self, document: DocumentHandle) -> Option<&mut RuntimeDocument> {
    self.documents.get_mut(&document)
  }

  pub fn document(&self, document: DocumentHandle) -> Option<&Document> {
    self.document_entry(document).map(|entry| &entry.document)
  }

  pub fn document_mut(&mut self, document: DocumentHandle) -> Option<&mut Document> {
    self
      .document_entry_mut(document)
      .map(|entry| &mut entry.document)
  }

  pub fn surface_entry(&self, surface: SurfaceHandle) -> Option<&RuntimeSurface> {
    self.surfaces.get(&surface)
  }

  pub fn surface_entry_mut(&mut self, surface: SurfaceHandle) -> Option<&mut RuntimeSurface> {
    self.surfaces.get_mut(&surface)
  }

  pub fn surface(&self, surface: SurfaceHandle) -> Option<&Surface> {
    self.surface_entry(surface).map(|entry| &entry.surface)
  }

  pub fn surface_mut(&mut self, surface: SurfaceHandle) -> Option<&mut Surface> {
    self
      .surface_entry_mut(surface)
      .map(|entry| &mut entry.surface)
  }

  pub fn document_for_surface(&self, surface: SurfaceHandle) -> Option<DocumentHandle> {
    self.surface_entry(surface).map(|entry| entry.document)
  }

  pub fn surfaces_for_document(&self, document: DocumentHandle) -> Vec<SurfaceHandle> {
    self
      .surfaces
      .iter()
      .filter_map(|(surface, entry)| (entry.document == document).then_some(*surface))
      .collect()
  }

  pub fn advance_document_syntax(&mut self, document: DocumentHandle) -> RuntimeResult<bool> {
    self.advance_document_syntax_with_budget(document, None, Duration::from_millis(1))
  }

  pub fn drain_syntax_work(
    &mut self,
    max_documents: usize,
    budget_per_document: Duration,
  ) -> RuntimeResult<usize> {
    let mut advanced = 0;
    let mut attempted = Vec::new();

    while attempted.len() < max_documents {
      let Some(document) = self.next_document_for_syntax_work(&attempted) else {
        break;
      };
      attempted.push(document);

      let interest = self.primary_interest_for_document(document);
      if self.advance_document_syntax_with_budget(
        document,
        interest.as_ref(),
        budget_per_document,
      )? {
        advanced += 1;
      }
    }

    Ok(advanced)
  }

  pub fn render_surface(&mut self, surface: SurfaceHandle) -> RuntimeResult<RenderFrame> {
    let registry = self.prepare_surface_render(surface)?;
    self.with_surface_document_mut(surface, move |surface, document| {
      surface.raw_frame_delta(document, Some(registry.as_ref()))
    })
  }

  fn prepare_surface_render(
    &mut self,
    surface: SurfaceHandle,
  ) -> RuntimeResult<Arc<the_lib::syntax::LanguageRegistry>> {
    let document_handle = self
      .surfaces
      .get(&surface)
      .ok_or(RuntimeError::UnknownSurface(surface))?
      .document;
    let interest = self.with_surface_document_mut(surface, |surface, document| {
      surface.syntax_interest(document)
    })?;

    self
      .syntax_scheduler
      .record_surface_interest(surface, interest.clone());
    self.syntax_scheduler.touch_document(document_handle);
    let _ = self.advance_document_syntax_with_budget(
      document_handle,
      Some(&interest),
      Duration::from_millis(2),
    )?;

    Ok(self.syntax.registry_arc())
  }

  pub fn with_surface_document_mut<R>(
    &mut self,
    surface: SurfaceHandle,
    f: impl FnOnce(&mut Surface, &mut Document) -> R,
  ) -> RuntimeResult<R> {
    let document = self
      .surfaces
      .get(&surface)
      .ok_or(RuntimeError::UnknownSurface(surface))?
      .document;

    let before_revision = self
      .documents
      .get(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?
      .document
      .revision();

    let result = {
      let surface = &mut self
        .surfaces
        .get_mut(&surface)
        .ok_or(RuntimeError::UnknownSurface(surface))?
        .surface;
      let document_ref = &mut self
        .documents
        .get_mut(&document)
        .ok_or(RuntimeError::UnknownDocument(document))?
        .document;
      f(surface, document_ref)
    };

    let after_revision = self
      .documents
      .get(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?
      .document
      .revision();
    if after_revision != before_revision {
      self.syntax_scheduler.touch_document(document);
    }

    Ok(result)
  }

  fn advance_document_syntax_with_budget(
    &mut self,
    document: DocumentHandle,
    interest: Option<&SurfaceSyntaxInterest>,
    budget: Duration,
  ) -> RuntimeResult<bool> {
    let entry = self
      .documents
      .get_mut(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?;
    let advanced = self
      .syntax
      .advance_document_syntax(&mut entry.document, interest, budget)?;
    if advanced {
      self.syntax_scheduler.touch_document(document);
    }
    Ok(advanced)
  }

  fn next_document_for_syntax_work(&self, attempted: &[DocumentHandle]) -> Option<DocumentHandle> {
    let mut best: Option<(SyntaxWorkPriority, std::cmp::Reverse<u64>, DocumentHandle)> = None;

    for document in self.documents.keys().copied() {
      if attempted.contains(&document) {
        continue;
      }

      let Some(priority) = self.syntax_work_priority(document) else {
        continue;
      };
      let candidate = (
        priority,
        std::cmp::Reverse(self.syntax_scheduler.last_touch(document)),
        document,
      );
      if best.as_ref().is_none_or(|best| candidate < *best) {
        best = Some(candidate);
      }
    }

    best.map(|(_, _, document)| document)
  }

  fn syntax_work_priority(&self, document: DocumentHandle) -> Option<SyntaxWorkPriority> {
    let invalidation = self.document(document)?.syntax_invalidation()?;
    if invalidation.tree.is_empty() {
      return None;
    }

    let demand_ranges = self.document_demand_ranges(document);
    if demand_ranges
      .iter()
      .any(|range| invalidation.tree.intersects(range))
    {
      return Some(SyntaxWorkPriority::Demanded);
    }

    if self.surfaces_for_document(document).is_empty() {
      Some(SyntaxWorkPriority::Background)
    } else {
      Some(SyntaxWorkPriority::Visible)
    }
  }

  fn primary_interest_for_document(
    &self,
    document: DocumentHandle,
  ) -> Option<SurfaceSyntaxInterest> {
    self
      .surfaces_for_document(document)
      .into_iter()
      .filter_map(|surface| {
        self
          .syntax_scheduler
          .surface_demands
          .get(&surface)
          .map(|record| (record.touch, record.interest.clone()))
      })
      .max_by_key(|(touch, _)| *touch)
      .map(|(_, interest)| interest)
  }

  fn document_demand_ranges(&self, document: DocumentHandle) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();

    for surface in self.surfaces_for_document(document) {
      if let Some(record) = self.syntax_scheduler.surface_demands.get(&surface) {
        merge_range(&mut ranges, record.interest.byte_range.clone());
      }
    }

    ranges
  }

  pub fn open_file(&mut self, path: &Path) -> RuntimeResult<DocumentHandle> {
    let normalized = normalize_file_path(path)?;

    if let Some(resource) = self.resource_paths.get(&normalized).copied()
      && let Some(document) = self.resource_documents.get(&resource).copied()
    {
      return Ok(document);
    }

    let text = read_text_file(&normalized)?;
    let mut document = Document::new(text);
    self
      .syntax
      .activate_document(&mut document, Some(&normalized));

    let resource = self.insert_or_get_resource(normalized.clone())?;
    let handle = DocumentHandle::next(&mut self.next_document_id);
    let saved_revision = Some(document.revision());

    self.documents.insert(handle, RuntimeDocument {
      document,
      resource: Some(resource),
      saved_revision,
      readonly: false,
    });
    self.resource_documents.insert(resource, handle);
    self.syntax_scheduler.touch_document(handle);
    Ok(handle)
  }

  pub fn create_scratch(&mut self, text: Rope) -> RuntimeResult<DocumentHandle> {
    let handle = DocumentHandle::next(&mut self.next_document_id);
    self.documents.insert(handle, RuntimeDocument {
      document:       Document::new(text),
      resource:       None,
      saved_revision: None,
      readonly:       false,
    });
    self.syntax_scheduler.touch_document(handle);
    Ok(handle)
  }

  pub fn create_surface(&mut self, document: DocumentHandle) -> RuntimeResult<SurfaceHandle> {
    self.create_surface_with_viewport(document, Viewport::new(0, 1, 0, 80))
  }

  pub fn set_surface_viewport(
    &mut self,
    surface: SurfaceHandle,
    viewport: Viewport,
  ) -> RuntimeResult<()> {
    self.with_surface_document_mut(surface, move |surface, document| {
      surface.set_viewport_reconcile_document(document, viewport);
    })?;
    Ok(())
  }

  pub fn set_surface_measurements(
    &mut self,
    surface: SurfaceHandle,
    measurements: CellGridMetrics,
  ) -> RuntimeResult<()> {
    let entry = self
      .surfaces
      .get_mut(&surface)
      .ok_or(RuntimeError::UnknownSurface(surface))?;
    entry.surface.set_measurements(measurements);
    Ok(())
  }

  pub fn set_surface_soft_wrap(
    &mut self,
    surface: SurfaceHandle,
    soft_wrap: bool,
  ) -> RuntimeResult<()> {
    let entry = self
      .surfaces
      .get_mut(&surface)
      .ok_or(RuntimeError::UnknownSurface(surface))?;
    let mut format = entry.surface.text_format().clone();
    format.soft_wrap = soft_wrap;
    entry.surface.set_text_format(format);
    Ok(())
  }

  pub fn set_surface_active_cursor(
    &mut self,
    surface: SurfaceHandle,
    cursor_id: Option<u64>,
  ) -> RuntimeResult<()> {
    let entry = self
      .surfaces
      .get_mut(&surface)
      .ok_or(RuntimeError::UnknownSurface(surface))?;
    let cursor_id = cursor_id.and_then(NonZeroU64::new).map(the_lib::selection::CursorId::new);
    entry.surface.set_active_cursor(cursor_id)?;
    Ok(())
  }

  pub fn apply_surface_edit(
    &mut self,
    surface: SurfaceHandle,
    edit: the_lib::operation::Edit,
  ) -> RuntimeResult<()> {
    self.with_surface_document_mut(surface, move |surface, document| {
      surface.apply_edit(document, edit)
    })??;
    Ok(())
  }

  pub fn apply_surface_operation(
    &mut self,
    surface: SurfaceHandle,
    operation: OperationSpec,
  ) -> RuntimeResult<()> {
    let syntax_registry = self.prepare_surface_render(surface)?;
    let output = {
      let surface_entry = self
        .surfaces
        .get(&surface)
        .ok_or(RuntimeError::UnknownSurface(surface))?;
      let document_entry = self
        .documents
        .get(&surface_entry.document)
        .ok_or(RuntimeError::UnknownDocument(surface_entry.document))?;
      let document = &document_entry.document;
      let state = document.active_syntax();
      let indent_query = state.and_then(|state| {
        syntax_registry
          .indent_query(state.language_id())
          .ok()
          .flatten()
      });
      let syntax = state.map(|state| state.syntax());
      execute_operation(
        document.text(),
        surface_entry.surface.selection(),
        surface_entry.surface.visual_goals(),
        operation,
        document.line_ending().as_str(),
        &document.indent_style(),
        indent_query,
        syntax.as_deref(),
        Some(syntax_registry.as_ref()),
      )?
    };

    for atom in output.atoms {
      self.apply_surface_operation_atom(surface, atom)?;
    }

    Ok(())
  }

  fn apply_surface_operation_atom(
    &mut self,
    surface: SurfaceHandle,
    atom: OperationAtom,
  ) -> RuntimeResult<()> {
    match atom {
      OperationAtom::Selection(selection) => {
        self.with_surface_document_mut(surface, move |surface, document| {
          surface.apply_selection(document, selection)
        })??;
      },
      OperationAtom::Motion(result) => {
        self.with_surface_document_mut(surface, move |surface, document| {
          surface.apply_motion_result(document, result)
        })??;
      },
      OperationAtom::Edit(edit) => {
        self.with_surface_document_mut(surface, move |surface, document| {
          surface.apply_edit(document, edit)
        })??;
      },
      OperationAtom::RegisterWrite(write) => {
        self.registers.write(write.name, write.values);
      },
    }
    Ok(())
  }

  pub fn commit_surface_undo_checkpoint(&mut self, surface: SurfaceHandle) -> RuntimeResult<()> {
    self.with_surface_document_mut(surface, |surface, _document| {
      surface.commit_undo_checkpoint()
    })??;
    Ok(())
  }

  pub fn undo_surface(&mut self, surface: SurfaceHandle, count: usize) -> RuntimeResult<()> {
    let count = count.max(1);
    self.with_surface_document_mut(surface, move |surface, document| {
      for _ in 0..count {
        if !surface.undo(document)? {
          break;
        }
      }
      Ok::<_, the_lib::surface::SurfaceError>(())
    })??;
    Ok(())
  }

  pub fn redo_surface(&mut self, surface: SurfaceHandle, count: usize) -> RuntimeResult<()> {
    let count = count.max(1);
    self.with_surface_document_mut(surface, move |surface, document| {
      for _ in 0..count {
        if !surface.redo(document)? {
          break;
        }
      }
      Ok::<_, the_lib::surface::SurfaceError>(())
    })??;
    Ok(())
  }

  pub fn earlier_surface(&mut self, surface: SurfaceHandle, count: usize) -> RuntimeResult<()> {
    let count = count.max(1);
    self.with_surface_document_mut(surface, move |surface, document| {
      for _ in 0..count {
        if !surface.earlier(document, the_lib::history::UndoKind::Steps(1))? {
          break;
        }
      }
      Ok::<_, the_lib::surface::SurfaceError>(())
    })??;
    Ok(())
  }

  pub fn later_surface(&mut self, surface: SurfaceHandle, count: usize) -> RuntimeResult<()> {
    let count = count.max(1);
    self.with_surface_document_mut(surface, move |surface, document| {
      for _ in 0..count {
        if !surface.later(document, the_lib::history::UndoKind::Steps(1))? {
          break;
        }
      }
      Ok::<_, the_lib::surface::SurfaceError>(())
    })??;
    Ok(())
  }

  pub fn create_surface_with_viewport(
    &mut self,
    document: DocumentHandle,
    viewport: Viewport,
  ) -> RuntimeResult<SurfaceHandle> {
    let document_ref = self
      .documents
      .get(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?;
    let handle = SurfaceHandle::next(&mut self.next_surface_id);
    let render = CellGridRenderSurface::new(
      SurfaceId::new(handle.get()).expect("runtime surface handle must be non-zero"),
      TextFormat::default(),
    );
    let surface = Surface::new(&document_ref.document, render, viewport)?;

    self
      .surfaces
      .insert(handle, RuntimeSurface { surface, document });
    self.syntax_scheduler.touch_document(document);
    Ok(handle)
  }

  pub fn save(&mut self, document: DocumentHandle) -> RuntimeResult<()> {
    let entry = self
      .documents
      .get_mut(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?;

    if entry.readonly {
      return Err(RuntimeError::ReadonlyDocument { document });
    }

    let resource = entry
      .resource
      .ok_or(RuntimeError::DocumentHasNoResource { document })?;
    let path = self
      .resources
      .get(&resource)
      .ok_or(RuntimeError::UnknownResource(resource))?
      .path()
      .to_path_buf();

    write_text_file(&path, entry.document.text())?;
    entry.saved_revision = Some(entry.document.revision());
    Ok(())
  }

  pub fn save_as(&mut self, document: DocumentHandle, path: PathBuf) -> RuntimeResult<ResourceId> {
    let normalized = normalize_file_path(&path)?;
    if let Some(existing) = self.resource_paths.get(&normalized).copied()
      && self.resource_documents.get(&existing).copied() != Some(document)
    {
      return Err(RuntimeError::ResourceAlreadyOpen {
        document: self.resource_documents[&existing],
      });
    }

    {
      let entry = self
        .documents
        .get_mut(&document)
        .ok_or(RuntimeError::UnknownDocument(document))?;
      if entry.readonly {
        return Err(RuntimeError::ReadonlyDocument { document });
      }
      write_text_file(&normalized, entry.document.text())?;
    }

    let resource = self.insert_or_get_resource(normalized.clone())?;

    let previous_resource = self
      .documents
      .get(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?
      .resource;
    if let Some(previous) = previous_resource {
      self.resource_documents.remove(&previous);
    }

    let entry = self
      .documents
      .get_mut(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?;
    entry.resource = Some(resource);
    entry.saved_revision = Some(entry.document.revision());
    self
      .syntax
      .activate_document(&mut entry.document, Some(&normalized));
    self.resource_documents.insert(resource, document);
    self.syntax_scheduler.touch_document(document);

    Ok(resource)
  }

  fn insert_or_get_resource(&mut self, path: PathBuf) -> RuntimeResult<ResourceId> {
    if let Some(resource) = self.resource_paths.get(&path).copied() {
      return Ok(resource);
    }

    let resource = ResourceId::next(&mut self.next_resource_id);
    let record = Resource::file(&path)?;
    self.resource_paths.insert(path, resource);
    self.resources.insert(resource, record);
    Ok(resource)
  }
}

fn merge_range(ranges: &mut Vec<Range<usize>>, mut range: Range<usize>) {
  if range.start >= range.end {
    return;
  }

  let mut idx = 0;
  while idx < ranges.len() {
    if range.end < ranges[idx].start {
      break;
    }
    if ranges[idx].end < range.start {
      idx += 1;
      continue;
    }

    range.start = range.start.min(ranges[idx].start);
    range.end = range.end.max(ranges[idx].end);
    ranges.remove(idx);
  }

  ranges.insert(idx, range);
}

#[cfg(test)]
mod tests {
  use std::borrow::Cow;

  use tempfile::tempdir;
  use the_lib::{
    clipboard::ClipboardType,
    syntax::{
      DocumentSyntax,
      DocumentSyntaxState,
      NullResources,
      QueryKind,
      SyntaxResources,
      tree_sitter::Grammar,
    },
    transaction::Transaction,
  };

  #[derive(Debug, Clone, Copy)]
  struct JsonResources {
    grammar: Grammar,
    query:   &'static str,
  }

  impl SyntaxResources for JsonResources {
    fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
      (grammar_name == "json").then_some(self.grammar)
    }

    fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
      (language_id == "json" && kind == QueryKind::Highlights).then_some(Cow::Borrowed(self.query))
    }
  }

  use super::*;

  #[test]
  fn create_scratch_and_surface() {
    let mut runtime = RuntimeSession::default();
    let document = runtime.create_scratch(Rope::from("hello")).unwrap();
    let surface = runtime.create_surface(document).unwrap();

    assert_eq!(
      runtime.document(document).unwrap().text().to_string(),
      "hello"
    );
    assert_eq!(runtime.document_for_surface(surface), Some(document));
    assert_eq!(
      runtime.surface(surface).unwrap().selection().ranges()[0],
      the_lib::selection::Range::point(0)
    );
  }

  #[test]
  fn open_file_activates_syntax_and_save_updates_baseline() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();

    let syntax = RuntimeSyntax::new(
      toml::from_str(
        r#"
          [[language]]
          name = "rust"
          scope = "source.rust"
          file-types = ["rs"]
        "#,
      )
      .unwrap(),
      NullResources,
    )
    .unwrap();
    let mut runtime = RuntimeSession::new(syntax, SystemClipboard::provider());

    let document = runtime.open_file(&path).unwrap();
    assert!(matches!(
      runtime.document(document).unwrap().syntax(),
      DocumentSyntax::Assigned {
        state: DocumentSyntaxState::Unavailable(_),
        ..
      }
    ));
    assert!(!runtime.document_entry(document).unwrap().is_dirty());

    runtime
      .document_mut(document)
      .unwrap()
      .set_line_ending(the_lib::line_ending::LineEnding::LF);
    runtime.save(document).unwrap();
    assert!(!runtime.document_entry(document).unwrap().is_dirty());
  }

  #[test]
  fn save_as_attaches_resource_to_scratch_document() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("scratch.txt");
    let mut runtime = RuntimeSession::default();
    let document = runtime.create_scratch(Rope::from("hello")).unwrap();

    let resource = runtime.save_as(document, path.clone()).unwrap();

    assert_eq!(std::fs::read_to_string(path).unwrap(), "hello");
    assert_eq!(
      runtime.document_entry(document).unwrap().resource,
      Some(resource)
    );
    assert!(!runtime.document_entry(document).unwrap().is_dirty());
  }

  #[test]
  fn with_surface_document_mut_allows_joint_mutation() {
    let mut runtime = RuntimeSession::default();
    let document = runtime.create_scratch(Rope::from("hello")).unwrap();
    let surface = runtime.create_surface(document).unwrap();

    runtime
      .with_surface_document_mut(surface, |surface, document| {
        let tx = the_lib::transaction::Transaction::insert_at(document.text().len_chars(), 5, "!")
          .unwrap();
        surface
          .apply_edit(document, the_lib::operation::Edit {
            transaction: tx,
            selection:   the_lib::selection::Selection::point(6),
          })
          .unwrap();
      })
      .unwrap();

    assert_eq!(
      runtime.document(document).unwrap().text().to_string(),
      "hello!"
    );
  }

  #[test]
  fn session_exposes_clipboard_and_registers() {
    let runtime = RuntimeSession::default();
    let _ = runtime.write_clipboard("hello", ClipboardType::Clipboard);
    let _ = runtime.read_clipboard(ClipboardType::Clipboard);
    assert!(!runtime.clipboard_name().is_empty());
    assert!(runtime.registers().is_empty());
  }

  #[test]
  fn drain_syntax_work_advances_dirty_document_baseline() {
    let grammar = Grammar::try_from(tree_sitter_json::LANGUAGE).unwrap();
    let syntax = RuntimeSyntax::from_toml_str(
      r#"
        [[language]]
        name = "json"
        scope = "source.json"
        file-types = ["json"]
      "#,
      JsonResources {
        grammar,
        query: "(string) @string\n(number) @number",
      },
    )
    .unwrap();

    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    std::fs::write(&path, "{\"name\": 1}\n").unwrap();

    let mut runtime = RuntimeSession::new(syntax, SystemClipboard::provider());
    let document = runtime.open_file(&path).unwrap();
    let surface = runtime.create_surface(document).unwrap();

    runtime
      .with_surface_document_mut(surface, |surface, document| {
        let tx = Transaction::insert_at(
          document.text().len_chars(),
          document.text().len_chars() - 2,
          "2",
        )
        .unwrap();
        surface
          .apply_edit(document, the_lib::operation::Edit {
            transaction: tx,
            selection:   the_lib::selection::Selection::point(document.text().len_chars() - 1),
          })
          .unwrap();
      })
      .unwrap();

    assert!(
      !runtime
        .document(document)
        .unwrap()
        .syntax_invalidation()
        .unwrap()
        .tree
        .is_empty()
    );

    let advanced = runtime
      .drain_syntax_work(1, Duration::from_millis(20))
      .unwrap();
    assert_eq!(advanced, 1);
    assert!(
      runtime
        .document(document)
        .unwrap()
        .syntax_invalidation()
        .unwrap()
        .tree
        .is_empty()
    );
  }

  #[test]
  fn apply_surface_operation_undo_redo_respect_count() {
    let mut runtime = RuntimeSession::default();
    let document = runtime.create_scratch(Rope::from("hello")).unwrap();
    let surface = runtime.create_surface(document).unwrap();

    runtime
      .apply_surface_operation(surface, OperationSpec {
        name:         "insert_text".into(),
        count:        None,
        text:         Some("!".into()),
        values:       None,
        after:        None,
        line_ending:  None,
        indent:       None,
        indent_unit:  None,
        indent_width: None,
        tab_width:    None,
      })
      .unwrap();
    runtime
      .apply_surface_operation(surface, OperationSpec {
        name:         "insert_text".into(),
        count:        None,
        text:         Some("?".into()),
        values:       None,
        after:        None,
        line_ending:  None,
        indent:       None,
        indent_unit:  None,
        indent_width: None,
        tab_width:    None,
      })
      .unwrap();

    assert_eq!(
      runtime.document(document).unwrap().text().to_string(),
      "!?hello"
    );

    runtime.undo_surface(surface, 2).unwrap();
    assert_eq!(
      runtime.document(document).unwrap().text().to_string(),
      "hello"
    );

    runtime.redo_surface(surface, 2).unwrap();
    assert_eq!(
      runtime.document(document).unwrap().text().to_string(),
      "!?hello"
    );
  }

  #[test]
  fn yank_and_paste_use_runtime_registers() {
    let mut runtime = RuntimeSession::default();
    let document = runtime.create_scratch(Rope::from("hello")).unwrap();
    let surface = runtime.create_surface(document).unwrap();

    runtime
      .with_surface_document_mut(surface, |surface, document| {
        surface.apply_selection(document, the_lib::selection::Selection::single(0, 5))
      })
      .unwrap()
      .unwrap();
    runtime
      .apply_surface_operation(surface, OperationSpec {
        name:         "yank".into(),
        count:        None,
        text:         None,
        values:       None,
        after:        None,
        line_ending:  None,
        indent:       None,
        indent_unit:  None,
        indent_width: None,
        tab_width:    None,
      })
      .unwrap();
    let values = runtime.read_register('"').unwrap();
    runtime
      .apply_surface_operation(surface, OperationSpec {
        name:         "paste".into(),
        count:        None,
        text:         None,
        values:       Some(values),
        after:        Some(true),
        line_ending:  None,
        indent:       None,
        indent_unit:  None,
        indent_width: None,
        tab_width:    None,
      })
      .unwrap();

    assert_eq!(
      runtime.document(document).unwrap().text().to_string(),
      "hellohello"
    );
  }

  #[test]
  fn default_runtime_session_renders_visible_syntax_on_initial_rust_rows() {
    let path = Path::new("the-lib/examples/demo_editor.rs");
    if !path.exists() {
      return;
    }

    let mut runtime = RuntimeSession::default();
    let document = runtime.open_file(path).unwrap();
    let surface = runtime
      .create_surface_with_viewport(document, Viewport::new(0, 20, 0, 120))
      .unwrap();

    let frame = runtime.render_surface(surface).unwrap();

    let visible_rows = frame
      .rows
      .iter()
      .map(|row| (row.surface_row, row.text.to_string()))
      .collect::<Vec<_>>();

    let named_spans = frame
      .syntax
      .iter()
      .filter_map(|span| {
        runtime
          .syntax()
          .registry()
          .highlight_name(span.kind)
          .map(|name| (span.surface_row, span.start_col, span.end_col, name))
      })
      .collect::<Vec<_>>();
    let colored_rows = frame
      .syntax
      .iter()
      .map(|span| span.surface_row)
      .collect::<std::collections::BTreeSet<_>>();

    let first_nonempty_row = visible_rows
      .iter()
      .find(|(_, text)| !text.trim().is_empty())
      .map(|(row, _)| *row)
      .expect("expected at least one non-empty visible row");

    assert!(
      colored_rows.contains(&first_nonempty_row),
      "expected syntax highlighting on first non-empty visible row {first_nonempty_row}; \
       rows={visible_rows:?}, spans={:?}",
      frame
        .syntax
        .iter()
        .take(20)
        .map(|span| (span.surface_row, span.start_col, span.end_col))
        .collect::<Vec<_>>()
    );
    assert!(
      !named_spans.is_empty(),
      "expected highlight names to resolve; spans={:?}",
      frame
        .syntax
        .iter()
        .take(20)
        .map(|span| (span.surface_row, span.start_col, span.end_col, span.kind))
        .collect::<Vec<_>>()
    );
  }
}
