use std::path::{
  Path,
  PathBuf,
};

use the_lib::editor::{
  BufferSnapshot as EditorBufferSnapshot,
  Editor,
};

use crate::command::DefaultContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BufferTabsOrder {
  #[default]
  Natural,
  Mru,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferTabsSnapshotOptions {
  pub order:                BufferTabsOrder,
  pub min_tabs_to_show:     usize,
  pub include_directory_hint: bool,
}

impl Default for BufferTabsSnapshotOptions {
  fn default() -> Self {
    Self {
      order: BufferTabsOrder::Natural,
      min_tabs_to_show: 2,
      include_directory_hint: true,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferTabItemSnapshot {
  pub buffer_index:    usize,
  pub title:           String,
  pub modified:        bool,
  pub is_active:       bool,
  pub file_path:       Option<PathBuf>,
  pub directory_hint:  Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferTabsSnapshot {
  pub visible:             bool,
  pub order:               BufferTabsOrder,
  pub active_tab:          Option<usize>,
  pub active_buffer_index: Option<usize>,
  pub tabs:                Vec<BufferTabItemSnapshot>,
}

pub fn buffer_tabs_snapshot<Ctx: DefaultContext>(ctx: &Ctx) -> BufferTabsSnapshot {
  buffer_tabs_snapshot_with_options(ctx, BufferTabsSnapshotOptions::default())
}

pub fn activate_buffer_tab<Ctx: DefaultContext>(ctx: &mut Ctx, buffer_index: usize) -> bool {
  ctx.activate_buffer_by_index(buffer_index)
}

pub fn buffer_tabs_snapshot_with_options<Ctx: DefaultContext>(
  ctx: &Ctx,
  options: BufferTabsSnapshotOptions,
) -> BufferTabsSnapshot {
  buffer_tabs_snapshot_for_editor_with_options(ctx.editor_ref(), options)
}

pub fn buffer_tabs_snapshot_for_editor(editor: &Editor) -> BufferTabsSnapshot {
  buffer_tabs_snapshot_for_editor_with_options(editor, BufferTabsSnapshotOptions::default())
}

pub fn buffer_tabs_snapshot_for_editor_with_options(
  editor: &Editor,
  options: BufferTabsSnapshotOptions,
) -> BufferTabsSnapshot {
  let raw_tabs = match options.order {
    BufferTabsOrder::Natural => (0..editor.buffer_count())
      .filter_map(|index| editor.buffer_snapshot(index))
      .collect::<Vec<_>>(),
    BufferTabsOrder::Mru => editor.buffer_snapshots_mru(),
  };

  let tabs = raw_tabs
    .iter()
    .map(|tab| map_buffer_snapshot(tab, options.include_directory_hint))
    .collect::<Vec<_>>();

  let active_tab = tabs.iter().position(|tab| tab.is_active);
  let active_buffer_index = tabs.get(active_tab.unwrap_or(usize::MAX)).map(|tab| tab.buffer_index);
  let visible = tabs.len() >= options.min_tabs_to_show.max(1);

  BufferTabsSnapshot {
    visible,
    order: options.order,
    active_tab,
    active_buffer_index,
    tabs,
  }
}

fn map_buffer_snapshot(
  snapshot: &EditorBufferSnapshot,
  include_directory_hint: bool,
) -> BufferTabItemSnapshot {
  let directory_hint = if include_directory_hint {
    snapshot
      .file_path
      .as_deref()
      .and_then(directory_hint_for_path)
  } else {
    None
  };

  BufferTabItemSnapshot {
    buffer_index: snapshot.buffer_index,
    title: snapshot.display_name.clone(),
    modified: snapshot.modified,
    is_active: snapshot.is_active,
    file_path: snapshot.file_path.clone(),
    directory_hint,
  }
}

fn directory_hint_for_path(path: &Path) -> Option<String> {
  path
    .parent()
    .and_then(|parent| parent.file_name())
    .and_then(|name| name.to_str())
    .map(str::to_owned)
}

#[cfg(test)]
mod tests {
  use std::{
    num::NonZeroUsize,
    path::PathBuf,
  };

  use ropey::Rope;
  use the_lib::{
    document::{
      Document,
      DocumentId,
    },
    editor::{
      Editor,
      EditorId,
    },
    position::Position,
    render::graphics::Rect,
    view::ViewState,
  };

  use super::{
    BufferTabsOrder,
    BufferTabsSnapshotOptions,
    buffer_tabs_snapshot_for_editor_with_options,
  };

  fn test_view() -> ViewState {
    ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0))
  }

  fn test_editor() -> Editor {
    let doc = Document::new(DocumentId::new(NonZeroUsize::new(1).unwrap()), Rope::from_str("one"));
    Editor::new(EditorId::new(NonZeroUsize::new(1).unwrap()), doc, test_view())
  }

  fn open_named_buffer(editor: &mut Editor, name: &str, path: Option<&str>) -> usize {
    let idx = editor.open_buffer(
      Rope::from_str(name),
      test_view(),
      path.map(PathBuf::from),
    );
    editor.document_mut().set_display_name(name);
    idx
  }

  #[test]
  fn buffer_tabs_snapshot_hides_with_single_tab_by_default() {
    let editor = test_editor();
    let snapshot = buffer_tabs_snapshot_for_editor_with_options(
      &editor,
      BufferTabsSnapshotOptions::default(),
    );
    assert_eq!(snapshot.tabs.len(), 1);
    assert!(!snapshot.visible);
    assert_eq!(snapshot.active_tab, Some(0));
    assert_eq!(snapshot.active_buffer_index, Some(0));
  }

  #[test]
  fn buffer_tabs_snapshot_uses_natural_order() {
    let mut editor = test_editor();
    editor.document_mut().set_display_name("a.rs");
    let _ = open_named_buffer(&mut editor, "b.rs", Some("/tmp/proj/src/b.rs"));
    let c = open_named_buffer(&mut editor, "c.rs", Some("/tmp/proj/tests/c.rs"));
    assert!(editor.set_active_buffer(c));

    let snapshot = buffer_tabs_snapshot_for_editor_with_options(
      &editor,
      BufferTabsSnapshotOptions {
        order: BufferTabsOrder::Natural,
        ..BufferTabsSnapshotOptions::default()
      },
    );

    let ids: Vec<usize> = snapshot.tabs.iter().map(|tab| tab.buffer_index).collect();
    assert_eq!(ids, vec![0, 1, 2]);
    assert_eq!(snapshot.active_tab, Some(2));
    assert_eq!(snapshot.active_buffer_index, Some(2));
    assert_eq!(snapshot.tabs[1].directory_hint.as_deref(), Some("src"));
    assert_eq!(snapshot.tabs[2].directory_hint.as_deref(), Some("tests"));
  }

  #[test]
  fn buffer_tabs_snapshot_supports_mru_order() {
    let mut editor = test_editor();
    editor.document_mut().set_display_name("a.rs");
    let b = open_named_buffer(&mut editor, "b.rs", None);
    let c = open_named_buffer(&mut editor, "c.rs", None);

    assert!(editor.set_active_buffer(0));
    assert!(editor.set_active_buffer(b));
    assert!(editor.set_active_buffer(c));

    let snapshot = buffer_tabs_snapshot_for_editor_with_options(
      &editor,
      BufferTabsSnapshotOptions {
        order: BufferTabsOrder::Mru,
        ..BufferTabsSnapshotOptions::default()
      },
    );

    let ids: Vec<usize> = snapshot.tabs.iter().map(|tab| tab.buffer_index).collect();
    assert_eq!(ids[0], c);
    assert_eq!(snapshot.active_tab, Some(0));
    assert_eq!(snapshot.active_buffer_index, Some(c));
    assert!(ids.contains(&b));
    assert!(ids.contains(&0));
  }

  #[test]
  fn buffer_tabs_snapshot_can_disable_directory_hints() {
    let mut editor = test_editor();
    editor.document_mut().set_display_name("a.rs");
    let _ = open_named_buffer(&mut editor, "b.rs", Some("/tmp/proj/src/b.rs"));

    let snapshot = buffer_tabs_snapshot_for_editor_with_options(
      &editor,
      BufferTabsSnapshotOptions {
        include_directory_hint: false,
        ..BufferTabsSnapshotOptions::default()
      },
    );

    assert!(snapshot.tabs.iter().all(|tab| tab.directory_hint.is_none()));
  }
}
