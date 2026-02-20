use std::path::Path;

use the_lib::render::{
  LayoutIntent,
  UiConstraints,
  UiEmphasis,
  UiInsets,
  UiLayer,
  UiNode,
  UiPanel,
  UiStatusBar,
  UiStyle,
  UiStyledSpan,
};

use crate::{
  DefaultContext,
  Mode,
  PendingInput,
  file_picker_icon_glyph,
  file_picker_icon_name_for_path,
  message_bar::inline_statusline_message,
};

pub const STATUSLINE_ID: &str = "statusline";

pub fn statusline_present(tree: &the_lib::render::UiTree) -> bool {
  tree.overlays.iter().any(|node| {
    match node {
      UiNode::Panel(panel) => panel.id == STATUSLINE_ID,
      UiNode::StatusBar(status) => status.id.as_deref() == Some(STATUSLINE_ID),
      _ => false,
    }
  })
}

pub fn build_statusline_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> UiNode {
  let viewport_width = ctx.editor_ref().view().viewport.width as usize;
  let cursor_pick = cursor_pick_status(ctx);
  let cursor_pick_part = cursor_pick.map(|status| {
    let action = if status.remove { "remove" } else { "collapse" };
    format!("{action} {}/{}", status.current, status.total)
  });
  let pending_keys = pending_keys_text(ctx);
  let watch_part = ctx.watch_statusline_text().filter(|text| !text.is_empty());
  let lsp_part = ctx.lsp_statusline_text().filter(|text| !text.is_empty());
  let vcs_part = ctx
    .vcs_statusline_text()
    .filter(|text| !text.is_empty())
    .map(|text| format!("{} {text}", file_picker_icon_glyph("git_branch", false)));
  let doc = ctx.editor_ref().document();
  let slice = doc.text().slice(..);
  let selection = doc.selection();
  let range = if let Some(active_cursor) = ctx.editor_ref().view().active_cursor {
    selection.range_by_id(active_cursor).copied()
  } else {
    selection.ranges().first().copied()
  };

  let (line, col) = if let Some(range) = range {
    let line = range.cursor_line(slice);
    let col = range.cursor(slice).saturating_sub(slice.line_to_char(line));
    (line + 1, col + 1)
  } else {
    (1, 1)
  };

  let file_name = ctx
    .file_path()
    .and_then(|path| path.file_name())
    .and_then(|name| name.to_str())
    .map(str::to_string)
    .unwrap_or_else(|| doc.display_name().to_string());
  let left_icon = ctx
    .file_path()
    .map(file_picker_icon_name_for_path)
    .or_else(|| {
      if file_name.is_empty() {
        None
      } else {
        Some(file_picker_icon_name_for_path(Path::new(
          file_name.as_str(),
        )))
      }
    })
    .map(str::to_string);

  let mut left = format!("{}  {}", mode_label(ctx.mode(), cursor_pick), file_name);
  let flags = doc.flags();
  if flags.modified {
    left.push_str(" [+]");
  }
  if flags.readonly {
    left.push_str(" [RO]");
  }

  let cursor_text = if selection.ranges().len() > 1 {
    format!("{} sel  {line}:{col}", selection.ranges().len())
  } else {
    format!("{line}:{col}")
  };
  let pending_part = pending_keys.filter(|pending| !pending.is_empty());
  let message_part = inline_statusline_message(ctx)
    .filter(|message| !message.is_empty())
    .and_then(|message| {
      let mut budget = viewport_width.saturating_sub(cursor_text.chars().count() + 8);
      if let Some(pending) = pending_part.as_ref() {
        budget = budget.saturating_sub(pending.chars().count() + 2);
      }
      if let Some(cursor_pick) = cursor_pick_part.as_ref() {
        budget = budget.saturating_sub(cursor_pick.chars().count() + 2);
      }
      if let Some(lsp) = lsp_part.as_ref() {
        budget = budget.saturating_sub(lsp.chars().count() + 2);
      }
      if let Some(watch) = watch_part.as_ref() {
        budget = budget.saturating_sub(watch.chars().count() + 2);
      }
      if let Some(vcs) = vcs_part.as_ref() {
        budget = budget.saturating_sub(vcs.chars().count() + 2);
      }
      let clamped = clamp_with_ellipsis(&message, budget.min(96));
      if clamped.is_empty() {
        None
      } else {
        Some(clamped)
      }
    });

  let mut right_parts = Vec::new();
  let mut right_segments = Vec::new();
  if let Some(cursor_pick) = cursor_pick_part {
    let mut pick_style = UiStyle::default();
    pick_style.emphasis = UiEmphasis::Strong;
    right_segments.push(UiStyledSpan {
      text:  cursor_pick.clone(),
      style: Some(pick_style),
    });
    right_parts.push(cursor_pick);
  }
  if let Some(pending) = pending_part {
    right_segments.push(UiStyledSpan {
      text:  pending.clone(),
      style: None,
    });
    right_parts.push(pending);
  }
  if let Some(lsp) = lsp_part {
    let mut lsp_style = UiStyle::default();
    lsp_style.emphasis = UiEmphasis::Muted;
    right_segments.push(UiStyledSpan {
      text:  lsp.clone(),
      style: Some(lsp_style),
    });
    right_parts.push(lsp);
  }
  if let Some(watch) = watch_part {
    let mut watch_style = UiStyle::default();
    watch_style.emphasis = UiEmphasis::Strong;
    right_segments.push(UiStyledSpan {
      text:  watch.clone(),
      style: Some(watch_style),
    });
    right_parts.push(watch);
  }
  if let Some(vcs) = vcs_part {
    let mut vcs_style = UiStyle::default();
    vcs_style.emphasis = UiEmphasis::Muted;
    right_segments.push(UiStyledSpan {
      text:  vcs.clone(),
      style: Some(vcs_style),
    });
    right_parts.push(vcs);
  }
  if let Some(message) = message_part {
    right_segments.push(UiStyledSpan {
      text:  message.clone(),
      style: None,
    });
    right_parts.push(message);
  }
  right_segments.push(UiStyledSpan {
    text:  cursor_text.clone(),
    style: None,
  });
  right_parts.push(cursor_text);
  let right = right_parts.join("  ");

  let status = UiStatusBar {
    id: Some(STATUSLINE_ID.to_string()),
    left,
    center: String::new(),
    right,
    style: UiStyle::default().with_role("statusline"),
    left_icon,
    right_segments,
  };

  let mut panel = UiPanel::new(
    STATUSLINE_ID,
    LayoutIntent::Bottom,
    UiNode::StatusBar(status),
  );
  panel.style = UiStyle::default().with_role("statusline");
  panel.style.border = None;
  panel.layer = UiLayer::Background;
  panel.constraints = UiConstraints {
    min_height: Some(1),
    max_height: Some(1),
    padding: UiInsets {
      left:   1,
      right:  1,
      top:    0,
      bottom: 0,
    },
    ..UiConstraints::default()
  };

  UiNode::Panel(panel)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CursorPickStatus {
  remove:  bool,
  current: usize,
  total:   usize,
}

fn cursor_pick_status<Ctx: DefaultContext>(ctx: &Ctx) -> Option<CursorPickStatus> {
  match ctx.pending_input() {
    Some(PendingInput::CursorPick {
      remove,
      candidates,
      index,
      ..
    }) if !candidates.is_empty() => {
      Some(CursorPickStatus {
        remove:  *remove,
        current: (*index)
          .min(candidates.len().saturating_sub(1))
          .saturating_add(1),
        total:   candidates.len(),
      })
    },
    _ => None,
  }
}

fn mode_label(mode: Mode, cursor_pick: Option<CursorPickStatus>) -> &'static str {
  if let Some(status) = cursor_pick {
    return if status.remove { "REM" } else { "COL" };
  }

  match mode {
    Mode::Normal => "NOR",
    Mode::Insert => "INS",
    Mode::Select => "SEL",
    Mode::Command => "CMD",
  }
}

fn pending_keys_text<Ctx: DefaultContext>(ctx: &mut Ctx) -> Option<String> {
  let pending = ctx.keymaps().pending();
  if pending.is_empty() {
    return None;
  }
  Some(
    pending
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>()
      .join(" "),
  )
}

fn clamp_with_ellipsis(text: &str, max_chars: usize) -> String {
  if max_chars == 0 {
    return String::new();
  }
  let count = text.chars().count();
  if count <= max_chars {
    return text.to_string();
  }
  if max_chars == 1 {
    return "…".to_string();
  }

  let mut out = String::new();
  for ch in text.chars().take(max_chars - 1) {
    out.push(ch);
  }
  out.push('…');
  out
}
