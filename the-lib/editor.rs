//! Minimal editor/surface state for the-lib.
//!
//! This is intentionally small: it owns a set of document buffers plus
//! per-pane view/render state. IO, UI, and dispatch logic live outside
//! of the-lib.

use std::{
  collections::BTreeMap,
  num::NonZeroUsize,
  path::{
    Path,
    PathBuf,
  },
};

use ropey::Rope;

use crate::{
  document::{
    Document,
    DocumentError,
    DocumentId,
  },
  render::{
    graphics::Rect,
    plan::RenderCache,
  },
  selection::{
    CursorId,
    Selection,
  },
  split_tree::{
    PaneDirection,
    PaneId,
    PaneNeighbors,
    SplitAxis,
    SplitNodeId,
    SplitSeparator,
    SplitTree,
  },
  syntax::Loader,
  transaction::Transaction,
  view::ViewState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EditorId(NonZeroUsize);

impl EditorId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for EditorId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(NonZeroUsize);

impl BufferId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }

  pub const fn as_u64(self) -> u64 {
    self.0.get() as u64
  }

  pub fn from_u64(id: u64) -> Option<Self> {
    let id = usize::try_from(id).ok()?;
    NonZeroUsize::new(id).map(Self::new)
  }
}

impl From<NonZeroUsize> for BufferId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenBufferPolicy {
  pub reuse_active_untitled_buffer: bool,
}

impl Default for OpenBufferPolicy {
  fn default() -> Self {
    Self {
      reuse_active_untitled_buffer: true,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PaneItemsState {
  items:  Vec<PaneContent>,
  active: usize,
}

impl PaneItemsState {
  fn new(initial: PaneContent) -> Self {
    Self {
      items:  vec![initial],
      active: 0,
    }
  }

  fn active_content(&self) -> Option<PaneContent> {
    self.items.get(self.active).copied()
  }

  fn activate(&mut self, content: PaneContent) {
    if let Some(index) = self.items.iter().position(|item| *item == content) {
      self.active = index;
    } else {
      self.items.push(content);
      self.active = self.items.len().saturating_sub(1);
    }
  }

  fn remove(&mut self, content: PaneContent) -> bool {
    let Some(index) = self.items.iter().position(|item| *item == content) else {
      return false;
    };
    self.items.remove(index);
    if self.items.is_empty() {
      self.active = 0;
    } else if self.active > index {
      self.active -= 1;
    } else if self.active >= self.items.len() {
      self.active = self.items.len() - 1;
    }
    true
  }

  fn merge_in<I>(&mut self, items: I)
  where
    I: IntoIterator<Item = PaneContent>,
  {
    for item in items {
      if !self.items.contains(&item) {
        self.items.push(item);
      }
    }
    if !self.items.is_empty() {
      self.active = self.active.min(self.items.len() - 1);
    }
  }

  fn preferred_buffer_id(&self) -> Option<BufferId> {
    match self.active_content() {
      Some(PaneContent::EditorBuffer { buffer_id }) => Some(buffer_id),
      _ => self.items.iter().rev().find_map(|item| {
        match item {
          PaneContent::EditorBuffer { buffer_id } => Some(*buffer_id),
          PaneContent::ClientSurface { .. } => None,
        }
      }),
    }
  }
}

#[derive(Debug)]
struct EditorSurfaceState {
  layout_viewport:        Rect,
  split_tree:             SplitTree,
  pane_content:           BTreeMap<PaneId, PaneContent>,
  pane_items:             BTreeMap<PaneId, PaneItemsState>,
  pane_views:             BTreeMap<PaneId, ViewState>,
  client_surfaces:        BTreeMap<ClientSurfaceId, ClientSurfaceAttachment>,
  next_client_surface_id: NonZeroUsize,
}

impl EditorSurfaceState {
  fn new(view: &ViewState, first_buffer_id: BufferId) -> Self {
    let split_tree = SplitTree::new();
    let initial_content = PaneContent::EditorBuffer {
      buffer_id: first_buffer_id,
    };
    let mut pane_content = BTreeMap::new();
    pane_content.insert(split_tree.active_pane(), initial_content);
    let mut pane_items = BTreeMap::new();
    pane_items.insert(split_tree.active_pane(), PaneItemsState::new(initial_content));
    let mut pane_views = BTreeMap::new();
    pane_views.insert(split_tree.active_pane(), view.clone());

    Self {
      layout_viewport: view.viewport,
      split_tree,
      pane_content,
      pane_items,
      pane_views,
      client_surfaces: BTreeMap::new(),
      next_client_surface_id: NonZeroUsize::new(1).expect("nonzero"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct EditorPolicyState {
  open_buffer: OpenBufferPolicy,
}

#[derive(Debug)]
pub struct Editor {
  id:                EditorId,
  buffers:           Vec<BufferState>,
  active_buffer:     BufferId,
  surface:           EditorSurfaceState,
  next_buffer_id:    NonZeroUsize,
  next_document_id:  NonZeroUsize,
  access_history:    Vec<BufferId>,
  modified_history:  Vec<BufferId>,
  policy:            EditorPolicyState,
  jumplist_backward: Vec<JumpEntry>,
  jumplist_forward:  Vec<JumpEntry>,
}

#[derive(Debug)]
struct BufferState {
  id:                       BufferId,
  document:                 Document,
  view:                     ViewState,
  render_cache:             RenderCache,
  file_path:                Option<PathBuf>,
  object_selection_history: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JumpEntry {
  buffer_id:     BufferId,
  selection:     Selection,
  active_cursor: Option<crate::selection::CursorId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneSnapshot {
  pub pane_id:        PaneId,
  pub buffer_id:      BufferId,
  pub rect:           Rect,
  pub is_active_pane: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePaneSnapshot {
  pub pane_id:        PaneId,
  pub content:        PaneContent,
  pub rect:           Rect,
  pub is_active_pane: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenTarget {
  Active,
  Pane(PaneId),
  Neighbor {
    direction:         PaneDirection,
    create_if_missing: bool,
  },
  Split {
    axis:      SplitAxis,
    focus_new: bool,
  },
}

impl OpenTarget {
  pub const fn active() -> Self {
    Self::Active
  }

  pub const fn pane(pane: PaneId) -> Self {
    Self::Pane(pane)
  }

  pub const fn neighbor(direction: PaneDirection) -> Self {
    Self::Neighbor {
      direction,
      create_if_missing: false,
    }
  }

  pub const fn neighbor_or_split(direction: PaneDirection) -> Self {
    Self::Neighbor {
      direction,
      create_if_missing: true,
    }
  }

  pub const fn split(axis: SplitAxis) -> Self {
    Self::Split {
      axis,
      focus_new: true,
    }
  }

  pub const fn split_without_focus(axis: SplitAxis) -> Self {
    Self::Split {
      axis,
      focus_new: false,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedOpenTarget {
  pub pane:             PaneId,
  pub restore_focus_to: Option<PaneId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientSurfaceId(NonZeroUsize);

impl ClientSurfaceId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for ClientSurfaceId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneContentKind {
  EditorBuffer,
  ClientSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneContent {
  EditorBuffer { buffer_id: BufferId },
  ClientSurface { surface_id: ClientSurfaceId },
}

impl PaneContent {
  pub const fn kind(self) -> PaneContentKind {
    match self {
      Self::EditorBuffer { .. } => PaneContentKind::EditorBuffer,
      Self::ClientSurface { .. } => PaneContentKind::ClientSurface,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClientSurfaceAttachment {
  surface_id:    ClientSurfaceId,
  attached_pane: Option<PaneId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSurfaceSnapshot {
  pub client_surface_id: ClientSurfaceId,
  pub attached_pane:     Option<PaneId>,
  pub is_active:         bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferSnapshot {
  pub buffer_id:    BufferId,
  pub file_path:    Option<PathBuf>,
  pub display_name: String,
  pub modified:     bool,
  pub is_active:    bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSurfaceSnapshot {
  pub pane_id:      PaneId,
  pub buffer_id:    BufferId,
  pub file_path:    Option<PathBuf>,
  pub display_name: String,
  pub modified:     bool,
  pub is_active:    bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneItemSnapshot {
  pub content:   PaneContent,
  pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneItemGroupSnapshot {
  pub pane_id:        PaneId,
  pub is_active_pane: bool,
  pub items:          Vec<PaneItemSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpSnapshot {
  pub buffer_id:     BufferId,
  pub selection:     Selection,
  pub active_cursor: Option<CursorId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyTransactionResult {
  pub buffer_id:       BufferId,
  pub changed:         bool,
  pub syntax_attached: bool,
}

impl BufferState {
  fn new(id: BufferId, document: Document, view: ViewState, file_path: Option<PathBuf>) -> Self {
    Self {
      id,
      document,
      view,
      render_cache: RenderCache::default(),
      file_path,
      object_selection_history: Vec::new(),
    }
  }
}

impl Editor {
  fn index_of_buffer_id(&self, buffer_id: BufferId) -> Option<usize> {
    self
      .buffers
      .iter()
      .position(|buffer| buffer.id == buffer_id)
  }

  fn buffer_id_at_index(&self, index: usize) -> Option<BufferId> {
    self.buffers.get(index).map(|buffer| buffer.id)
  }

  fn alloc_buffer_id(&mut self) -> BufferId {
    let id = BufferId::new(self.next_buffer_id);
    let next = self.next_buffer_id.get().saturating_add(1);
    self.next_buffer_id = NonZeroUsize::new(next).unwrap_or(self.next_buffer_id);
    id
  }

  fn active_buffer_index_internal(&self) -> usize {
    self
      .index_of_buffer_id(self.active_buffer)
      .expect("active buffer must exist")
  }

  fn buffer_snapshot_for_index(&self, index: usize) -> Option<BufferSnapshot> {
    let buffer = self.buffers.get(index)?;
    Some(BufferSnapshot {
      buffer_id:    buffer.id,
      file_path:    buffer.file_path.clone(),
      display_name: buffer.document.display_name().into_owned(),
      modified:     buffer.document.flags().modified,
      is_active:    buffer.id == self.active_buffer,
    })
  }

  pub fn new(id: EditorId, document: Document, view: ViewState) -> Self {
    let next_doc = document.id().get().get().saturating_add(1);
    let next_document_id = NonZeroUsize::new(next_doc).unwrap_or(document.id().get());
    let first_buffer_id = BufferId::new(NonZeroUsize::new(1).expect("nonzero"));

    Self {
      id,
      buffers: vec![BufferState::new(
        first_buffer_id,
        document,
        view.clone(),
        None,
      )],
      active_buffer: first_buffer_id,
      surface: EditorSurfaceState::new(&view, first_buffer_id),
      next_buffer_id: NonZeroUsize::new(2).expect("nonzero"),
      next_document_id,
      policy: EditorPolicyState::default(),
      access_history: Vec::new(),
      modified_history: Vec::new(),
      jumplist_backward: Vec::new(),
      jumplist_forward: Vec::new(),
    }
  }

  fn pane_items_state_mut(&mut self, pane: PaneId, fallback: PaneContent) -> &mut PaneItemsState {
    self
      .surface
      .pane_items
      .entry(pane)
      .or_insert_with(|| PaneItemsState::new(fallback))
  }

  fn activate_pane_item(&mut self, pane: PaneId, content: PaneContent) {
    self.pane_items_state_mut(pane, content).activate(content);
  }

  fn remove_pane_item(&mut self, pane: PaneId, content: PaneContent) -> bool {
    self
      .surface
      .pane_items
      .get_mut(&pane)
      .is_some_and(|state| state.remove(content))
  }

  fn remove_pane_item_from_all(&mut self, content: PaneContent) {
    for state in self.surface.pane_items.values_mut() {
      let _ = state.remove(content);
    }
  }

  fn merge_pane_items(&mut self, pane: PaneId, items: Vec<PaneContent>) {
    if items.is_empty() {
      return;
    }
    let fallback = self
      .surface
      .pane_content
      .get(&pane)
      .copied()
      .or_else(|| items.first().copied())
      .unwrap_or(PaneContent::EditorBuffer {
        buffer_id: self.active_buffer,
      });
    self.pane_items_state_mut(pane, fallback).merge_in(items);
  }

  fn restore_editor_buffer_in_pane(&mut self, pane: PaneId) {
    let fallback = self
      .surface
      .pane_items
      .get(&pane)
      .and_then(PaneItemsState::preferred_buffer_id)
      .unwrap_or(self.active_buffer);
    let content = PaneContent::EditorBuffer {
      buffer_id: fallback,
    };
    self.surface.pane_content.insert(pane, content);
    self.activate_pane_item(pane, content);
    let fallback_view = self
      .buffer_view(fallback)
      .expect("active buffer must provide fallback view");
    self.surface.pane_views.entry(pane).or_insert(fallback_view);
    if pane == self.active_pane_id() {
      self.active_buffer = fallback;
    }
  }

  fn alloc_client_surface_id(&mut self) -> ClientSurfaceId {
    let id = ClientSurfaceId::new(self.surface.next_client_surface_id);
    let next = self.surface.next_client_surface_id.get().saturating_add(1);
    self.surface.next_client_surface_id =
      NonZeroUsize::new(next).unwrap_or(self.surface.next_client_surface_id);
    id
  }

  fn detach_client_surface_in_pane(&mut self, pane: PaneId) -> Option<ClientSurfaceId> {
    let Some(PaneContent::ClientSurface { surface_id }) =
      self.surface.pane_content.get(&pane).copied()
    else {
      return None;
    };
    if let Some(surface) = self.surface.client_surfaces.get_mut(&surface_id) {
      surface.attached_pane = None;
    }
    Some(surface_id)
  }

  fn remove_client_surface(&mut self, surface_id: ClientSurfaceId) -> bool {
    let Some(surface) = self.surface.client_surfaces.remove(&surface_id) else {
      return false;
    };

    self.remove_pane_item_from_all(PaneContent::ClientSurface { surface_id });

    if let Some(pane) = surface.attached_pane
      && matches!(
        self.surface.pane_content.get(&pane),
        Some(PaneContent::ClientSurface { surface_id: attached }) if *attached == surface_id
      )
    {
      self.restore_editor_buffer_in_pane(pane);
    }
    true
  }

  fn attach_client_surface_to_pane(&mut self, pane: PaneId, surface_id: ClientSurfaceId) -> bool {
    let Some(current) = self.surface.client_surfaces.get(&surface_id).copied() else {
      return false;
    };

    self.sync_pane_view_to_buffer(pane);
    let _ = self.detach_client_surface_in_pane(pane);

    if let Some(attached_pane) = current.attached_pane
      && attached_pane != pane
      && matches!(
        self.surface.pane_content.get(&attached_pane),
        Some(PaneContent::ClientSurface { surface_id: attached }) if *attached == surface_id
      )
    {
      let _ = self.remove_pane_item(attached_pane, PaneContent::ClientSurface { surface_id });
      self.restore_editor_buffer_in_pane(attached_pane);
    }

    let content = PaneContent::ClientSurface { surface_id };
    self.surface.pane_content.insert(pane, content);
    self.activate_pane_item(pane, content);
    if let Some(surface) = self.surface.client_surfaces.get_mut(&surface_id) {
      surface.attached_pane = Some(pane);
    }
    true
  }

  fn first_editor_pane(&self) -> Option<PaneId> {
    self
      .surface
      .split_tree
      .pane_order()
      .into_iter()
      .find(|pane| {
        matches!(
          self.surface.pane_content.get(pane),
          Some(PaneContent::EditorBuffer { .. })
        )
      })
  }

  fn activate_buffer(&mut self, buffer_id: BufferId) -> bool {
    let Some(index) = self.index_of_buffer_id(buffer_id) else {
      return false;
    };

    let target_pane = match self.active_pane_content() {
      Some(PaneContent::EditorBuffer { .. }) | None => self.active_pane_id(),
      Some(PaneContent::ClientSurface { .. }) => {
        if let Some(existing_editor_pane) = self.first_editor_pane() {
          let _ = self
            .surface
            .split_tree
            .set_active_pane(existing_editor_pane);
          existing_editor_pane
        } else {
          self.surface.split_tree.split_active(SplitAxis::Vertical)
        }
      },
    };

    self.sync_pane_view_to_buffer(target_pane);

    if buffer_id != self.active_buffer {
      self.access_history.push(self.active_buffer);
      self.active_buffer = buffer_id;
    }
    let content = PaneContent::EditorBuffer { buffer_id };
    self.surface.pane_content.insert(target_pane, content);
    self.activate_pane_item(target_pane, content);
    self
      .surface
      .pane_views
      .insert(target_pane, self.buffers[index].view.clone());
    true
  }

  fn touch_modified_history(&mut self, buffer_id: BufferId) {
    if let Some(pos) = self
      .modified_history
      .iter()
      .position(|entry| *entry == buffer_id)
    {
      self.modified_history.remove(pos);
    }
    self.modified_history.push(buffer_id);
  }

  pub fn id(&self) -> EditorId {
    self.id
  }

  pub fn document(&self) -> &Document {
    &self.buffers[self.active_buffer_index_internal()].document
  }

  pub fn document_for_buffer(&self, buffer_id: BufferId) -> Option<&Document> {
    let index = self.index_of_buffer_id(buffer_id)?;
    Some(&self.buffers[index].document)
  }

  pub fn document_mut(&mut self) -> &mut Document {
    let index = self.active_buffer_index_internal();
    &mut self.buffers[index].document
  }

  pub fn document_mut_for_buffer(&mut self, buffer_id: BufferId) -> Option<&mut Document> {
    let index = self.index_of_buffer_id(buffer_id)?;
    Some(&mut self.buffers[index].document)
  }

  pub fn view(&self) -> ViewState {
    if matches!(
      self.active_pane_content(),
      Some(PaneContent::EditorBuffer { .. })
    ) {
      if let Some(view) = self.pane_view(self.active_pane_id()) {
        return view;
      }
    }
    self.buffers[self.active_buffer_index_internal()]
      .view
      .clone()
  }

  pub fn view_mut(&mut self) -> &mut ViewState {
    if matches!(
      self.active_pane_content(),
      Some(PaneContent::EditorBuffer { .. })
    ) {
      return self
        .pane_view_mut(self.active_pane_id())
        .expect("active editor pane must have a view");
    }
    let index = self.active_buffer_index_internal();
    &mut self.buffers[index].view
  }

  pub fn render_cache(&self) -> &RenderCache {
    &self.buffers[self.active_buffer_index_internal()].render_cache
  }

  pub fn render_cache_mut(&mut self) -> &mut RenderCache {
    let index = self.active_buffer_index_internal();
    &mut self.buffers[index].render_cache
  }

  pub fn document_and_cache(&mut self) -> (&Document, &mut RenderCache) {
    let index = self.active_buffer_index_internal();
    let buffer = &mut self.buffers[index];
    (&buffer.document, &mut buffer.render_cache)
  }

  pub fn buffer_count(&self) -> usize {
    self.buffers.len()
  }

  pub fn active_buffer_id(&self) -> BufferId {
    self.active_buffer
  }

  pub fn buffer_snapshot(&self, buffer_id: BufferId) -> Option<BufferSnapshot> {
    let index = self.index_of_buffer_id(buffer_id)?;
    self.buffer_snapshot_for_index(index)
  }

  pub fn buffer_snapshots(&self) -> Vec<BufferSnapshot> {
    self
      .buffers
      .iter()
      .enumerate()
      .filter_map(|(index, _)| self.buffer_snapshot_for_index(index))
      .collect()
  }

  pub fn buffer_snapshots_mru(&self) -> Vec<BufferSnapshot> {
    let len = self.buffers.len();
    if len == 0 {
      return Vec::new();
    }

    let mut order = Vec::with_capacity(len);
    order.push(self.active_buffer);

    for buffer_id in self.access_history.iter().rev().copied() {
      if !order.contains(&buffer_id) && self.index_of_buffer_id(buffer_id).is_some() {
        order.push(buffer_id);
      }
    }

    for buffer in &self.buffers {
      if !order.contains(&buffer.id) {
        order.push(buffer.id);
      }
    }

    order
      .into_iter()
      .filter_map(|buffer_id| self.buffer_snapshot(buffer_id))
      .collect()
  }

  pub fn pane_item_snapshots(&self) -> Vec<PaneItemGroupSnapshot> {
    let active_pane = self.active_pane_id();
    self
      .surface
      .split_tree
      .pane_order()
      .into_iter()
      .filter_map(|pane_id| {
        let items = if let Some(state) = self.surface.pane_items.get(&pane_id) {
          state
            .items
            .iter()
            .enumerate()
            .map(|(index, content)| {
              PaneItemSnapshot {
                content:   *content,
                is_active: index == state.active,
              }
            })
            .collect::<Vec<_>>()
        } else {
          let content = self.surface.pane_content.get(&pane_id).copied()?;
          vec![PaneItemSnapshot {
            content,
            is_active: true,
          }]
        };
        Some(PaneItemGroupSnapshot {
          pane_id,
          is_active_pane: pane_id == active_pane,
          items,
        })
      })
      .collect()
  }

  pub fn pane_count(&self) -> usize {
    self.surface.split_tree.pane_count()
  }

  pub fn layout_viewport(&self) -> Rect {
    self.surface.layout_viewport
  }

  pub fn set_layout_viewport(&mut self, viewport: Rect) {
    self.surface.layout_viewport = viewport;
  }

  pub fn active_pane_id(&self) -> PaneId {
    self.surface.split_tree.active_pane()
  }

  pub fn set_active_pane(&mut self, pane: PaneId) -> bool {
    let previous_active = self.active_pane_id();
    if !self.surface.split_tree.set_active_pane(pane) {
      return false;
    }
    self.sync_pane_view_to_buffer(previous_active);
    let Some(content) = self.surface.pane_content.get(&pane).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_id } = content {
      self.active_buffer = buffer_id;
      let fallback = self
        .buffer_view(buffer_id)
        .expect("active editor pane must resolve view");
      self.surface.pane_views.entry(pane).or_insert(fallback);
    }
    true
  }

  pub fn pane_snapshots(&self, area: Rect) -> Vec<PaneSnapshot> {
    self
      .frame_pane_snapshots(area)
      .into_iter()
      .filter_map(|pane| {
        match pane.content {
          PaneContent::EditorBuffer { buffer_id } => {
            Some(PaneSnapshot {
              pane_id: pane.pane_id,
              buffer_id,
              rect: pane.rect,
              is_active_pane: pane.is_active_pane,
            })
          },
          PaneContent::ClientSurface { .. } => None,
        }
      })
      .collect()
  }

  pub fn editor_surface_snapshots(&self) -> Vec<EditorSurfaceSnapshot> {
    self
      .pane_snapshots(self.layout_viewport())
      .into_iter()
      .filter_map(|pane| {
        let buffer = self.buffer_snapshot(pane.buffer_id)?;
        Some(EditorSurfaceSnapshot {
          pane_id:      pane.pane_id,
          buffer_id:    buffer.buffer_id,
          file_path:    buffer.file_path,
          display_name: buffer.display_name,
          modified:     buffer.modified,
          is_active:    pane.is_active_pane,
        })
      })
      .collect()
  }

  pub fn frame_pane_snapshots(&self, area: Rect) -> Vec<FramePaneSnapshot> {
    let active = self.surface.split_tree.active_pane();
    self
      .surface
      .split_tree
      .layout(area)
      .into_iter()
      .filter_map(|(pane_id, rect)| {
        let Some(content) = self.surface.pane_content.get(&pane_id).copied() else {
          return None;
        };
        Some(FramePaneSnapshot {
          pane_id,
          content,
          rect,
          is_active_pane: pane_id == active,
        })
      })
      .collect()
  }

  pub fn pane_separators(&self, area: Rect) -> Vec<SplitSeparator> {
    self.surface.split_tree.separators(area)
  }

  pub fn resize_split(&mut self, split_id: SplitNodeId, x: u16, y: u16) -> bool {
    self
      .surface
      .split_tree
      .resize_split(split_id, self.surface.layout_viewport, x, y)
  }

  pub fn pane_buffer_id(&self, pane: PaneId) -> Option<BufferId> {
    match self.surface.pane_content.get(&pane).copied() {
      Some(PaneContent::EditorBuffer { buffer_id }) => Some(buffer_id),
      _ => None,
    }
  }

  pub fn pane_client_surface_id(&self, pane: PaneId) -> Option<ClientSurfaceId> {
    match self.surface.pane_content.get(&pane).copied() {
      Some(PaneContent::ClientSurface { surface_id }) => Some(surface_id),
      _ => None,
    }
  }

  pub fn pane_in_direction(&self, pane: PaneId, direction: PaneDirection) -> Option<PaneId> {
    self.surface.split_tree.pane_in_direction(pane, direction)
  }

  pub fn pane_neighbors(&self, pane: PaneId) -> Option<PaneNeighbors> {
    self.surface.split_tree.pane_neighbors(pane)
  }

  pub fn pane_content(&self, pane: PaneId) -> Option<PaneContent> {
    self.surface.pane_content.get(&pane).copied()
  }

  pub fn pane_rect(&self, pane: PaneId) -> Option<Rect> {
    self
      .surface
      .split_tree
      .layout(self.surface.layout_viewport)
      .into_iter()
      .find_map(|(candidate, rect)| (candidate == pane).then_some(rect))
  }

  pub fn pane_view(&self, pane: PaneId) -> Option<ViewState> {
    match self.surface.pane_content.get(&pane).copied() {
      Some(PaneContent::EditorBuffer { buffer_id }) => {
        let mut view = self
          .surface
          .pane_views
          .get(&pane)
          .cloned()
          .or_else(|| self.buffer_view(buffer_id))?;
        if let Some(rect) = self.pane_rect(pane) {
          view.viewport = rect;
        }
        Some(view)
      },
      _ => None,
    }
  }

  pub fn pane_view_mut(&mut self, pane: PaneId) -> Option<&mut ViewState> {
    let buffer_id = match self.surface.pane_content.get(&pane).copied() {
      Some(PaneContent::EditorBuffer { buffer_id }) => buffer_id,
      _ => return None,
    };
    let fallback = self.buffer_view(buffer_id)?;
    let pane_rect = self.pane_rect(pane);
    let view = self.surface.pane_views.entry(pane).or_insert(fallback);
    if let Some(rect) = pane_rect {
      view.viewport = rect;
    }
    Some(view)
  }

  pub fn pane_content_kind(&self, pane: PaneId) -> Option<PaneContentKind> {
    self.pane_content(pane).map(PaneContent::kind)
  }

  pub fn active_pane_content(&self) -> Option<PaneContent> {
    self.pane_content(self.active_pane_id())
  }

  pub fn active_pane_content_kind(&self) -> Option<PaneContentKind> {
    self.active_pane_content().map(PaneContent::kind)
  }

  pub fn active_client_surface_id(&self) -> Option<ClientSurfaceId> {
    match self.active_pane_content() {
      Some(PaneContent::ClientSurface { surface_id }) => Some(surface_id),
      _ => None,
    }
  }

  pub fn client_surface_snapshots(&self) -> Vec<ClientSurfaceSnapshot> {
    let active_pane = self.active_pane_id();
    self
      .surface
      .client_surfaces
      .values()
      .copied()
      .map(|surface| {
        ClientSurfaceSnapshot {
          client_surface_id: surface.surface_id,
          attached_pane:     surface.attached_pane,
          is_active:         surface.attached_pane == Some(active_pane),
        }
      })
      .collect()
  }

  pub fn is_active_pane_client_surface(&self) -> bool {
    matches!(
      self.active_pane_content(),
      Some(PaneContent::ClientSurface { .. })
    )
  }

  pub fn create_client_surface(&mut self) -> ClientSurfaceId {
    let surface_id = self.alloc_client_surface_id();
    self
      .surface
      .client_surfaces
      .insert(surface_id, ClientSurfaceAttachment {
        surface_id,
        attached_pane: None,
      });
    surface_id
  }

  pub fn open_client_surface_in_active_pane(&mut self, surface_id: ClientSurfaceId) -> bool {
    self.attach_client_surface_to_pane(self.active_pane_id(), surface_id)
  }

  pub fn replace_active_pane_with_client_surface(&mut self, surface_id: ClientSurfaceId) -> bool {
    self.open_client_surface_in_active_pane(surface_id)
  }

  pub fn open_client_surface(&mut self, target: OpenTarget, surface_id: ClientSurfaceId) -> bool {
    let Some(resolved) = self.resolve_open_target(target) else {
      return false;
    };
    if !self.attach_client_surface_to_pane(resolved.pane, surface_id) {
      return false;
    }
    if let Some(previous) = resolved.restore_focus_to {
      let _ = self.set_active_pane(previous);
    }
    true
  }

  pub fn close_active_client_surface(&mut self) -> bool {
    let pane = self.active_pane_id();
    let Some(PaneContent::ClientSurface { surface_id }) =
      self.surface.pane_content.get(&pane).copied()
    else {
      return false;
    };
    self.remove_client_surface(surface_id)
  }

  pub fn hide_active_client_surface(&mut self) -> bool {
    let pane = self.active_pane_id();
    let Some(_surface_id) = self.detach_client_surface_in_pane(pane) else {
      return false;
    };
    self.restore_editor_buffer_in_pane(pane);
    true
  }

  pub fn set_active_buffer_in_pane(&mut self, pane: PaneId, buffer_id: BufferId) -> bool {
    let Some(index) = self.index_of_buffer_id(buffer_id) else {
      return false;
    };
    if !self.surface.split_tree.contains_pane(pane) {
      return false;
    }
    let was_active = pane == self.active_pane_id();
    self.sync_pane_view_to_buffer(pane);
    let _ = self.detach_client_surface_in_pane(pane);
    let content = PaneContent::EditorBuffer { buffer_id };
    self.surface.pane_content.insert(pane, content);
    self.activate_pane_item(pane, content);
    self
      .surface
      .pane_views
      .insert(pane, self.buffers[index].view.clone());
    if was_active {
      self.active_buffer = buffer_id;
    }
    true
  }

  pub fn buffer_view(&self, buffer_id: BufferId) -> Option<ViewState> {
    if self.active_buffer == buffer_id {
      return Some(self.view());
    }

    if let Some((pane, _)) = self
      .surface
      .pane_content
      .iter()
      .find(|(_, content)| matches!(content, PaneContent::EditorBuffer { buffer_id: pane_buffer_id } if *pane_buffer_id == buffer_id))
    {
      return self.pane_view(*pane);
    }

    self
      .index_of_buffer_id(buffer_id)
      .and_then(|index| self.buffers.get(index))
      .map(|buffer| buffer.view.clone())
  }

  pub fn buffer_document(&self, buffer_id: BufferId) -> Option<&Document> {
    let index = self.index_of_buffer_id(buffer_id)?;
    self.buffers.get(index).map(|buffer| &buffer.document)
  }

  pub fn buffer_file_path(&self, buffer_id: BufferId) -> Option<&Path> {
    let index = self.index_of_buffer_id(buffer_id)?;
    self.buffers.get(index)?.file_path.as_deref()
  }

  pub fn buffer_document_mut(&mut self, buffer_id: BufferId) -> Option<&mut Document> {
    let index = self.index_of_buffer_id(buffer_id)?;
    self
      .buffers
      .get_mut(index)
      .map(|buffer| &mut buffer.document)
  }

  pub fn find_buffer_by_id(&self, buffer_id: u64) -> Option<BufferId> {
    let buffer_id = BufferId::from_u64(buffer_id)?;
    self.index_of_buffer_id(buffer_id).map(|_| buffer_id)
  }

  pub fn set_buffer_viewport(&mut self, buffer_id: BufferId, viewport: Rect) -> bool {
    let mut changed = false;
    for (pane, content) in self.surface.pane_content.clone() {
      if let PaneContent::EditorBuffer {
        buffer_id: pane_buffer_id,
      } = content
        && pane_buffer_id == buffer_id
      {
        if let Some(view) = self.pane_view_mut(pane) {
          view.viewport = viewport;
          changed = true;
        }
      }
    }
    if changed {
      return true;
    }
    let Some(index) = self.index_of_buffer_id(buffer_id) else {
      return false;
    };
    let Some(buffer) = self.buffers.get_mut(index) else {
      return false;
    };
    buffer.view.viewport = viewport;
    true
  }

  pub fn set_pane_viewport(&mut self, pane: PaneId, viewport: Rect) -> bool {
    let Some(view) = self.pane_view_mut(pane) else {
      return false;
    };
    view.viewport = viewport;
    true
  }

  pub fn document_and_cache_at_mut(
    &mut self,
    buffer_id: BufferId,
  ) -> Option<(&Document, &mut RenderCache)> {
    let index = self.index_of_buffer_id(buffer_id)?;
    let buffer = self.buffers.get_mut(index)?;
    Some((&buffer.document, &mut buffer.render_cache))
  }

  pub fn split_active_pane(&mut self, axis: SplitAxis) -> bool {
    let source_pane = self.active_pane_id();
    let current_content = self
      .active_pane_content()
      .unwrap_or(PaneContent::EditorBuffer {
        buffer_id: self.active_buffer,
      });
    let current_view = self
      .surface
      .pane_views
      .get(&source_pane)
      .cloned()
      .unwrap_or_else(|| self.view());
    let pane = self.surface.split_tree.split_active(axis);
    let content = match current_content {
      PaneContent::EditorBuffer { buffer_id } => PaneContent::EditorBuffer { buffer_id },
      PaneContent::ClientSurface { .. } => {
        PaneContent::EditorBuffer {
          buffer_id: self.active_buffer,
        }
      },
    };
    self.surface.pane_content.insert(pane, content);
    self.surface.pane_items.insert(pane, PaneItemsState::new(content));
    self.surface.pane_views.insert(pane, current_view);
    if let PaneContent::EditorBuffer { buffer_id } = content {
      self.active_buffer = buffer_id;
    }
    true
  }

  pub fn close_active_pane(&mut self) -> bool {
    let closing = self.surface.split_tree.active_pane();
    self.sync_pane_view_to_buffer(closing);
    let closing_items = self
      .surface
      .pane_items
      .get(&closing)
      .map(|state| state.items.clone())
      .unwrap_or_default();
    let Ok(next_active) = self.surface.split_tree.close_active() else {
      return false;
    };

    let _ = self.detach_client_surface_in_pane(closing);
    self.surface.pane_content.remove(&closing);
    self.surface.pane_items.remove(&closing);
    self.surface.pane_views.remove(&closing);
    self.merge_pane_items(next_active, closing_items);
    let Some(next_content) = self.surface.pane_content.get(&next_active).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_id } = next_content {
      self.active_buffer = buffer_id;
      let fallback = self
        .buffer_view(self.active_buffer)
        .expect("active pane buffer must exist");
      self
        .surface
        .pane_views
        .entry(next_active)
        .or_insert(fallback);
    }
    true
  }

  pub fn only_active_pane(&mut self) -> bool {
    if self.surface.split_tree.pane_count() <= 1 {
      return false;
    }
    let active = self.surface.split_tree.active_pane();
    let Some(active_content) = self.surface.pane_content.get(&active).copied() else {
      return false;
    };

    let panes_to_sync: Vec<PaneId> = self.surface.pane_content.keys().copied().collect();
    for pane in panes_to_sync {
      self.sync_pane_view_to_buffer(pane);
    }
    let removed_panes: Vec<PaneId> = self
      .surface
      .pane_content
      .keys()
      .copied()
      .filter(|pane| *pane != active)
      .collect();
    let removed_items = removed_panes
      .iter()
      .flat_map(|pane| {
        self
          .surface
          .pane_items
          .get(pane)
          .map(|state| state.items.clone())
          .unwrap_or_default()
      })
      .collect::<Vec<_>>();
    for pane in &removed_panes {
      let _ = self.detach_client_surface_in_pane(*pane);
    }

    self.surface.split_tree.only_active();
    self.surface.pane_content.retain(|pane, _| *pane == active);
    self.surface.pane_items.retain(|pane, _| *pane == active);
    self.merge_pane_items(active, removed_items);
    self.surface.pane_views.retain(|pane, _| *pane == active);
    if let PaneContent::EditorBuffer { buffer_id } = active_content {
      self.active_buffer = buffer_id;
      let fallback = self
        .buffer_view(self.active_buffer)
        .expect("active pane buffer must exist");
      self.surface.pane_views.entry(active).or_insert(fallback);
    }
    true
  }

  pub fn rotate_active_pane(&mut self, next: bool) -> bool {
    let previous_active = self.active_pane_id();
    if !self.surface.split_tree.rotate_focus(next) {
      return false;
    }
    self.sync_pane_view_to_buffer(previous_active);
    let active = self.surface.split_tree.active_pane();
    let Some(content) = self.surface.pane_content.get(&active).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_id } = content {
      self.active_buffer = buffer_id;
      let fallback = self
        .buffer_view(self.active_buffer)
        .expect("active pane buffer must exist");
      self.surface.pane_views.entry(active).or_insert(fallback);
    }
    true
  }

  pub fn jump_active_pane(&mut self, direction: PaneDirection) -> bool {
    let previous_active = self.active_pane_id();
    if !self.surface.split_tree.jump_active(direction) {
      return false;
    }
    self.sync_pane_view_to_buffer(previous_active);
    let active = self.surface.split_tree.active_pane();
    let Some(content) = self.surface.pane_content.get(&active).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_id } = content {
      self.active_buffer = buffer_id;
      let fallback = self
        .buffer_view(self.active_buffer)
        .expect("active pane buffer must exist");
      self.surface.pane_views.entry(active).or_insert(fallback);
    }
    true
  }

  pub fn move_pane(&mut self, pane: PaneId, target: PaneId, direction: PaneDirection) -> bool {
    let previous_active = self.active_pane_id();
    if !self.surface.split_tree.move_pane(pane, target, direction) {
      return false;
    }
    self.sync_pane_view_to_buffer(previous_active);
    let active = self.surface.split_tree.active_pane();
    let Some(content) = self.surface.pane_content.get(&active).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_id } = content {
      self.active_buffer = buffer_id;
      let fallback = self
        .buffer_view(self.active_buffer)
        .expect("active pane buffer must exist");
      self.surface.pane_views.entry(active).or_insert(fallback);
    }
    true
  }

  pub fn swap_active_pane(&mut self, direction: PaneDirection) -> bool {
    self.surface.split_tree.swap_active(direction)
  }

  pub fn transpose_active_pane_branch(&mut self) -> bool {
    self.surface.split_tree.transpose_active_branch()
  }

  pub fn focus_client_surface(&mut self, surface_id: ClientSurfaceId) -> bool {
    let Some(surface) = self.surface.client_surfaces.get(&surface_id).copied() else {
      return false;
    };

    if let Some(pane) = surface.attached_pane {
      return self.set_active_pane(pane);
    }

    let pane = self.active_pane_id();
    self.attach_client_surface_to_pane(pane, surface_id)
  }

  pub fn active_terminal_id(&self) -> Option<ClientSurfaceId> {
    self.active_client_surface_id()
  }

  pub fn terminal_surface_snapshots(&self) -> Vec<ClientSurfaceSnapshot> {
    self.client_surface_snapshots()
  }

  pub fn is_active_pane_terminal(&self) -> bool {
    self.is_active_pane_client_surface()
  }

  pub fn open_terminal_in_active_pane(&mut self) -> ClientSurfaceId {
    let surface_id = self.create_client_surface();
    let _ = self.open_client_surface_in_active_pane(surface_id);
    surface_id
  }

  pub fn replace_active_pane_with_terminal(&mut self) -> ClientSurfaceId {
    self.open_terminal_in_active_pane()
  }

  pub fn close_terminal_in_active_pane(&mut self) -> bool {
    self.close_active_client_surface()
  }

  pub fn hide_active_terminal_surface(&mut self) -> bool {
    self.hide_active_client_surface()
  }

  pub fn focus_terminal_surface(&mut self, surface_id: ClientSurfaceId) -> bool {
    self.focus_client_surface(surface_id)
  }

  pub fn resolve_open_target(&mut self, target: OpenTarget) -> Option<ResolvedOpenTarget> {
    let previous_active = self.active_pane_id();

    match target {
      OpenTarget::Active => {
        Some(ResolvedOpenTarget {
          pane:             previous_active,
          restore_focus_to: None,
        })
      },
      OpenTarget::Pane(pane) => {
        if !self.set_active_pane(pane) {
          return None;
        }
        Some(ResolvedOpenTarget {
          pane,
          restore_focus_to: None,
        })
      },
      OpenTarget::Neighbor {
        direction,
        create_if_missing,
      } => {
        if let Some(pane) = self.pane_in_direction(previous_active, direction) {
          let _ = self.set_active_pane(pane);
          return Some(ResolvedOpenTarget {
            pane,
            restore_focus_to: None,
          });
        }
        if !create_if_missing {
          return None;
        }

        let axis = direction.split_axis();
        if !self.split_active_pane(axis) {
          return None;
        }

        let pane = self.active_pane_id();
        if direction.places_before() {
          let _ = self
            .surface
            .split_tree
            .move_pane(pane, previous_active, direction);
        }

        Some(ResolvedOpenTarget {
          pane,
          restore_focus_to: None,
        })
      },
      OpenTarget::Split { axis, focus_new } => {
        if !self.split_active_pane(axis) {
          return None;
        }
        let pane = self.active_pane_id();
        Some(ResolvedOpenTarget {
          pane,
          restore_focus_to: (!focus_new).then_some(previous_active),
        })
      },
    }
  }

  pub fn set_active_buffer_preserving_terminal(&mut self, buffer_id: BufferId) -> bool {
    self.activate_buffer(buffer_id)
  }

  pub fn set_active_buffer(&mut self, buffer_id: BufferId) -> bool {
    self.set_active_buffer_preserving_terminal(buffer_id)
  }

  pub fn switch_buffer_forward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    let current = self.active_buffer_index_internal();
    let next = (current + step) % len;
    let Some(buffer_id) = self.buffer_id_at_index(next) else {
      return false;
    };
    self.set_active_buffer(buffer_id)
  }

  pub fn switch_buffer_backward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    let current = self.active_buffer_index_internal();
    let next = (current + len - step) % len;
    let Some(buffer_id) = self.buffer_id_at_index(next) else {
      return false;
    };
    self.set_active_buffer(buffer_id)
  }

  pub fn active_file_path(&self) -> Option<&Path> {
    self.buffers[self.active_buffer_index_internal()]
      .file_path
      .as_deref()
  }

  pub fn open_buffer_policy(&self) -> OpenBufferPolicy {
    self.policy.open_buffer
  }

  pub fn should_reuse_active_untitled_buffer_for_open(&self) -> bool {
    if !self.policy.open_buffer.reuse_active_untitled_buffer {
      return false;
    }

    let Some(active) = self.buffers.get(self.active_buffer_index_internal()) else {
      return false;
    };
    if active.file_path.is_some() || active.document.flags().modified {
      return false;
    }
    let active_index = self.active_buffer;
    let pane_refs = self
      .surface
      .pane_content
      .values()
      .filter(|&&content| {
        matches!(content, PaneContent::EditorBuffer { buffer_id } if buffer_id == active_index)
      })
      .count();
    pane_refs <= 1
  }

  pub fn replace_active_buffer(&mut self, text: Rope, file_path: Option<PathBuf>) -> bool {
    let current_view = self.view();
    let active_index = self.active_buffer_index_internal();
    if self.buffers.get(active_index).is_none() {
      return false;
    }
    let document_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);
    let document = Document::new(document_id, text);
    let buffer_id = self.active_buffer;
    self.buffers[active_index] = BufferState::new(buffer_id, document, current_view, file_path);
    true
  }

  pub fn set_active_file_path(&mut self, path: Option<PathBuf>) {
    let index = self.active_buffer_index_internal();
    self.buffers[index].file_path = path;
  }

  pub fn find_buffer_by_path(&self, path: &Path) -> Option<BufferId> {
    self
      .buffers
      .iter()
      .find(|buffer| buffer.file_path.as_deref() == Some(path))
      .map(|buffer| buffer.id)
  }

  pub fn rename_file_path(&mut self, from: &Path, to: PathBuf) -> bool {
    let mut changed = false;
    let display_name = to
      .file_name()
      .map(|name| name.to_string_lossy().to_string())
      .unwrap_or_else(|| to.display().to_string());

    for buffer in &mut self.buffers {
      if buffer.file_path.as_deref() != Some(from) {
        continue;
      }
      buffer.file_path = Some(to.clone());
      buffer.document.set_display_name(display_name.clone());
      changed = true;
    }

    changed
  }

  pub fn open_buffer(
    &mut self,
    text: Rope,
    view: ViewState,
    file_path: Option<PathBuf>,
  ) -> BufferId {
    let document_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);
    let document = Document::new(document_id, text);
    let buffer_id = self.alloc_buffer_id();

    self
      .buffers
      .push(BufferState::new(buffer_id, document, view, file_path));
    let _ = self.activate_buffer(buffer_id);
    buffer_id
  }

  fn sync_pane_view_to_buffer(&mut self, pane: PaneId) {
    let Some(PaneContent::EditorBuffer { buffer_id }) =
      self.surface.pane_content.get(&pane).copied()
    else {
      return;
    };
    let Some(view) = self.surface.pane_views.get(&pane).cloned() else {
      return;
    };
    if let Some(index) = self.index_of_buffer_id(buffer_id)
      && let Some(buffer) = self.buffers.get_mut(index)
    {
      buffer.view = view;
    }
  }

  pub fn open_buffer_without_activation(
    &mut self,
    text: Rope,
    view: ViewState,
    file_path: Option<PathBuf>,
  ) -> BufferId {
    let document_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);
    let document = Document::new(document_id, text);
    let buffer_id = self.alloc_buffer_id();
    self
      .buffers
      .push(BufferState::new(buffer_id, document, view, file_path));
    buffer_id
  }

  pub fn close_buffer(&mut self, buffer_id: BufferId) -> bool {
    let len = self.buffers.len();
    let Some(index) = self.index_of_buffer_id(buffer_id) else {
      return false;
    };
    if len <= 1 {
      return false;
    }

    let replacement_before_index = if index > 0 { index - 1 } else { 1 };
    let Some(replacement_before) = self.buffer_id_at_index(replacement_before_index) else {
      return false;
    };
    let closing_content = PaneContent::EditorBuffer { buffer_id };
    let panes_showing_buffer = self
      .surface
      .pane_content
      .iter()
      .filter_map(|(pane, content)| {
        matches!(content, PaneContent::EditorBuffer { buffer_id: pane_buffer_id } if *pane_buffer_id == buffer_id)
          .then_some(*pane)
      })
      .collect::<Vec<_>>();

    for state in self.surface.pane_items.values_mut() {
      if state.remove(closing_content) && state.items.is_empty() {
        state.activate(PaneContent::EditorBuffer {
          buffer_id: replacement_before,
        });
      }
    }

    for pane in panes_showing_buffer {
      let next_content = self
        .surface
        .pane_items
        .get(&pane)
        .and_then(PaneItemsState::active_content)
        .unwrap_or(PaneContent::EditorBuffer {
          buffer_id: replacement_before,
        });
      match next_content {
        PaneContent::EditorBuffer { buffer_id: next_buffer_id } => {
          self.surface.pane_content.insert(pane, next_content);
          if let Some(next_index) = self.index_of_buffer_id(next_buffer_id) {
            self
              .surface
              .pane_views
              .insert(pane, self.buffers[next_index].view.clone());
          }
        },
        PaneContent::ClientSurface { surface_id } => {
          self.surface.pane_content.insert(pane, next_content);
          if let Some(surface) = self.surface.client_surfaces.get_mut(&surface_id) {
            surface.attached_pane = Some(pane);
          }
        },
      }
    }

    self.buffers.remove(index);

    if self.active_buffer == buffer_id {
      self.active_buffer = replacement_before;
    }

    if let Some(PaneContent::EditorBuffer { buffer_id }) = self
      .surface
      .pane_content
      .get(&self.surface.split_tree.active_pane())
    {
      self.active_buffer = *buffer_id;
    }

    self.access_history.retain(|entry| *entry != buffer_id);
    self.modified_history.retain(|entry| *entry != buffer_id);
    self
      .jumplist_backward
      .retain(|entry| entry.buffer_id != buffer_id);
    self
      .jumplist_forward
      .retain(|entry| entry.buffer_id != buffer_id);

    true
  }

  pub fn move_buffer(&mut self, from: BufferId, to: BufferId) -> bool {
    let len = self.buffers.len();
    let Some(from_index) = self.index_of_buffer_id(from) else {
      return false;
    };
    let Some(to_index) = self.index_of_buffer_id(to) else {
      return false;
    };
    if len <= 1 || from_index == to_index {
      return false;
    }

    let buffer = self.buffers.remove(from_index);
    self.buffers.insert(to_index, buffer);

    true
  }

  pub fn goto_last_accessed_buffer(&mut self) -> bool {
    while let Some(buffer_id) = self.access_history.pop() {
      if self.index_of_buffer_id(buffer_id).is_some() && buffer_id != self.active_buffer {
        return self.set_active_buffer(buffer_id);
      }
    }
    false
  }

  pub fn mark_active_buffer_modified(&mut self) {
    self.touch_modified_history(self.active_buffer);
  }

  pub fn push_object_selection(&mut self, selection: Selection) {
    let index = self.active_buffer_index_internal();
    self.buffers[index].object_selection_history.push(selection);
  }

  pub fn pop_object_selection(&mut self) -> Option<Selection> {
    let index = self.active_buffer_index_internal();
    self.buffers[index].object_selection_history.pop()
  }

  pub fn clear_object_selections(&mut self) {
    let index = self.active_buffer_index_internal();
    self.buffers[index].object_selection_history.clear();
  }

  pub fn goto_last_modified_buffer(&mut self) -> bool {
    let current = self.active_buffer;
    let Some(index) = self
      .modified_history
      .iter()
      .rev()
      .copied()
      .find(|buffer_id| self.index_of_buffer_id(*buffer_id).is_some() && *buffer_id != current)
    else {
      return false;
    };
    self.set_active_buffer(index)
  }

  pub fn last_modification_position(&self) -> Option<usize> {
    self.document().history().last_edit_pos()
  }

  pub fn apply_transaction_to_active_buffer(
    &mut self,
    transaction: &Transaction,
    syntax_loader: Option<&Loader>,
  ) -> Result<ApplyTransactionResult, DocumentError> {
    let buffer_id = self.active_buffer;
    let changed = !transaction.changes().is_empty();
    self
      .document_mut()
      .apply_transaction_with_syntax(transaction, syntax_loader)?;
    if changed {
      self.mark_active_buffer_modified();
    }
    Ok(ApplyTransactionResult {
      buffer_id,
      changed,
      syntax_attached: self.document().syntax().is_some(),
    })
  }

  pub fn apply_transaction_to_buffer(
    &mut self,
    buffer_id: BufferId,
    transaction: &Transaction,
    syntax_loader: Option<&Loader>,
  ) -> std::result::Result<ApplyTransactionResult, String> {
    let Some(index) = self.index_of_buffer_id(buffer_id) else {
      return Err("buffer not found for transaction".to_string());
    };
    let changed = !transaction.changes().is_empty();
    self.buffers[index]
      .document
      .apply_transaction_with_syntax(transaction, syntax_loader)
      .map_err(|err| err.to_string())?;
    if changed {
      self.touch_modified_history(buffer_id);
    }
    Ok(ApplyTransactionResult {
      buffer_id,
      changed,
      syntax_attached: self.buffers[index].document.syntax().is_some(),
    })
  }

  pub fn jumplist_backward_snapshots(&self) -> Vec<JumpSnapshot> {
    self
      .jumplist_backward
      .iter()
      .rev()
      .map(|entry| {
        JumpSnapshot {
          buffer_id:     entry.buffer_id,
          selection:     entry.selection.clone(),
          active_cursor: entry.active_cursor,
        }
      })
      .collect()
  }

  pub fn activate_jump_snapshot(&mut self, jump: &JumpSnapshot) -> bool {
    self.apply_jump_entry(JumpEntry {
      buffer_id:     jump.buffer_id,
      selection:     jump.selection.clone(),
      active_cursor: jump.active_cursor,
    })
  }

  fn current_jump_entry(&self) -> JumpEntry {
    JumpEntry {
      buffer_id:     self.active_buffer,
      selection:     self.document().selection().clone(),
      active_cursor: self.view().active_cursor,
    }
  }

  fn apply_jump_entry(&mut self, entry: JumpEntry) -> bool {
    if self.index_of_buffer_id(entry.buffer_id).is_none() {
      return false;
    }

    if !self.activate_buffer(entry.buffer_id) {
      return false;
    }

    if self
      .document_mut()
      .set_selection(entry.selection.clone())
      .is_err()
    {
      return false;
    }

    self.view_mut().active_cursor = entry.active_cursor;
    let cursor_ids: Vec<_> = self
      .document()
      .selection()
      .cursor_ids()
      .iter()
      .copied()
      .collect();
    self.view_mut().retain_cursor_visual_goals(&cursor_ids);
    true
  }

  pub fn save_jump(&mut self) -> bool {
    let current = self.current_jump_entry();
    if self
      .jumplist_backward
      .last()
      .is_some_and(|entry| *entry == current)
    {
      return false;
    }
    self.jumplist_backward.push(current);
    self.jumplist_forward.clear();
    true
  }

  pub fn jump_backward(&mut self, count: usize) -> bool {
    let mut moved = false;
    let mut current = self.current_jump_entry();
    let mut remaining = count.max(1);

    while remaining > 0 {
      let Some(target) = self.jumplist_backward.pop() else {
        break;
      };

      if target == current {
        continue;
      }

      self.jumplist_forward.push(current.clone());
      if self.apply_jump_entry(target.clone()) {
        moved = true;
        current = target;
        remaining = remaining.saturating_sub(1);
      } else {
        break;
      }
    }

    moved
  }

  pub fn jump_forward(&mut self, count: usize) -> bool {
    let mut moved = false;
    let mut current = self.current_jump_entry();
    let mut remaining = count.max(1);

    while remaining > 0 {
      let Some(target) = self.jumplist_forward.pop() else {
        break;
      };

      if target == current {
        continue;
      }

      self.jumplist_backward.push(current.clone());
      if self.apply_jump_entry(target.clone()) {
        moved = true;
        current = target;
        remaining = remaining.saturating_sub(1);
      } else {
        break;
      }
    }

    moved
  }
}

#[cfg(test)]
mod tests {
  use std::{
    num::NonZeroUsize,
    path::{
      Path,
      PathBuf,
    },
  };

  use ropey::Rope;

  use super::*;
  use crate::{
    document::DocumentId,
    position::Position,
    render::graphics::Rect,
    selection::{
      Range,
      Selection,
    },
    split_tree::{
      PaneDirection,
      SplitAxis,
    },
    transaction::Transaction,
  };

  fn first_buffer_id(editor: &Editor) -> BufferId {
    editor
      .buffer_snapshots()
      .first()
      .map(|snapshot| snapshot.buffer_id)
      .expect("editor has a first buffer")
  }

  #[test]
  fn editor_owns_document_and_view() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("hello"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());

    let editor = Editor::new(editor_id, doc, view);
    assert_eq!(editor.id(), editor_id);
    assert_eq!(editor.document().id(), doc_id);
    assert_eq!(editor.view().viewport, Rect::new(0, 0, 80, 24));
  }

  #[test]
  fn editor_switches_buffers_with_wraparound() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let view2 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(1, 0));
    let view3 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(2, 0));
    let first = first_buffer_id(&editor);
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    let third = editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert_eq!(editor.buffer_count(), 3);
    assert_eq!(editor.active_buffer_id(), third);

    assert!(editor.switch_buffer_forward(1));
    assert_eq!(editor.active_buffer_id(), first);

    assert!(editor.switch_buffer_backward(1));
    assert_eq!(editor.active_buffer_id(), third);
  }

  #[test]
  fn editor_goto_last_accessed_buffer_toggles_between_recent_buffers() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let view2 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(1, 0));
    let view3 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(2, 0));
    let second = editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    let third = editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert_eq!(editor.active_buffer_id(), third);
    assert!(editor.goto_last_accessed_buffer());
    assert_eq!(editor.active_buffer_id(), second);
    assert!(editor.goto_last_accessed_buffer());
    assert_eq!(editor.active_buffer_id(), third);
  }

  #[test]
  fn editor_goto_last_modified_buffer_uses_recent_edit_order() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let view2 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(1, 0));
    let view3 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(2, 0));
    let first = first_buffer_id(&editor);
    let second = editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    let third = editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert!(!editor.goto_last_modified_buffer());
    editor.mark_active_buffer_modified();
    let _ = editor.set_active_buffer(first);
    editor.mark_active_buffer_modified();
    let _ = editor.set_active_buffer(second);

    assert!(editor.goto_last_modified_buffer());
    assert_eq!(editor.active_buffer_id(), first);
    assert!(editor.goto_last_modified_buffer());
    assert_eq!(editor.active_buffer_id(), third);
  }

  #[test]
  fn editor_last_modification_position_reflects_committed_changes() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert_eq!(editor.last_modification_position(), None);

    let tx = Transaction::change(
      editor.document().text(),
      std::iter::once((0, 0, Some("edit ".into()))),
    )
    .expect("insert transaction");
    editor
      .document_mut()
      .apply_transaction(&tx)
      .expect("apply insert");
    editor.document_mut().commit().expect("commit insert");

    assert_eq!(editor.last_modification_position(), Some(5));
  }

  #[test]
  fn editor_object_selection_history_is_scoped_per_buffer() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = Selection::point(0);
    editor.push_object_selection(first.clone());

    let view2 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    assert!(editor.pop_object_selection().is_none());

    let second = Selection::point(1);
    editor.push_object_selection(second.clone());
    assert_eq!(editor.pop_object_selection(), Some(second));

    assert!(editor.switch_buffer_backward(1));
    assert_eq!(editor.pop_object_selection(), Some(first));
  }

  #[test]
  fn editor_split_close_and_only_active_pane() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert_eq!(editor.pane_count(), 1);
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    assert_eq!(editor.pane_count(), 2);
    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    assert_eq!(editor.pane_count(), 3);

    assert!(editor.close_active_pane());
    assert_eq!(editor.pane_count(), 2);
    assert!(editor.only_active_pane());
    assert_eq!(editor.pane_count(), 1);
    assert!(!editor.only_active_pane());
  }

  #[test]
  fn split_panes_clone_and_keep_independent_view_state() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(3, 5));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = editor.active_pane_id();
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let second = editor.active_pane_id();

    assert_eq!(editor.pane_view(first).unwrap().scroll, Position::new(3, 5));
    assert_eq!(
      editor.pane_view(second).unwrap().scroll,
      Position::new(3, 5)
    );

    assert!(editor.set_active_pane(first));
    editor.view_mut().scroll = Position::new(11, 2);
    editor.view_mut().viewport = Rect::new(0, 0, 40, 24);

    assert!(editor.set_active_pane(second));
    editor.view_mut().scroll = Position::new(19, 7);
    editor.view_mut().viewport = Rect::new(40, 0, 40, 24);

    assert_eq!(
      editor.pane_view(first).unwrap().scroll,
      Position::new(11, 2)
    );
    assert_eq!(
      editor.pane_view(first).unwrap().viewport,
      Rect::new(0, 0, 40, 24)
    );
    assert_eq!(
      editor.pane_view(second).unwrap().scroll,
      Position::new(19, 7)
    );
    assert_eq!(
      editor.pane_view(second).unwrap().viewport,
      Rect::new(40, 0, 40, 24)
    );
  }

  #[test]
  fn active_view_uses_current_split_rect() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one\ntwo\nthree\n"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Vertical));
    assert!(editor.split_active_pane(SplitAxis::Horizontal));

    let active_rect = editor
      .frame_pane_snapshots(editor.layout_viewport())
      .into_iter()
      .find(|pane| pane.is_active_pane)
      .map(|pane| pane.rect)
      .expect("active pane rect");

    assert_eq!(editor.view().viewport, active_rect);
  }

  #[test]
  fn editor_open_terminal_in_active_pane_sets_terminal_content() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);
    let active = editor.active_pane_id();

    assert_eq!(
      editor.pane_content_kind(active),
      Some(PaneContentKind::EditorBuffer)
    );
    assert!(!editor.is_active_pane_terminal());

    let first = editor.open_terminal_in_active_pane();
    assert_eq!(
      editor.pane_content(active),
      Some(PaneContent::ClientSurface { surface_id: first })
    );
    assert_eq!(editor.active_terminal_id(), Some(first));
    assert!(editor.is_active_pane_terminal());

    let second = editor.replace_active_pane_with_terminal();
    assert_ne!(first, second);
    assert_eq!(
      editor.pane_content(active),
      Some(PaneContent::ClientSurface { surface_id: second })
    );
  }

  #[test]
  fn editor_close_terminal_in_active_pane_restores_editor_content() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);
    let active = editor.active_pane_id();
    let first = first_buffer_id(&editor);

    let _ = editor.open_terminal_in_active_pane();
    assert!(editor.close_terminal_in_active_pane());
    assert_eq!(
      editor.pane_content(active),
      Some(PaneContent::EditorBuffer { buffer_id: first })
    );
    assert!(!editor.is_active_pane_terminal());
    assert!(!editor.close_terminal_in_active_pane());
  }

  #[test]
  fn editor_pane_item_snapshots_track_local_buffers_and_hidden_terminal() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view.clone());

    let first = first_buffer_id(&editor);
    let second = editor.open_buffer(Rope::from("two"), view, Some(PathBuf::from("/tmp/two.txt")));
    let terminal = editor.open_terminal_in_active_pane();
    assert!(editor.hide_active_terminal_surface());

    let snapshots = editor.pane_item_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].items, vec![
      PaneItemSnapshot {
        content:   PaneContent::EditorBuffer { buffer_id: first },
        is_active: false,
      },
      PaneItemSnapshot {
        content:   PaneContent::EditorBuffer { buffer_id: second },
        is_active: true,
      },
      PaneItemSnapshot {
        content:   PaneContent::ClientSurface { surface_id: terminal },
        is_active: false,
      },
    ]);
  }

  #[test]
  fn editor_close_active_pane_merges_local_items_into_survivor() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view.clone());

    let first = first_buffer_id(&editor);
    let second = editor.open_buffer(Rope::from("two"), view.clone(), Some(PathBuf::from("/tmp/two.txt")));
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let third = editor.open_buffer(Rope::from("three"), view, Some(PathBuf::from("/tmp/three.txt")));

    assert!(editor.close_active_pane());

    let snapshots = editor.pane_item_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].items, vec![
      PaneItemSnapshot {
        content:   PaneContent::EditorBuffer { buffer_id: first },
        is_active: false,
      },
      PaneItemSnapshot {
        content:   PaneContent::EditorBuffer { buffer_id: second },
        is_active: true,
      },
      PaneItemSnapshot {
        content:   PaneContent::EditorBuffer { buffer_id: third },
        is_active: false,
      },
    ]);
  }

  #[test]
  fn editor_split_active_terminal_pane_uses_editor_buffer_for_new_pane() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let _ = editor.open_terminal_in_active_pane();
    assert!(editor.is_active_pane_terminal());
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    assert_eq!(editor.pane_count(), 2);
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::EditorBuffer)
    );

    assert!(editor.rotate_active_pane(true));
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::ClientSurface)
    );
  }

  #[test]
  fn editor_set_active_buffer_preserves_terminal_when_editor_pane_exists() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view.clone());
    let first = first_buffer_id(&editor);

    let _second = editor.open_buffer(Rope::from("two"), view, Some(PathBuf::from("/tmp/two.txt")));
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.is_active_pane_terminal());

    assert!(editor.set_active_buffer(first));
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::EditorBuffer)
    );
    assert_eq!(editor.pane_count(), 2);
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().any(
        |pane| matches!(pane.content, PaneContent::ClientSurface { surface_id: id } if id == terminal_id)
      ),
      "terminal pane should remain present after buffer activation"
    );
  }

  #[test]
  fn editor_set_active_buffer_from_terminal_only_layout_creates_editor_split() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view.clone());
    let first = first_buffer_id(&editor);

    let terminal_id = editor.open_terminal_in_active_pane();
    assert_eq!(editor.pane_count(), 1);
    assert!(editor.is_active_pane_terminal());

    assert!(editor.set_active_buffer(first));
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::EditorBuffer)
    );
    assert_eq!(editor.pane_count(), 2);
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().any(
        |pane| matches!(pane.content, PaneContent::ClientSurface { surface_id: id } if id == terminal_id)
      ),
      "terminal pane should survive and become non-active after split"
    );
  }

  #[test]
  fn editor_open_buffer_from_terminal_only_layout_creates_editor_split() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view.clone());

    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.is_active_pane_terminal());

    let opened = editor.open_buffer(Rope::from("two"), view, Some(PathBuf::from("/tmp/two.txt")));
    assert_eq!(editor.active_buffer_id(), opened);
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::EditorBuffer)
    );
    assert_eq!(editor.pane_count(), 2);
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().any(
        |pane| matches!(pane.content, PaneContent::ClientSurface { surface_id: id } if id == terminal_id)
      ),
      "opening a buffer should not remove the existing terminal pane"
    );
  }

  #[test]
  fn editor_set_active_buffer_in_terminal_pane_detaches_terminal_surface() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let active_pane = editor.active_pane_id();
    let first = first_buffer_id(&editor);
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.set_active_buffer_in_pane(active_pane, first));
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::EditorBuffer { buffer_id: first })
    );
    assert_eq!(editor.terminal_surface_snapshots(), vec![
      ClientSurfaceSnapshot {
        client_surface_id: terminal_id,
        attached_pane:     None,
        is_active:         false,
      }
    ]);
    assert!(editor.focus_terminal_surface(terminal_id));
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::ClientSurface {
        surface_id: terminal_id,
      })
    );
  }

  #[test]
  fn editor_only_active_pane_detaches_non_active_terminal_surface() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = first_buffer_id(&editor);
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.set_active_buffer(first));
    assert_eq!(editor.pane_count(), 2);
    assert!(editor.only_active_pane());
    assert_eq!(editor.pane_count(), 1);
    assert_eq!(editor.terminal_surface_snapshots(), vec![
      ClientSurfaceSnapshot {
        client_surface_id: terminal_id,
        attached_pane:     None,
        is_active:         false,
      }
    ]);
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().all(|pane| {
        !matches!(pane.content, PaneContent::ClientSurface { surface_id: id } if id == terminal_id)
      }),
      "terminal pane should be detached from layout after only_active_pane"
    );
  }

  #[test]
  fn editor_focus_terminal_surface_reattaches_detached_terminal() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = first_buffer_id(&editor);
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.set_active_buffer(first));
    assert!(editor.only_active_pane());
    assert!(editor.focus_terminal_surface(terminal_id));
    assert!(editor.is_active_pane_terminal());
    assert_eq!(editor.active_terminal_id(), Some(terminal_id));
    let snapshots = editor.terminal_surface_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].client_surface_id, terminal_id);
    assert_eq!(snapshots[0].attached_pane, Some(editor.active_pane_id()));
    assert!(snapshots[0].is_active);
  }

  #[test]
  fn editor_hide_active_terminal_surface_detaches_without_destroying_terminal() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let active_pane = editor.active_pane_id();
    let first = first_buffer_id(&editor);
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.hide_active_terminal_surface());
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::EditorBuffer { buffer_id: first })
    );
    assert_eq!(editor.terminal_surface_snapshots(), vec![
      ClientSurfaceSnapshot {
        client_surface_id: terminal_id,
        attached_pane:     None,
        is_active:         false,
      }
    ]);
    assert!(editor.focus_terminal_surface(terminal_id));
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::ClientSurface {
        surface_id: terminal_id,
      })
    );
  }

  #[test]
  fn editor_pane_snapshots_include_layout_and_active_marker() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 120, 40), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Vertical));
    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    let panes = editor.pane_snapshots(Rect::new(0, 0, 120, 40));
    assert_eq!(panes.len(), 3);
    assert_eq!(panes.iter().filter(|pane| pane.is_active_pane).count(), 1);
    let total_area: usize = panes
      .iter()
      .map(|pane| pane.rect.width as usize * pane.rect.height as usize)
      .sum();
    assert_eq!(total_area, 120 * 40);
  }

  #[test]
  fn editor_surface_snapshots_track_visible_editor_panes() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 120, 40), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = first_buffer_id(&editor);
    editor.document_mut().set_display_name("one.rs");
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let second = editor.open_buffer(
      Rope::from("two"),
      ViewState::new(Rect::new(0, 0, 120, 40), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/project/src/two.rs")),
    );
    assert!(editor.set_active_buffer(second));

    let surfaces = editor.editor_surface_snapshots();
    assert_eq!(surfaces.len(), 2);
    assert_eq!(
      surfaces.iter().filter(|surface| surface.is_active).count(),
      1
    );
    assert!(surfaces.iter().any(|surface| surface.buffer_id == first));
    assert!(surfaces.iter().any(|surface| surface.buffer_id == second));
    assert!(surfaces.iter().any(|surface| {
      surface.buffer_id == second
        && surface.file_path == Some(PathBuf::from("/tmp/project/src/two.rs"))
    }));
  }

  #[test]
  fn editor_resize_split_updates_pane_geometry() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 100, 30), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let separator = editor
      .pane_separators(editor.layout_viewport())
      .into_iter()
      .find(|separator| separator.axis == SplitAxis::Vertical)
      .expect("vertical separator");

    assert!(editor.resize_split(separator.split_id, 25, 0));
    let panes = editor.pane_snapshots(editor.layout_viewport());
    assert_eq!(panes.len(), 2);
    let min_x = panes.iter().map(|pane| pane.rect.x).min().unwrap_or(0);
    let left = panes
      .iter()
      .find(|pane| pane.rect.x == min_x)
      .expect("left pane");
    assert_eq!(left.rect.width, 25);
  }

  #[test]
  fn editor_rotate_active_pane_switches_bound_buffer() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = first_buffer_id(&editor);
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let second = editor.open_buffer(
      Rope::from("two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    assert_eq!(editor.active_buffer_id(), second);

    assert!(editor.rotate_active_pane(true));
    assert_eq!(editor.active_buffer_id(), first);
    assert!(editor.rotate_active_pane(false));
    assert_eq!(editor.active_buffer_id(), second);
  }

  #[test]
  fn editor_jump_active_pane_switches_to_neighbor_buffer() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = first_buffer_id(&editor);
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let right = editor.open_buffer(
      Rope::from("right"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right.txt")),
    );
    assert_eq!(editor.active_buffer_id(), right);

    assert!(editor.jump_active_pane(PaneDirection::Left));
    assert_eq!(editor.active_buffer_id(), first);
    assert!(!editor.jump_active_pane(PaneDirection::Up));
  }

  #[test]
  fn editor_swap_active_pane_preserves_active_buffer_binding() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let right_top = editor.open_buffer(
      Rope::from("right-top"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right-top.txt")),
    );
    assert_eq!(editor.active_buffer_id(), right_top);

    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    let right_bottom = editor.open_buffer(
      Rope::from("right-bottom"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right-bottom.txt")),
    );
    assert_eq!(editor.active_buffer_id(), right_bottom);

    assert!(editor.swap_active_pane(PaneDirection::Up));
    assert_eq!(editor.active_buffer_id(), right_bottom);
    assert!(editor.jump_active_pane(PaneDirection::Down));
    assert_eq!(editor.active_buffer_id(), right_top);
  }

  #[test]
  fn editor_move_pane_reorders_tree_and_focuses_moved_buffer() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let viewport = Rect::new(0, 0, 80, 24);
    let view = ViewState::new(viewport, Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let left_pane = editor.active_pane_id();
    let first = first_buffer_id(&editor);
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let right_top_pane = editor.active_pane_id();
    let right_top = editor.open_buffer(
      Rope::from("right-top"),
      ViewState::new(viewport, Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right-top.txt")),
    );
    assert_eq!(editor.active_buffer_id(), right_top);

    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    let right_bottom_pane = editor.active_pane_id();
    let right_bottom = editor.open_buffer(
      Rope::from("right-bottom"),
      ViewState::new(viewport, Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right-bottom.txt")),
    );
    assert_eq!(editor.active_buffer_id(), right_bottom);

    assert!(editor.move_pane(left_pane, right_top_pane, PaneDirection::Right));
    assert_eq!(editor.active_pane_id(), left_pane);
    assert_eq!(editor.active_buffer_id(), first);
    assert_eq!(
      editor
        .frame_pane_snapshots(viewport)
        .into_iter()
        .map(|pane| pane.pane_id)
        .collect::<Vec<_>>(),
      vec![right_top_pane, left_pane, right_bottom_pane]
    );
  }

  #[test]
  fn editor_move_pane_preserves_terminal_attachment() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let viewport = Rect::new(0, 0, 80, 24);
    let view = ViewState::new(viewport, Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let terminal_id = editor.replace_active_pane_with_terminal();
    let terminal_pane = editor.active_pane_id();

    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let editor_pane = editor.active_pane_id();

    assert!(editor.move_pane(terminal_pane, editor_pane, PaneDirection::Right));
    assert_eq!(editor.active_pane_id(), terminal_pane);
    assert_eq!(editor.terminal_surface_snapshots(), vec![
      ClientSurfaceSnapshot {
        client_surface_id: terminal_id,
        attached_pane:     Some(terminal_pane),
        is_active:         true,
      }
    ]);
    assert_eq!(
      editor
        .frame_pane_snapshots(viewport)
        .into_iter()
        .map(|pane| pane.pane_id)
        .collect::<Vec<_>>(),
      vec![editor_pane, terminal_pane]
    );
  }

  #[test]
  fn editor_split_and_new_scratch_flow_keeps_original_buffer_in_other_pane() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = first_buffer_id(&editor);
    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    let scratch = editor.open_buffer(
      Rope::new(),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      None,
    );
    assert_eq!(editor.active_buffer_id(), scratch);
    assert_eq!(editor.buffer_count(), 2);

    assert!(editor.jump_active_pane(PaneDirection::Up));
    assert_eq!(editor.active_buffer_id(), first);
  }

  #[test]
  fn editor_can_reuse_unmodified_unshared_untitled_buffer_for_open() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::new());
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.should_reuse_active_untitled_buffer_for_open());

    let _ = editor.document_mut().replace_range(Range::point(0), "x");
    assert!(!editor.should_reuse_active_untitled_buffer_for_open());
  }

  #[test]
  fn editor_does_not_reuse_untitled_buffer_when_shared_across_panes() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::new());
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    assert!(!editor.should_reuse_active_untitled_buffer_for_open());
  }

  #[test]
  fn editor_replace_active_buffer_reuses_slot_index() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from_str("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(
      editor.replace_active_buffer(Rope::from_str("two"), Some(PathBuf::from("/tmp/two.txt")),)
    );

    assert_eq!(editor.buffer_count(), 1);
    assert_eq!(editor.active_buffer_id(), first_buffer_id(&editor));
    assert_eq!(editor.document().text().to_string(), "two");
    assert_eq!(
      editor
        .active_file_path()
        .map(|path| path.to_string_lossy().to_string()),
      Some("/tmp/two.txt".to_string())
    );
  }

  #[test]
  fn editor_close_buffer_remaps_active_index() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from_str("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = first_buffer_id(&editor);
    let b = editor.open_buffer(
      Rope::from_str("two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    let c = editor.open_buffer(
      Rope::from_str("three"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert_ne!(b, first);
    assert_ne!(c, first);
    assert!(editor.close_buffer(b));
    assert_eq!(editor.buffer_count(), 2);
    assert_eq!(editor.active_buffer_id(), c);
    assert_eq!(
      editor
        .buffer_snapshot(c)
        .and_then(|s| s.file_path.map(|p| p.to_string_lossy().to_string())),
      Some("/tmp/three.txt".into())
    );
  }

  #[test]
  fn editor_cannot_close_last_buffer() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from_str("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(!editor.close_buffer(first_buffer_id(&editor)));
    assert_eq!(editor.buffer_count(), 1);
  }

  #[test]
  fn editor_move_buffer_reorders_and_preserves_active_buffer_identity() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from_str("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);
    let first = first_buffer_id(&editor);

    let b = editor.open_buffer(
      Rope::from_str("two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    let c = editor.open_buffer(
      Rope::from_str("three"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/three.txt")),
    );
    assert_eq!(editor.active_buffer_id(), c);

    assert!(editor.move_buffer(c, first));
    assert_eq!(editor.active_buffer_id(), c);
    let ordered: Vec<BufferId> = editor
      .buffer_snapshots()
      .into_iter()
      .map(|snapshot| snapshot.buffer_id)
      .collect();
    assert_eq!(ordered, vec![c, first, b]);
    assert_eq!(
      editor
        .buffer_snapshot(c)
        .and_then(|s| s.file_path.map(|p| p.to_string_lossy().to_string())),
      Some("/tmp/three.txt".into())
    );
    assert_eq!(
      editor
        .buffer_snapshot(b)
        .and_then(|s| s.file_path.map(|p| p.to_string_lossy().to_string())),
      Some("/tmp/two.txt".into())
    );
  }

  #[test]
  fn editor_transpose_active_pane_requires_a_split_branch() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(!editor.transpose_active_pane_branch());
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    assert!(editor.transpose_active_pane_branch());
  }

  #[test]
  fn editor_jumplist_moves_between_saved_selections() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("alpha beta gamma"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let _ = editor.document_mut().set_selection(Selection::point(0));
    assert!(editor.save_jump());
    let _ = editor.document_mut().set_selection(Selection::point(6));
    assert!(editor.save_jump());
    let _ = editor.document_mut().set_selection(Selection::point(11));

    assert!(editor.jump_backward(1));
    assert_eq!(editor.document().selection().ranges()[0], Range::point(6));
    assert!(editor.jump_backward(1));
    assert_eq!(editor.document().selection().ranges()[0], Range::point(0));
    assert!(!editor.jump_backward(1));

    assert!(editor.jump_forward(1));
    assert_eq!(editor.document().selection().ranges()[0], Range::point(6));
    assert!(editor.jump_forward(1));
    assert_eq!(editor.document().selection().ranges()[0], Range::point(11));
    assert!(!editor.jump_forward(1));
  }

  #[test]
  fn editor_jumplist_can_switch_buffers() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);
    let first = first_buffer_id(&editor);

    let _ = editor.document_mut().set_selection(Selection::point(1));
    assert!(editor.save_jump());

    let second = editor.open_buffer(
      Rope::from("two two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    assert_eq!(editor.active_buffer_id(), second);
    let _ = editor.document_mut().set_selection(Selection::point(4));
    assert!(editor.save_jump());
    let _ = editor.document_mut().set_selection(Selection::point(7));

    assert!(editor.jump_backward(1));
    assert_eq!(editor.active_buffer_id(), second);
    assert_eq!(editor.document().selection().ranges()[0], Range::point(4));

    assert!(editor.jump_backward(1));
    assert_eq!(editor.active_buffer_id(), first);
    assert_eq!(editor.document().selection().ranges()[0], Range::point(1));
  }

  #[test]
  fn editor_buffer_snapshots_mru_orders_active_then_recent() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);
    let first = first_buffer_id(&editor);

    let second = editor.open_buffer(
      Rope::from("two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    let third = editor.open_buffer(
      Rope::from("three"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/three.txt")),
    );
    assert!(editor.set_active_buffer(second));

    let snapshots = editor.buffer_snapshots_mru();
    assert_eq!(snapshots.len(), 3);
    assert_eq!(snapshots[0].buffer_id, second);
    assert_eq!(snapshots[1].buffer_id, third);
    assert_eq!(snapshots[2].buffer_id, first);
    assert!(snapshots[0].is_active);
    assert_eq!(
      snapshots[0].file_path.as_deref(),
      Some(Path::new("/tmp/two.txt"))
    );
  }

  #[test]
  fn editor_activate_jump_snapshot_restores_saved_selection() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("alpha beta gamma"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let _ = editor.document_mut().set_selection(Selection::point(0));
    assert!(editor.save_jump());
    let _ = editor.document_mut().set_selection(Selection::point(6));
    assert!(editor.save_jump());

    let snapshots = editor.jumplist_backward_snapshots();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].selection.ranges()[0], Range::point(6));
    assert_eq!(snapshots[1].selection.ranges()[0], Range::point(0));

    assert!(editor.activate_jump_snapshot(&snapshots[1]));
    assert_eq!(editor.document().selection().ranges()[0], Range::point(0));
  }

  #[test]
  fn pane_queries_report_neighbors_and_rects() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let left = editor.active_pane_id();
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let right = editor.active_pane_id();

    let neighbors = editor.pane_neighbors(left).expect("neighbors");
    assert_eq!(neighbors.right, Some(right));
    assert_eq!(
      editor.pane_in_direction(right, PaneDirection::Left),
      Some(left)
    );
    assert!(editor.pane_rect(left).is_some());
    assert!(editor.pane_rect(right).is_some());
  }

  #[test]
  fn resolve_open_target_can_create_neighbor_without_stealing_focus_after_open() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let first = editor.active_pane_id();
    let resolved = editor
      .resolve_open_target(OpenTarget::Split {
        axis:      SplitAxis::Vertical,
        focus_new: false,
      })
      .expect("resolved split target");

    assert_ne!(resolved.pane, first);
    assert_eq!(resolved.restore_focus_to, Some(first));
    assert_eq!(editor.active_pane_id(), resolved.pane);
  }
}
