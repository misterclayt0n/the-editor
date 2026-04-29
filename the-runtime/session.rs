use std::{
  collections::BTreeMap,
  num::NonZeroU64,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
};

use ropey::Rope;
use the_lib::{
  clipboard::{
    ClipboardProvider,
    ClipboardType,
  },
  document::Document,
  registers::Registers,
  render::{
    RenderSurface,
    SurfaceId,
    TextAnnotations,
    TextFormat,
    Viewport,
  },
  surface::Surface,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Error)]
pub enum RuntimeError {
  #[error(transparent)]
  Io(#[from] std::io::Error),

  #[error(transparent)]
  Surface(#[from] the_lib::surface::SurfaceError),

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

  pub fn refresh_document_syntax_and_highlights(
    &mut self,
    document: DocumentHandle,
  ) -> RuntimeResult<()> {
    let highlights = {
      let entry = self
        .documents
        .get_mut(&document)
        .ok_or(RuntimeError::UnknownDocument(document))?;
      self.syntax.reparse_document(&mut entry.document)?;
      self.syntax.collect_document_highlights(&entry.document)
    };

    for surface in self
      .surfaces
      .values_mut()
      .filter(|entry| entry.document == document)
    {
      surface.surface.set_syntax_highlights(highlights.clone());
    }

    Ok(())
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

    let surface = &mut self
      .surfaces
      .get_mut(&surface)
      .ok_or(RuntimeError::UnknownSurface(surface))?
      .surface;
    let document = &mut self
      .documents
      .get_mut(&document)
      .ok_or(RuntimeError::UnknownDocument(document))?
      .document;

    Ok(f(surface, document))
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
    Ok(handle)
  }

  pub fn create_surface(&mut self, document: DocumentHandle) -> RuntimeResult<SurfaceHandle> {
    self.create_surface_with_viewport(document, Viewport::new(0, 1, 0, 80))
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
    let render = RenderSurface::new(
      SurfaceId::new(handle.get()).expect("runtime surface handle must be non-zero"),
      TextFormat::default(),
      TextAnnotations::default(),
    );
    let surface = Surface::new(&document_ref.document, render, viewport)?;

    self
      .surfaces
      .insert(handle, RuntimeSurface { surface, document });
    self.refresh_document_syntax_and_highlights(document)?;
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
    self.refresh_document_syntax_and_highlights(document)?;

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

#[cfg(test)]
mod tests {
  use tempfile::tempdir;
  use the_lib::{
    clipboard::ClipboardType,
    syntax::{
      DocumentSyntax,
      DocumentSyntaxState,
      NullResources,
    },
  };

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
          .apply_edit(document, the_lib::edit::Edit {
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

    let frame = runtime
      .with_surface_document_mut(surface, |surface, document| surface.render(document, None))
      .unwrap();

    let syntax = frame.layers.syntax.expect("syntax layer delta");
    let visible_rows = frame
      .changed_rows
      .iter()
      .map(|row| (row.placement.surface_row, row.text.to_string()))
      .collect::<Vec<_>>();

    let named_spans = syntax
      .spans
      .iter()
      .filter_map(|span| {
        runtime
          .syntax()
          .registry()
          .highlight_name(span.kind)
          .map(|name| (span.surface_row, span.start_col, span.end_col, name))
      })
      .collect::<Vec<_>>();
    let colored_rows = syntax
      .spans
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
      syntax
        .spans
        .iter()
        .take(20)
        .map(|span| (span.surface_row, span.start_col, span.end_col))
        .collect::<Vec<_>>()
    );
    assert!(
      !named_spans.is_empty(),
      "expected highlight names to resolve; spans={:?}",
      syntax
        .spans
        .iter()
        .take(20)
        .map(|span| (span.surface_row, span.start_col, span.end_col, span.kind))
        .collect::<Vec<_>>()
    );
  }
}
