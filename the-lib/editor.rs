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
  pane_buffers:      BTreeMap<PaneId, usize>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferSnapshot {
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
  fn buffer_snapshot_for_index(&self, index: usize) -> Option<BufferSnapshot> {
    let buffer = self.buffers.get(index)?;
    Some(BufferSnapshot {
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
    let mut pane_buffers = BTreeMap::new();
    pane_buffers.insert(split_tree.active_pane(), 0);

    Self {
      id,
      buffers: vec![BufferState::new(document, view, None)],
      active_buffer: 0,
      layout_viewport: view.viewport,
      split_tree,
      pane_buffers,
      next_document_id,
      access_history: Vec::new(),
      modified_history: Vec::new(),
      jumplist_backward: Vec::new(),
      jumplist_forward: Vec::new(),
    }
  }

  fn activate_buffer(&mut self, index: usize) -> bool {
    if index >= self.buffers.len() {
      return false;
    }
    if index != self.active_buffer {
      self.access_history.push(self.active_buffer);
      self.active_buffer = index;
    }
    let active_pane = self.split_tree.active_pane();
    self.pane_buffers.insert(active_pane, index);
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
    let Some(&buffer) = self.pane_buffers.get(&pane) else {
      return false;
    };
    self.active_buffer = buffer;
    true
  }

  pub fn pane_snapshots(&self, area: Rect) -> Vec<PaneSnapshot> {
    let active = self.split_tree.active_pane();
    self
      .split_tree
      .layout(area)
      .into_iter()
      .filter_map(|(pane_id, rect)| {
        self
          .pane_buffers
          .get(&pane_id)
          .copied()
          .map(|buffer_index| {
            PaneSnapshot {
              pane_id,
              buffer_index,
              rect,
              is_active_pane: pane_id == active,
            }
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
    self.pane_buffers.get(&pane).copied()
  }

  pub fn buffer_view(&self, index: usize) -> Option<ViewState> {
    self.buffers.get(index).map(|buffer| buffer.view)
  }

  pub fn buffer_document(&self, index: usize) -> Option<&Document> {
    self.buffers.get(index).map(|buffer| &buffer.document)
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
    let current_buffer = self.active_buffer;
    let pane = self.split_tree.split_active(axis);
    self.pane_buffers.insert(pane, current_buffer);
    self.active_buffer = current_buffer;
    true
  }

  pub fn close_active_pane(&mut self) -> bool {
    let closing = self.split_tree.active_pane();
    let Ok(next_active) = self.split_tree.close_active() else {
      return false;
    };

    self.pane_buffers.remove(&closing);
    let Some(&next_buffer) = self.pane_buffers.get(&next_active) else {
      return false;
    };
    self.active_buffer = next_buffer;
    true
  }

  pub fn only_active_pane(&mut self) -> bool {
    if self.split_tree.pane_count() <= 1 {
      return false;
    }
    let active = self.split_tree.active_pane();
    let Some(&active_buffer) = self.pane_buffers.get(&active) else {
      return false;
    };

    self.split_tree.only_active();
    self.pane_buffers.retain(|pane, _| *pane == active);
    self.active_buffer = active_buffer;
    true
  }

  pub fn rotate_active_pane(&mut self, next: bool) -> bool {
    if !self.split_tree.rotate_focus(next) {
      return false;
    }
    let active = self.split_tree.active_pane();
    let Some(&buffer) = self.pane_buffers.get(&active) else {
      return false;
    };
    self.active_buffer = buffer;
    true
  }

  pub fn jump_active_pane(&mut self, direction: PaneDirection) -> bool {
    if !self.split_tree.jump_active(direction) {
      return false;
    }
    let active = self.split_tree.active_pane();
    let Some(&buffer) = self.pane_buffers.get(&active) else {
      return false;
    };
    self.active_buffer = buffer;
    true
  }

  pub fn swap_active_pane(&mut self, direction: PaneDirection) -> bool {
    self.split_tree.swap_active(direction)
  }

  pub fn transpose_active_pane_branch(&mut self) -> bool {
    self.split_tree.transpose_active_branch()
  }

  pub fn set_active_buffer(&mut self, index: usize) -> bool {
    self.activate_buffer(index)
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
