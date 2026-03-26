use std::path::Path;

use the_lib::diagnostics::DiagnosticCounts;

use crate::{
  DefaultContext,
  Mode,
  PendingInput,
  SearchPromptKind,
  file_picker_icon_glyph,
  file_picker_icon_name_for_path,
  message_bar::inline_statusline_message,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatuslineEmphasis {
  Normal,
  Muted,
  Strong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatuslineSegment {
  pub text:     String,
  pub emphasis: StatuslineEmphasis,
}

impl StatuslineSegment {
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      text:     text.into(),
      emphasis: StatuslineEmphasis::Normal,
    }
  }

  pub fn muted(mut self) -> Self {
    self.emphasis = StatuslineEmphasis::Muted;
    self
  }

  pub fn strong(mut self) -> Self {
    self.emphasis = StatuslineEmphasis::Strong;
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatuslineSnapshot {
  pub left:           String,
  pub left_icon:      Option<String>,
  pub right_segments: Vec<StatuslineSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CursorPickStatus {
  remove:  bool,
  current: usize,
  total:   usize,
}

pub fn build_statusline_snapshot<Ctx: DefaultContext>(ctx: &mut Ctx) -> StatuslineSnapshot {
  let viewport_width = ctx.editor_ref().view().viewport.width as usize;
  let cursor_pick = cursor_pick_status(ctx);
  let cursor_pick_part = cursor_pick.map(|status| {
    let action = if status.remove { "remove" } else { "collapse" };
    format!("{action} {}/{}", status.current, status.total)
  });
  let pending_keys = pending_keys_text(ctx);
  let watch_part = ctx.watch_statusline_text().filter(|text| !text.is_empty());
  let lsp_part = ctx.lsp_statusline_text().filter(|text| !text.is_empty());
  let diagnostic_counts = ctx.diagnostic_statusline_counts();
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
  let default_left_icon = ctx
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
      if let Some(counts) = diagnostic_counts {
        let diagnostics_width = diagnostic_statusline_segments(counts)
          .iter()
          .map(|segment| segment.text.chars().count() + 2)
          .sum::<usize>();
        budget = budget.saturating_sub(diagnostics_width);
      }
      let clamped = clamp_with_ellipsis(&message, budget.min(96));
      if clamped.is_empty() {
        None
      } else {
        Some(clamped)
      }
    });

  let mut right_segments = Vec::new();
  if let Some(cursor_pick) = cursor_pick_part {
    right_segments.push(StatuslineSegment::new(cursor_pick).strong());
  }
  if let Some(pending) = pending_part {
    right_segments.push(StatuslineSegment::new(pending));
  }
  if let Some(lsp) = lsp_part {
    right_segments.push(StatuslineSegment::new(lsp).muted());
  }
  if let Some(watch) = watch_part {
    right_segments.push(StatuslineSegment::new(watch).strong());
  }
  if let Some(vcs) = vcs_part {
    right_segments.push(StatuslineSegment::new(vcs).muted());
  }
  if let Some(counts) = diagnostic_counts {
    right_segments.extend(diagnostic_statusline_segments(counts));
  }
  if let Some(message) = message_part {
    right_segments.push(StatuslineSegment::new(message));
  }
  right_segments.push(StatuslineSegment::new(cursor_text));

  if ctx.command_palette().is_open {
    let (query, cursor) = command_palette_prompt_query_and_cursor(ctx);
    left = command_palette_statusline_text(query, cursor);
    return StatuslineSnapshot {
      left,
      left_icon: None,
      right_segments,
    };
  }

  if ctx.search_prompt_ref().active {
    let prompt = ctx.search_prompt_ref();
    left = search_statusline_text(prompt.kind, prompt.query.as_str(), prompt.cursor);
    return StatuslineSnapshot {
      left,
      left_icon: None,
      right_segments,
    };
  }

  StatuslineSnapshot {
    left,
    left_icon: default_left_icon,
    right_segments,
  }
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

fn diagnostic_statusline_segments(counts: DiagnosticCounts) -> Vec<StatuslineSegment> {
  if counts.total == 0 {
    return Vec::new();
  }

  let mut segments = Vec::new();
  if counts.errors > 0 {
    segments.push(
      StatuslineSegment::new(format!(
        "{} {}",
        file_picker_icon_glyph("diagnostic_error", false),
        counts.errors
      ))
      .strong(),
    );
  }
  if counts.warnings > 0 {
    segments.push(
      StatuslineSegment::new(format!(
        "{} {}",
        file_picker_icon_glyph("diagnostic_warning", false),
        counts.warnings
      ))
      .strong(),
    );
  }
  if counts.information > 0 {
    segments.push(
      StatuslineSegment::new(format!(
        "{} {}",
        file_picker_icon_glyph("diagnostic_info", false),
        counts.information
      ))
      .muted(),
    );
  }
  if counts.hints > 0 {
    segments.push(
      StatuslineSegment::new(format!(
        "{} {}",
        file_picker_icon_glyph("diagnostic_hint", false),
        counts.hints
      ))
      .muted(),
    );
  }
  segments
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

fn command_palette_prompt_query_and_cursor<Ctx: DefaultContext>(ctx: &Ctx) -> (&str, usize) {
  let raw = ctx.command_prompt_ref().input.as_str();
  if let Some(stripped) = raw.strip_prefix(':') {
    (stripped, ctx.command_prompt_ref().cursor.saturating_sub(1))
  } else {
    (raw, ctx.command_prompt_ref().cursor)
  }
}

fn command_palette_statusline_text(query: &str, cursor: usize) -> String {
  let mut cursor = cursor.min(query.len());
  while cursor > 0 && !query.is_char_boundary(cursor) {
    cursor -= 1;
  }
  if !query.is_char_boundary(cursor) {
    cursor = 0;
  }
  let (before, after) = query.split_at(cursor);
  format!("CMD {before}█{after}")
}

fn search_statusline_text(kind: SearchPromptKind, query: &str, cursor: usize) -> String {
  let mut cursor = cursor.min(query.len());
  while cursor > 0 && !query.is_char_boundary(cursor) {
    cursor -= 1;
  }
  if !query.is_char_boundary(cursor) {
    cursor = 0;
  }
  let (before, after) = query.split_at(cursor);
  let prefix = match kind {
    SearchPromptKind::Search => "FIND",
    SearchPromptKind::SelectRegex => "SELECT",
    SearchPromptKind::SplitSelection => "SPLIT",
    SearchPromptKind::KeepSelections => "KEEP",
    SearchPromptKind::RemoveSelections => "REMOVE",
    SearchPromptKind::RenameSymbol => "RENAME",
    SearchPromptKind::ShellPipe => "PIPE",
    SearchPromptKind::ShellPipeTo => "PIPE-TO",
    SearchPromptKind::ShellInsertOutput => "INSERT-OUTPUT",
    SearchPromptKind::ShellAppendOutput => "APPEND-OUTPUT",
    SearchPromptKind::ShellKeepPipe => "KEEP-PIPE",
  };
  format!("{prefix} {before}█{after}")
}

#[cfg(test)]
mod tests {
  use the_lib::diagnostics::DiagnosticCounts;

  use super::diagnostic_statusline_segments;

  #[test]
  fn diagnostic_statusline_segments_only_emit_non_zero_counts() {
    let segments = diagnostic_statusline_segments(DiagnosticCounts {
      total:       2,
      errors:      1,
      warnings:    0,
      information: 1,
      hints:       0,
    });

    let texts = segments
      .into_iter()
      .map(|segment| segment.text)
      .collect::<Vec<_>>();
    assert_eq!(texts, vec![" 1".to_string(), " 1".to_string()]);
  }
}
