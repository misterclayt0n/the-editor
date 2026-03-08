//! Minimal editor/surface state for the-lib.
//!
//! This is intentionally small: it owns a set of document buffers plus
//! per-buffer view/render state. IO, UI, and dispatch logic live outside
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
    SplitAxis,
    SplitNodeId,
    SplitSeparator,
    SplitTree,
  },
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

#[derive(Debug)]
pub struct Editor {
  id:                EditorId,
  buffers:           Vec<BufferState>,
  active_buffer:     usize,
  layout_viewport:   Rect,
  split_tree:        SplitTree,
  pane_content:      BTreeMap<PaneId, PaneContent>,
  terminal_surfaces: BTreeMap<TerminalId, TerminalSurface>,
  next_terminal_id:  NonZeroUsize,
  next_document_id:  NonZeroUsize,
  access_history:    Vec<usize>,
  modified_history:  Vec<usize>,
  jumplist_backward: Vec<JumpEntry>,
  jumplist_forward:  Vec<JumpEntry>,
}

#[derive(Debug)]
struct BufferState {
  document:                 Document,
  view:                     ViewState,
  render_cache:             RenderCache,
  file_path:                Option<PathBuf>,
  object_selection_history: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JumpEntry {
  buffer_index:  usize,
  selection:     Selection,
  active_cursor: Option<crate::selection::CursorId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneSnapshot {
  pub pane_id:        PaneId,
  pub buffer_index:   usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TerminalId(NonZeroUsize);

impl TerminalId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for TerminalId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneContentKind {
  EditorBuffer,
  Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneContent {
  EditorBuffer { buffer_index: usize },
  Terminal { terminal_id: TerminalId },
}

impl PaneContent {
  pub const fn kind(self) -> PaneContentKind {
    match self {
      Self::EditorBuffer { .. } => PaneContentKind::EditorBuffer,
      Self::Terminal { .. } => PaneContentKind::Terminal,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalSurface {
  terminal_id:    TerminalId,
  attached_pane:  Option<PaneId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSurfaceSnapshot {
  pub terminal_id:   TerminalId,
  pub attached_pane: Option<PaneId>,
  pub is_active:     bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferSnapshot {
  pub buffer_id:    u64,
  pub buffer_index: usize,
  pub file_path:    Option<PathBuf>,
  pub display_name: String,
  pub modified:     bool,
  pub is_active:    bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSurfaceSnapshot {
  pub pane_id:      PaneId,
  pub buffer_id:    u64,
  pub buffer_index: usize,
  pub file_path:    Option<PathBuf>,
  pub display_name: String,
  pub modified:     bool,
  pub is_active:    bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpSnapshot {
  pub buffer_index:  usize,
  pub selection:     Selection,
  pub active_cursor: Option<CursorId>,
}

impl BufferState {
  fn new(document: Document, view: ViewState, file_path: Option<PathBuf>) -> Self {
    Self {
      document,
      view,
      render_cache: RenderCache::default(),
      file_path,
      object_selection_history: Vec::new(),
    }
  }
}

impl Editor {
  fn remap_index_after_close(index: usize, closed: usize) -> Option<usize> {
    if index == closed {
      None
    } else if index > closed {
      Some(index - 1)
    } else {
      Some(index)
    }
  }

  fn remap_index_after_move(index: usize, from: usize, to: usize) -> usize {
    if from == to {
      return index;
    }
    if index == from {
      return to;
    }
    if from < to {
      if (from + 1..=to).contains(&index) {
        index - 1
      } else {
        index
      }
    } else if (to..from).contains(&index) {
      index + 1
    } else {
      index
    }
  }

  fn buffer_snapshot_for_index(&self, index: usize) -> Option<BufferSnapshot> {
    let buffer = self.buffers.get(index)?;
    Some(BufferSnapshot {
      buffer_id:    buffer.document.id().get().get() as u64,
      buffer_index: index,
      file_path:    buffer.file_path.clone(),
      display_name: buffer.document.display_name().into_owned(),
      modified:     buffer.document.flags().modified,
      is_active:    index == self.active_buffer,
    })
  }

  pub fn new(id: EditorId, document: Document, view: ViewState) -> Self {
    let next_doc = document.id().get().get().saturating_add(1);
    let next_document_id = NonZeroUsize::new(next_doc).unwrap_or(document.id().get());
    let split_tree = SplitTree::new();
    let mut pane_content = BTreeMap::new();
    pane_content.insert(split_tree.active_pane(), PaneContent::EditorBuffer {
      buffer_index: 0,
    });

    Self {
      id,
      buffers: vec![BufferState::new(document, view, None)],
      active_buffer: 0,
      layout_viewport: view.viewport,
      split_tree,
      pane_content,
      terminal_surfaces: BTreeMap::new(),
      next_terminal_id: NonZeroUsize::new(1).expect("nonzero"),
      next_document_id,
      access_history: Vec::new(),
      modified_history: Vec::new(),
      jumplist_backward: Vec::new(),
      jumplist_forward: Vec::new(),
    }
  }

  fn alloc_terminal_id(&mut self) -> TerminalId {
    let id = TerminalId::new(self.next_terminal_id);
    let next = self.next_terminal_id.get().saturating_add(1);
    self.next_terminal_id = NonZeroUsize::new(next).unwrap_or(self.next_terminal_id);
    id
  }

  fn detach_terminal_surface_in_pane(&mut self, pane: PaneId) -> Option<TerminalId> {
    let Some(PaneContent::Terminal { terminal_id }) = self.pane_content.get(&pane).copied() else {
      return None;
    };
    if let Some(surface) = self.terminal_surfaces.get_mut(&terminal_id) {
      surface.attached_pane = None;
    }
    Some(terminal_id)
  }

  fn remove_terminal_surface(&mut self, terminal_id: TerminalId) -> bool {
    let Some(surface) = self.terminal_surfaces.remove(&terminal_id) else {
      return false;
    };

    if let Some(pane) = surface.attached_pane
      && matches!(
        self.pane_content.get(&pane),
        Some(PaneContent::Terminal { terminal_id: attached }) if *attached == terminal_id
      )
    {
      let fallback = self.active_buffer.min(self.buffers.len().saturating_sub(1));
      self.pane_content.insert(pane, PaneContent::EditorBuffer {
        buffer_index: fallback,
      });
    }
    true
  }

  fn first_editor_pane(&self) -> Option<PaneId> {
    self
      .split_tree
      .pane_order()
      .into_iter()
      .find(|pane| {
        matches!(
          self.pane_content.get(pane),
          Some(PaneContent::EditorBuffer { .. })
        )
      })
  }

  fn activate_buffer(&mut self, index: usize) -> bool {
    if index >= self.buffers.len() {
      return false;
    }

    let target_pane = match self.active_pane_content() {
      Some(PaneContent::EditorBuffer { .. }) | None => self.active_pane_id(),
      Some(PaneContent::Terminal { .. }) => {
        if let Some(existing_editor_pane) = self.first_editor_pane() {
          let _ = self.split_tree.set_active_pane(existing_editor_pane);
          existing_editor_pane
        } else {
          self.split_tree.split_active(SplitAxis::Vertical)
        }
      },
    };

    if index != self.active_buffer {
      self.access_history.push(self.active_buffer);
      self.active_buffer = index;
    }
    self
      .pane_content
      .insert(target_pane, PaneContent::EditorBuffer {
        buffer_index: index,
      });
    true
  }

  fn touch_modified_history(&mut self, index: usize) {
    if let Some(pos) = self
      .modified_history
      .iter()
      .position(|entry| *entry == index)
    {
      self.modified_history.remove(pos);
    }
    self.modified_history.push(index);
  }

  pub fn id(&self) -> EditorId {
    self.id
  }

  pub fn document(&self) -> &Document {
    &self.buffers[self.active_buffer].document
  }

  pub fn document_mut(&mut self) -> &mut Document {
    &mut self.buffers[self.active_buffer].document
  }

  pub fn view(&self) -> ViewState {
    self.buffers[self.active_buffer].view
  }

  pub fn view_mut(&mut self) -> &mut ViewState {
    &mut self.buffers[self.active_buffer].view
  }

  pub fn render_cache(&self) -> &RenderCache {
    &self.buffers[self.active_buffer].render_cache
  }

  pub fn render_cache_mut(&mut self) -> &mut RenderCache {
    &mut self.buffers[self.active_buffer].render_cache
  }

  pub fn document_and_cache(&mut self) -> (&Document, &mut RenderCache) {
    let buffer = &mut self.buffers[self.active_buffer];
    (&buffer.document, &mut buffer.render_cache)
  }

  pub fn buffer_count(&self) -> usize {
    self.buffers.len()
  }

  pub fn active_buffer_index(&self) -> usize {
    self.active_buffer
  }

  pub fn buffer_snapshot(&self, index: usize) -> Option<BufferSnapshot> {
    self.buffer_snapshot_for_index(index)
  }

  pub fn buffer_snapshots_mru(&self) -> Vec<BufferSnapshot> {
    let len = self.buffers.len();
    if len == 0 {
      return Vec::new();
    }

    let mut order = Vec::with_capacity(len);
    let mut seen = vec![false; len];

    if self.active_buffer < len {
      order.push(self.active_buffer);
      seen[self.active_buffer] = true;
    }

    for index in self.access_history.iter().rev().copied() {
      if index < len && !seen[index] {
        seen[index] = true;
        order.push(index);
      }
    }

    for index in 0..len {
      if !seen[index] {
        order.push(index);
      }
    }

    order
      .into_iter()
      .filter_map(|index| self.buffer_snapshot_for_index(index))
      .collect()
  }

  pub fn pane_count(&self) -> usize {
    self.split_tree.pane_count()
  }

  pub fn layout_viewport(&self) -> Rect {
    self.layout_viewport
  }

  pub fn set_layout_viewport(&mut self, viewport: Rect) {
    self.layout_viewport = viewport;
  }

  pub fn active_pane_id(&self) -> PaneId {
    self.split_tree.active_pane()
  }

  pub fn set_active_pane(&mut self, pane: PaneId) -> bool {
    if !self.split_tree.set_active_pane(pane) {
      return false;
    }
    let Some(content) = self.pane_content.get(&pane).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_index } = content {
      self.active_buffer = buffer_index.min(self.buffers.len().saturating_sub(1));
    }
    true
  }

  pub fn pane_snapshots(&self, area: Rect) -> Vec<PaneSnapshot> {
    self
      .frame_pane_snapshots(area)
      .into_iter()
      .filter_map(|pane| {
        match pane.content {
          PaneContent::EditorBuffer { buffer_index } => {
            Some(PaneSnapshot {
              pane_id: pane.pane_id,
              buffer_index,
              rect: pane.rect,
              is_active_pane: pane.is_active_pane,
            })
          },
          PaneContent::Terminal { .. } => None,
        }
      })
      .collect()
  }

  pub fn editor_surface_snapshots(&self) -> Vec<EditorSurfaceSnapshot> {
    self
      .pane_snapshots(self.layout_viewport())
      .into_iter()
      .filter_map(|pane| {
        let buffer = self.buffer_snapshot_for_index(pane.buffer_index)?;
        Some(EditorSurfaceSnapshot {
          pane_id:      pane.pane_id,
          buffer_id:    buffer.buffer_id,
          buffer_index: buffer.buffer_index,
          file_path:    buffer.file_path,
          display_name: buffer.display_name,
          modified:     buffer.modified,
          is_active:    pane.is_active_pane,
        })
      })
      .collect()
  }

  pub fn frame_pane_snapshots(&self, area: Rect) -> Vec<FramePaneSnapshot> {
    let active = self.split_tree.active_pane();
    self
      .split_tree
      .layout(area)
      .into_iter()
      .filter_map(|(pane_id, rect)| {
        let Some(content) = self.pane_content.get(&pane_id).copied() else {
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
    self.split_tree.separators(area)
  }

  pub fn resize_split(&mut self, split_id: SplitNodeId, x: u16, y: u16) -> bool {
    self
      .split_tree
      .resize_split(split_id, self.layout_viewport, x, y)
  }

  pub fn pane_buffer_index(&self, pane: PaneId) -> Option<usize> {
    match self.pane_content.get(&pane).copied() {
      Some(PaneContent::EditorBuffer { buffer_index }) => Some(buffer_index),
      _ => None,
    }
  }

  pub fn pane_content(&self, pane: PaneId) -> Option<PaneContent> {
    self.pane_content.get(&pane).copied()
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

  pub fn active_terminal_id(&self) -> Option<TerminalId> {
    match self.active_pane_content() {
      Some(PaneContent::Terminal { terminal_id }) => Some(terminal_id),
      _ => None,
    }
  }

  pub fn terminal_surface_snapshots(&self) -> Vec<TerminalSurfaceSnapshot> {
    let active_pane = self.active_pane_id();
    self
      .terminal_surfaces
      .values()
      .copied()
      .map(|surface| {
        TerminalSurfaceSnapshot {
          terminal_id:   surface.terminal_id,
          attached_pane: surface.attached_pane,
          is_active:     surface.attached_pane == Some(active_pane),
        }
      })
      .collect()
  }

  pub fn is_active_pane_terminal(&self) -> bool {
    matches!(
      self.active_pane_content(),
      Some(PaneContent::Terminal { .. })
    )
  }

  pub fn open_terminal_in_active_pane(&mut self) -> TerminalId {
    let pane = self.active_pane_id();
    let _ = self.detach_terminal_surface_in_pane(pane);
    let terminal_id = self.alloc_terminal_id();
    self.terminal_surfaces.insert(terminal_id, TerminalSurface {
      terminal_id,
      attached_pane: Some(pane),
    });
    self
      .pane_content
      .insert(pane, PaneContent::Terminal { terminal_id });
    terminal_id
  }

  pub fn replace_active_pane_with_terminal(&mut self) -> TerminalId {
    self.open_terminal_in_active_pane()
  }

  pub fn close_terminal_in_active_pane(&mut self) -> bool {
    let pane = self.active_pane_id();
    let Some(PaneContent::Terminal { terminal_id }) = self.pane_content.get(&pane).copied() else {
      return false;
    };
    self.remove_terminal_surface(terminal_id)
  }

  pub fn hide_active_terminal_surface(&mut self) -> bool {
    let pane = self.active_pane_id();
    let Some(_terminal_id) = self.detach_terminal_surface_in_pane(pane) else {
      return false;
    };
    let fallback = self.active_buffer.min(self.buffers.len().saturating_sub(1));
    self.pane_content.insert(pane, PaneContent::EditorBuffer {
      buffer_index: fallback,
    });
    true
  }

  pub fn set_active_buffer_in_pane(&mut self, pane: PaneId, index: usize) -> bool {
    if index >= self.buffers.len() || !self.split_tree.contains_pane(pane) {
      return false;
    }
    let was_active = pane == self.active_pane_id();
    let _ = self.detach_terminal_surface_in_pane(pane);
    self.pane_content.insert(pane, PaneContent::EditorBuffer {
      buffer_index: index,
    });
    if was_active {
      self.active_buffer = index;
    }
    true
  }

  pub fn buffer_view(&self, index: usize) -> Option<ViewState> {
    self.buffers.get(index).map(|buffer| buffer.view)
  }

  pub fn buffer_document(&self, index: usize) -> Option<&Document> {
    self.buffers.get(index).map(|buffer| &buffer.document)
  }

  pub fn buffer_document_mut(&mut self, index: usize) -> Option<&mut Document> {
    self
      .buffers
      .get_mut(index)
      .map(|buffer| &mut buffer.document)
  }

  pub fn find_buffer_by_id(&self, buffer_id: u64) -> Option<usize> {
    self
      .buffers
      .iter()
      .position(|buffer| buffer.document.id().get().get() as u64 == buffer_id)
  }

  pub fn set_buffer_viewport(&mut self, index: usize, viewport: Rect) -> bool {
    let Some(buffer) = self.buffers.get_mut(index) else {
      return false;
    };
    buffer.view.viewport = viewport;
    true
  }

  pub fn document_and_cache_at_mut(
    &mut self,
    index: usize,
  ) -> Option<(&Document, &mut RenderCache)> {
    let buffer = self.buffers.get_mut(index)?;
    Some((&buffer.document, &mut buffer.render_cache))
  }

  pub fn split_active_pane(&mut self, axis: SplitAxis) -> bool {
    let current_content = self
      .active_pane_content()
      .unwrap_or(PaneContent::EditorBuffer {
        buffer_index: self.active_buffer,
      });
    let pane = self.split_tree.split_active(axis);
    let content = match current_content {
      PaneContent::EditorBuffer { buffer_index } => PaneContent::EditorBuffer { buffer_index },
      PaneContent::Terminal { .. } => {
        PaneContent::EditorBuffer {
          buffer_index: self.active_buffer,
        }
      },
    };
    self.pane_content.insert(pane, content);
    if let PaneContent::EditorBuffer { buffer_index } = content {
      self.active_buffer = buffer_index.min(self.buffers.len().saturating_sub(1));
    }
    true
  }

  pub fn close_active_pane(&mut self) -> bool {
    let closing = self.split_tree.active_pane();
    let Ok(next_active) = self.split_tree.close_active() else {
      return false;
    };

    let _ = self.detach_terminal_surface_in_pane(closing);
    self.pane_content.remove(&closing);
    let Some(next_content) = self.pane_content.get(&next_active).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_index } = next_content {
      self.active_buffer = buffer_index.min(self.buffers.len().saturating_sub(1));
    } else {
      self.active_buffer = self.active_buffer.min(self.buffers.len().saturating_sub(1));
    }
    true
  }

  pub fn only_active_pane(&mut self) -> bool {
    if self.split_tree.pane_count() <= 1 {
      return false;
    }
    let active = self.split_tree.active_pane();
    let Some(active_content) = self.pane_content.get(&active).copied() else {
      return false;
    };

    let removed_panes: Vec<PaneId> = self
      .pane_content
      .keys()
      .copied()
      .filter(|pane| *pane != active)
      .collect();
    for pane in removed_panes {
      let _ = self.detach_terminal_surface_in_pane(pane);
    }

    self.split_tree.only_active();
    self.pane_content.retain(|pane, _| *pane == active);
    if let PaneContent::EditorBuffer { buffer_index } = active_content {
      self.active_buffer = buffer_index.min(self.buffers.len().saturating_sub(1));
    } else {
      self.active_buffer = self.active_buffer.min(self.buffers.len().saturating_sub(1));
    }
    true
  }

  pub fn rotate_active_pane(&mut self, next: bool) -> bool {
    if !self.split_tree.rotate_focus(next) {
      return false;
    }
    let active = self.split_tree.active_pane();
    let Some(content) = self.pane_content.get(&active).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_index } = content {
      self.active_buffer = buffer_index.min(self.buffers.len().saturating_sub(1));
    }
    true
  }

  pub fn jump_active_pane(&mut self, direction: PaneDirection) -> bool {
    if !self.split_tree.jump_active(direction) {
      return false;
    }
    let active = self.split_tree.active_pane();
    let Some(content) = self.pane_content.get(&active).copied() else {
      return false;
    };
    if let PaneContent::EditorBuffer { buffer_index } = content {
      self.active_buffer = buffer_index.min(self.buffers.len().saturating_sub(1));
    }
    true
  }

  pub fn swap_active_pane(&mut self, direction: PaneDirection) -> bool {
    self.split_tree.swap_active(direction)
  }

  pub fn transpose_active_pane_branch(&mut self) -> bool {
    self.split_tree.transpose_active_branch()
  }

  pub fn focus_terminal_surface(&mut self, terminal_id: TerminalId) -> bool {
    let Some(surface) = self.terminal_surfaces.get(&terminal_id).copied() else {
      return false;
    };

    if let Some(pane) = surface.attached_pane {
      return self.set_active_pane(pane);
    }

    let pane = self.active_pane_id();
    let _ = self.detach_terminal_surface_in_pane(pane);
    self.pane_content.insert(pane, PaneContent::Terminal { terminal_id });
    if let Some(surface) = self.terminal_surfaces.get_mut(&terminal_id) {
      surface.attached_pane = Some(pane);
    }
    true
  }

  pub fn set_active_buffer_preserving_terminal(&mut self, index: usize) -> bool {
    self.activate_buffer(index)
  }

  pub fn set_active_buffer(&mut self, index: usize) -> bool {
    self.set_active_buffer_preserving_terminal(index)
  }

  pub fn switch_buffer_forward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    self.set_active_buffer((self.active_buffer + step) % len)
  }

  pub fn switch_buffer_backward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    self.set_active_buffer((self.active_buffer + len - step) % len)
  }

  pub fn active_file_path(&self) -> Option<&Path> {
    self.buffers[self.active_buffer].file_path.as_deref()
  }

  pub fn can_reuse_active_untitled_buffer_for_open(&self) -> bool {
    let Some(active) = self.buffers.get(self.active_buffer) else {
      return false;
    };
    if active.file_path.is_some() || active.document.flags().modified {
      return false;
    }
    let active_index = self.active_buffer;
    let pane_refs = self
      .pane_content
      .values()
      .filter(|&&content| {
        matches!(content, PaneContent::EditorBuffer { buffer_index } if buffer_index == active_index)
      })
      .count();
    pane_refs <= 1
  }

  pub fn replace_active_buffer(&mut self, text: Rope, file_path: Option<PathBuf>) -> bool {
    let Some(current_view) = self
      .buffers
      .get(self.active_buffer)
      .map(|buffer| buffer.view)
    else {
      return false;
    };
    let document_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);
    let document = Document::new(document_id, text);
    self.buffers[self.active_buffer] = BufferState::new(document, current_view, file_path);
    true
  }

  pub fn set_active_file_path(&mut self, path: Option<PathBuf>) {
    self.buffers[self.active_buffer].file_path = path;
  }

  pub fn find_buffer_by_path(&self, path: &Path) -> Option<usize> {
    self
      .buffers
      .iter()
      .position(|buffer| buffer.file_path.as_deref() == Some(path))
  }

  pub fn open_buffer(&mut self, text: Rope, view: ViewState, file_path: Option<PathBuf>) -> usize {
    let document_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);
    let document = Document::new(document_id, text);

    self
      .buffers
      .push(BufferState::new(document, view, file_path));
    let next_index = self.buffers.len() - 1;
    let _ = self.activate_buffer(next_index);
    next_index
  }

  pub fn open_buffer_without_activation(
    &mut self,
    text: Rope,
    view: ViewState,
    file_path: Option<PathBuf>,
  ) -> usize {
    let document_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);
    let document = Document::new(document_id, text);
    self
      .buffers
      .push(BufferState::new(document, view, file_path));
    self.buffers.len() - 1
  }

  pub fn close_buffer(&mut self, index: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 || index >= len {
      return false;
    }

    let replacement_before = if index > 0 { index - 1 } else { 1 };

    for content in self.pane_content.values_mut() {
      if let PaneContent::EditorBuffer { buffer_index } = content
        && *buffer_index == index
      {
        *buffer_index = replacement_before;
      }
    }

    self.buffers.remove(index);

    for content in self.pane_content.values_mut() {
      if let PaneContent::EditorBuffer { buffer_index } = content
        && *buffer_index > index
      {
        *buffer_index -= 1;
      }
    }

    if self.active_buffer == index {
      self.active_buffer = replacement_before;
    } else if self.active_buffer > index {
      self.active_buffer -= 1;
    }

    if let Some(PaneContent::EditorBuffer { buffer_index }) =
      self.pane_content.get(&self.split_tree.active_pane())
    {
      self.active_buffer = (*buffer_index).min(self.buffers.len().saturating_sub(1));
    } else {
      self.active_buffer = self.active_buffer.min(self.buffers.len().saturating_sub(1));
    }

    self.access_history.retain_mut(|entry| {
      match Self::remap_index_after_close(*entry, index) {
        Some(remapped) => {
          *entry = remapped;
          true
        },
        None => false,
      }
    });

    self.modified_history.retain_mut(|entry| {
      match Self::remap_index_after_close(*entry, index) {
        Some(remapped) => {
          *entry = remapped;
          true
        },
        None => false,
      }
    });

    self.jumplist_backward.retain_mut(|entry| {
      let Some(remapped) = Self::remap_index_after_close(entry.buffer_index, index) else {
        return false;
      };
      entry.buffer_index = remapped;
      true
    });
    self.jumplist_forward.retain_mut(|entry| {
      let Some(remapped) = Self::remap_index_after_close(entry.buffer_index, index) else {
        return false;
      };
      entry.buffer_index = remapped;
      true
    });

    true
  }

  pub fn move_buffer(&mut self, from: usize, to: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 || from >= len || to >= len || from == to {
      return false;
    }

    let buffer = self.buffers.remove(from);
    self.buffers.insert(to, buffer);

    self.active_buffer = Self::remap_index_after_move(self.active_buffer, from, to);
    for content in self.pane_content.values_mut() {
      if let PaneContent::EditorBuffer { buffer_index } = content {
        *buffer_index = Self::remap_index_after_move(*buffer_index, from, to);
      }
    }
    for entry in &mut self.access_history {
      *entry = Self::remap_index_after_move(*entry, from, to);
    }
    for entry in &mut self.modified_history {
      *entry = Self::remap_index_after_move(*entry, from, to);
    }
    for entry in &mut self.jumplist_backward {
      entry.buffer_index = Self::remap_index_after_move(entry.buffer_index, from, to);
    }
    for entry in &mut self.jumplist_forward {
      entry.buffer_index = Self::remap_index_after_move(entry.buffer_index, from, to);
    }

    true
  }

  pub fn goto_last_accessed_buffer(&mut self) -> bool {
    while let Some(index) = self.access_history.pop() {
      if index < self.buffers.len() && index != self.active_buffer {
        return self.set_active_buffer(index);
      }
    }
    false
  }

  pub fn mark_active_buffer_modified(&mut self) {
    self.touch_modified_history(self.active_buffer);
  }

  pub fn push_object_selection(&mut self, selection: Selection) {
    self.buffers[self.active_buffer]
      .object_selection_history
      .push(selection);
  }

  pub fn pop_object_selection(&mut self) -> Option<Selection> {
    self.buffers[self.active_buffer]
      .object_selection_history
      .pop()
  }

  pub fn clear_object_selections(&mut self) {
    self.buffers[self.active_buffer]
      .object_selection_history
      .clear();
  }

  pub fn goto_last_modified_buffer(&mut self) -> bool {
    let current = self.active_buffer;
    let Some(index) = self
      .modified_history
      .iter()
      .rev()
      .copied()
      .find(|idx| *idx < self.buffers.len() && *idx != current)
    else {
      return false;
    };
    self.set_active_buffer(index)
  }

  pub fn last_modification_position(&self) -> Option<usize> {
    self.document().history().last_edit_pos()
  }

  pub fn jumplist_backward_snapshots(&self) -> Vec<JumpSnapshot> {
    self
      .jumplist_backward
      .iter()
      .rev()
      .map(|entry| {
        JumpSnapshot {
          buffer_index:  entry.buffer_index,
          selection:     entry.selection.clone(),
          active_cursor: entry.active_cursor,
        }
      })
      .collect()
  }

  pub fn activate_jump_snapshot(&mut self, jump: &JumpSnapshot) -> bool {
    self.apply_jump_entry(JumpEntry {
      buffer_index:  jump.buffer_index,
      selection:     jump.selection.clone(),
      active_cursor: jump.active_cursor,
    })
  }

  fn current_jump_entry(&self) -> JumpEntry {
    JumpEntry {
      buffer_index:  self.active_buffer,
      selection:     self.document().selection().clone(),
      active_cursor: self.view().active_cursor,
    }
  }

  fn apply_jump_entry(&mut self, entry: JumpEntry) -> bool {
    if entry.buffer_index >= self.buffers.len() {
      return false;
    }

    if !self.activate_buffer(entry.buffer_index) {
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
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert_eq!(editor.buffer_count(), 3);
    assert_eq!(editor.active_buffer_index(), 2);

    assert!(editor.switch_buffer_forward(1));
    assert_eq!(editor.active_buffer_index(), 0);

    assert!(editor.switch_buffer_backward(1));
    assert_eq!(editor.active_buffer_index(), 2);
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
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert_eq!(editor.active_buffer_index(), 2);
    assert!(editor.goto_last_accessed_buffer());
    assert_eq!(editor.active_buffer_index(), 1);
    assert!(editor.goto_last_accessed_buffer());
    assert_eq!(editor.active_buffer_index(), 2);
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
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert!(!editor.goto_last_modified_buffer());
    editor.mark_active_buffer_modified();
    let _ = editor.set_active_buffer(0);
    editor.mark_active_buffer_modified();
    let _ = editor.set_active_buffer(1);

    assert!(editor.goto_last_modified_buffer());
    assert_eq!(editor.active_buffer_index(), 0);
    assert!(editor.goto_last_modified_buffer());
    assert_eq!(editor.active_buffer_index(), 2);
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
      Some(PaneContent::Terminal { terminal_id: first })
    );
    assert_eq!(editor.active_terminal_id(), Some(first));
    assert!(editor.is_active_pane_terminal());

    let second = editor.replace_active_pane_with_terminal();
    assert_ne!(first, second);
    assert_eq!(
      editor.pane_content(active),
      Some(PaneContent::Terminal {
        terminal_id: second,
      })
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

    let _ = editor.open_terminal_in_active_pane();
    assert!(editor.close_terminal_in_active_pane());
    assert_eq!(
      editor.pane_content(active),
      Some(PaneContent::EditorBuffer { buffer_index: 0 })
    );
    assert!(!editor.is_active_pane_terminal());
    assert!(!editor.close_terminal_in_active_pane());
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
      Some(PaneContentKind::Terminal)
    );
  }

  #[test]
  fn editor_set_active_buffer_preserves_terminal_when_editor_pane_exists() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let second = editor.open_buffer(
      Rope::from("two"),
      view,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    assert_eq!(second, 1);
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.is_active_pane_terminal());

    assert!(editor.set_active_buffer(0));
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::EditorBuffer)
    );
    assert_eq!(editor.pane_count(), 2);
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().any(
        |pane| matches!(pane.content, PaneContent::Terminal { terminal_id: id } if id == terminal_id)
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
    let mut editor = Editor::new(editor_id, doc, view);

    let terminal_id = editor.open_terminal_in_active_pane();
    assert_eq!(editor.pane_count(), 1);
    assert!(editor.is_active_pane_terminal());

    assert!(editor.set_active_buffer(0));
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::EditorBuffer)
    );
    assert_eq!(editor.pane_count(), 2);
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().any(
        |pane| matches!(pane.content, PaneContent::Terminal { terminal_id: id } if id == terminal_id)
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
    let mut editor = Editor::new(editor_id, doc, view);

    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.is_active_pane_terminal());

    let opened = editor.open_buffer(
      Rope::from("two"),
      view,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    assert_eq!(opened, 1);
    assert_eq!(editor.active_buffer_index(), 1);
    assert_eq!(
      editor.active_pane_content_kind(),
      Some(PaneContentKind::EditorBuffer)
    );
    assert_eq!(editor.pane_count(), 2);
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().any(
        |pane| matches!(pane.content, PaneContent::Terminal { terminal_id: id } if id == terminal_id)
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
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.set_active_buffer_in_pane(active_pane, 0));
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::EditorBuffer { buffer_index: 0 })
    );
    assert_eq!(
      editor.terminal_surface_snapshots(),
      vec![TerminalSurfaceSnapshot {
        terminal_id,
        attached_pane: None,
        is_active: false,
      }]
    );
    assert!(editor.focus_terminal_surface(terminal_id));
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::Terminal { terminal_id })
    );
  }

  #[test]
  fn editor_only_active_pane_detaches_non_active_terminal_surface() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.set_active_buffer(0));
    assert_eq!(editor.pane_count(), 2);
    assert!(editor.only_active_pane());
    assert_eq!(editor.pane_count(), 1);
    assert_eq!(
      editor.terminal_surface_snapshots(),
      vec![TerminalSurfaceSnapshot {
        terminal_id,
        attached_pane: None,
        is_active: false,
      }]
    );
    let panes = editor.frame_pane_snapshots(editor.layout_viewport());
    assert!(
      panes.iter().all(|pane| {
        !matches!(pane.content, PaneContent::Terminal { terminal_id: id } if id == terminal_id)
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

    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.set_active_buffer(0));
    assert!(editor.only_active_pane());
    assert!(editor.focus_terminal_surface(terminal_id));
    assert!(editor.is_active_pane_terminal());
    assert_eq!(editor.active_terminal_id(), Some(terminal_id));
    let snapshots = editor.terminal_surface_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].terminal_id, terminal_id);
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
    let terminal_id = editor.open_terminal_in_active_pane();
    assert!(editor.hide_active_terminal_surface());
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::EditorBuffer { buffer_index: 0 })
    );
    assert_eq!(
      editor.terminal_surface_snapshots(),
      vec![TerminalSurfaceSnapshot {
        terminal_id,
        attached_pane: None,
        is_active: false,
      }]
    );
    assert!(editor.focus_terminal_surface(terminal_id));
    assert_eq!(
      editor.pane_content(active_pane),
      Some(PaneContent::Terminal { terminal_id })
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

    editor.document_mut().set_display_name("one.rs");
    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let two_idx = editor.open_buffer(
      Rope::from("two"),
      ViewState::new(Rect::new(0, 0, 120, 40), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/project/src/two.rs")),
    );
    assert!(editor.set_active_buffer(two_idx));

    let surfaces = editor.editor_surface_snapshots();
    assert_eq!(surfaces.len(), 2);
    assert_eq!(surfaces.iter().filter(|surface| surface.is_active).count(), 1);
    assert!(surfaces.iter().any(|surface| surface.buffer_index == 0));
    assert!(surfaces.iter().any(|surface| surface.buffer_index == two_idx));
    assert!(surfaces.iter().any(|surface| {
      surface.buffer_index == two_idx
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

    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let second_idx = editor.open_buffer(
      Rope::from("two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    assert_eq!(editor.active_buffer_index(), second_idx);

    assert!(editor.rotate_active_pane(true));
    assert_eq!(editor.active_buffer_index(), 0);
    assert!(editor.rotate_active_pane(false));
    assert_eq!(editor.active_buffer_index(), second_idx);
  }

  #[test]
  fn editor_jump_active_pane_switches_to_neighbor_buffer() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Vertical));
    let right_idx = editor.open_buffer(
      Rope::from("right"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right.txt")),
    );
    assert_eq!(editor.active_buffer_index(), right_idx);

    assert!(editor.jump_active_pane(PaneDirection::Left));
    assert_eq!(editor.active_buffer_index(), 0);
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
    let right_top_idx = editor.open_buffer(
      Rope::from("right-top"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right-top.txt")),
    );
    assert_eq!(editor.active_buffer_index(), right_top_idx);

    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    let right_bottom_idx = editor.open_buffer(
      Rope::from("right-bottom"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/right-bottom.txt")),
    );
    assert_eq!(editor.active_buffer_index(), right_bottom_idx);

    assert!(editor.swap_active_pane(PaneDirection::Up));
    assert_eq!(editor.active_buffer_index(), right_bottom_idx);
    assert!(editor.jump_active_pane(PaneDirection::Down));
    assert_eq!(editor.active_buffer_index(), right_top_idx);
  }

  #[test]
  fn editor_split_and_new_scratch_flow_keeps_original_buffer_in_other_pane() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    let scratch_idx = editor.open_buffer(
      Rope::new(),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      None,
    );
    assert_eq!(editor.active_buffer_index(), scratch_idx);
    assert_eq!(editor.buffer_count(), 2);

    assert!(editor.jump_active_pane(PaneDirection::Up));
    assert_eq!(editor.active_buffer_index(), 0);
  }

  #[test]
  fn editor_can_reuse_unmodified_unshared_untitled_buffer_for_open() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::new());
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.can_reuse_active_untitled_buffer_for_open());

    let _ = editor.document_mut().replace_range(Range::point(0), "x");
    assert!(!editor.can_reuse_active_untitled_buffer_for_open());
  }

  #[test]
  fn editor_does_not_reuse_untitled_buffer_when_shared_across_panes() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::new());
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert!(editor.split_active_pane(SplitAxis::Horizontal));
    assert!(!editor.can_reuse_active_untitled_buffer_for_open());
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
    assert_eq!(editor.active_buffer_index(), 0);
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

    assert_eq!(b, 1);
    assert_eq!(c, 2);
    assert!(editor.close_buffer(1));
    assert_eq!(editor.buffer_count(), 2);
    assert_eq!(editor.active_buffer_index(), 1);
    assert_eq!(
      editor
        .buffer_snapshot(1)
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

    assert!(!editor.close_buffer(0));
    assert_eq!(editor.buffer_count(), 1);
  }

  #[test]
  fn editor_move_buffer_reorders_and_preserves_active_buffer_identity() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from_str("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

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
    assert_eq!(editor.active_buffer_index(), c);

    assert!(editor.move_buffer(c, 0));
    assert_eq!(editor.active_buffer_index(), 0);
    assert_eq!(
      editor
        .buffer_snapshot(0)
        .and_then(|s| s.file_path.map(|p| p.to_string_lossy().to_string())),
      Some("/tmp/three.txt".into())
    );
    assert_eq!(
      editor
        .buffer_snapshot(2)
        .and_then(|s| s.file_path.map(|p| p.to_string_lossy().to_string())),
      Some("/tmp/two.txt".into())
    );
    assert_eq!(b, 1);
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

    let _ = editor.document_mut().set_selection(Selection::point(1));
    assert!(editor.save_jump());

    let second_idx = editor.open_buffer(
      Rope::from("two two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    assert_eq!(editor.active_buffer_index(), second_idx);
    let _ = editor.document_mut().set_selection(Selection::point(4));
    assert!(editor.save_jump());
    let _ = editor.document_mut().set_selection(Selection::point(7));

    assert!(editor.jump_backward(1));
    assert_eq!(editor.active_buffer_index(), second_idx);
    assert_eq!(editor.document().selection().ranges()[0], Range::point(4));

    assert!(editor.jump_backward(1));
    assert_eq!(editor.active_buffer_index(), 0);
    assert_eq!(editor.document().selection().ranges()[0], Range::point(1));
  }

  #[test]
  fn editor_buffer_snapshots_mru_orders_active_then_recent() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    editor.open_buffer(
      Rope::from("two"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/two.txt")),
    );
    editor.open_buffer(
      Rope::from("three"),
      ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0)),
      Some(PathBuf::from("/tmp/three.txt")),
    );
    assert!(editor.set_active_buffer(1));

    let snapshots = editor.buffer_snapshots_mru();
    assert_eq!(snapshots.len(), 3);
    assert_eq!(snapshots[0].buffer_index, 1);
    assert_eq!(snapshots[1].buffer_index, 2);
    assert_eq!(snapshots[2].buffer_index, 0);
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
}
