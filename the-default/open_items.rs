use std::{
  collections::BTreeMap,
  path::{
    Path,
    PathBuf,
  },
};

use the_lib::{
  editor::{
    BufferId,
    ClientSurfaceId,
    Editor,
    PaneContent,
    PaneItemGroupSnapshot as EditorPaneItemGroupSnapshot,
  },
  split_tree::PaneId,
};

use crate::{
  command::DefaultContext,
  file_tree::FileTreeDecorations,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenItemKind {
  Buffer,
  Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneOpenItemsSnapshotOptions {
  pub include_directory_hint: bool,
}

impl Default for PaneOpenItemsSnapshotOptions {
  fn default() -> Self {
    Self {
      include_directory_hint: true,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneOpenItemSnapshot {
  pub kind:              OpenItemKind,
  pub buffer_id:         Option<BufferId>,
  pub client_surface_id: Option<ClientSurfaceId>,
  pub title:             String,
  pub subtitle:          Option<String>,
  pub file_path:         Option<PathBuf>,
  pub modified:          bool,
  pub decorations:       FileTreeDecorations,
  pub is_active:         bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneOpenItemGroupSnapshot {
  pub pane_id:        PaneId,
  pub is_active_pane: bool,
  pub active_index:   Option<usize>,
  pub items:          Vec<PaneOpenItemSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneOpenItemsSnapshot {
  pub visible: bool,
  pub groups:  Vec<PaneOpenItemGroupSnapshot>,
}

pub fn pane_open_items_snapshot<Ctx: DefaultContext>(ctx: &Ctx) -> PaneOpenItemsSnapshot {
  pane_open_items_snapshot_with_options(ctx, PaneOpenItemsSnapshotOptions::default())
}

pub fn pane_open_items_snapshot_with_options<Ctx: DefaultContext>(
  ctx: &Ctx,
  options: PaneOpenItemsSnapshotOptions,
) -> PaneOpenItemsSnapshot {
  pane_open_items_snapshot_for_editor_with_options(ctx.editor_ref(), options)
}

pub fn pane_open_items_snapshot_for_editor(editor: &Editor) -> PaneOpenItemsSnapshot {
  pane_open_items_snapshot_for_editor_with_options(editor, PaneOpenItemsSnapshotOptions::default())
}

pub fn pane_open_items_snapshot_for_editor_with_options(
  editor: &Editor,
  options: PaneOpenItemsSnapshotOptions,
) -> PaneOpenItemsSnapshot {
  let groups = editor
    .pane_item_snapshots()
    .into_iter()
    .map(|group| map_group_snapshot(editor, group, options))
    .collect::<Vec<_>>();

  PaneOpenItemsSnapshot {
    visible: !groups.is_empty(),
    groups,
  }
}

pub fn decorate_pane_open_items_snapshot(
  snapshot: &mut PaneOpenItemsSnapshot,
  decorations: &BTreeMap<PathBuf, FileTreeDecorations>,
) {
  for group in &mut snapshot.groups {
    for item in &mut group.items {
      item.decorations = item
        .file_path
        .as_ref()
        .and_then(|path| decorations.get(path).copied())
        .unwrap_or_default();
    }
  }
}

fn map_group_snapshot(
  editor: &Editor,
  group: EditorPaneItemGroupSnapshot,
  options: PaneOpenItemsSnapshotOptions,
) -> PaneOpenItemGroupSnapshot {
  let items = group
    .items
    .into_iter()
    .map(|item| map_item_snapshot(editor, item.content, item.is_active, options))
    .collect::<Vec<_>>();
  let active_index = items.iter().position(|item| item.is_active);

  PaneOpenItemGroupSnapshot {
    pane_id: group.pane_id,
    is_active_pane: group.is_active_pane,
    active_index,
    items,
  }
}

fn map_item_snapshot(
  editor: &Editor,
  content: PaneContent,
  is_active: bool,
  options: PaneOpenItemsSnapshotOptions,
) -> PaneOpenItemSnapshot {
  match content {
    PaneContent::EditorBuffer { buffer_id } => {
      let snapshot = editor
        .buffer_snapshot(buffer_id)
        .expect("pane item buffer must exist");
      let subtitle = if options.include_directory_hint {
        snapshot
          .file_path
          .as_deref()
          .and_then(directory_hint_for_path)
      } else {
        None
      };
      PaneOpenItemSnapshot {
        kind: OpenItemKind::Buffer,
        buffer_id: Some(buffer_id),
        client_surface_id: None,
        title: snapshot.display_name,
        subtitle,
        file_path: snapshot.file_path,
        modified: snapshot.modified,
        decorations: FileTreeDecorations::default(),
        is_active,
      }
    },
    PaneContent::ClientSurface { surface_id } => {
      PaneOpenItemSnapshot {
        kind: OpenItemKind::Terminal,
        buffer_id: None,
        client_surface_id: Some(surface_id),
        title: format!("terminal {}", surface_id.get().get()),
        subtitle: None,
        file_path: None,
        modified: false,
        decorations: FileTreeDecorations::default(),
        is_active,
      }
    },
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
    collections::BTreeMap,
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
      PaneContent,
    },
    position::Position,
    render::graphics::Rect,
    view::ViewState,
  };

  use super::{
    OpenItemKind,
    PaneOpenItemsSnapshotOptions,
    decorate_pane_open_items_snapshot,
    pane_open_items_snapshot_for_editor_with_options,
  };
  use crate::file_tree::{
    FileTreeDecorations,
    FileTreeVcsKind,
  };

  fn test_view() -> ViewState {
    ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0))
  }

  fn test_editor() -> Editor {
    let doc = Document::new(
      DocumentId::new(NonZeroUsize::new(1).unwrap()),
      Rope::from_str("one"),
    );
    Editor::new(
      EditorId::new(NonZeroUsize::new(1).unwrap()),
      doc,
      test_view(),
    )
  }

  #[test]
  fn pane_open_items_snapshot_groups_items_by_pane() {
    let mut editor = test_editor();
    editor.document_mut().set_display_name("one.rs");
    let _second = editor.open_buffer(
      Rope::from_str("two"),
      test_view(),
      Some(PathBuf::from("/tmp/proj/src/two.rs")),
    );
    let terminal = editor.open_terminal_in_active_pane();
    assert!(editor.hide_active_terminal_surface());
    assert!(editor.split_active_pane(the_lib::split_tree::SplitAxis::Vertical));

    let snapshot = pane_open_items_snapshot_for_editor_with_options(
      &editor,
      PaneOpenItemsSnapshotOptions::default(),
    );

    assert_eq!(snapshot.groups.len(), 2);
    assert_eq!(snapshot.groups[0].items.len(), 3);
    assert_eq!(snapshot.groups[0].items[0].kind, OpenItemKind::Buffer);
    assert_eq!(snapshot.groups[0].items[1].kind, OpenItemKind::Buffer);
    assert_eq!(snapshot.groups[0].items[2].kind, OpenItemKind::Terminal);
    assert_eq!(
      snapshot.groups[0].items[2].client_surface_id,
      Some(terminal)
    );
    assert_eq!(snapshot.groups[1].items.len(), 1);
    assert_eq!(
      snapshot.groups[1].items[0].buffer_id,
      snapshot.groups[0].items[1].buffer_id
    );
  }

  #[test]
  fn decorate_pane_open_items_snapshot_only_applies_path_decorations() {
    let mut editor = test_editor();
    let _second = editor.open_buffer(
      Rope::from_str("two"),
      test_view(),
      Some(PathBuf::from("/tmp/proj/src/two.rs")),
    );
    let _terminal = editor.open_terminal_in_active_pane();
    assert!(editor.hide_active_terminal_surface());

    let mut snapshot = pane_open_items_snapshot_for_editor_with_options(
      &editor,
      PaneOpenItemsSnapshotOptions::default(),
    );
    let mut decorations = BTreeMap::new();
    decorations.insert(PathBuf::from("/tmp/proj/src/two.rs"), FileTreeDecorations {
      vcs:        Some(FileTreeVcsKind::Modified),
      diagnostic: None,
    });

    decorate_pane_open_items_snapshot(&mut snapshot, &decorations);

    assert_eq!(
      snapshot.groups[0].items[0].decorations,
      FileTreeDecorations::default()
    );
    assert_eq!(
      snapshot.groups[0].items[1].decorations.vcs,
      Some(FileTreeVcsKind::Modified)
    );
    assert_eq!(snapshot.groups[0].items[2].kind, OpenItemKind::Terminal);
    assert_eq!(
      snapshot.groups[0].items[2].decorations,
      FileTreeDecorations::default()
    );
    assert_eq!(snapshot.groups[0].items[2].subtitle, None);
    assert_eq!(snapshot.groups[0].items[2].file_path, None);
    assert_eq!(
      snapshot.groups[0].items[2].title,
      format!(
        "terminal {}",
        match snapshot.groups[0].items[2].client_surface_id {
          Some(id) => id.get().get(),
          None => 0,
        }
      )
    );
    assert!(matches!(
      editor.active_pane_content(),
      Some(PaneContent::EditorBuffer { .. })
    ));
  }
}
