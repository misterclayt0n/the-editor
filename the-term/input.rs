//! Input handling - maps key events to dispatch calls.

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
  DefaultContext,
  Mode,
  PointerButton as SharedPointerButton,
  PointerEvent,
  PointerEventOutcome,
  PointerKind,
  activate_file_tree_index,
  close_signature_help,
  completion_docs_scroll,
  handle_command_prompt_key,
  handle_file_tree_key,
  handle_pointer_event as dispatch_pointer_event,
  open_file_picker_index,
  scroll_file_picker_list,
  scroll_file_picker_preview,
  scroll_file_tree,
  select_file_tree_index,
  set_completion_docs_scroll,
  set_file_picker_list_offset,
  set_file_picker_preview_offset,
  signature_help_docs_scroll,
};
use the_lib::split_tree::{
  SplitAxis,
  SplitNodeId,
};

use crate::{
  Ctx,
  ctx::{
    CompletionDocsDragState,
    FilePickerDragState,
    FileTreeLayout,
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

  let normalized_code = normalize_shifted_key_code(event.code, event.modifiers);
  let modifiers = to_modifiers(event.modifiers, normalized_code);
  let Some(key) = to_key(normalized_code) else {
    return;
  };

  let key_event = KeyEvent { key, modifiers };

  if ctx.mode() == Mode::Command {
    let command_key = match event.code {
      KeyCode::Tab => {
        Some(KeyEvent {
          key:       Key::Down,
          modifiers: Modifiers::empty(),
        })
      },
      KeyCode::BackTab => {
        Some(KeyEvent {
          key:       Key::Up,
          modifiers: Modifiers::empty(),
        })
      },
      _ => None,
    };
    if let Some(command_key) = command_key {
      if handle_command_prompt_key(ctx, command_key) {
        return;
      }
    } else if handle_command_prompt_key(ctx, key_event) {
      return;
    }
  }

  if handle_file_tree_term_key(ctx, key_event) {
    return;
  }

  if handle_file_tree_key(ctx, key_event) {
    return;
  }

  the_default::handle_key(ctx, key_event);
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
  let _ = dispatch_pointer_event(ctx, pointer_event);
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

  let Some(layout) = ctx.completion_docs_layout else {
    ctx.completion_docs_drag = None;
    let overlay_active =
      ctx.completion_menu.active || ctx.hover_docs.is_some() || ctx.signature_help.active;
    return if overlay_active {
      PointerEventOutcome::Handled
    } else if let Some(outcome) = handle_file_tree_pointer(ctx, event, x, y) {
      outcome
    } else {
      PointerEventOutcome::Continue
    };
  };

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
  PointerEventOutcome::Handled
}

fn handle_file_tree_pointer(
  ctx: &mut Ctx,
  event: PointerEvent,
  x: u16,
  y: u16,
) -> Option<PointerEventOutcome> {
  let layout = ctx.file_tree_layout?;
  if !point_in_rect(x, y, layout.pane) {
    return None;
  }

  let previous_buffer_id = ctx.editor.active_buffer_id();
  let pane_changed = if ctx.editor.active_pane_id() != layout.pane_id {
    let changed = ctx.editor.set_active_pane(layout.pane_id);
    if changed {
      ctx.did_change_active_pane(previous_buffer_id);
    }
    changed
  } else {
    false
  };

  let mut should_render = pane_changed;
  match event.kind {
    PointerKind::Down(SharedPointerButton::Left) => {
      if let Some(index) = file_tree_index_at(layout, x, y, ctx.file_tree.rows.len()) {
        should_render |= select_file_tree_index(ctx, index);
        if event.click_count >= 2 {
          should_render |= activate_file_tree_index(ctx, index, None);
        }
      }
    },
    PointerKind::Scroll => {
      let delta = pointer_scroll_delta_lines(event);
      if delta != 0 {
        should_render |= scroll_file_tree(ctx, delta, layout.visible_rows);
      }
    },
    PointerKind::Move
    | PointerKind::Up(SharedPointerButton::Left)
    | PointerKind::Drag(SharedPointerButton::Left) => {},
    _ => {},
  }

  if should_render {
    ctx.request_render();
  }
  Some(PointerEventOutcome::Handled)
}

fn handle_file_tree_term_key(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if !the_default::is_active_file_tree(ctx) || ctx.mode() == Mode::Command || ctx.file_picker.active
  {
    return false;
  }

  let outcome = match key.key {
    Key::Char(']') => select_next_file_tree_row(ctx, |row| row.decorations.vcs.is_some()),
    Key::Char('[') => select_prev_file_tree_row(ctx, |row| row.decorations.vcs.is_some()),
    Key::Char('}') => {
      select_next_file_tree_row(ctx, |row| row.decorations.diagnostic.is_some())
    },
    Key::Char('{') => {
      select_prev_file_tree_row(ctx, |row| row.decorations.diagnostic.is_some())
    },
    _ => false,
  };

  if outcome {
    ctx.request_render();
  }
  outcome
}

fn select_next_file_tree_row(
  ctx: &mut Ctx,
  predicate: impl Fn(&the_default::FileTreeRow) -> bool,
) -> bool {
  let len = ctx.file_tree.rows.len();
  if len == 0 {
    return false;
  }
  let start = ctx.file_tree.selected.unwrap_or(0);
  for step in 1..=len {
    let index = (start + step) % len;
    if predicate(&ctx.file_tree.rows[index]) {
      return select_file_tree_index(ctx, index);
    }
  }
  false
}

fn select_prev_file_tree_row(
  ctx: &mut Ctx,
  predicate: impl Fn(&the_default::FileTreeRow) -> bool,
) -> bool {
  let len = ctx.file_tree.rows.len();
  if len == 0 {
    return false;
  }
  let start = ctx.file_tree.selected.unwrap_or(0);
  for step in 1..=len {
    let index = (start + len - (step % len)) % len;
    if predicate(&ctx.file_tree.rows[index]) {
      return select_file_tree_index(ctx, index);
    }
  }
  false
}

fn file_tree_index_at(layout: FileTreeLayout, x: u16, y: u16, row_count: usize) -> Option<usize> {
  if !point_in_rect(x, y, layout.list) {
    return None;
  }
  let row = usize::from(y.saturating_sub(layout.list.y));
  let index = layout.scroll_offset.saturating_add(row);
  (index < row_count).then_some(index)
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
  use crossterm::event::{
    KeyCode,
    KeyEvent,
    KeyModifiers,
    MouseEvent,
  };
  use the_default::{
    CommandPaletteSource,
    CompletionMenuItem,
    DefaultContext,
    is_active_file_tree,
    set_file_tree_visible_rows,
    show_completion_menu,
  };
  use the_lib::{
    selection::Selection,
    split_tree::SplitAxis,
    transaction::Transaction,
  };

  use super::*;

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

  fn install_file_tree_layout(ctx: &mut Ctx, visible_rows: usize) -> crate::ctx::FileTreeLayout {
    let surface_id = ctx.file_tree.surface_id.expect("tree surface");
    let pane_id = ctx
      .editor
      .client_surface_snapshots()
      .into_iter()
      .find(|surface| surface.client_surface_id == surface_id)
      .and_then(|surface| surface.attached_pane)
      .expect("tree pane");
    set_file_tree_visible_rows(ctx, visible_rows);
    let layout = crate::ctx::FileTreeLayout {
      pane_id,
      pane: Rect::new(0, 1, 24, visible_rows as u16 + 1),
      header: Rect::new(0, 1, 24, 1),
      list: Rect::new(0, 2, 24, visible_rows as u16),
      visible_rows,
      scroll_offset: ctx.file_tree.scroll_offset,
    };
    ctx.file_tree_layout = Some(layout);
    layout
  }

  #[test]
  fn space_shift_slash_opens_action_palette() {
    let mut ctx = Ctx::new(None).expect("ctx");

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
    let mut ctx = Ctx::new(None).expect("ctx");
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
    let mut ctx = Ctx::new(None).expect("ctx");

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
  fn file_tree_navigation_still_works_after_focus_reset() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, key_event(KeyCode::Char(' ')));
    handle_key(&mut ctx, key_event(KeyCode::Char('e')));

    assert!(is_active_file_tree(&ctx));
    let before = ctx.file_tree.selected.expect("tree selection");

    ctx.handle_terminal_focus_lost();
    ctx.handle_terminal_focus_gained();

    handle_key(&mut ctx, key_event(KeyCode::Char('j')));

    assert!(is_active_file_tree(&ctx));
    assert!(ctx.file_tree.selected.expect("tree selection after move") > before);
  }

  #[test]
  fn clicking_file_tree_row_focuses_tree_and_selects_row() {
    let mut ctx = Ctx::new(None).expect("ctx");
    handle_key(&mut ctx, key_event(KeyCode::Char(' ')));
    handle_key(&mut ctx, key_event(KeyCode::Char('e')));

    let editor_pane = ctx.file_tree.last_editor_pane.expect("editor pane");
    assert!(ctx.editor.set_active_pane(editor_pane));
    let layout = install_file_tree_layout(&mut ctx, 8);

    handle_mouse(
      &mut ctx,
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        3,
        layout.list.y + 2,
      ),
    );

    assert!(is_active_file_tree(&ctx));
    assert_eq!(ctx.file_tree.selected, Some(2));
  }

  #[test]
  fn double_clicking_file_tree_file_opens_it() {
    let mut ctx = Ctx::new(None).expect("ctx");
    handle_key(&mut ctx, key_event(KeyCode::Char(' ')));
    handle_key(&mut ctx, key_event(KeyCode::Char('e')));
    let file_index = ctx
      .file_tree
      .rows
      .iter()
      .position(|row| !row.is_dir)
      .expect("file row");
    let visible_rows = file_index.saturating_add(2).max(8);
    let layout = install_file_tree_layout(&mut ctx, visible_rows);
    let row_y = layout
      .list
      .y
      .saturating_add((file_index - layout.scroll_offset) as u16);
    let file_path = ctx.file_tree.rows[file_index].path.clone();

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Down(MouseButton::Left), 3, row_y),
    );
    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::Down(MouseButton::Left), 3, row_y),
    );

    assert_eq!(ctx.file_path.as_deref(), Some(file_path.as_path()));
    assert!(!is_active_file_tree(&ctx));
  }

  #[test]
  fn mouse_wheel_scrolls_file_tree_list() {
    let mut ctx = Ctx::new(None).expect("ctx");
    handle_key(&mut ctx, key_event(KeyCode::Char(' ')));
    handle_key(&mut ctx, key_event(KeyCode::Char('e')));
    let layout = install_file_tree_layout(&mut ctx, 4);

    handle_mouse(
      &mut ctx,
      mouse_event(MouseEventKind::ScrollDown, 3, layout.list.y + 1),
    );

    assert!(ctx.file_tree.scroll_offset > 0);
  }

  #[test]
  fn file_tree_git_navigation_jumps_to_next_changed_row() {
    let mut ctx = Ctx::new(None).expect("ctx");
    handle_key(&mut ctx, key_event(KeyCode::Char(' ')));
    handle_key(&mut ctx, key_event(KeyCode::Char('e')));

    let target = 3usize.min(ctx.file_tree.rows.len().saturating_sub(1));
    ctx.file_tree.rows[target].decorations = the_default::FileTreeDecorations {
      vcs:        Some(the_default::FileTreeVcsKind::Modified),
      diagnostic: None,
    };
    ctx.file_tree.selected = Some(0);

    handle_key(&mut ctx, key_event(KeyCode::Char(']')));

    assert_eq!(ctx.file_tree.selected, Some(target));
  }

  #[test]
  fn file_tree_diagnostic_navigation_jumps_to_next_diagnostic_row() {
    let mut ctx = Ctx::new(None).expect("ctx");
    handle_key(&mut ctx, key_event(KeyCode::Char(' ')));
    handle_key(&mut ctx, key_event(KeyCode::Char('e')));

    let target = 4usize.min(ctx.file_tree.rows.len().saturating_sub(1));
    ctx.file_tree.rows[target].decorations = the_default::FileTreeDecorations {
      vcs:        None,
      diagnostic: Some(the_lib::diagnostics::DiagnosticSeverity::Warning),
    };
    ctx.file_tree.selected = Some(0);

    handle_key(&mut ctx, key_event(KeyCode::Char('}')));

    assert_eq!(ctx.file_tree.selected, Some(target));
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
}
