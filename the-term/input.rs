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
  open_file_picker_index,
  scroll_file_picker_list,
  scroll_file_picker_preview,
  set_file_picker_list_offset,
  set_file_picker_preview_offset,
  ui_event as dispatch_ui_event,
};
use the_lib::render::{
  UiEvent,
  UiEventKind,
  UiKey,
  UiKeyEvent,
  UiModifiers,
};

use crate::{
  Ctx,
  ctx::FilePickerDragState,
  dispatch::{
    Key,
    KeyEvent,
    Modifiers,
  },
  picker_layout::{
    compute_file_picker_layout,
    compute_scrollbar_metrics,
    point_in_rect,
    scroll_offset_from_thumb,
  },
};

/// Orchestration function - maps keyboard input to dispatch calls.
pub fn handle_key(ctx: &mut Ctx, event: CrosstermKeyEvent) {
  // Only handle key press events, not release or repeat
  if event.kind != KeyEventKind::Press {
    return;
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
  if !ctx.file_picker.active {
    return;
  }

  let viewport = ctx.editor.view().viewport;
  let viewport = Rect::new(viewport.x, viewport.y, viewport.width, viewport.height);
  let layout = ctx
    .file_picker_layout
    .or_else(|| compute_file_picker_layout(viewport, &ctx.file_picker));
  let Some(layout) = layout else {
    return;
  };
  ctx.file_picker_layout = Some(layout);

  let x = event.column;
  let y = event.row;
  match event.kind {
    MouseEventKind::Down(MouseButton::Left) => {
      handle_left_down(ctx, layout, x, y);
    },
    MouseEventKind::Drag(MouseButton::Left) => {
      handle_left_drag(ctx, layout, y);
    },
    MouseEventKind::Up(MouseButton::Left) => {
      ctx.file_picker_drag = None;
    },
    MouseEventKind::ScrollUp => {
      handle_wheel(ctx, layout, x, y, -3);
    },
    MouseEventKind::ScrollDown => {
      handle_wheel(ctx, layout, x, y, 3);
    },
    _ => {},
  }
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
      ctx.file_picker_drag = Some(FilePickerDragState::ListScrollbar { grab_offset });
      return;
    }
  }

  if point_in_rect(x, y, layout.list_content) {
    let row = y.saturating_sub(layout.list_content.y) as usize;
    let index = layout.list_scroll_offset.saturating_add(row);
    if index < picker.matched_count() {
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
      ctx.file_picker_drag = Some(FilePickerDragState::PreviewScrollbar { grab_offset });
      return;
    }
  }

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
    return;
  }

  if let Some(preview_content) = layout.preview_content
    && (point_in_rect(x, y, preview_content)
      || layout
        .preview_scrollbar
        .is_some_and(|track| point_in_rect(x, y, track)))
  {
    scroll_file_picker_preview(ctx, delta, layout.preview_visible_rows());
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
