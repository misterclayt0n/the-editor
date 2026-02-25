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
  close_signature_help,
  completion_docs_scroll,
  handle_pointer_event as dispatch_pointer_event,
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
  render::{
    UiEvent,
    UiEventKind,
    UiKey,
    UiKeyEvent,
    UiModifiers,
  },
  split_tree::{
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

  if ctx.mode() == Mode::Command {
    if let Some(mut key) = to_ui_key(event.code) {
      if matches!(event.code, KeyCode::Tab | KeyCode::BackTab) {
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

  let modifiers = to_modifiers(event.modifiers, event.code);
  let Some(key) = to_key(event.code) else {
    return;
  };

  let key_event = KeyEvent { key, modifiers };

  ctx.dispatch().pre_on_keypress(ctx, key_event);
}

pub fn handle_mouse(ctx: &mut Ctx, event: CrosstermMouseEvent) {
  let Some(pointer_event) = crossterm_mouse_to_pointer_event(event) else {
    return;
  };
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

  if handle_pane_resize_pointer(ctx, event.kind, x, y) {
    return PointerEventOutcome::Handled;
  }

  let Some(layout) = ctx.completion_docs_layout else {
    ctx.completion_docs_drag = None;
    return PointerEventOutcome::Handled;
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
  let x = event
    .logical_col
    .or_else(|| {
      if event.x < 0 {
        None
      } else {
        Some((event.x.min(i32::from(u16::MAX))) as u16)
      }
    })?;
  let y = event
    .logical_row
    .or_else(|| {
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
    CompletionMenuItem,
    DefaultContext,
    show_completion_menu,
  };
  use the_lib::{
    selection::Selection,
    split_tree::SplitAxis,
    transaction::Transaction,
  };

  use super::*;
  use crate::dispatch::build_dispatch;

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
}
