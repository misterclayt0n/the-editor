//! Input handling - maps key events to dispatch calls.

use std::path::{
  Path,
  PathBuf,
};

use crossterm::event::{
  KeyCode,
  KeyEvent as CrosstermKeyEvent,
  KeyEventKind,
  KeyModifiers,
  MouseButton,
  MouseEvent as CrosstermMouseEvent,
  MouseEventKind,
};
use ratatui::layout::Rect;
use the_default::{
  CommandPaletteItem,
  ContextMenuActionId,
  FileTreeContextMenuOptions,
  FileTreeContextMenuRequest,
  DefaultContext,
  FileTreeNodeKind,
  Mode,
  PointerButton as SharedPointerButton,
  PointerEvent,
  PointerEventOutcome,
  PointerKind,
  build_file_tree_context_menu_with_providers,
  build_file_tree_row_layouts_with_providers,
  close_signature_help,
  completion_docs_scroll,
  execute_file_tree_op,
  handle_pointer_event as dispatch_pointer_event,
  open_action_palette_with_items,
  open_command_palette_with_input,
  open_file_picker_index,
  scroll_file_picker_list,
  scroll_file_picker_preview,
  set_completion_docs_scroll,
  set_file_picker_list_offset,
  set_file_picker_preview_offset,
  signature_help_docs_scroll,
  ui_event as dispatch_ui_event,
};
use the_lib::{
  editor::{
    OpenTarget,
    PaneContentKind,
  },
  render::{
    graphics::Rect as LibRect,
    UiEvent,
    UiEventKind,
    UiKey,
    UiKeyEvent,
    UiModifiers,
  },
  split_tree::{
    PaneDirection,
    PaneId,
    SplitAxis,
    SplitNodeId,
  },
};

use crate::{
  Ctx,
  ctx::{
    CompletionDocsDragState,
    FilePickerDragState,
    PaneResizeDragState,
  },
  dispatch::{
    Key,
    KeyEvent,
    Modifiers,
  },
  docs_panel::DocsPanelSource,
  picker_layout::{
    compute_file_picker_layout,
    compute_scrollbar_metrics,
    point_in_rect,
    scroll_offset_from_thumb,
  },
};

/// Orchestration function - maps keyboard input to dispatch calls.
pub fn handle_key(ctx: &mut Ctx, event: CrosstermKeyEvent) {
  // Ignore key releases, but accept both press + repeat so held keys keep
  // driving navigation/scroll in overlays.
  if event.kind == KeyEventKind::Release {
    return;
  }

  // Pointer-driven scrolling should detach viewport follow until the next
  // keyboard action.
  ctx.mouse_viewport_detached = false;

  if matches!(event.code, KeyCode::Esc) && ctx.hover_docs.is_some() {
    ctx.hover_docs = None;
    ctx.hover_docs_scroll = 0;
    ctx.request_render();
  }

  if matches!(event.code, KeyCode::Esc) && ctx.signature_help.active {
    close_signature_help(ctx);
  }

  if ctx.hover_docs.is_some() && !ctx.completion_menu.active {
    match event.code {
      KeyCode::PageUp => {
        ctx.hover_docs_scroll = ctx.hover_docs_scroll.saturating_sub(6);
        ctx.request_render();
        return;
      },
      KeyCode::PageDown => {
        ctx.hover_docs_scroll = ctx.hover_docs_scroll.saturating_add(6);
        ctx.request_render();
        return;
      },
      _ => {},
    }
  }

  if ctx.signature_help.active && !ctx.completion_menu.active {
    match event.code {
      KeyCode::PageUp => {
        signature_help_docs_scroll(ctx, -6);
        return;
      },
      KeyCode::PageDown => {
        signature_help_docs_scroll(ctx, 6);
        return;
      },
      _ => {},
    }
  }

  if ctx.mode() == Mode::Normal
    && !ctx.file_picker.active
    && !ctx.completion_menu.active
    && !ctx.signature_help.active
    && ctx.hover_docs.is_none()
    && !event
      .modifiers
      .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
  {
    let normalized_code = normalize_shifted_key_code(event.code, event.modifiers);
    if handle_explorer_key(ctx, normalized_code) {
      return;
    }
  }

  if ctx.mode() == Mode::Command {
    let normalized_code = normalize_shifted_key_code(event.code, event.modifiers);
    if let Some(mut key) = to_ui_key(normalized_code) {
      if matches!(normalized_code, KeyCode::Tab | KeyCode::BackTab) {
        key = if event.modifiers.contains(KeyModifiers::SHIFT) || event.code == KeyCode::BackTab {
          UiKey::Up
        } else {
          UiKey::Down
        };
      }

      let ui_event = UiEvent {
        target: None,
        kind:   UiEventKind::Key(UiKeyEvent {
          key,
          modifiers: to_ui_modifiers(event.modifiers),
        }),
      };
      let _ = dispatch_ui_event(ctx, ui_event);
      return;
    }
  }

  let normalized_code = normalize_shifted_key_code(event.code, event.modifiers);
  let modifiers = to_modifiers(event.modifiers, normalized_code);
  let Some(key) = to_key(normalized_code) else {
    return;
  };

  let key_event = KeyEvent { key, modifiers };

  ctx.dispatch().pre_on_keypress(ctx, key_event);
}

pub fn handle_mouse(ctx: &mut Ctx, event: CrosstermMouseEvent) {
  let click_count = match event.kind {
    MouseEventKind::Down(MouseButton::Left) => {
      ctx.pointer_click_count_for_left_down(event.column, event.row)
    },
    _ => 0,
  };
  let Some(mut pointer_event) = crossterm_mouse_to_pointer_event(event) else {
    return;
  };
  if click_count != 0 {
    pointer_event = pointer_event.with_click_count(click_count);
  }
  let dispatch = ctx.dispatch();
  let _ = dispatch_pointer_event(&*dispatch, ctx, pointer_event);
}

pub(crate) fn handle_pointer_event(ctx: &mut Ctx, event: PointerEvent) -> PointerEventOutcome {
  let Some((x, y)) = pointer_event_coords(event) else {
    return PointerEventOutcome::Continue;
  };

  if ctx.file_picker.active {
    ctx.pane_resize_drag = None;
    let viewport = ctx.editor.view().viewport;
    let viewport = Rect::new(viewport.x, viewport.y, viewport.width, viewport.height);
    let layout = ctx
      .file_picker_layout
      .or_else(|| compute_file_picker_layout(viewport, &ctx.file_picker));
    let Some(layout) = layout else {
      return PointerEventOutcome::Handled;
    };
    ctx.file_picker_layout = Some(layout);

    match event.kind {
      PointerKind::Down(SharedPointerButton::Left) => {
        set_list_hover_from_position(ctx, layout, x, y);
        handle_left_down(ctx, layout, x, y);
      },
      PointerKind::Drag(SharedPointerButton::Left) => {
        handle_left_drag(ctx, layout, y);
      },
      PointerKind::Up(SharedPointerButton::Left) => {
        ctx.file_picker_drag = None;
      },
      PointerKind::Scroll => {
        let delta = pointer_scroll_delta_lines(event);
        if delta != 0 {
          handle_wheel(ctx, layout, x, y, delta);
        }
      },
      PointerKind::Move => {
        set_list_hover_from_position(ctx, layout, x, y);
      },
      _ => {},
    }
    return PointerEventOutcome::Handled;
  }

  let tab_rows = ctx.buffer_tabs_top_chrome_rows();
  if ctx.buffer_tab_drag.is_some() {
    let width = ctx.editor.layout_viewport().width.max(1);
    match event.kind {
      PointerKind::Drag(SharedPointerButton::Left) => {
        ctx.update_buffer_tab_hover(x, y, width);
        ctx.update_buffer_tab_drag(x, y, width);
        return PointerEventOutcome::Handled;
      },
      PointerKind::Up(SharedPointerButton::Left) => {
        if let Some((buffer_id, moved)) = ctx.finish_buffer_tab_drag(x, y, width)
          && !moved
        {
          let _ = ctx.activate_buffer_tab(buffer_id);
        }
        return PointerEventOutcome::Handled;
      },
      _ => {},
    }
  }
  if tab_rows > 0 && y < tab_rows {
    let width = ctx.editor.layout_viewport().width.max(1);
    match event.kind {
      PointerKind::Move => {
        ctx.update_buffer_tab_hover(x, y, width);
      },
      PointerKind::Down(SharedPointerButton::Left) => {
        ctx.update_buffer_tab_hover(x, y, width);
        if let Some(buffer_id) = ctx.buffer_tab_close_buffer_id_at(x, y, width) {
          let _ = ctx.close_buffer_tab(buffer_id);
          return PointerEventOutcome::Handled;
        }
        if let Some(slot) = ctx.buffer_tab_slot_at(x, y, width) {
          ctx.begin_buffer_tab_drag(slot, x);
        }
      },
      _ => {},
    }
    return PointerEventOutcome::Handled;
  }
  if tab_rows > 0 && matches!(event.kind, PointerKind::Move) {
    ctx.clear_buffer_tab_hover();
  }

  if handle_pane_resize_pointer(ctx, event.kind, x, y) {
    return PointerEventOutcome::Handled;
  }

  if let Some(layout) = ctx.completion_docs_layout {
    match event.kind {
      PointerKind::Down(SharedPointerButton::Left) => {
        handle_completion_docs_left_down(ctx, layout, x, y);
      },
      PointerKind::Drag(SharedPointerButton::Left) => {
        handle_completion_docs_drag(ctx, layout, y);
      },
      PointerKind::Move => {
        if ctx.completion_docs_drag.is_some() {
          handle_completion_docs_drag(ctx, layout, y);
        }
      },
      PointerKind::Up(SharedPointerButton::Left) => {
        ctx.completion_docs_drag = None;
      },
      PointerKind::Scroll => {
        let delta = pointer_scroll_delta_lines(event);
        if delta != 0 {
          handle_completion_docs_wheel(ctx, layout, x, y, delta);
        }
      },
      _ => {},
    }
    return PointerEventOutcome::Handled;
  }

  ctx.completion_docs_drag = None;
  if handle_explorer_pointer_event(ctx, event, x, y) {
    return PointerEventOutcome::Handled;
  }

  let overlay_active =
    ctx.completion_menu.active || ctx.hover_docs.is_some() || ctx.signature_help.active;
  if overlay_active {
    PointerEventOutcome::Handled
  } else {
    PointerEventOutcome::Continue
  }
}

#[derive(Debug, Clone, Copy)]
struct ExplorerPaneFrame {
  surface_id: the_lib::editor::ClientSurfaceId,
  pane_id:    PaneId,
  rect:       LibRect,
}

fn point_in_pane_rect(x: u16, y: u16, rect: LibRect) -> bool {
  x >= rect.x
    && x < rect.x.saturating_add(rect.width)
    && y >= rect.y
    && y < rect.y.saturating_add(rect.height)
}

fn active_explorer_pane_frame(ctx: &Ctx) -> Option<ExplorerPaneFrame> {
  let surface_id = ctx.active_explorer_surface_id()?;
  let pane_id = ctx.editor.active_pane_id();
  let rect = ctx.editor.pane_rect(pane_id)?;
  Some(ExplorerPaneFrame {
    surface_id,
    pane_id,
    rect,
  })
}

fn explorer_pane_frame_at(ctx: &Ctx, x: u16, y: u16) -> Option<ExplorerPaneFrame> {
  ctx
    .editor
    .frame_pane_snapshots(ctx.editor.layout_viewport())
    .into_iter()
    .find_map(|pane| {
      let surface_id = match pane.content {
        the_lib::editor::PaneContent::ClientSurface { surface_id }
          if ctx.explorer_surface(surface_id).is_some() => surface_id,
        _ => return None,
      };
      point_in_pane_rect(x, y, pane.rect).then_some(ExplorerPaneFrame {
        surface_id,
        pane_id: pane.pane_id,
        rect: pane.rect,
      })
    })
}

fn explorer_row_layouts_for_surface(
  ctx: &mut Ctx,
  frame: ExplorerPaneFrame,
) -> (usize, Vec<the_default::FileTreeRowLayout>) {
  let surface = ctx
    .explorer_surface(frame.surface_id)
    .cloned()
    .expect("explorer surface");
  let max_nodes = surface
    .scroll_offset
    .saturating_add(frame.rect.height as usize)
    .saturating_add(256)
    .max(512);
  let snapshot = ctx.file_tree.snapshot(max_nodes);
  let row_layouts = build_file_tree_row_layouts_with_providers(ctx, &snapshot);
  let max_scroll = row_layouts.len().saturating_sub(frame.rect.height as usize);
  let scroll_offset = surface.scroll_offset.min(max_scroll);
  if scroll_offset != surface.scroll_offset
    && let Some(state) = ctx.explorer_surface_mut(frame.surface_id)
  {
    state.scroll_offset = scroll_offset;
  }
  (scroll_offset, row_layouts)
}

fn normalize_explorer_selection(
  ctx: &mut Ctx,
  frame: ExplorerPaneFrame,
  request_render: bool,
) -> bool {
  let snapshot = ctx.file_tree.snapshot(usize::MAX);
  if snapshot.nodes.is_empty() {
    return false;
  }

  let selection_is_visible = ctx
    .file_tree
    .selected_path()
    .map(|selected| snapshot.nodes.iter().any(|node| node.path == selected))
    .unwrap_or(false);
  let changed = if selection_is_visible {
    false
  } else {
    let fallback = snapshot
      .active_path
      .clone()
      .filter(|path| snapshot.nodes.iter().any(|node| node.path == *path))
      .or_else(|| snapshot.nodes.first().map(|node| node.path.clone()));
    if let Some(path) = fallback {
      ctx.file_tree.select_path(&path)
    } else {
      false
    }
  };

  let selected = ctx.file_tree.selected_path().map(PathBuf::from);
  let selected_index = selected.and_then(|selected| {
    snapshot
      .nodes
      .iter()
      .position(|node| node.path == selected)
  });
  let Some(selected_index) = selected_index else {
    return changed;
  };

  let visible_rows = frame.rect.height.max(1) as usize;
  let mut scroll_changed = false;
  if let Some(surface) = ctx.explorer_surface_mut(frame.surface_id) {
    let max_scroll = snapshot.nodes.len().saturating_sub(visible_rows);
    let mut next_scroll = surface.scroll_offset.min(max_scroll);
    if selected_index < next_scroll {
      next_scroll = selected_index;
    } else if selected_index >= next_scroll.saturating_add(visible_rows) {
      next_scroll = selected_index.saturating_add(1).saturating_sub(visible_rows);
    }
    if next_scroll != surface.scroll_offset {
      surface.scroll_offset = next_scroll.min(max_scroll);
      scroll_changed = true;
    }
  }

  if request_render && (changed || scroll_changed) {
    ctx.request_render();
  }
  changed || scroll_changed
}

fn move_explorer_selection(ctx: &mut Ctx, frame: ExplorerPaneFrame, delta: isize) -> bool {
  let snapshot = ctx.file_tree.snapshot(usize::MAX);
  if snapshot.nodes.is_empty() {
    return false;
  }

  let current_index = ctx
    .file_tree
    .selected_path()
    .and_then(|selected| snapshot.nodes.iter().position(|node| node.path == selected));
  let next_index = match (current_index, delta.is_negative()) {
    (Some(index), true) => index.saturating_sub(delta.unsigned_abs()),
    (Some(index), false) => index
      .saturating_add(delta as usize)
      .min(snapshot.nodes.len().saturating_sub(1)),
    (None, true) => snapshot.nodes.len().saturating_sub(1),
    (None, false) => 0,
  };
  let changed = ctx.file_tree.select_path(&snapshot.nodes[next_index].path);
  let normalized = normalize_explorer_selection(ctx, frame, false);
  if changed || normalized {
    ctx.request_render();
  }
  changed || normalized
}

fn explorer_default_open_target(ctx: &Ctx, pane_id: PaneId) -> OpenTarget {
  let preferred = [PaneDirection::Right, PaneDirection::Left, PaneDirection::Down, PaneDirection::Up];
  if let Some(neighbors) = ctx.editor.pane_neighbors(pane_id) {
    for direction in preferred {
      if let Some(pane) = neighbors.in_direction(direction)
        && matches!(
          ctx.editor.pane_content_kind(pane),
          Some(PaneContentKind::EditorBuffer)
        )
      {
        return OpenTarget::Pane(pane);
      }
    }
  }

  OpenTarget::neighbor_or_split(PaneDirection::Right)
}

fn explorer_primary_action(ctx: &mut Ctx, frame: ExplorerPaneFrame) -> bool {
  let Some(op) = ctx
    .file_tree
    .open_selected_op(explorer_default_open_target(ctx, frame.pane_id))
  else {
    return false;
  };
  let changed = execute_file_tree_op(ctx, &op).is_ok();
  let normalized = normalize_explorer_selection(ctx, frame, false);
  if changed || normalized {
    ctx.request_render();
  }
  changed || normalized
}

fn explorer_select_parent_or_collapse(ctx: &mut Ctx, frame: ExplorerPaneFrame) -> bool {
  let Some(selected) = ctx.file_tree.selected_path().map(PathBuf::from) else {
    return move_explorer_selection(ctx, frame, 0);
  };

  if selected.is_dir() && ctx.file_tree.is_expanded(&selected) {
    if ctx.file_tree.set_expanded(&selected, false) {
      let _ = normalize_explorer_selection(ctx, frame, false);
      ctx.request_render();
      return true;
    }
  }

  let Some(root) = ctx.file_tree.root().map(PathBuf::from) else {
    return false;
  };
  let parent = selected.parent().map(Path::to_path_buf).unwrap_or(root.clone());
  if !parent.starts_with(&root) {
    return false;
  }
  let changed = ctx.file_tree.select_path(&parent);
  let normalized = normalize_explorer_selection(ctx, frame, false);
  if changed || normalized {
    ctx.request_render();
  }
  changed || normalized
}

fn explorer_sync_to_active_path(ctx: &mut Ctx, frame: ExplorerPaneFrame) -> bool {
  let working_directory = ctx.effective_working_directory();
  let active_path = ctx.editor.active_file_path().map(Path::to_path_buf);
  ctx
    .file_tree
    .sync_for_active_file(&working_directory, active_path.as_deref());
  normalize_explorer_selection(ctx, frame, true)
}

fn open_explorer_prompt(ctx: &mut Ctx, input: &str) -> bool {
  open_command_palette_with_input(ctx, input);
  true
}

fn command_palette_item_for_file_tree_action(
  item: the_default::ContextMenuItem,
) -> Option<CommandPaletteItem> {
  if !item.enabled {
    return None;
  }

  let mut palette_item = CommandPaletteItem::new(item.title.clone());
  if item.destructive {
    palette_item = palette_item.badge("delete").emphasize();
  }

  let palette_item = match item.id {
    ContextMenuActionId::FileTreeOpen => palette_item.on_typable_command("explorer-open", ""),
    ContextMenuActionId::FileTreeOpenSplitRight => {
      palette_item.on_typable_command("explorer-open-split-right", "")
    },
    ContextMenuActionId::FileTreeOpenSplitDown => {
      palette_item.on_typable_command("explorer-open-split-down", "")
    },
    ContextMenuActionId::FileTreeExpand => palette_item.on_typable_command("explorer-expand", ""),
    ContextMenuActionId::FileTreeCollapse => {
      palette_item.on_typable_command("explorer-collapse", "")
    },
    ContextMenuActionId::FileTreeNewFile => {
      palette_item.on_typable_command("explorer-new-file", "")
    },
    ContextMenuActionId::FileTreeNewFolder => {
      palette_item.on_typable_command("explorer-new-folder", "")
    },
    ContextMenuActionId::FileTreeRename => {
      palette_item.on_typable_command("explorer-rename", "")
    },
    ContextMenuActionId::FileTreeDelete => {
      palette_item.on_typable_command("explorer-delete", "")
    },
    ContextMenuActionId::FileTreeRefresh => {
      palette_item.on_typable_command("explorer-refresh", "")
    },
    _ => return None,
  };

  Some(palette_item)
}

fn open_explorer_context_menu(ctx: &mut Ctx, row: &the_default::FileTreeRowLayout) -> bool {
  let Some(root) = ctx.file_tree.root().map(Path::to_path_buf) else {
    return false;
  };

  let request = FileTreeContextMenuRequest {
    path: row.path.clone(),
    options: FileTreeContextMenuOptions {
      is_directory:      row.kind == FileTreeNodeKind::Directory,
      expanded:          row.kind == FileTreeNodeKind::Directory && ctx.file_tree.is_expanded(&row.path),
      is_workspace_root: row.path == root,
    },
  };
  let snapshot = build_file_tree_context_menu_with_providers(ctx, &request);
  let items = snapshot
    .sections
    .into_iter()
    .flat_map(|section| section.items)
    .filter_map(command_palette_item_for_file_tree_action)
    .collect::<Vec<_>>();
  if items.is_empty() {
    return false;
  }

  open_action_palette_with_items(ctx, Mode::Normal, items);
  true
}

fn handle_explorer_key(ctx: &mut Ctx, code: KeyCode) -> bool {
  let Some(frame) = active_explorer_pane_frame(ctx) else {
    return false;
  };

  match code {
    KeyCode::Char('j') | KeyCode::Down => move_explorer_selection(ctx, frame, 1),
    KeyCode::Char('k') | KeyCode::Up => move_explorer_selection(ctx, frame, -1),
    KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => explorer_primary_action(ctx, frame),
    KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
      explorer_select_parent_or_collapse(ctx, frame)
    },
    KeyCode::PageDown => move_explorer_selection(ctx, frame, frame.rect.height.max(1) as isize),
    KeyCode::PageUp => move_explorer_selection(ctx, frame, -(frame.rect.height.max(1) as isize)),
    KeyCode::Char('u') => {
      let op = ctx.file_tree.refresh_op();
      let changed = execute_file_tree_op(ctx, &op).is_ok();
      let normalized = normalize_explorer_selection(ctx, frame, false);
      if changed || normalized {
        ctx.request_render();
      }
      changed || normalized
    },
    KeyCode::Char('a') => open_explorer_prompt(ctx, "explorer-new-file "),
    KeyCode::Char('A') => open_explorer_prompt(ctx, "explorer-new-folder "),
    KeyCode::Char('r') => {
      let input = ctx
        .file_tree
        .selected_path()
        .and_then(Path::file_name)
        .map(|name| format!("explorer-rename {}", name.to_string_lossy()))
        .unwrap_or_else(|| "explorer-rename ".to_string());
      open_explorer_prompt(ctx, &input)
    },
    KeyCode::Char('d') => open_explorer_prompt(ctx, "explorer-delete "),
    KeyCode::Char('.') => explorer_sync_to_active_path(ctx, frame),
    KeyCode::Char('H') => {
      let changed = ctx.file_tree.toggle_show_hidden();
      let normalized = normalize_explorer_selection(ctx, frame, false);
      if changed || normalized {
        ctx.request_render();
      }
      changed || normalized
    },
    KeyCode::Char('I') => {
      let changed = ctx.file_tree.toggle_show_ignored();
      let normalized = normalize_explorer_selection(ctx, frame, false);
      if changed || normalized {
        ctx.request_render();
      }
      changed || normalized
    },
    KeyCode::Char('Z') => {
      let changed = ctx.file_tree.close_all(None);
      let normalized = normalize_explorer_selection(ctx, frame, false);
      if changed || normalized {
        ctx.request_render();
      }
      changed || normalized
    },
    _ => false,
  }
}

fn handle_explorer_pointer_event(
  ctx: &mut Ctx,
  event: PointerEvent,
  x: u16,
  y: u16,
) -> bool {
  let Some(frame) = explorer_pane_frame_at(ctx, x, y) else {
    return false;
  };

  let should_focus = matches!(
    event.kind,
    PointerKind::Down(SharedPointerButton::Left | SharedPointerButton::Right)
      | PointerKind::Scroll
  );
  if should_focus {
    let _ = ctx.focus_explorer_surface(frame.surface_id);
  }

  let (scroll_offset, row_layouts) = explorer_row_layouts_for_surface(ctx, frame);
  let row_index = scroll_offset.saturating_add(y.saturating_sub(frame.rect.y) as usize);
  let row = row_layouts.get(row_index).cloned();
  let disclosure_hit = row.as_ref().is_some_and(|row| {
    if row.kind != FileTreeNodeKind::Directory {
      return false;
    }
    let guide_width = row.guides.ancestor_columns.len() as u16 * 2
      + match row.guides.connector {
        the_default::FileTreeGuideConnector::None => 0,
        _ => 2,
      };
    let disclosure_x = frame.rect.x.saturating_add(guide_width);
    let disclosure_width = row.disclosure_glyph.chars().count() as u16;
    x >= disclosure_x && x < disclosure_x.saturating_add(disclosure_width.max(1))
  });

  match event.kind {
    PointerKind::Down(SharedPointerButton::Left) => {
      if let Some(row) = row {
        let changed = ctx.file_tree.select_path(&row.path);
        if disclosure_hit {
          let expanded = !ctx.file_tree.is_expanded(&row.path);
          let _ = ctx.file_tree.set_expanded(&row.path, expanded);
        } else if event.click_count >= 2 {
          let _ = explorer_primary_action(ctx, frame);
        }
        let normalized = normalize_explorer_selection(ctx, frame, false);
        if changed || normalized || disclosure_hit || event.click_count >= 2 {
          ctx.request_render();
        }
      }
      true
    },
    PointerKind::Down(SharedPointerButton::Right) => {
      if let Some(row) = row {
        let changed = ctx.file_tree.select_path(&row.path);
        let normalized = normalize_explorer_selection(ctx, frame, false);
        let opened_menu = open_explorer_context_menu(ctx, &row);
        if changed || normalized || opened_menu {
          ctx.request_render();
        }
      }
      true
    },
    PointerKind::Scroll => {
      let delta = pointer_scroll_delta_lines(event);
      if delta == 0 {
        return true;
      }
      let max_scroll = row_layouts.len().saturating_sub(frame.rect.height.max(1) as usize);
      if let Some(surface) = ctx.explorer_surface_mut(frame.surface_id) {
        let next = if delta.is_negative() {
          surface.scroll_offset.saturating_sub(delta.unsigned_abs())
        } else {
          surface.scroll_offset.saturating_add(delta as usize).min(max_scroll)
        };
        if next != surface.scroll_offset {
          surface.scroll_offset = next;
          ctx.request_render();
        }
      }
      true
    },
    PointerKind::Move => {
      if let Some(surface) = ctx.explorer_surface_mut(frame.surface_id) {
        let hovered = row.map(|_| row_index);
        if surface.hovered_row != hovered {
          surface.hovered_row = hovered;
          ctx.request_render();
        }
      }
      true
    },
    PointerKind::Up(SharedPointerButton::Left | SharedPointerButton::Right) => true,
    _ => true,
  }
}

fn handle_pane_resize_mouse(ctx: &mut Ctx, kind: MouseEventKind, x: u16, y: u16) -> bool {
  match kind {
    MouseEventKind::Down(MouseButton::Left) => {
      let Some(split_id) = hit_split_separator(ctx, x, y) else {
        return false;
      };
      ctx.pane_resize_drag = Some(PaneResizeDragState::Split { split_id });
      if ctx.editor.resize_split(split_id, x, y) {
        ctx.request_render();
      }
      true
    },
    MouseEventKind::Drag(MouseButton::Left) => {
      let Some(PaneResizeDragState::Split { split_id }) = ctx.pane_resize_drag else {
        return false;
      };
      if ctx.editor.resize_split(split_id, x, y) {
        ctx.request_render();
      }
      true
    },
    MouseEventKind::Moved => {
      let Some(PaneResizeDragState::Split { split_id }) = ctx.pane_resize_drag else {
        return false;
      };
      if ctx.editor.resize_split(split_id, x, y) {
        ctx.request_render();
      }
      true
    },
    MouseEventKind::Up(MouseButton::Left) => ctx.pane_resize_drag.take().is_some(),
    _ => false,
  }
}

fn handle_pane_resize_pointer(ctx: &mut Ctx, kind: PointerKind, x: u16, y: u16) -> bool {
  match kind {
    PointerKind::Down(SharedPointerButton::Left) => {
      let Some(split_id) = hit_split_separator(ctx, x, y) else {
        return false;
      };
      ctx.pane_resize_drag = Some(PaneResizeDragState::Split { split_id });
      if ctx.editor.resize_split(split_id, x, y) {
        ctx.request_render();
      }
      true
    },
    PointerKind::Drag(SharedPointerButton::Left) => {
      let Some(PaneResizeDragState::Split { split_id }) = ctx.pane_resize_drag else {
        return false;
      };
      if ctx.editor.resize_split(split_id, x, y) {
        ctx.request_render();
      }
      true
    },
    PointerKind::Move => {
      let Some(PaneResizeDragState::Split { split_id }) = ctx.pane_resize_drag else {
        return false;
      };
      if ctx.editor.resize_split(split_id, x, y) {
        ctx.request_render();
      }
      true
    },
    PointerKind::Up(SharedPointerButton::Left) => ctx.pane_resize_drag.take().is_some(),
    _ => false,
  }
}

fn hit_split_separator(ctx: &Ctx, x: u16, y: u16) -> Option<SplitNodeId> {
  const HIT_TOLERANCE: u16 = 1;
  let viewport = ctx.editor.layout_viewport();
  let mut closest: Option<(SplitNodeId, u16)> = None;

  for separator in ctx.editor.pane_separators(viewport) {
    let (distance, in_span) = match separator.axis {
      SplitAxis::Vertical => {
        (
          separator.line.abs_diff(x),
          y >= separator.span_start && y < separator.span_end,
        )
      },
      SplitAxis::Horizontal => {
        (
          separator.line.abs_diff(y),
          x >= separator.span_start && x < separator.span_end,
        )
      },
    };
    if !in_span || distance > HIT_TOLERANCE {
      continue;
    }
    if let Some((_, current_distance)) = closest {
      if distance < current_distance {
        closest = Some((separator.split_id, distance));
      }
    } else {
      closest = Some((separator.split_id, distance));
    }
  }

  closest.map(|(id, _)| id)
}

fn handle_left_down(ctx: &mut Ctx, layout: crate::picker_layout::FilePickerLayout, x: u16, y: u16) {
  let picker = &ctx.file_picker;

  if let Some(track) = layout.list_scrollbar_track
    && point_in_rect(x, y, track)
  {
    let visible_rows = layout.list_visible_rows();
    let total_matches = picker.matched_count();
    if let Some(metrics) = compute_scrollbar_metrics(
      track,
      total_matches,
      visible_rows,
      layout.list_scroll_offset,
    ) {
      let thumb_start = track.y.saturating_add(metrics.thumb_offset);
      let thumb_end = thumb_start.saturating_add(metrics.thumb_height);
      let mut grab_offset = y.saturating_sub(thumb_start);
      if y < thumb_start || y >= thumb_end {
        grab_offset = metrics.thumb_height / 2;
      }
      let clamped_y = y
        .saturating_sub(track.y)
        .saturating_sub(grab_offset)
        .min(metrics.max_thumb_offset);
      let offset = scroll_offset_from_thumb(metrics, clamped_y);
      set_file_picker_list_offset(ctx, offset);
      ctx.file_picker.hovered = None;
      ctx.file_picker_drag = Some(FilePickerDragState::ListScrollbar { grab_offset });
      return;
    }
  }

  if point_in_rect(x, y, layout.list_content) {
    let row = y.saturating_sub(layout.list_content.y) as usize;
    let index = layout.list_scroll_offset.saturating_add(row);
    let selectable = ctx
      .file_picker
      .matched_item(index)
      .as_deref()
      .is_some_and(|item| {
        !matches!(
          &item.action,
          the_default::FilePickerItemAction::GroupHeader { .. }
        )
      });
    if index < picker.matched_count() && selectable {
      open_file_picker_index(ctx, index);
      ctx.file_picker_drag = None;
      return;
    }
  }

  if let Some(track) = layout.preview_scrollbar
    && point_in_rect(x, y, track)
  {
    let visible_rows = layout.preview_visible_rows();
    let total_lines = picker.preview_line_count();
    if let Some(metrics) = compute_scrollbar_metrics(
      track,
      total_lines,
      visible_rows,
      layout.preview_scroll_offset,
    ) {
      let thumb_start = track.y.saturating_add(metrics.thumb_offset);
      let thumb_end = thumb_start.saturating_add(metrics.thumb_height);
      let mut grab_offset = y.saturating_sub(thumb_start);
      if y < thumb_start || y >= thumb_end {
        grab_offset = metrics.thumb_height / 2;
      }
      let clamped_y = y
        .saturating_sub(track.y)
        .saturating_sub(grab_offset)
        .min(metrics.max_thumb_offset);
      let offset = scroll_offset_from_thumb(metrics, clamped_y);
      set_file_picker_preview_offset(ctx, offset, visible_rows);
      ctx.file_picker.hovered = None;
      ctx.file_picker_drag = Some(FilePickerDragState::PreviewScrollbar { grab_offset });
      return;
    }
  }

  ctx.file_picker.hovered = None;
  ctx.file_picker_drag = None;
}

fn handle_left_drag(ctx: &mut Ctx, layout: crate::picker_layout::FilePickerLayout, y: u16) {
  let Some(drag) = ctx.file_picker_drag else {
    return;
  };

  match drag {
    FilePickerDragState::ListScrollbar { grab_offset } => {
      let Some(track) = layout.list_scrollbar_track else {
        return;
      };
      let visible_rows = layout.list_visible_rows();
      let total_matches = ctx.file_picker.matched_count();
      let Some(metrics) = compute_scrollbar_metrics(
        track,
        total_matches,
        visible_rows,
        layout.list_scroll_offset,
      ) else {
        return;
      };
      let thumb_offset = y
        .saturating_sub(track.y)
        .saturating_sub(grab_offset)
        .min(metrics.max_thumb_offset);
      let offset = scroll_offset_from_thumb(metrics, thumb_offset);
      set_file_picker_list_offset(ctx, offset);
    },
    FilePickerDragState::PreviewScrollbar { grab_offset } => {
      let Some(track) = layout.preview_scrollbar else {
        return;
      };
      let visible_rows = layout.preview_visible_rows();
      let total_lines = ctx.file_picker.preview_line_count();
      let Some(metrics) = compute_scrollbar_metrics(
        track,
        total_lines,
        visible_rows,
        layout.preview_scroll_offset,
      ) else {
        return;
      };
      let thumb_offset = y
        .saturating_sub(track.y)
        .saturating_sub(grab_offset)
        .min(metrics.max_thumb_offset);
      let offset = scroll_offset_from_thumb(metrics, thumb_offset);
      set_file_picker_preview_offset(ctx, offset, visible_rows);
    },
  }
}

fn handle_wheel(
  ctx: &mut Ctx,
  layout: crate::picker_layout::FilePickerLayout,
  x: u16,
  y: u16,
  delta: isize,
) {
  if point_in_rect(x, y, layout.list_content)
    || layout
      .list_scrollbar_track
      .is_some_and(|track| point_in_rect(x, y, track))
  {
    scroll_file_picker_list(ctx, delta);
    set_list_hover_from_position(ctx, layout, x, y);
    return;
  }

  if let Some(preview_content) = layout.preview_content
    && (point_in_rect(x, y, preview_content)
      || layout
        .preview_scrollbar
        .is_some_and(|track| point_in_rect(x, y, track)))
  {
    ctx.file_picker.hovered = None;
    scroll_file_picker_preview(ctx, delta, layout.preview_visible_rows());
  }
}

fn set_list_hover_from_position(
  ctx: &mut Ctx,
  layout: crate::picker_layout::FilePickerLayout,
  x: u16,
  y: u16,
) {
  let next_hover = if point_in_rect(x, y, layout.list_content) {
    let row = y.saturating_sub(layout.list_content.y) as usize;
    let index = layout.list_scroll_offset.saturating_add(row);
    (index < ctx.file_picker.matched_count()).then_some(index)
  } else {
    None
  };

  if ctx.file_picker.hovered != next_hover {
    ctx.file_picker.hovered = next_hover;
    ctx.request_render();
  }
}

fn completion_docs_metrics(
  ctx: &Ctx,
  layout: crate::picker_layout::CompletionDocsLayout,
) -> Option<crate::picker_layout::ScrollbarMetrics> {
  let scroll = docs_scroll_for_source(ctx, layout.source);
  let track = layout.scrollbar_track?;
  compute_scrollbar_metrics(track, layout.total_rows, layout.visible_rows.max(1), scroll)
}

fn docs_scrollbar_hit_rect(layout: crate::picker_layout::CompletionDocsLayout) -> Option<Rect> {
  let track = layout.scrollbar_track?;
  let hit_x = track.x.saturating_sub(1);
  let hit_w = track.width.saturating_add(track.x.saturating_sub(hit_x));
  Some(Rect::new(hit_x, track.y, hit_w.max(1), track.height))
}

fn docs_scroll_for_source(ctx: &Ctx, source: DocsPanelSource) -> usize {
  match source {
    DocsPanelSource::Completion => ctx.completion_menu.docs_scroll,
    DocsPanelSource::Hover => ctx.hover_docs_scroll,
    DocsPanelSource::Signature => ctx.signature_help.docs_scroll,
    DocsPanelSource::CommandPalette => 0,
  }
}

fn set_docs_scroll_for_source(
  ctx: &mut Ctx,
  layout: crate::picker_layout::CompletionDocsLayout,
  scroll: usize,
) {
  let max_scroll = layout.total_rows.saturating_sub(layout.visible_rows);
  let scroll = scroll.min(max_scroll);
  match layout.source {
    DocsPanelSource::Completion => set_completion_docs_scroll(ctx, scroll),
    DocsPanelSource::Hover => {
      if ctx.hover_docs_scroll != scroll {
        ctx.hover_docs_scroll = scroll;
        ctx.request_render();
      }
    },
    DocsPanelSource::Signature => {
      if ctx.signature_help.docs_scroll != scroll {
        ctx.signature_help.docs_scroll = scroll;
        ctx.request_render();
      }
    },
    DocsPanelSource::CommandPalette => {},
  }
}

fn scroll_docs_for_source(
  ctx: &mut Ctx,
  layout: crate::picker_layout::CompletionDocsLayout,
  delta: isize,
) {
  match layout.source {
    DocsPanelSource::Completion => completion_docs_scroll(ctx, delta),
    DocsPanelSource::Hover => {
      let max_scroll = layout.total_rows.saturating_sub(layout.visible_rows);
      let next = if delta.is_negative() {
        ctx.hover_docs_scroll.saturating_sub(delta.unsigned_abs())
      } else {
        ctx.hover_docs_scroll.saturating_add(delta as usize)
      };
      let next = next.min(max_scroll);
      if ctx.hover_docs_scroll != next {
        ctx.hover_docs_scroll = next;
        ctx.request_render();
      }
    },
    DocsPanelSource::Signature => {
      signature_help_docs_scroll(ctx, delta);
    },
    DocsPanelSource::CommandPalette => {},
  }
}

fn handle_completion_docs_left_down(
  ctx: &mut Ctx,
  layout: crate::picker_layout::CompletionDocsLayout,
  x: u16,
  y: u16,
) {
  let Some(track) = layout.scrollbar_track else {
    ctx.completion_docs_drag = None;
    return;
  };
  let in_grab_zone = docs_scrollbar_hit_rect(layout).is_some_and(|hit| point_in_rect(x, y, hit));
  if !in_grab_zone {
    ctx.completion_docs_drag = None;
    return;
  }

  let Some(metrics) = completion_docs_metrics(ctx, layout) else {
    ctx.completion_docs_drag = None;
    return;
  };

  let thumb_start = track.y.saturating_add(metrics.thumb_offset);
  let thumb_end = thumb_start.saturating_add(metrics.thumb_height);
  let mut grab_offset = y.saturating_sub(thumb_start);
  if y < thumb_start || y >= thumb_end {
    grab_offset = metrics.thumb_height / 2;
  }
  let clamped_y = y
    .saturating_sub(track.y)
    .saturating_sub(grab_offset)
    .min(metrics.max_thumb_offset);
  let scroll = scroll_offset_from_thumb(metrics, clamped_y);
  set_docs_scroll_for_source(ctx, layout, scroll);
  ctx.completion_docs_drag = Some(CompletionDocsDragState::Scrollbar { grab_offset });
}

fn handle_completion_docs_drag(
  ctx: &mut Ctx,
  layout: crate::picker_layout::CompletionDocsLayout,
  y: u16,
) {
  let Some(CompletionDocsDragState::Scrollbar { grab_offset }) = ctx.completion_docs_drag else {
    return;
  };
  let Some(track) = layout.scrollbar_track else {
    return;
  };
  let Some(metrics) = completion_docs_metrics(ctx, layout) else {
    return;
  };

  let thumb_offset = y
    .saturating_sub(track.y)
    .saturating_sub(grab_offset)
    .min(metrics.max_thumb_offset);
  let scroll = scroll_offset_from_thumb(metrics, thumb_offset);
  set_docs_scroll_for_source(ctx, layout, scroll);
}

fn handle_completion_docs_wheel(
  ctx: &mut Ctx,
  layout: crate::picker_layout::CompletionDocsLayout,
  x: u16,
  y: u16,
  delta: isize,
) {
  let in_content = point_in_rect(x, y, layout.content);
  let in_scrollbar = docs_scrollbar_hit_rect(layout).is_some_and(|hit| point_in_rect(x, y, hit));
  if in_content || in_scrollbar {
    scroll_docs_for_source(ctx, layout, delta);
  }
}

fn to_key(code: KeyCode) -> Option<Key> {
  match code {
    KeyCode::Char(c) => Some(Key::Char(c)),
    KeyCode::Enter => Some(Key::Enter),
    KeyCode::Tab => Some(Key::Tab),
    KeyCode::BackTab => Some(Key::Tab),
    KeyCode::Esc => Some(Key::Escape),
    KeyCode::Backspace => Some(Key::Backspace),
    KeyCode::Delete => Some(Key::Delete),
    KeyCode::Insert => Some(Key::Insert),
    KeyCode::Home => Some(Key::Home),
    KeyCode::End => Some(Key::End),
    KeyCode::PageUp => Some(Key::PageUp),
    KeyCode::PageDown => Some(Key::PageDown),
    KeyCode::Left => Some(Key::Left),
    KeyCode::Right => Some(Key::Right),
    KeyCode::Up => Some(Key::Up),
    KeyCode::Down => Some(Key::Down),
    KeyCode::F(1) => Some(Key::F1),
    KeyCode::F(2) => Some(Key::F2),
    KeyCode::F(3) => Some(Key::F3),
    KeyCode::F(4) => Some(Key::F4),
    KeyCode::F(5) => Some(Key::F5),
    KeyCode::F(6) => Some(Key::F6),
    KeyCode::F(7) => Some(Key::F7),
    KeyCode::F(8) => Some(Key::F8),
    KeyCode::F(9) => Some(Key::F9),
    KeyCode::F(10) => Some(Key::F10),
    KeyCode::F(11) => Some(Key::F11),
    KeyCode::F(12) => Some(Key::F12),
    _ => None,
  }
}

fn normalize_shifted_key_code(code: KeyCode, modifiers: KeyModifiers) -> KeyCode {
  if !modifiers.contains(KeyModifiers::SHIFT) {
    return code;
  }

  match code {
    KeyCode::Char(ch) => {
      let shifted = match ch {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '`' => '~',
        _ => ch,
      };
      KeyCode::Char(shifted)
    },
    other => other,
  }
}

fn to_ui_key(code: KeyCode) -> Option<UiKey> {
  match code {
    KeyCode::Char(c) => Some(UiKey::Char(c)),
    KeyCode::Enter => Some(UiKey::Enter),
    KeyCode::Tab => Some(UiKey::Tab),
    KeyCode::BackTab => Some(UiKey::Tab),
    KeyCode::Esc => Some(UiKey::Escape),
    KeyCode::Backspace => Some(UiKey::Backspace),
    KeyCode::Delete => Some(UiKey::Delete),
    KeyCode::Home => Some(UiKey::Home),
    KeyCode::End => Some(UiKey::End),
    KeyCode::PageUp => Some(UiKey::PageUp),
    KeyCode::PageDown => Some(UiKey::PageDown),
    KeyCode::Left => Some(UiKey::Left),
    KeyCode::Right => Some(UiKey::Right),
    KeyCode::Up => Some(UiKey::Up),
    KeyCode::Down => Some(UiKey::Down),
    _ => None,
  }
}

fn to_modifiers(modifiers: KeyModifiers, code: KeyCode) -> Modifiers {
  let mut out = Modifiers::empty();
  if modifiers.contains(KeyModifiers::CONTROL) {
    out.insert(Modifiers::CTRL);
  }
  if modifiers.contains(KeyModifiers::ALT) {
    out.insert(Modifiers::ALT);
  }
  if modifiers.contains(KeyModifiers::SHIFT) {
    // Don't include SHIFT for characters that are inherently shifted
    // (uppercase letters, symbols produced by shift+number, etc.)
    // The shift is already represented in the character itself.
    let dominated_by_char =
      matches!(code, KeyCode::Char(c) if c.is_uppercase() || "~!@#$%^&*()_+{}|:\"<>?".contains(c));
    if !dominated_by_char {
      out.insert(Modifiers::SHIFT);
    }
  }
  out
}

fn to_ui_modifiers(modifiers: KeyModifiers) -> UiModifiers {
  UiModifiers {
    ctrl:  modifiers.contains(KeyModifiers::CONTROL),
    alt:   modifiers.contains(KeyModifiers::ALT),
    shift: modifiers.contains(KeyModifiers::SHIFT),
    meta:  modifiers.contains(KeyModifiers::SUPER),
  }
}

fn to_pointer_modifiers(modifiers: KeyModifiers) -> Modifiers {
  let mut out = Modifiers::empty();
  if modifiers.contains(KeyModifiers::CONTROL) {
    out.insert(Modifiers::CTRL);
  }
  if modifiers.contains(KeyModifiers::ALT) {
    out.insert(Modifiers::ALT);
  }
  if modifiers.contains(KeyModifiers::SHIFT) {
    out.insert(Modifiers::SHIFT);
  }
  out
}

fn crossterm_mouse_to_pointer_event(event: CrosstermMouseEvent) -> Option<PointerEvent> {
  let kind = match event.kind {
    MouseEventKind::Down(MouseButton::Left) => PointerKind::Down(SharedPointerButton::Left),
    MouseEventKind::Down(MouseButton::Middle) => PointerKind::Down(SharedPointerButton::Middle),
    MouseEventKind::Down(MouseButton::Right) => PointerKind::Down(SharedPointerButton::Right),
    MouseEventKind::Drag(MouseButton::Left) => PointerKind::Drag(SharedPointerButton::Left),
    MouseEventKind::Drag(MouseButton::Middle) => PointerKind::Drag(SharedPointerButton::Middle),
    MouseEventKind::Drag(MouseButton::Right) => PointerKind::Drag(SharedPointerButton::Right),
    MouseEventKind::Up(MouseButton::Left) => PointerKind::Up(SharedPointerButton::Left),
    MouseEventKind::Up(MouseButton::Middle) => PointerKind::Up(SharedPointerButton::Middle),
    MouseEventKind::Up(MouseButton::Right) => PointerKind::Up(SharedPointerButton::Right),
    MouseEventKind::Moved => PointerKind::Move,
    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => PointerKind::Scroll,
    _ => return None,
  };

  let mut pointer = PointerEvent::new(kind, i32::from(event.column), i32::from(event.row))
    .with_logical_pos(event.column, event.row)
    .with_modifiers(to_pointer_modifiers(event.modifiers));

  pointer = match event.kind {
    MouseEventKind::ScrollUp => pointer.with_scroll_delta(0.0, -3.0),
    MouseEventKind::ScrollDown => pointer.with_scroll_delta(0.0, 3.0),
    _ => pointer,
  };

  Some(pointer)
}

fn pointer_event_coords(event: PointerEvent) -> Option<(u16, u16)> {
  let x = event.logical_col.or_else(|| {
    if event.x < 0 {
      None
    } else {
      Some((event.x.min(i32::from(u16::MAX))) as u16)
    }
  })?;
  let y = event.logical_row.or_else(|| {
    if event.y < 0 {
      None
    } else {
      Some((event.y.min(i32::from(u16::MAX))) as u16)
    }
  })?;
  Some((x, y))
}

fn pointer_scroll_delta_lines(event: PointerEvent) -> isize {
  if event.scroll_y > 0.0 {
    event.scroll_y.round() as isize
  } else if event.scroll_y < 0.0 {
    event.scroll_y.round() as isize
  } else {
    0
  }
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    path::{
      Path,
      PathBuf,
    },
    time::SystemTime,
  };

  use crossterm::event::{
    KeyCode,
    KeyEvent,
    KeyModifiers,
    MouseEvent,
  };
  use the_default::{
    CommandEvent,
    CommandPaletteSource,
    CommandRegistry,
    CompletionMenuItem,
    DefaultContext,
    submit_command_palette,
    show_completion_menu,
  };
  use the_lib::{
    selection::Selection,
    split_tree::SplitAxis,
    transaction::Transaction,
  };

  use super::*;
  use crate::dispatch::build_dispatch;

  struct TempExplorerDir {
    path: PathBuf,
  }

  impl TempExplorerDir {
    fn new(prefix: &str) -> Self {
      let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
      let path = std::env::temp_dir().join(format!(
        "the-editor-input-explorer-{prefix}-{}-{nonce}",
        std::process::id(),
      ));
      fs::create_dir_all(&path).expect("create temp explorer dir");
      Self { path }
    }

    fn as_path(&self) -> &Path {
      &self.path
    }

    fn write_file(&self, relative: &str, content: &str) -> PathBuf {
      let path = self.path.join(relative);
      if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create explorer dir parent");
      }
      fs::write(&path, content).expect("write explorer temp file");
      path
    }
  }

  impl Drop for TempExplorerDir {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.path);
    }
  }

  fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
      kind,
      column,
      row,
      modifiers: KeyModifiers::empty(),
    }
  }

  fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
  }

  fn key_event_with_kind(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
    let mut event = key_event(code);
    event.kind = kind;
    event
  }

  #[test]
  fn space_shift_slash_opens_action_palette() {
    let dispatch = build_dispatch::<Ctx>();
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_dispatch(&dispatch);

    handle_key(&mut ctx, key_event(KeyCode::Char(' ')));

    let mut shifted_slash = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::SHIFT);
    shifted_slash.kind = KeyEventKind::Press;
    handle_key(&mut ctx, shifted_slash);

    assert!(ctx.command_palette.is_open);
    assert!(matches!(
      ctx.command_palette.source,
      CommandPaletteSource::ActionPalette
    ));
  }

  #[test]
  fn normalize_shifted_slash_into_question_mark() {
    let code = normalize_shifted_key_code(KeyCode::Char('/'), KeyModifiers::SHIFT);
    assert_eq!(to_key(code), Some(Key::Char('?')));
    assert!(!to_modifiers(KeyModifiers::SHIFT, code).shift());
  }

  #[test]
  fn normalize_shifted_semicolon_into_colon() {
    let code = normalize_shifted_key_code(KeyCode::Char(';'), KeyModifiers::SHIFT);
    assert_eq!(to_key(code), Some(Key::Char(':')));
    assert!(!to_modifiers(KeyModifiers::SHIFT, code).shift());
  }

  #[test]
  fn completion_docs_wheel_scrolls_when_pointer_is_inside_docs() {
    let mut ctx = Ctx::new(None).expect("ctx");
    show_completion_menu(&mut ctx, vec![CompletionMenuItem::new("item")]);
    ctx.completion_docs_layout = Some(crate::picker_layout::CompletionDocsLayout {
      panel:           Rect::new(0, 0, 20, 8),
      content:         Rect::new(1, 1, 18, 6),
      scrollbar_track: Some(Rect::new(19, 1, 1, 6)),
      visible_rows:    6,
      total_rows:      24,
      source:          DocsPanelSource::Completion,
    });

    handle_mouse(&mut ctx, mouse_event(MouseEventKind::ScrollDown, 2, 2));
    assert_eq!(ctx.completion_menu.docs_scroll, 3);
  }

  #[test]
  fn completion_docs_scrollbar_drag_updates_scroll_offset() {
    let mut ctx = Ctx::new(None).expect("ctx");
    show_completion_menu(&mut ctx, vec![CompletionMenuItem::new("item")]);
    ctx.completion_docs_layout = Some(crate::picker_layout::CompletionDocsLayout {
      panel:           Rect::new(0, 0, 20, 8),
      content:         Rect::new(1, 1, 18, 6),
      scrollbar_track: Some(Rect::new(19, 1, 1, 6)),
      visible_rows:    6,
      total_rows:      30,
      source:          DocsPanelSource::Completion,
    });

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Down(MouseButton::Left), 19, 6),
    );
    assert!(ctx.completion_docs_drag.is_some());
    let after_down = ctx.completion_menu.docs_scroll;
    assert!(after_down > 0);

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Drag(MouseButton::Left), 19, 6),
    );
    assert!(ctx.completion_menu.docs_scroll >= after_down);
  }

  #[test]
  fn hover_docs_wheel_scrolls_hover_state() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs = Some("hover docs".to_string());
    ctx.hover_docs_scroll = 0;
    ctx.completion_docs_layout = Some(crate::picker_layout::CompletionDocsLayout {
      panel:           Rect::new(0, 0, 20, 8),
      content:         Rect::new(1, 1, 18, 6),
      scrollbar_track: Some(Rect::new(19, 1, 1, 6)),
      visible_rows:    6,
      total_rows:      24,
      source:          DocsPanelSource::Hover,
    });

    handle_mouse(&mut ctx, mouse_event(MouseEventKind::ScrollDown, 2, 2));

    assert_eq!(ctx.hover_docs_scroll, 3);
    assert_eq!(ctx.completion_menu.docs_scroll, 0);
  }

  #[test]
  fn hover_docs_scrollbar_drag_updates_scroll_offset() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs = Some("hover docs".to_string());
    ctx.hover_docs_scroll = 0;
    ctx.completion_docs_layout = Some(crate::picker_layout::CompletionDocsLayout {
      panel:           Rect::new(0, 0, 20, 8),
      content:         Rect::new(1, 1, 18, 6),
      scrollbar_track: Some(Rect::new(19, 1, 1, 6)),
      visible_rows:    6,
      total_rows:      30,
      source:          DocsPanelSource::Hover,
    });

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Down(MouseButton::Left), 19, 6),
    );
    assert!(ctx.completion_docs_drag.is_some());
    let after_down = ctx.hover_docs_scroll;
    assert!(after_down > 0);

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Drag(MouseButton::Left), 19, 6),
    );
    assert!(ctx.hover_docs_scroll >= after_down);
  }

  #[test]
  fn hover_docs_scrollbar_drag_starts_from_adjacent_grab_zone() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs = Some("hover docs".to_string());
    ctx.hover_docs_scroll = 0;
    ctx.completion_docs_layout = Some(crate::picker_layout::CompletionDocsLayout {
      panel:           Rect::new(0, 0, 20, 8),
      content:         Rect::new(1, 1, 18, 6),
      scrollbar_track: Some(Rect::new(19, 1, 1, 6)),
      visible_rows:    6,
      total_rows:      30,
      source:          DocsPanelSource::Hover,
    });

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Down(MouseButton::Left), 18, 6),
    );
    assert!(ctx.completion_docs_drag.is_some());
  }

  #[test]
  fn completion_docs_moved_updates_scroll_when_drag_is_active() {
    let mut ctx = Ctx::new(None).expect("ctx");
    show_completion_menu(&mut ctx, vec![CompletionMenuItem::new("item")]);
    ctx.completion_docs_layout = Some(crate::picker_layout::CompletionDocsLayout {
      panel:           Rect::new(0, 0, 20, 8),
      content:         Rect::new(1, 1, 18, 6),
      scrollbar_track: Some(Rect::new(19, 1, 1, 6)),
      visible_rows:    6,
      total_rows:      30,
      source:          DocsPanelSource::Completion,
    });

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Down(MouseButton::Left), 19, 2),
    );
    let after_down = ctx.completion_menu.docs_scroll;
    handle_mouse(&mut ctx, mouse_event(MouseEventKind::Moved, 19, 6));
    assert!(ctx.completion_menu.docs_scroll >= after_down);
  }

  #[test]
  fn escape_clears_hover_docs_overlay() {
    let dispatch = build_dispatch::<Ctx>();
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_dispatch(&dispatch);
    ctx.mode = Mode::Command;
    ctx.hover_docs = Some("hover docs".to_string());
    ctx.hover_docs_scroll = 5;
    ctx.needs_render = false;

    handle_key(&mut ctx, key_event(KeyCode::Esc));

    assert!(ctx.hover_docs.is_none());
    assert_eq!(ctx.hover_docs_scroll, 0);
    assert!(ctx.needs_render);
  }

  #[test]
  fn hover_docs_page_down_repeat_continues_scrolling() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs = Some("hover docs".to_string());
    ctx.hover_docs_scroll = 0;

    handle_key(&mut ctx, key_event(KeyCode::PageDown));
    assert_eq!(ctx.hover_docs_scroll, 6);

    handle_key(
      &mut ctx,
      key_event_with_kind(KeyCode::PageDown, KeyEventKind::Repeat),
    );
    assert_eq!(ctx.hover_docs_scroll, 12);
  }

  #[test]
  fn repeat_down_event_repeats_normal_mode_movement() {
    let dispatch = build_dispatch::<Ctx>();
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_dispatch(&dispatch);

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("one\ntwo\nthree\n".into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let _ = ctx.editor.document_mut().set_selection(Selection::point(0));

    let cursor_line = |ctx: &Ctx| {
      let doc = ctx.editor.document();
      let text = doc.text().slice(..);
      text.char_to_line(doc.selection().ranges()[0].cursor(text))
    };
    assert_eq!(cursor_line(&ctx), 0);

    handle_key(
      &mut ctx,
      key_event_with_kind(KeyCode::Down, KeyEventKind::Press),
    );
    assert_eq!(cursor_line(&ctx), 1);

    handle_key(
      &mut ctx,
      key_event_with_kind(KeyCode::Down, KeyEventKind::Repeat),
    );
    assert_eq!(cursor_line(&ctx), 2);
  }

  #[test]
  fn pane_separator_drag_resizes_horizontal_split() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(120, 40);
    assert!(ctx.editor.split_active_pane(SplitAxis::Horizontal));

    let viewport = ctx.editor.layout_viewport();
    let separator = ctx
      .editor
      .pane_separators(viewport)
      .into_iter()
      .find(|separator| separator.axis == SplitAxis::Horizontal)
      .expect("horizontal separator");
    let drag_x = separator.span_start + (separator.span_end - separator.span_start) / 2;

    handle_mouse(
      &mut ctx,
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        drag_x,
        separator.line,
      ),
    );
    assert!(ctx.pane_resize_drag.is_some());

    let target_y = separator.line.saturating_add(4);
    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Drag(MouseButton::Left), drag_x, target_y),
    );

    let moved = ctx
      .editor
      .pane_separators(viewport)
      .into_iter()
      .find(|next| next.split_id == separator.split_id)
      .expect("updated separator");
    assert!(moved.line > separator.line);

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Up(MouseButton::Left), drag_x, target_y),
    );
    assert!(ctx.pane_resize_drag.is_none());
  }

  #[test]
  fn pane_separator_drag_resizes_vertical_split() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(120, 40);
    assert!(ctx.editor.split_active_pane(SplitAxis::Vertical));

    let viewport = ctx.editor.layout_viewport();
    let separator = ctx
      .editor
      .pane_separators(viewport)
      .into_iter()
      .find(|separator| separator.axis == SplitAxis::Vertical)
      .expect("vertical separator");
    let drag_y = separator.span_start + (separator.span_end - separator.span_start) / 2;

    handle_mouse(
      &mut ctx,
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        separator.line,
        drag_y,
      ),
    );
    assert!(ctx.pane_resize_drag.is_some());

    let target_x = separator.line.saturating_add(6);
    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Drag(MouseButton::Left), target_x, drag_y),
    );

    let moved = ctx
      .editor
      .pane_separators(viewport)
      .into_iter()
      .find(|next| next.split_id == separator.split_id)
      .expect("updated separator");
    assert!(moved.line > separator.line);
  }

  #[test]
  fn explorer_keyboard_navigation_selects_visible_rows() {
    let dir = TempExplorerDir::new("keyboard-nav");
    dir.write_file("alpha.txt", "alpha\n");
    dir.write_file("nested/beta.txt", "beta\n");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));

    let snapshot = ctx.file_tree.snapshot(usize::MAX);
    assert!(snapshot.nodes.len() >= 2);

    handle_key(&mut ctx, key_event(KeyCode::Char('j')));
    assert_eq!(
      ctx.file_tree.selected_path(),
      Some(snapshot.nodes[0].path.as_path())
    );

    handle_key(&mut ctx, key_event(KeyCode::Char('j')));
    assert_eq!(
      ctx.file_tree.selected_path(),
      Some(snapshot.nodes[1].path.as_path())
    );
  }

  #[test]
  fn explorer_enter_opens_file_in_neighbor_or_split() {
    let dir = TempExplorerDir::new("enter-open");
    let alpha = dir.write_file("alpha.txt", "alpha\n");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));
    assert!(ctx.file_tree.select_path(&alpha));

    handle_key(&mut ctx, key_event(KeyCode::Enter));

    assert_eq!(
      ctx
        .editor
        .active_file_path()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str()),
      alpha.file_name().and_then(|name| name.to_str())
    );
    assert_eq!(
      ctx.editor.active_pane_content_kind(),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );
    assert_eq!(ctx.explorer_surface_snapshots().len(), 1);
    assert!(ctx.editor.pane_count() >= 2);
  }

  #[test]
  fn explorer_double_click_opens_selected_file() {
    let dir = TempExplorerDir::new("double-click");
    let alpha = dir.write_file("alpha.txt", "alpha\n");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(48, 12);
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));

    let surface_id = ctx.active_explorer_surface_id().expect("explorer surface");
    let pane = ctx
      .editor
      .frame_pane_snapshots(ctx.editor.layout_viewport())
      .into_iter()
      .find(|pane| {
        matches!(
          pane.content,
          the_lib::editor::PaneContent::ClientSurface { surface_id: id } if id == surface_id
        )
      })
      .expect("explorer pane");
    let snapshot = ctx.file_tree.snapshot(usize::MAX);
    let row_index = snapshot
      .nodes
      .iter()
      .position(|node| node.name == "alpha.txt")
      .expect("alpha row");
    let x = pane.rect.x.saturating_add(6);
    let y = pane.rect.y.saturating_add(row_index as u16);

    handle_mouse(&mut ctx, mouse_event(MouseEventKind::Down(MouseButton::Left), x, y));
    handle_mouse(&mut ctx, mouse_event(MouseEventKind::Down(MouseButton::Left), x, y));

    assert_eq!(
      ctx
        .editor
        .active_file_path()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str()),
      alpha.file_name().and_then(|name| name.to_str())
    );
    assert_eq!(
      ctx.editor.active_pane_content_kind(),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );
  }

  #[test]
  fn explorer_mouse_wheel_scrolls_surface() {
    let dir = TempExplorerDir::new("wheel-scroll");
    for index in 0..32 {
      dir.write_file(&format!("item-{index:02}.txt"), "item\n");
    }

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(48, 8);
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));

    let surface_id = ctx.active_explorer_surface_id().expect("explorer surface");
    let pane = ctx
      .editor
      .frame_pane_snapshots(ctx.editor.layout_viewport())
      .into_iter()
      .find(|pane| {
        matches!(
          pane.content,
          the_lib::editor::PaneContent::ClientSurface { surface_id: id } if id == surface_id
        )
      })
      .expect("explorer pane");

    assert_eq!(
      ctx.explorer_surface(surface_id).map(|surface| surface.scroll_offset),
      Some(0)
    );
    handle_mouse(
      &mut ctx,
      mouse_event(
        MouseEventKind::ScrollDown,
        pane.rect.x.saturating_add(2),
        pane.rect.y.saturating_add(2),
      ),
    );
    assert!(
      ctx
        .explorer_surface(surface_id)
        .is_some_and(|surface| surface.scroll_offset > 0)
    );
  }

  #[test]
  fn explorer_key_prompts_open_command_palette_for_operations() {
    let dir = TempExplorerDir::new("prompt-keys");
    dir.write_file("alpha.txt", "alpha\n");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));

    handle_key(&mut ctx, key_event(KeyCode::Char('a')));
    assert!(ctx.command_palette.is_open);
    assert!(matches!(ctx.command_palette.source, CommandPaletteSource::CommandLine));
    assert_eq!(ctx.command_prompt_ref().input, "explorer-new-file ");

    ctx.set_mode(Mode::Normal);
    handle_key(&mut ctx, key_event(KeyCode::Char('d')));
    assert_eq!(ctx.command_prompt_ref().input, "explorer-delete ");
  }

  #[test]
  fn explorer_new_file_command_creates_file() {
    let dir = TempExplorerDir::new("new-file-command");
    dir.write_file("alpha.txt", "alpha\n");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));

    let registry: *const CommandRegistry<Ctx> = ctx.command_registry_ref();
    let result = unsafe {
      (&*registry).execute(
        &mut ctx,
        "explorer-new-file",
        "nested/created.txt",
        CommandEvent::Validate,
      )
    };
    assert!(result.is_ok());
    assert!(dir.as_path().join("nested/created.txt").exists());
  }

  #[test]
  fn explorer_rename_and_delete_commands_operate_on_selected_path() {
    let dir = TempExplorerDir::new("rename-delete");
    let alpha = dir.write_file("alpha.txt", "alpha\n");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));
    assert!(ctx.file_tree.select_path(&alpha));

    let registry: *const CommandRegistry<Ctx> = ctx.command_registry_ref();
    let rename_result = unsafe {
      (&*registry).execute(
        &mut ctx,
        "explorer-rename",
        "renamed.txt",
        CommandEvent::Validate,
      )
    };
    assert!(rename_result.is_ok());
    let renamed = dir.as_path().join("renamed.txt");
    assert!(renamed.exists());
    assert!(!alpha.exists());

    let reject_delete = unsafe {
      (&*registry).execute(
        &mut ctx,
        "explorer-delete",
        "nope",
        CommandEvent::Validate,
      )
    };
    assert!(reject_delete.is_err());
    assert!(renamed.exists());

    let accept_delete = unsafe {
      (&*registry).execute(
        &mut ctx,
        "explorer-delete",
        "DELETE",
        CommandEvent::Validate,
      )
    };
    assert!(accept_delete.is_ok());
    assert!(!renamed.exists());
  }

  #[test]
  fn explorer_right_click_opens_context_action_palette() {
    let dir = TempExplorerDir::new("context-menu");
    dir.write_file("alpha.txt", "alpha\n");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(48, 12);
    ctx.working_directory.current = Some(dir.as_path().to_path_buf());
    assert!(<Ctx as DefaultContext>::open_native_file_explorer(&mut ctx, false));

    let surface_id = ctx.active_explorer_surface_id().expect("explorer surface");
    let pane = ctx
      .editor
      .frame_pane_snapshots(ctx.editor.layout_viewport())
      .into_iter()
      .find(|pane| {
        matches!(
          pane.content,
          the_lib::editor::PaneContent::ClientSurface { surface_id: id } if id == surface_id
        )
      })
      .expect("explorer pane");
    let snapshot = ctx.file_tree.snapshot(usize::MAX);
    let row_index = snapshot
      .nodes
      .iter()
      .position(|node| node.name == "alpha.txt")
      .expect("alpha row");
    let x = pane.rect.x.saturating_add(6);
    let y = pane.rect.y.saturating_add(row_index as u16);

    handle_mouse(&mut ctx, mouse_event(MouseEventKind::Down(MouseButton::Right), x, y));

    assert!(ctx.command_palette.is_open);
    assert!(matches!(ctx.command_palette.source, CommandPaletteSource::ActionPalette));
    assert!(ctx.command_palette.items.iter().any(|item| item.title == "Open"));
    assert!(ctx.command_palette.items.iter().any(|item| item.title == "Delete"));

    let delete_index = ctx
      .command_palette
      .items
      .iter()
      .position(|item| item.title == "Delete")
      .expect("delete item");
    ctx.command_palette.selected = Some(delete_index);
    assert!(submit_command_palette(&mut ctx));
    assert!(ctx.command_palette.is_open);
    assert_eq!(ctx.command_prompt_ref().input, "explorer-delete ");
  }
}
