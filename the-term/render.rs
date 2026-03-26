//! Rendering - converts RenderPlan to ratatui draw calls.

use std::{
  borrow::Cow,
  collections::BTreeMap,
  env,
  fs::OpenOptions,
  hash::{
    DefaultHasher,
    Hash,
    Hasher,
  },
  io::Write,
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Mutex,
    OnceLock,
    atomic::{
      AtomicU64,
      Ordering,
    },
  },
  time::Instant,
};

use ratatui::{
  prelude::*,
  style::Modifier,
  text::{
    Line,
    Span,
  },
  widgets::{
    Block,
    BorderType,
    Borders,
    Clear,
    Paragraph,
    Widget,
    block::Title,
  },
};
use ropey::Rope;
use serde_json::{
  Value,
  json,
};
use the_default::{
  DefaultContext,
  FilePickerPreviewLineKind,
  FileTreeSnapshot,
  Mode,
  OverlayRect as DefaultOverlayRect,
  PendingInput,
  SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER,
  SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER,
  StatuslineEmphasis,
  build_statusline_snapshot,
  command_palette_filtered_indices,
  completion_docs_panel_rect as default_completion_docs_panel_rect,
  completion_panel_rect as default_completion_panel_rect,
  file_picker_icon_glyph,
  file_picker_icon_name_for_path,
  file_picker_preview_window,
  file_tree_snapshot,
  frame_render_plan,
  set_file_tree_visible_rows,
  set_picker_visible_rows,
  signature_help_markdown,
  signature_help_panel_rect as default_signature_help_panel_rect,
};
use the_lib::{
  diagnostics::{
    Diagnostic,
    DiagnosticSeverity,
  },
  docs_markdown::{
    DocsBlock,
    DocsInlineKind,
    DocsInlineRun,
    DocsListMarker,
    DocsSemanticKind,
    language_filename_hints,
    parse_markdown_blocks,
  },
  editor::{
    BufferId,
    PaneContent,
  },
  render::{
    FrameGenerationState,
    FrameRenderPlan,
    InlineDiagnostic,
    InlineDiagnosticFilter,
    InlineDiagnosticRenderLine,
    InlineDiagnosticsConfig,
    InlineDiagnosticsViewportLayout,
    NoHighlights,
    PaneRenderPlan,
    RenderDiagnosticGutterStyles,
    RenderDiffGutterStyles,
    RenderGenerationState,
    RenderLayerRowHashes,
    RenderPlan,
    RenderStyles,
    SelectionMatchHighlightOptions,
    SyntaxHighlightAdapter,
    add_selection_match_highlights,
    apply_diagnostic_gutter_markers,
    apply_diff_gutter_markers,
    apply_row_insertions,
    apply_virtual_lines_layout,
    base_render_layer_row_hashes,
    build_plan,
    finish_frame_generations,
    finish_render_generations,
    graphics::{
      Color as LibColor,
      CursorKind as LibCursorKind,
      Modifier as LibModifier,
      Style as LibStyle,
      UnderlineStyle as LibUnderlineStyle,
    },
    gutter_width_for_document,
    render_inline_diagnostics_for_viewport,
    render_virtual_lines_for_viewport,
    text_annotations::TextAnnotations,
    visual_pos_at_char,
  },
  selection::Range,
  split_tree::{
    PaneId,
    SplitAxis,
  },
  syntax::{
    Highlight,
    Syntax,
  },
};
use the_lsp::text_sync::{
  file_uri_for_path,
  utf16_position_to_char_idx,
};

use crate::{
  Ctx,
  ctx::{
    DiagnosticUnderlineRenderSpan,
    FileTreeDecorations,
    FileTreeLayout,
    FileTreeVcsKind,
    TermCursorMode,
    TermHardwareCursor,
  },
  docs_panel::DocsPanelSource,
  picker_layout::{
    CompletionDocsLayout,
    FilePickerLayout,
    compute_file_picker_layout,
    compute_scrollbar_metrics,
  },
};

#[derive(Debug)]
struct TermRenderPerfConfig {
  min_duration_ms: f64,
  file_path:       Option<PathBuf>,
  start:           Instant,
}

#[derive(Debug, Default)]
struct TermRenderPerfState {
  last_scroll: Option<(usize, usize)>,
}

static TERM_RENDER_PERF_CONFIG: OnceLock<Option<TermRenderPerfConfig>> = OnceLock::new();
static TERM_RENDER_PERF_STATE: OnceLock<Mutex<TermRenderPerfState>> = OnceLock::new();
static TERM_RENDER_PERF_SEQ: AtomicU64 = AtomicU64::new(0);

fn term_render_perf_config() -> Option<&'static TermRenderPerfConfig> {
  TERM_RENDER_PERF_CONFIG
    .get_or_init(|| {
      if env::var("THE_TERM_DEBUG_RENDER_PERF").ok().as_deref() != Some("1") {
        return None;
      }

      let min_duration_ms = env::var("THE_TERM_DEBUG_RENDER_PERF_MIN_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .unwrap_or(1.0);
      let file_path = env::var("THE_TERM_DEBUG_RENDER_PERF_FILE")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from);
      Some(TermRenderPerfConfig {
        min_duration_ms,
        file_path,
        start: Instant::now(),
      })
    })
    .as_ref()
}

fn term_render_perf_should_log(duration_ms: f64) -> bool {
  term_render_perf_config().is_some_and(|cfg| duration_ms >= cfg.min_duration_ms)
}

fn term_render_perf_state() -> &'static Mutex<TermRenderPerfState> {
  TERM_RENDER_PERF_STATE.get_or_init(|| Mutex::new(TermRenderPerfState::default()))
}

fn term_render_perf_scroll_changed(row: usize, col: usize) -> bool {
  let Ok(mut state) = term_render_perf_state().lock() else {
    return false;
  };
  let previous = state.last_scroll.replace((row, col));
  previous != Some((row, col))
}

fn term_render_perf_write(message: String) {
  let Some(cfg) = term_render_perf_config() else {
    return;
  };
  let elapsed_ms = cfg.start.elapsed().as_millis();
  let line = format!("[termrender +{elapsed_ms}ms] {message}\n");
  eprint!("{line}");
  if let Some(path) = &cfg.file_path {
    append_term_render_perf_line(path, line.as_bytes());
  }
}

fn append_term_render_perf_line(path: &Path, data: &[u8]) {
  if let Some(parent) = path.parent() {
    let _ = std::fs::create_dir_all(parent);
  }

  if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
    let _ = file.write_all(data);
  }
}

pub fn last_render_perf_seq() -> u64 {
  TERM_RENDER_PERF_SEQ.load(Ordering::Relaxed)
}

pub fn log_present_perf(ctx: &Ctx, phase: &str, draw_ms: f64, cursor_ms: f64, total_ms: f64) {
  if !term_render_perf_should_log(total_ms) {
    return;
  }

  let view = ctx.editor.view();
  let cursor = match ctx.term_cursor_mode {
    TermCursorMode::Hidden => "hidden".to_string(),
    TermCursorMode::Hardware(cursor) => {
      format!("{}:{}:{:?}", cursor.x, cursor.y, cursor.kind)
    },
  };
  term_render_perf_write(format!(
    "kind=present seq={} phase={} total={total_ms:.2}ms draw={draw_ms:.2}ms \
     cursor={cursor_ms:.2}ms scroll={}:{} viewport={}x{} hardware_cursor={}",
    last_render_perf_seq(),
    phase,
    view.scroll.row,
    view.scroll.col,
    view.viewport.width,
    view.viewport.height,
    cursor,
  ));
}

fn resolve_term_cursor_mode(
  hide_cursor: bool,
  ui_cursor: Option<(u16, u16)>,
  active_editor_cursor: Option<(u16, u16)>,
  active_editor_cursor_kind: Option<LibCursorKind>,
) -> TermCursorMode {
  if hide_cursor || ui_cursor.is_some() {
    return TermCursorMode::Hidden;
  }

  if let (Some((x, y)), Some(kind)) = (active_editor_cursor, active_editor_cursor_kind)
    && matches!(kind, LibCursorKind::Bar | LibCursorKind::Underline)
  {
    return TermCursorMode::Hardware(TermHardwareCursor { x, y, kind });
  }

  TermCursorMode::Hidden
}

fn lib_color_to_ratatui(color: the_lib::render::graphics::Color) -> Color {
  use the_lib::render::graphics::Color as LibColor;
  match color {
    LibColor::Reset => Color::Reset,
    LibColor::Black => Color::Black,
    LibColor::Red => Color::Red,
    LibColor::Green => Color::Green,
    LibColor::Yellow => Color::Yellow,
    LibColor::Blue => Color::Blue,
    LibColor::Magenta => Color::Magenta,
    LibColor::Cyan => Color::Cyan,
    LibColor::Gray => Color::DarkGray,
    LibColor::LightRed => Color::LightRed,
    LibColor::LightGreen => Color::LightGreen,
    LibColor::LightYellow => Color::LightYellow,
    LibColor::LightBlue => Color::LightBlue,
    LibColor::LightMagenta => Color::LightMagenta,
    LibColor::LightCyan => Color::LightCyan,
    LibColor::LightGray => Color::Gray,
    LibColor::White => Color::White,
    LibColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
    LibColor::Indexed(idx) => Color::Indexed(idx),
  }
}

fn lib_modifier_to_ratatui(mods: LibModifier) -> Modifier {
  let mut out = Modifier::empty();
  if mods.contains(LibModifier::BOLD) {
    out.insert(Modifier::BOLD);
  }
  if mods.contains(LibModifier::DIM) {
    out.insert(Modifier::DIM);
  }
  if mods.contains(LibModifier::ITALIC) {
    out.insert(Modifier::ITALIC);
  }
  if mods.contains(LibModifier::SLOW_BLINK) {
    out.insert(Modifier::SLOW_BLINK);
  }
  if mods.contains(LibModifier::RAPID_BLINK) {
    out.insert(Modifier::RAPID_BLINK);
  }
  if mods.contains(LibModifier::REVERSED) {
    out.insert(Modifier::REVERSED);
  }
  if mods.contains(LibModifier::HIDDEN) {
    out.insert(Modifier::HIDDEN);
  }
  if mods.contains(LibModifier::CROSSED_OUT) {
    out.insert(Modifier::CROSSED_OUT);
  }
  out
}

fn lib_style_to_ratatui(style: LibStyle) -> Style {
  let mut out = Style::default();
  if let Some(fg) = style.fg {
    out = out.fg(lib_color_to_ratatui(fg));
  }
  if let Some(bg) = style.bg {
    out = out.bg(lib_color_to_ratatui(bg));
  }
  if let Some(color) = style.underline_color {
    out = out.underline_color(lib_color_to_ratatui(color));
  }
  if let Some(underline) = style.underline_style {
    if !matches!(underline, LibUnderlineStyle::Reset) {
      out = out.add_modifier(Modifier::UNDERLINED);
    }
  }
  let add = lib_modifier_to_ratatui(style.add_modifier);
  let sub = lib_modifier_to_ratatui(style.sub_modifier);
  out = out.add_modifier(add);
  out = out.remove_modifier(sub);
  out
}

fn render_styles_from_theme(ctx: &Ctx) -> RenderStyles {
  let theme = &ctx.ui_theme;
  let cursor_shapes = ctx.cursor_shapes;
  let (cursor_kind, active_cursor_kind) = match ctx.mode {
    Mode::Insert => (cursor_shapes.insert, cursor_shapes.insert),
    Mode::Select => (cursor_shapes.select, cursor_shapes.select),
    Mode::Normal | Mode::Command => (cursor_shapes.normal, cursor_shapes.normal),
  };
  let selection = theme.try_get("ui.selection").unwrap_or_default();
  let cursor = theme.try_get("ui.cursor").unwrap_or_default();
  let active_cursor = if matches!(
    ctx.pending_input.as_ref(),
    Some(PendingInput::CursorPick { .. })
  ) {
    theme
      .try_get("ui.cursor.match")
      .or_else(|| theme.try_get("ui.cursor.active"))
      .or_else(|| theme.try_get("ui.cursor"))
      .unwrap_or_default()
  } else {
    theme
      .try_get("ui.cursor.active")
      .or_else(|| theme.try_get("ui.cursor"))
      .unwrap_or_default()
  };
  RenderStyles {
    selection,
    cursor,
    active_cursor,
    cursor_kind,
    active_cursor_kind,
    non_block_cursor_uses_head: true,
    gutter: theme.try_get("ui.linenr").unwrap_or_default(),
    gutter_active: theme
      .try_get("ui.linenr.selected")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn render_diagnostic_styles_from_theme(
  theme: &the_lib::render::theme::Theme,
) -> RenderDiagnosticGutterStyles {
  RenderDiagnosticGutterStyles {
    error:   theme
      .try_get("error")
      .or_else(|| theme.try_get("diagnostic.error"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    warning: theme
      .try_get("warning")
      .or_else(|| theme.try_get("diagnostic.warning"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    info:    theme
      .try_get("info")
      .or_else(|| theme.try_get("diagnostic.info"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    hint:    theme
      .try_get("hint")
      .or_else(|| theme.try_get("diagnostic.hint"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn render_diff_styles_from_theme(theme: &the_lib::render::theme::Theme) -> RenderDiffGutterStyles {
  RenderDiffGutterStyles {
    added:    theme
      .try_get("diff.plus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    modified: theme
      .try_get("diff.delta")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    removed:  theme
      .try_get("diff.minus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn diagnostics_for_buffer<'a>(ctx: &'a Ctx, buffer_id: BufferId) -> Option<&'a [Diagnostic]> {
  let uri = file_uri_for_path(ctx.editor.buffer_file_path(buffer_id)?)?;
  Some(&ctx.diagnostics.document(&uri)?.diagnostics)
}

fn diagnostics_by_line(diagnostics: &[Diagnostic]) -> BTreeMap<usize, DiagnosticSeverity> {
  let mut out = BTreeMap::new();
  for diagnostic in diagnostics {
    let line = diagnostic.range.start.line as usize;
    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
    match out.get(&line).copied() {
      Some(prev) if diagnostic_severity_rank(prev) >= diagnostic_severity_rank(severity) => {},
      _ => {
        out.insert(line, severity);
      },
    }
  }
  out
}

fn active_diagnostics_by_line(ctx: &Ctx) -> BTreeMap<usize, DiagnosticSeverity> {
  diagnostics_by_line(diagnostics_for_buffer(ctx, ctx.editor.active_buffer_id()).unwrap_or(&[]))
}

fn diagnostic_theme_style(
  theme: &the_lib::render::theme::Theme,
  severity: DiagnosticSeverity,
) -> LibStyle {
  match severity {
    DiagnosticSeverity::Error => {
      theme
        .try_get("error")
        .or_else(|| theme.try_get("diagnostic.error"))
        .unwrap_or_default()
    },
    DiagnosticSeverity::Warning => {
      theme
        .try_get("warning")
        .or_else(|| theme.try_get("diagnostic.warning"))
        .unwrap_or_default()
    },
    DiagnosticSeverity::Information => {
      theme
        .try_get("info")
        .or_else(|| theme.try_get("diagnostic.info"))
        .unwrap_or_default()
    },
    DiagnosticSeverity::Hint => {
      theme
        .try_get("hint")
        .or_else(|| theme.try_get("diagnostic.hint"))
        .unwrap_or_default()
    },
  }
}

fn diagnostic_underline_theme_style(
  theme: &the_lib::render::theme::Theme,
  severity: DiagnosticSeverity,
) -> LibStyle {
  let mut style = match severity {
    DiagnosticSeverity::Error => {
      theme
        .try_get("diagnostic.error")
        .or_else(|| theme.try_get("error"))
        .unwrap_or_default()
    },
    DiagnosticSeverity::Warning => {
      theme
        .try_get("diagnostic.warning")
        .or_else(|| theme.try_get("warning"))
        .unwrap_or_default()
    },
    DiagnosticSeverity::Information => {
      theme
        .try_get("diagnostic.info")
        .or_else(|| theme.try_get("info"))
        .unwrap_or_default()
    },
    DiagnosticSeverity::Hint => {
      theme
        .try_get("diagnostic.hint")
        .or_else(|| theme.try_get("hint"))
        .unwrap_or_default()
    },
  };

  if style.underline_color.is_none()
    && let Some(fg) = style.fg
  {
    style = style.underline_color(fg);
  }
  if style.underline_style.is_none()
    || matches!(style.underline_style, Some(LibUnderlineStyle::Reset))
  {
    style = style.underline_style(LibUnderlineStyle::Line);
  }

  style
}

fn diagnostic_visible_row_end_cols(plan: &RenderPlan) -> Vec<usize> {
  let mut row_end_cols = vec![plan.scroll.col; plan.viewport.height as usize];
  for line in &plan.lines {
    let row = line.row as usize;
    if row >= row_end_cols.len() {
      continue;
    }
    let end_col = line
      .spans
      .iter()
      .map(|span| plan.scroll.col + span.col.saturating_add(span.cols) as usize)
      .max()
      .unwrap_or(plan.scroll.col);
    row_end_cols[row] = row_end_cols[row].max(end_col);
  }
  row_end_cols
}

fn diagnostic_row_visible_end_col(
  plan: &RenderPlan,
  row: usize,
  row_visible_end_cols: &[usize],
) -> usize {
  let relative = row.saturating_sub(plan.scroll.row);
  row_visible_end_cols
    .get(relative)
    .copied()
    .unwrap_or(plan.scroll.col)
}

fn diagnostic_underlines_for_document<'a>(
  text: &'a Rope,
  diagnostics: &[Diagnostic],
  plan: &RenderPlan,
  text_fmt: &'a the_lib::render::text_format::TextFormat,
  annotations: &mut TextAnnotations<'a>,
) -> Vec<DiagnosticUnderlineRenderSpan> {
  if diagnostics.is_empty() {
    return Vec::new();
  }

  let row_start = plan.scroll.row;
  let row_end = row_start.saturating_add(plan.viewport.height as usize);
  let col_start = plan.scroll.col;
  let col_end = col_start.saturating_add(plan.content_width());
  if row_start >= row_end || col_start >= col_end {
    return Vec::new();
  }

  let row_visible_end_cols = diagnostic_visible_row_end_cols(plan);
  let text_slice = text.slice(..);
  let text_len = text.len_chars();
  let mut out = Vec::with_capacity(diagnostics.len());

  for diagnostic in diagnostics {
    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);

    let mut start_char_idx = utf16_position_to_char_idx(
      text,
      diagnostic.range.start.line,
      diagnostic.range.start.character,
    )
    .min(text_len);
    let mut end_char_idx = utf16_position_to_char_idx(
      text,
      diagnostic.range.end.line,
      diagnostic.range.end.character,
    )
    .min(text_len);

    if end_char_idx < start_char_idx {
      std::mem::swap(&mut start_char_idx, &mut end_char_idx);
    }
    if end_char_idx == start_char_idx {
      if start_char_idx >= text_len {
        continue;
      }
      end_char_idx = start_char_idx.saturating_add(1).min(text_len);
    }

    let Some(mut start_pos) = visual_pos_at_char(text_slice, text_fmt, annotations, start_char_idx)
    else {
      continue;
    };
    let Some(mut end_pos) = visual_pos_at_char(text_slice, text_fmt, annotations, end_char_idx)
    else {
      continue;
    };

    if (end_pos.row, end_pos.col) < (start_pos.row, start_pos.col) {
      std::mem::swap(&mut start_pos, &mut end_pos);
    }

    for row in start_pos.row..=end_pos.row {
      if row < row_start || row >= row_end {
        continue;
      }

      let row_end_col = diagnostic_row_visible_end_col(plan, row, &row_visible_end_cols);
      let (mut from, mut to) = if row == start_pos.row && row == end_pos.row {
        (start_pos.col, end_pos.col)
      } else if row == start_pos.row {
        (start_pos.col, row_end_col)
      } else if row == end_pos.row {
        (col_start, end_pos.col)
      } else {
        (col_start, row_end_col)
      };

      from = from.max(col_start);
      to = to.min(row_end_col).min(col_end);
      if to <= from {
        continue;
      }

      out.push(DiagnosticUnderlineRenderSpan {
        row: (row - row_start) as u16,
        start_col: (from - col_start) as u16,
        end_col: (to - col_start) as u16,
        severity,
      });
    }
  }

  out
}

fn remap_relative_row_with_insertions(
  relative_row: usize,
  scroll_row: usize,
  viewport_height: usize,
  row_insertions: &[the_lib::render::RenderRowInsertion],
) -> Option<u16> {
  let absolute_row = scroll_row.saturating_add(relative_row);
  let inserted_before = row_insertions
    .iter()
    .filter(|insertion| insertion.base_row < absolute_row)
    .map(|insertion| insertion.inserted_rows)
    .sum::<usize>();
  let shifted_row = relative_row.saturating_add(inserted_before);
  (shifted_row < viewport_height).then_some(shifted_row as u16)
}

fn apply_row_insertions_to_underlines(
  entries: &mut Vec<DiagnosticUnderlineRenderSpan>,
  plan: &RenderPlan,
  row_insertions: &[the_lib::render::RenderRowInsertion],
) {
  if row_insertions.is_empty() {
    return;
  }

  entries.retain_mut(|entry| {
    let Some(row) = remap_relative_row_with_insertions(
      entry.row as usize,
      plan.scroll.row,
      plan.viewport.height as usize,
      row_insertions,
    ) else {
      return false;
    };
    entry.row = row;
    true
  });
}

fn update_render_row_hash(row_hashes: &mut [u64], row: usize, value: impl Hash) {
  let Some(slot) = row_hashes.get_mut(row) else {
    return;
  };
  let mut hasher = DefaultHasher::new();
  slot.hash(&mut hasher);
  value.hash(&mut hasher);
  *slot = hasher.finish();
}

fn diagnostic_severity_code(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 1,
    DiagnosticSeverity::Warning => 2,
    DiagnosticSeverity::Information => 3,
    DiagnosticSeverity::Hint => 4,
  }
}

fn append_inline_diagnostic_row_hashes(
  row_hashes: &mut [u64],
  lines: &[InlineDiagnosticRenderLine],
) {
  for line in lines {
    update_render_row_hash(
      row_hashes,
      line.row as usize,
      (
        line.col,
        line.text.as_str(),
        diagnostic_severity_code(line.severity),
      ),
    );
  }
}

fn append_diagnostic_underline_row_hashes(
  row_hashes: &mut [u64],
  entries: &[DiagnosticUnderlineRenderSpan],
) {
  for entry in entries {
    update_render_row_hash(
      row_hashes,
      entry.row as usize,
      (
        entry.start_col,
        entry.end_col,
        diagnostic_severity_code(entry.severity),
      ),
    );
  }
}

fn build_render_layer_row_hashes(
  plan: &RenderPlan,
  inline_diagnostic_lines: &[InlineDiagnosticRenderLine],
  diagnostic_underlines: &[DiagnosticUnderlineRenderSpan],
) -> RenderLayerRowHashes {
  let mut row_hashes = base_render_layer_row_hashes(plan);
  append_inline_diagnostic_row_hashes(&mut row_hashes.decoration_rows, inline_diagnostic_lines);
  append_diagnostic_underline_row_hashes(&mut row_hashes.decoration_rows, diagnostic_underlines);
  row_hashes
}

fn active_inline_diagnostics(ctx: &Ctx) -> Vec<InlineDiagnostic> {
  inline_diagnostics_from_document(
    ctx.editor.document().text(),
    diagnostics_for_buffer(ctx, ctx.editor.active_buffer_id()).unwrap_or(&[]),
  )
}

fn inline_diagnostics_from_document(
  text: &Rope,
  diagnostics: &[Diagnostic],
) -> Vec<InlineDiagnostic> {
  let mut out = Vec::with_capacity(diagnostics.len());
  for diagnostic in diagnostics {
    let message = diagnostic.message.trim();
    if message.is_empty() {
      continue;
    }
    let start_char_idx = utf16_position_to_char_idx(
      text,
      diagnostic.range.start.line,
      diagnostic.range.start.character,
    );
    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
    out.push(InlineDiagnostic::new(
      start_char_idx,
      severity,
      message.to_string(),
    ));
  }
  out.sort_by_key(|diagnostic| diagnostic.start_char_idx);
  out
}

fn parse_inline_diagnostic_filter(value: &str) -> Option<the_lib::render::InlineDiagnosticFilter> {
  let normalized = value.trim().to_ascii_lowercase();
  match normalized.as_str() {
    "disable" | "off" | "none" => Some(the_lib::render::InlineDiagnosticFilter::Disable),
    "hint" => {
      Some(the_lib::render::InlineDiagnosticFilter::Enable(
        DiagnosticSeverity::Hint,
      ))
    },
    "info" | "information" => {
      Some(the_lib::render::InlineDiagnosticFilter::Enable(
        DiagnosticSeverity::Information,
      ))
    },
    "warning" | "warn" => {
      Some(the_lib::render::InlineDiagnosticFilter::Enable(
        DiagnosticSeverity::Warning,
      ))
    },
    "error" => {
      Some(the_lib::render::InlineDiagnosticFilter::Enable(
        DiagnosticSeverity::Error,
      ))
    },
    _ => None,
  }
}

fn inline_diagnostic_filter_label(filter: InlineDiagnosticFilter) -> &'static str {
  match filter {
    InlineDiagnosticFilter::Disable => "disable",
    InlineDiagnosticFilter::Enable(DiagnosticSeverity::Error) => "error",
    InlineDiagnosticFilter::Enable(DiagnosticSeverity::Warning) => "warning",
    InlineDiagnosticFilter::Enable(DiagnosticSeverity::Information) => "info",
    InlineDiagnosticFilter::Enable(DiagnosticSeverity::Hint) => "hint",
  }
}

fn inline_diagnostics_trace_enabled() -> bool {
  match env::var("THE_TERM_INLINE_DIAGNOSTICS_TRACE") {
    Ok(value) => {
      matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
      )
    },
    Err(_) => false,
  }
}

fn inline_diagnostics_config() -> InlineDiagnosticsConfig {
  let mut config = InlineDiagnosticsConfig::default();

  if let Ok(value) = env::var("THE_TERM_INLINE_DIAGNOSTICS_CURSOR_LINE")
    && let Some(filter) = parse_inline_diagnostic_filter(&value)
  {
    config.cursor_line = filter;
  }

  if let Ok(value) = env::var("THE_TERM_INLINE_DIAGNOSTICS_OTHER_LINES")
    && let Some(filter) = parse_inline_diagnostic_filter(&value)
  {
    config.other_lines = filter;
  }

  if let Ok(value) = env::var("THE_TERM_INLINE_DIAGNOSTICS_MIN_WIDTH")
    && let Ok(parsed) = value.trim().parse::<u16>()
  {
    config.min_diagnostic_width = parsed.max(1);
  }

  if let Ok(value) = env::var("THE_TERM_INLINE_DIAGNOSTICS_PREFIX_LEN")
    && let Ok(parsed) = value.trim().parse::<u16>()
  {
    config.prefix_len = parsed;
  }

  if let Ok(value) = env::var("THE_TERM_INLINE_DIAGNOSTICS_MAX_WRAP")
    && let Ok(parsed) = value.trim().parse::<u16>()
  {
    config.max_wrap = parsed.max(1);
  }

  if let Ok(value) = env::var("THE_TERM_INLINE_DIAGNOSTICS_MAX_PER_LINE")
    && let Ok(parsed) = value.trim().parse::<usize>()
  {
    config.max_diagnostics = parsed;
  }

  config
}

fn end_of_line_diagnostics_filter() -> InlineDiagnosticFilter {
  if let Ok(value) = env::var("THE_TERM_END_OF_LINE_DIAGNOSTICS")
    && let Some(filter) = parse_inline_diagnostic_filter(&value)
  {
    return filter;
  }
  InlineDiagnosticFilter::Enable(DiagnosticSeverity::Hint)
}

fn active_cursor_char_idx(ctx: &Ctx) -> Option<usize> {
  let doc = ctx.editor.document();
  let selection = doc.selection();
  let range = if let Some(active_cursor) = ctx.editor.view().active_cursor {
    selection.range_by_id(active_cursor).copied()
  } else {
    selection.ranges().first().copied()
  }?;
  Some(range.cursor(doc.text().slice(..)))
}

fn active_cursor_line_idx(ctx: &Ctx) -> Option<usize> {
  let doc = ctx.editor.document();
  let selection = doc.selection();
  let range = if let Some(active_cursor) = ctx.editor.view().active_cursor {
    selection.range_by_id(active_cursor).copied()
  } else {
    selection.ranges().first().copied()
  }?;
  Some(range.cursor_line(doc.text().slice(..)))
}

fn diagnostic_severity_rank(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 4,
    DiagnosticSeverity::Warning => 3,
    DiagnosticSeverity::Information => 2,
    DiagnosticSeverity::Hint => 1,
  }
}

fn inline_diagnostic_text_style(
  theme: &the_lib::render::theme::Theme,
  severity: DiagnosticSeverity,
) -> Style {
  let base = lib_style_to_ratatui(theme.try_get("ui.virtual").unwrap_or_default());
  let severity_style = diagnostic_theme_style(theme, severity);
  base.patch(lib_style_to_ratatui(severity_style))
}

fn draw_inline_diagnostic_lines(
  buf: &mut Buffer,
  area: Rect,
  content_x: u16,
  plan: &RenderPlan,
  theme: &the_lib::render::theme::Theme,
  inline_diagnostic_lines: &[InlineDiagnosticRenderLine],
) {
  let row_start = plan.scroll.row;
  let row_end = row_start.saturating_add(plan.viewport.height as usize);
  let content_width = plan.content_width();
  if content_width == 0 {
    return;
  }

  for line in inline_diagnostic_lines {
    if line.row < row_start || line.row >= row_end {
      continue;
    }
    if line.col < plan.scroll.col {
      continue;
    }

    let visible_col = line.col.saturating_sub(plan.scroll.col);
    if visible_col >= content_width {
      continue;
    }

    let y = area.y + (line.row - row_start) as u16;
    let x = content_x + visible_col as u16;
    if x >= area.x + area.width || y >= area.y + area.height {
      continue;
    }

    let style = inline_diagnostic_text_style(theme, line.severity);
    let max_width = content_width.saturating_sub(visible_col);
    buf.set_stringn(x, y, line.text.as_str(), max_width, style);
  }
}

fn draw_diagnostic_underlines(
  buf: &mut Buffer,
  area: Rect,
  content_x: u16,
  theme: &the_lib::render::theme::Theme,
  diagnostic_underlines: &[DiagnosticUnderlineRenderSpan],
) {
  for underline in diagnostic_underlines {
    let y = area.y.saturating_add(underline.row);
    if y >= area.y + area.height {
      continue;
    }

    let x_start = content_x.saturating_add(underline.start_col);
    let x_end = content_x.saturating_add(underline.end_col);
    if x_start >= area.x + area.width || x_start >= x_end {
      continue;
    }

    let style = lib_style_to_ratatui(diagnostic_underline_theme_style(theme, underline.severity));
    let x_limit = x_end.min(area.x + area.width);
    for x in x_start..x_limit {
      let cell = buf.get_mut(x, y);
      cell.set_style(cell.style().patch(style));
    }
  }
}

fn select_end_of_line_diagnostic<'a>(
  diagnostics: impl Iterator<Item = &'a Diagnostic>,
  inline_filter: InlineDiagnosticFilter,
  eol_filter: InlineDiagnosticFilter,
) -> Option<&'a Diagnostic> {
  let InlineDiagnosticFilter::Enable(eol_min) = eol_filter else {
    return None;
  };
  let eol_min_rank = diagnostic_severity_rank(eol_min);

  diagnostics
    .filter(|diagnostic| {
      let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
      let severity_rank = diagnostic_severity_rank(severity);
      if severity_rank < eol_min_rank {
        return false;
      }
      match inline_filter {
        InlineDiagnosticFilter::Disable => true,
        InlineDiagnosticFilter::Enable(inline_min) => {
          severity_rank < diagnostic_severity_rank(inline_min)
        },
      }
    })
    .max_by_key(|diagnostic| {
      diagnostic_severity_rank(diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning))
    })
}

fn draw_end_of_line_diagnostics(
  buf: &mut Buffer,
  area: Rect,
  content_x: u16,
  plan: &RenderPlan,
  ctx: &mut Ctx,
  diagnostics: &[Diagnostic],
  cursor_doc_line: Option<usize>,
) {
  let content_width = plan.content_width();
  if content_width == 0 {
    return;
  }
  if diagnostics.is_empty() {
    return;
  }

  let eol_filter = end_of_line_diagnostics_filter();
  if matches!(eol_filter, InlineDiagnosticFilter::Disable) {
    return;
  }

  let inline_config =
    inline_diagnostics_config().prepare(content_width.max(1) as u16, ctx.mode() != Mode::Insert);
  let trace_enabled = inline_diagnostics_trace_enabled();
  let mut considered_rows = 0usize;
  let mut rows_with_diagnostics = 0usize;
  let mut selected_count = 0usize;
  let mut rendered_count = 0usize;
  let mut clipped_by_width = 0usize;
  let mut clipped_by_viewport = 0usize;
  let mut first_selected: Option<Value> = None;

  let mut line_end_cols: BTreeMap<u16, usize> = BTreeMap::new();
  for line in &plan.lines {
    let end_col = line
      .spans
      .iter()
      .map(|span| span.col as usize + span.cols as usize)
      .max()
      .unwrap_or(0);
    line_end_cols
      .entry(line.row)
      .and_modify(|current| *current = (*current).max(end_col))
      .or_insert(end_col);
  }

  for visible_row in &plan.visible_rows {
    if !visible_row.first_visual_line {
      continue;
    }
    considered_rows = considered_rows.saturating_add(1);

    let diagnostics_on_row = diagnostics
      .iter()
      .filter(|diagnostic| diagnostic.range.start.line as usize == visible_row.doc_line)
      .count();
    if diagnostics_on_row > 0 {
      rows_with_diagnostics = rows_with_diagnostics.saturating_add(1);
    }

    let inline_filter = if cursor_doc_line == Some(visible_row.doc_line) {
      inline_config.cursor_line
    } else {
      inline_config.other_lines
    };
    let Some(diagnostic) = select_end_of_line_diagnostic(
      diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.range.start.line as usize == visible_row.doc_line),
      inline_filter,
      eol_filter,
    ) else {
      continue;
    };
    selected_count = selected_count.saturating_add(1);

    let message = diagnostic
      .message
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .collect::<Vec<_>>()
      .join("  ");
    if message.is_empty() {
      continue;
    }

    let start_col = line_end_cols
      .get(&visible_row.row)
      .copied()
      .unwrap_or(0)
      .saturating_add(1);
    if start_col >= content_width {
      clipped_by_width = clipped_by_width.saturating_add(1);
      continue;
    }

    let x = content_x + start_col as u16;
    let y = area.y + visible_row.row;
    if x >= area.x + area.width || y >= area.y + area.height {
      clipped_by_viewport = clipped_by_viewport.saturating_add(1);
      continue;
    }

    let max_width = content_width.saturating_sub(start_col);
    if max_width == 0 {
      clipped_by_width = clipped_by_width.saturating_add(1);
      continue;
    }

    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
    let style = inline_diagnostic_text_style(&ctx.ui_theme, severity);
    buf.set_stringn(x, y, &message, max_width, style);
    rendered_count = rendered_count.saturating_add(1);
    if first_selected.is_none() {
      first_selected = Some(json!({
        "doc_line": visible_row.doc_line,
        "render_row": visible_row.row,
        "severity": format!("{:?}", severity),
        "start_col": start_col,
        "message_preview": message.chars().take(120).collect::<String>(),
        "inline_filter": inline_diagnostic_filter_label(inline_filter),
      }));
    }
  }

  if trace_enabled || (rows_with_diagnostics > 0 && rendered_count == 0) {
    ctx.log_render_trace_value(
      "eol_diagnostics_render",
      json!({
        "mode": format!("{:?}", ctx.mode()),
        "content_width": content_width,
        "eol_filter": inline_diagnostic_filter_label(eol_filter),
        "inline_cursor_filter": inline_diagnostic_filter_label(inline_config.cursor_line),
        "inline_other_filter": inline_diagnostic_filter_label(inline_config.other_lines),
        "cursor_doc_line": cursor_doc_line,
        "doc_diagnostic_count": diagnostics.len(),
        "considered_rows": considered_rows,
        "rows_with_diagnostics": rows_with_diagnostics,
        "selected_count": selected_count,
        "rendered_count": rendered_count,
        "clipped_by_width": clipped_by_width,
        "clipped_by_viewport": clipped_by_viewport,
        "first_selected": first_selected,
      }),
    );
  }
}

fn fill_rect(buf: &mut Buffer, rect: Rect, style: Style) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let line = " ".repeat(rect.width as usize);
  for y in rect.y..rect.y + rect.height {
    buf.set_string(rect.x, y, &line, style);
  }
}

fn truncate_in_place(text: &mut String, max_chars: usize) {
  if max_chars == 0 {
    text.clear();
    return;
  }
  let mut count = 0usize;
  let mut cut = None;
  for (idx, _) in text.char_indices() {
    if count == max_chars {
      cut = Some(idx);
      break;
    }
    count += 1;
  }
  if let Some(cut) = cut {
    text.truncate(cut);
  }
}

fn truncate_with_ellipsis_in_place(text: &mut String, max_chars: usize) {
  if max_chars == 0 {
    text.clear();
    return;
  }
  let original_len = text.chars().count();
  truncate_in_place(text, max_chars);
  if original_len <= max_chars || max_chars == 0 {
    return;
  }
  if max_chars == 1 {
    text.clear();
    text.push('…');
    return;
  }
  truncate_in_place(text, max_chars.saturating_sub(1));
  text.push('…');
}

fn draw_fuzzy_match_line(
  buf: &mut Buffer,
  x: u16,
  y: u16,
  text: &str,
  max_chars: usize,
  base_style: Style,
  fuzzy_style: Style,
  match_indices: &[usize],
) {
  if max_chars == 0 {
    return;
  }

  let mut next_match_iter = match_indices.iter().copied();
  let mut next_match = next_match_iter.next();
  for (char_index, ch) in text.chars().enumerate() {
    if char_index >= max_chars {
      break;
    }

    let mut style = base_style;
    if next_match.is_some_and(|idx| idx == char_index) {
      style = style.patch(fuzzy_style);
      next_match = next_match_iter.next();
    }

    let mut utf8 = [0u8; 4];
    let symbol = ch.encode_utf8(&mut utf8);
    buf.set_stringn(x.saturating_add(char_index as u16), y, symbol, 1, style);
  }
}

fn draw_box(buf: &mut Buffer, rect: Rect, border: Style, fill: Style) {
  if rect.width < 2 || rect.height < 2 {
    return;
  }

  fill_rect(buf, rect, fill);

  let top = "─".repeat((rect.width - 2) as usize);
  let bottom = top.clone();
  buf.set_string(rect.x + 1, rect.y, &top, border);
  buf.set_string(rect.x + 1, rect.y + rect.height - 1, &bottom, border);
  buf.set_string(rect.x, rect.y, "┌", border);
  buf.set_string(rect.x + rect.width - 1, rect.y, "┐", border);
  buf.set_string(rect.x, rect.y + rect.height - 1, "└", border);
  buf.set_string(
    rect.x + rect.width - 1,
    rect.y + rect.height - 1,
    "┘",
    border,
  );

  for y in rect.y + 1..rect.y + rect.height - 1 {
    buf.set_string(rect.x, y, "│", border);
    buf.set_string(rect.x + rect.width - 1, y, "│", border);
  }
}

fn inner_rect(rect: Rect) -> Rect {
  if rect.width < 2 || rect.height < 2 {
    return rect;
  }
  Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
}

#[derive(Clone, Copy)]
struct PanelStyles {
  text:   Style,
  fill:   Style,
  border: Style,
}

fn reset_style() -> Style {
  Style::reset()
}

fn reset_style_with_colors(fg: Color, bg: Color) -> Style {
  reset_style().fg(fg).bg(bg)
}

fn theme_scope_color(
  ctx: &Ctx,
  scope: &str,
  kind: fn(LibStyle) -> Option<LibColor>,
) -> Option<Color> {
  ctx
    .ui_theme
    .try_get(scope)
    .and_then(kind)
    .map(lib_color_to_ratatui)
}

fn theme_scope_any_color(ctx: &Ctx, scope: &str) -> Option<Color> {
  ctx
    .ui_theme
    .try_get(scope)
    .and_then(|style| style.fg.or(style.bg))
    .map(lib_color_to_ratatui)
}

fn file_picker_panel_styles(ctx: &Ctx) -> PanelStyles {
  let picker_scope = ctx.ui_theme.try_get("ui.file_picker");
  let text_scope = ctx.ui_theme.try_get("ui.text");
  let background_scope = ctx.ui_theme.try_get("ui.background");
  let window_scope = ctx.ui_theme.try_get("ui.window");
  let text_fg = picker_scope
    .and_then(|style| style.fg)
    .or_else(|| text_scope.and_then(|style| style.fg))
    .map(lib_color_to_ratatui)
    .unwrap_or(Color::Reset);
  let fill_bg = picker_scope
    .and_then(|style| style.bg)
    .or_else(|| background_scope.and_then(|style| style.bg))
    .map(lib_color_to_ratatui)
    .unwrap_or(Color::Black);
  let border_fg = picker_scope
    .and_then(|style| style.fg)
    .or_else(|| text_scope.and_then(|style| style.fg))
    .or_else(|| window_scope.and_then(|style| style.fg))
    .map(lib_color_to_ratatui)
    .unwrap_or(text_fg);

  PanelStyles {
    text:   Style::default().fg(text_fg),
    fill:   reset_style_with_colors(text_fg, fill_bg),
    border: reset_style_with_colors(border_fg, fill_bg),
  }
}

fn overlay_panel_styles(ctx: &Ctx, role: &str) -> PanelStyles {
  let text = theme_scope_color(ctx, role, |style| style.fg)
    .or_else(|| theme_scope_color(ctx, "ui.text", |style| style.fg))
    .unwrap_or(Color::Reset);
  let fill = theme_scope_color(ctx, role, |style| style.bg)
    .or_else(|| theme_scope_color(ctx, "ui.window", |style| style.bg))
    .or_else(|| theme_scope_color(ctx, "ui.background", |style| style.bg))
    .unwrap_or(Color::Black);
  let role_border_scope = format!("{role}.border");
  let border = theme_scope_any_color(ctx, role_border_scope.as_str())
    .or_else(|| theme_scope_any_color(ctx, "ui.popup.border"))
    .or_else(|| theme_scope_any_color(ctx, "ui.window"))
    .or_else(|| theme_scope_any_color(ctx, "ui.background.separator"))
    .unwrap_or(text);

  PanelStyles {
    text:   Style::default().fg(text),
    fill:   reset_style_with_colors(text, fill),
    border: reset_style_with_colors(border, fill),
  }
}

fn statusline_panel_styles(ctx: &Ctx) -> PanelStyles {
  overlay_panel_styles(ctx, "ui.statusline")
}

fn docs_panel_styles(ctx: &Ctx) -> PanelStyles {
  let text = theme_scope_color(ctx, "ui.text", |style| style.fg).unwrap_or(Color::Reset);
  let fill = theme_scope_color(ctx, "ui.popup", |style| style.bg)
    .or_else(|| theme_scope_color(ctx, "ui.background", |style| style.bg))
    .unwrap_or(Color::Black);
  let border = theme_scope_any_color(ctx, "ui.popup.border")
    .or_else(|| theme_scope_any_color(ctx, "ui.window"))
    .or_else(|| theme_scope_any_color(ctx, "ui.background.separator"))
    .unwrap_or(text);

  PanelStyles {
    text:   Style::default().fg(text),
    fill:   reset_style_with_colors(text, fill),
    border: reset_style_with_colors(border, fill),
  }
}

fn file_picker_is_diagnostics(picker: &the_default::FilePickerState) -> bool {
  picker.title.starts_with("Diagnostics ·") || picker.title.starts_with("Workspace Diagnostics ·")
}

fn file_picker_is_symbols(picker: &the_default::FilePickerState) -> bool {
  picker.title.starts_with("Lsp Symbols")
    || picker.title.starts_with("Document Symbols")
    || picker.title.starts_with("Workspace Symbols")
}

fn file_picker_is_live_grep(picker: &the_default::FilePickerState) -> bool {
  picker.title.starts_with("Live Grep") || picker.title.starts_with("Global Search")
}

fn split_prefix_chars(text: &str, max_chars: usize) -> (&str, &str) {
  if max_chars == 0 {
    return ("", text);
  }
  let mut seen = 0usize;
  for (idx, _) in text.char_indices() {
    if seen == max_chars {
      return (&text[..idx], &text[idx..]);
    }
    seen = seen.saturating_add(1);
  }
  (text, "")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SymbolsPickerDisplayRow {
  name:      String,
  container: String,
  detail:    String,
  kind:      String,
  path:      String,
  line:      usize,
  column:    usize,
  depth:     usize,
}

fn parse_symbols_picker_display(display: &str) -> SymbolsPickerDisplayRow {
  let mut fields = display.split('\t');
  let mut name = fields.next().unwrap_or_default().trim().to_string();
  let container = fields.next().unwrap_or_default().trim().to_string();
  let detail = fields.next().unwrap_or_default().trim().to_string();
  let kind = fields.next().unwrap_or_default().trim().to_string();
  let path = fields.next().unwrap_or_default().trim().to_string();
  let line = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let column = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let depth = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(0);

  if name.is_empty() {
    name = display.trim().to_string();
  }
  if name.is_empty() {
    name = "<unnamed>".to_string();
  }

  SymbolsPickerDisplayRow {
    name,
    container,
    detail,
    kind,
    path,
    line,
    column,
    depth,
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LiveGrepDisplayRow {
  path:    String,
  line:    usize,
  column:  usize,
  snippet: String,
}

fn live_grep_display_path(display: &str) -> &str {
  display
    .split_once('\t')
    .map(|(path, _)| path.trim())
    .unwrap_or_default()
}

fn live_grep_item_is_header(item: &the_default::FilePickerItem) -> bool {
  matches!(
    &item.action,
    the_default::FilePickerItemAction::GroupHeader { .. }
  )
}

fn live_grep_item_path<'a>(item: &'a the_default::FilePickerItem) -> &'a str {
  if live_grep_item_is_header(item) {
    item.display.trim()
  } else {
    live_grep_display_path(item.display.as_str())
  }
}

fn parse_live_grep_picker_display(display: &str) -> LiveGrepDisplayRow {
  if !display.contains('\t') {
    return LiveGrepDisplayRow {
      path:    String::new(),
      line:    1,
      column:  1,
      snippet: display.trim().to_string(),
    };
  }

  let mut fields = display.splitn(4, '\t');
  let path = fields.next().unwrap_or_default().trim().to_string();
  let line = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let column = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let mut snippet = fields.next().unwrap_or_default().to_string();
  if snippet.is_empty() && path.is_empty() {
    snippet = display.trim().to_string();
  }

  LiveGrepDisplayRow {
    path,
    line,
    column,
    snippet,
  }
}

fn symbol_picker_icon_glyph(kind: &str, fallback_icon: &str) -> &'static str {
  match kind {
    "FILE" => "󰈙",
    "MODULE" | "NAMESPACE" | "PACKAGE" => "󰆍",
    "CLASS" | "STRUCT" => "",
    "INTERFACE" => "",
    "METHOD" | "FUNCTION" | "CONSTRUCTOR" => "󰊕",
    "PROPERTY" => "󰜢",
    "FIELD" => "󰆨",
    "ENUM" => "",
    "ENUM_MEMBER" => "",
    "VARIABLE" => "󰀫",
    "CONSTANT" => "󰏿",
    "TYPE_PARAM" => "󰊄",
    "EVENT" => "",
    "OPERATOR" => "󰆕",
    "KEY" => "󰌆",
    _ => file_picker_icon_glyph(fallback_icon, false),
  }
}

fn symbol_picker_kind_color(kind: &str) -> Color {
  match kind {
    "METHOD" | "FUNCTION" | "CONSTRUCTOR" | "OPERATOR" => Color::Rgb(0xDB, 0xBF, 0xEF),
    "FIELD" | "VARIABLE" | "PROPERTY" | "VALUE" | "REFERENCE" => Color::Rgb(0xA4, 0xA0, 0xE8),
    "CLASS" | "INTERFACE" | "ENUM" | "STRUCT" | "TYPE_PARAM" => Color::Rgb(0xEF, 0xBA, 0x5D),
    "MODULE" | "NAMESPACE" | "PACKAGE" | "FILE" | "ENUM_MEMBER" | "CONSTANT" => {
      Color::Rgb(0xE8, 0xDC, 0xA0)
    },
    "EVENT" => Color::Rgb(0xF4, 0x78, 0x68),
    _ => Color::Rgb(0xCC, 0xCC, 0xCC),
  }
}

fn symbol_picker_tree_prefix(depth: usize, next_depth: usize) -> String {
  if depth == 0 {
    return String::new();
  }

  let mut prefix = String::new();
  for _ in 0..depth.saturating_sub(1) {
    prefix.push_str("│ ");
  }
  if next_depth > depth {
    prefix.push_str("├ ");
  } else {
    prefix.push_str("└ ");
  }
  prefix
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DiagnosticsPickerDisplayRow {
  severity: String,
  source:   String,
  code:     String,
  location: Option<String>,
  message:  String,
}

fn parse_diagnostics_picker_display(display: &str) -> DiagnosticsPickerDisplayRow {
  let (severity, rest) = split_prefix_chars(display, 7);
  let rest = rest.strip_prefix(' ').unwrap_or(rest);
  let (source, rest) = split_prefix_chars(rest, 14);
  let rest = rest.strip_prefix(' ').unwrap_or(rest);
  let (code, rest) = split_prefix_chars(rest, 16);
  let rest = rest.strip_prefix(' ').unwrap_or(rest).trim_start();

  let (location, message) = if let Some((location, message)) = rest.split_once("  ") {
    let location = location.trim();
    let message = message.trim();
    let location = if location.is_empty() {
      None
    } else {
      Some(location.to_string())
    };
    (location, message.to_string())
  } else {
    (None, rest.to_string())
  };

  DiagnosticsPickerDisplayRow {
    severity: severity.trim().to_string(),
    source: source.trim().to_string(),
    code: code.trim().to_string(),
    location,
    message,
  }
}

fn diagnostic_severity_from_icon(icon: &str) -> Option<DiagnosticSeverity> {
  match icon {
    "diagnostic_error" => Some(DiagnosticSeverity::Error),
    "diagnostic_warning" => Some(DiagnosticSeverity::Warning),
    "diagnostic_info" => Some(DiagnosticSeverity::Information),
    "diagnostic_hint" => Some(DiagnosticSeverity::Hint),
    _ => None,
  }
}

fn diagnostic_severity_color(
  theme: &the_lib::render::theme::Theme,
  severity: DiagnosticSeverity,
) -> Color {
  diagnostic_theme_style(theme, severity)
    .fg
    .map(lib_color_to_ratatui)
    .unwrap_or_else(|| {
      match severity {
        DiagnosticSeverity::Error => Color::LightRed,
        DiagnosticSeverity::Warning => Color::LightYellow,
        DiagnosticSeverity::Information => Color::LightBlue,
        DiagnosticSeverity::Hint => Color::LightCyan,
      }
    })
}

fn file_picker_preview_focus_styles(
  theme: &the_lib::render::theme::Theme,
  text_style: Style,
  accent_color: Option<Color>,
) -> (Style, Style) {
  let mut row_style = reset_style();
  if let Some(bg) = theme
    .try_get("ui.cursorline.active")
    .and_then(|style| style.bg)
    .map(lib_color_to_ratatui)
  {
    row_style = row_style.bg(bg);
  } else if let Some(bg) = theme
    .try_get("ui.file_picker.list.selected")
    .and_then(|style| style.bg)
    .map(lib_color_to_ratatui)
  {
    row_style = row_style.bg(bg);
  }
  if let Some(fg) = text_style.fg {
    row_style = row_style.fg(fg);
  }

  let marker_color = accent_color;
  let mut marker_style = Style::default().add_modifier(Modifier::BOLD);
  if let Some(color) = marker_color.or(text_style.fg) {
    marker_style = marker_style.fg(color);
  }

  (row_style, marker_style)
}

fn file_picker_match_highlight_style(
  theme: &the_lib::render::theme::Theme,
  text_style: Style,
  accent_color: Option<Color>,
) -> Style {
  let mut style = theme
    .try_get("search.match")
    .map(lib_style_to_ratatui)
    .unwrap_or_default()
    .add_modifier(Modifier::BOLD);

  if style.fg.is_none()
    && let Some(color) = accent_color.or(text_style.fg)
  {
    style = style.fg(color);
  }
  if style.bg.is_none()
    && let Some(bg) = theme
      .try_get("ui.selection")
      .and_then(|scope| scope.bg)
      .map(lib_color_to_ratatui)
  {
    style = style.bg(bg);
  }
  style
}

fn highlight_char_range(
  buf: &mut Buffer,
  x: u16,
  y: u16,
  max_width: u16,
  start: usize,
  end: usize,
  style: Style,
) {
  if max_width == 0 || end <= start {
    return;
  }
  let limit = max_width as usize;
  for idx in start..end {
    if idx >= limit {
      break;
    }
    let cell = buf.get_mut(x.saturating_add(idx as u16), y);
    cell.set_style(cell.style().patch(style));
  }
}

fn draw_diagnostics_picker_row(
  buf: &mut Buffer,
  row_rect: Rect,
  y: u16,
  item: &the_default::FilePickerItem,
  text_style: Style,
  theme: &the_lib::render::theme::Theme,
  selected_fg: Option<Color>,
  is_selected: bool,
  is_hovered: bool,
) {
  if row_rect.width == 0 {
    return;
  }

  let parsed = parse_diagnostics_picker_display(item.display.as_str());
  let severity = diagnostic_severity_from_icon(item.icon.as_str());
  let severity_label = if parsed.severity.is_empty() {
    match severity {
      Some(DiagnosticSeverity::Error) => "ERROR".to_string(),
      Some(DiagnosticSeverity::Warning) => "WARN".to_string(),
      Some(DiagnosticSeverity::Information) => "INFO".to_string(),
      Some(DiagnosticSeverity::Hint) => "HINT".to_string(),
      None => "INFO".to_string(),
    }
  } else {
    parsed.severity.clone()
  };

  let mut base_style = text_style;
  if is_selected && let Some(fg) = selected_fg {
    base_style = base_style.fg(fg);
  }
  if is_hovered {
    base_style = base_style.add_modifier(Modifier::UNDERLINED);
  }

  let severity_color = severity.map(|severity| diagnostic_severity_color(theme, severity));
  let mut severity_style = base_style.add_modifier(Modifier::BOLD);
  if let Some(color) = severity_color {
    severity_style = severity_style.fg(color);
  }
  let meta_style = base_style.add_modifier(Modifier::DIM);
  let code_style = theme
    .try_get("special")
    .and_then(|style| style.fg)
    .map(lib_color_to_ratatui)
    .map(|color| base_style.fg(color).add_modifier(Modifier::BOLD))
    .unwrap_or_else(|| meta_style.add_modifier(Modifier::BOLD));

  let row_end_x = row_rect.x.saturating_add(row_rect.width);
  let marker_symbol = if is_selected { "▌" } else { "▏" };
  let marker_style = if let Some(color) = severity_color {
    Style::default().fg(color)
  } else {
    meta_style
  };
  buf.set_stringn(row_rect.x, y, marker_symbol, 1, marker_style);

  let icon_x = row_rect.x.saturating_add(1);
  if icon_x < row_end_x {
    let icon = file_picker_icon_glyph(item.icon.as_str(), item.is_dir);
    let icon_style = if severity_color.is_some() {
      severity_style
    } else {
      base_style
    };
    buf.set_stringn(
      icon_x,
      y,
      icon,
      row_end_x.saturating_sub(icon_x) as usize,
      icon_style,
    );
  }

  let mut cursor_x = icon_x.saturating_add(2);
  if cursor_x >= row_end_x {
    return;
  }

  let mut severity_text = severity_label;
  truncate_in_place(&mut severity_text, 7);
  if !severity_text.is_empty() {
    let max = row_end_x.saturating_sub(cursor_x) as usize;
    buf.set_stringn(cursor_x, y, severity_text.as_str(), max, severity_style);
    cursor_x = cursor_x.saturating_add(severity_text.chars().count() as u16 + 1);
  }

  let mut draw_meta = |value: &str, style: Style, cursor_x: &mut u16| {
    if value.is_empty() || *cursor_x >= row_end_x {
      return;
    }
    let mut value = value.to_string();
    truncate_in_place(&mut value, row_end_x.saturating_sub(*cursor_x) as usize);
    if value.is_empty() {
      return;
    }
    buf.set_stringn(
      *cursor_x,
      y,
      value.as_str(),
      row_end_x.saturating_sub(*cursor_x) as usize,
      style,
    );
    *cursor_x = (*cursor_x).saturating_add(value.chars().count() as u16 + 1);
  };

  if parsed.source != "-" && !parsed.source.is_empty() {
    draw_meta(parsed.source.as_str(), meta_style, &mut cursor_x);
  }
  if parsed.code != "-" && !parsed.code.is_empty() {
    draw_meta(parsed.code.as_str(), code_style, &mut cursor_x);
  }

  let mut right_limit = row_end_x;
  if let Some(location) = parsed.location.as_deref().filter(|value| !value.is_empty()) {
    let mut location = location.to_string();
    let max_loc_chars = (row_rect.width as usize / 3).max(12);
    truncate_in_place(&mut location, max_loc_chars);
    let location_width = location.chars().count() as u16;
    if location_width > 0 && location_width.saturating_add(2) < row_end_x.saturating_sub(cursor_x) {
      let location_x = row_end_x.saturating_sub(location_width);
      buf.set_stringn(
        location_x,
        y,
        location.as_str(),
        location_width as usize,
        meta_style,
      );
      right_limit = location_x.saturating_sub(1);
    }
  }

  let mut message = if parsed.message.is_empty() {
    item.display.clone()
  } else {
    parsed.message
  };
  let max_message_width = right_limit.saturating_sub(cursor_x) as usize;
  if max_message_width == 0 {
    return;
  }
  truncate_in_place(&mut message, max_message_width);
  if !message.is_empty() {
    buf.set_stringn(cursor_x, y, message.as_str(), max_message_width, base_style);
  }
}

fn draw_symbols_picker_row(
  buf: &mut Buffer,
  row_rect: Rect,
  y: u16,
  item: &the_default::FilePickerItem,
  next_item: Option<&the_default::FilePickerItem>,
  text_style: Style,
  selected_fg: Option<Color>,
  fuzzy_highlight_style: Style,
  is_selected: bool,
  is_hovered: bool,
  match_indices: &[usize],
) {
  if row_rect.width == 0 {
    return;
  }

  let parsed = parse_symbols_picker_display(item.display.as_str());
  let next_depth = next_item
    .map(|item| parse_symbols_picker_display(item.display.as_str()).depth)
    .unwrap_or(0);
  let tree_prefix = symbol_picker_tree_prefix(parsed.depth, next_depth);
  let kind_color = symbol_picker_kind_color(parsed.kind.as_str());
  let icon = symbol_picker_icon_glyph(parsed.kind.as_str(), item.icon.as_str());
  let location = if parsed.path.is_empty() {
    format!("{}:{}", parsed.line, parsed.column)
  } else {
    format!("{}:{}:{}", parsed.path, parsed.line, parsed.column)
  };

  let mut base_style = text_style;
  if is_selected && let Some(fg) = selected_fg {
    base_style = base_style.fg(fg);
  }
  if is_hovered {
    base_style = base_style.add_modifier(Modifier::UNDERLINED);
  }
  let tree_style = base_style.add_modifier(Modifier::DIM);
  let icon_style = base_style.fg(kind_color).add_modifier(Modifier::BOLD);
  let kind_style = icon_style.add_modifier(Modifier::DIM);
  let detail_style = base_style.add_modifier(Modifier::DIM);

  let row_end_x = row_rect.x.saturating_add(row_rect.width);
  let marker_symbol = if is_selected { "▌" } else { "▏" };
  let marker_style = if is_selected {
    Style::default().fg(kind_color)
  } else {
    tree_style
  };
  buf.set_stringn(row_rect.x, y, marker_symbol, 1, marker_style);

  let mut cursor_x = row_rect.x.saturating_add(1);
  if cursor_x >= row_end_x {
    return;
  }

  if !tree_prefix.is_empty() {
    let max = row_end_x.saturating_sub(cursor_x) as usize;
    buf.set_stringn(cursor_x, y, tree_prefix.as_str(), max, tree_style);
    cursor_x = cursor_x.saturating_add(tree_prefix.chars().count() as u16);
  }
  if cursor_x >= row_end_x {
    return;
  }

  let icon_width = icon.chars().count() as u16;
  buf.set_stringn(
    cursor_x,
    y,
    icon,
    row_end_x.saturating_sub(cursor_x) as usize,
    icon_style,
  );
  cursor_x = cursor_x.saturating_add(icon_width.saturating_add(1));
  if cursor_x >= row_end_x {
    return;
  }

  let mut right_limit = row_end_x;
  let mut location_label = location;
  let max_loc_chars = (row_rect.width as usize / 3).max(14);
  truncate_in_place(&mut location_label, max_loc_chars);
  let location_width = location_label.chars().count() as u16;
  if location_width > 0 && location_width.saturating_add(2) < row_end_x.saturating_sub(cursor_x) {
    let location_x = row_end_x.saturating_sub(location_width);
    buf.set_stringn(
      location_x,
      y,
      location_label.as_str(),
      location_width as usize,
      detail_style,
    );
    right_limit = location_x.saturating_sub(1);
  }

  let mut kind_label = parsed.kind.clone();
  truncate_in_place(&mut kind_label, 13);
  let kind_width = kind_label.chars().count() as u16;
  if kind_width > 0 && kind_width.saturating_add(2) < right_limit.saturating_sub(cursor_x) {
    let kind_x = right_limit.saturating_sub(kind_width);
    buf.set_stringn(
      kind_x,
      y,
      kind_label.as_str(),
      kind_width as usize,
      kind_style,
    );
    right_limit = kind_x.saturating_sub(1);
  }

  let content_width = right_limit.saturating_sub(cursor_x) as usize;
  if content_width == 0 {
    return;
  }

  let name_len = parsed.name.chars().count();
  let name_match_indices: Vec<usize> = match_indices
    .iter()
    .copied()
    .filter(|index| *index < name_len)
    .collect();
  draw_fuzzy_match_line(
    buf,
    cursor_x,
    y,
    parsed.name.as_str(),
    content_width,
    base_style.add_modifier(Modifier::BOLD),
    fuzzy_highlight_style,
    &name_match_indices,
  );

  let mut suffix = String::new();
  if !parsed.detail.is_empty() {
    suffix.push_str("  ");
    suffix.push_str(parsed.detail.as_str());
  }
  if !parsed.container.is_empty() {
    if suffix.is_empty() {
      suffix.push_str("  ");
    } else {
      suffix.push_str("  · ");
    }
    suffix.push_str(parsed.container.as_str());
  }
  if suffix.is_empty() {
    return;
  }

  let name_width = name_len as u16;
  let suffix_x = cursor_x.saturating_add(name_width);
  if suffix_x >= right_limit {
    return;
  }
  let max_suffix = right_limit.saturating_sub(suffix_x) as usize;
  if max_suffix == 0 {
    return;
  }
  truncate_in_place(&mut suffix, max_suffix);
  if !suffix.is_empty() {
    buf.set_stringn(suffix_x, y, suffix.as_str(), max_suffix, detail_style);
  }
}

fn draw_live_grep_picker_row(
  buf: &mut Buffer,
  row_rect: Rect,
  y: u16,
  item: &the_default::FilePickerItem,
  previous_item: Option<&the_default::FilePickerItem>,
  text_style: Style,
  theme: &the_lib::render::theme::Theme,
  selected_fg: Option<Color>,
  is_selected: bool,
  is_hovered: bool,
  match_indices: &[usize],
) {
  if row_rect.width == 0 {
    return;
  }

  let is_header = live_grep_item_is_header(item);
  let parsed = parse_live_grep_picker_display(item.display.as_str());
  let previous_path = previous_item.map(live_grep_item_path).unwrap_or_default();
  let is_new_group = !is_header && !parsed.path.is_empty() && parsed.path != previous_path;

  let mut base_style = text_style;
  if is_selected && let Some(fg) = selected_fg {
    base_style = base_style.fg(fg);
  }
  if is_hovered {
    base_style = base_style.add_modifier(Modifier::UNDERLINED);
  }

  let accent = theme
    .try_get("search.match")
    .and_then(|scope| scope.fg)
    .or_else(|| theme.try_get("special").and_then(|scope| scope.fg))
    .map(lib_color_to_ratatui);
  let marker_style = if is_selected {
    Style::default()
      .fg(accent.unwrap_or(Color::LightBlue))
      .add_modifier(Modifier::BOLD)
  } else {
    base_style.add_modifier(Modifier::DIM)
  };
  let location_style = base_style.add_modifier(Modifier::DIM);
  let mut match_style = file_picker_match_highlight_style(theme, base_style, accent);
  // Live grep rows already have strong structural cues; keep match emphasis
  // foreground-only to avoid heavy selection-like background blocks.
  match_style.bg = base_style.bg;

  let row_end_x = row_rect.x.saturating_add(row_rect.width);
  let marker_symbol = if is_header {
    " "
  } else if is_selected {
    "▌"
  } else {
    "▏"
  };
  buf.set_stringn(row_rect.x, y, marker_symbol, 1, marker_style);

  let guide_x = row_rect.x.saturating_add(1);
  if guide_x < row_end_x {
    let guide_symbol = if is_header || is_new_group {
      " "
    } else {
      "│"
    };
    buf.set_stringn(
      guide_x,
      y,
      guide_symbol,
      1,
      base_style.add_modifier(Modifier::DIM),
    );
  }

  let icon_x = row_rect.x.saturating_add(2);
  if icon_x < row_end_x && (is_new_group || is_header) {
    let icon = file_picker_icon_glyph(item.icon.as_str(), item.is_dir);
    buf.set_stringn(
      icon_x,
      y,
      icon,
      row_end_x.saturating_sub(icon_x) as usize,
      base_style,
    );
  }

  let cursor_x = if is_new_group || is_header {
    icon_x.saturating_add(2)
  } else {
    icon_x.saturating_add(1)
  };
  if cursor_x >= row_end_x {
    return;
  }

  if is_header {
    let header_path = item.display.trim();
    let (dir_part, file_part) = match header_path.rsplit_once('/') {
      Some((dir, file)) => (dir, file),
      None => ("", header_path),
    };
    let file_name = if file_part.is_empty() {
      header_path
    } else {
      file_part
    };
    let mut cursor_x = cursor_x;
    let mut content_width = row_end_x.saturating_sub(cursor_x) as usize;
    if content_width == 0 {
      return;
    }

    if !file_name.is_empty() {
      let mut file_label = file_name.to_string();
      truncate_in_place(&mut file_label, content_width);
      let file_len = file_label.chars().count();
      if file_len > 0 {
        buf.set_stringn(
          cursor_x,
          y,
          file_label.as_str(),
          file_len,
          base_style.add_modifier(Modifier::BOLD),
        );
        cursor_x = cursor_x.saturating_add(file_len as u16);
        content_width = content_width.saturating_sub(file_len);
      }
    }

    if !dir_part.is_empty() && content_width > 3 {
      buf.set_stringn(cursor_x, y, "  ", 2, location_style);
      cursor_x = cursor_x.saturating_add(2);
      content_width = content_width.saturating_sub(2);

      let mut dir_label = dir_part.to_string();
      truncate_in_place(&mut dir_label, content_width);
      let dir_len = dir_label.chars().count();
      if dir_len > 0 {
        buf.set_stringn(cursor_x, y, dir_label.as_str(), dir_len, location_style);
      }
    }
    return;
  }

  let snippet = parsed.snippet.trim_end();
  let snippet_offset = item
    .display
    .chars()
    .count()
    .saturating_sub(snippet.chars().count());
  let snippet_match_indices: Vec<usize> = match_indices
    .iter()
    .copied()
    .filter(|index| *index >= snippet_offset)
    .map(|index| index - snippet_offset)
    .collect();

  let mut cursor_x = cursor_x;
  let mut content_width = row_end_x.saturating_sub(cursor_x) as usize;
  if content_width == 0 {
    return;
  }

  if is_new_group {
    let (dir_part, file_part) = match parsed.path.rsplit_once('/') {
      Some((dir, file)) => (dir, file),
      None => ("", parsed.path.as_str()),
    };
    let file_name = if file_part.is_empty() {
      parsed.path.as_str()
    } else {
      file_part
    };
    if !file_name.is_empty() {
      let mut file_label = file_name.to_string();
      truncate_in_place(&mut file_label, content_width);
      let file_len = file_label.chars().count();
      if file_len > 0 {
        buf.set_stringn(
          cursor_x,
          y,
          file_label.as_str(),
          file_len,
          base_style.add_modifier(Modifier::BOLD),
        );
        cursor_x = cursor_x.saturating_add(file_len as u16);
        content_width = content_width.saturating_sub(file_len);
      }
    }

    if !dir_part.is_empty() && content_width > 3 {
      buf.set_stringn(cursor_x, y, "  ", 2, location_style);
      cursor_x = cursor_x.saturating_add(2);
      content_width = content_width.saturating_sub(2);

      let mut dir_label = dir_part.to_string();
      truncate_in_place(&mut dir_label, content_width);
      let dir_len = dir_label.chars().count();
      if dir_len > 0 {
        buf.set_stringn(cursor_x, y, dir_label.as_str(), dir_len, location_style);
        cursor_x = cursor_x.saturating_add(dir_len as u16);
        content_width = content_width.saturating_sub(dir_len);
      }
    }

    if content_width > 3 {
      buf.set_stringn(cursor_x, y, "  ", 2, location_style);
      cursor_x = cursor_x.saturating_add(2);
      content_width = content_width.saturating_sub(2);
    }
  }

  let location_label = format!(":{}:{}", parsed.line, parsed.column);
  if content_width > 0 {
    let mut location = location_label;
    truncate_in_place(&mut location, content_width);
    let location_len = location.chars().count();
    if location_len > 0 {
      buf.set_stringn(cursor_x, y, location.as_str(), location_len, location_style);
      cursor_x = cursor_x.saturating_add(location_len as u16);
      content_width = content_width.saturating_sub(location_len);
    }
  }

  if content_width > 2 {
    buf.set_stringn(cursor_x, y, "  ", 2, location_style);
    cursor_x = cursor_x.saturating_add(2);
    content_width = content_width.saturating_sub(2);
  }

  if content_width > 0 {
    draw_fuzzy_match_line(
      buf,
      cursor_x,
      y,
      snippet,
      content_width,
      base_style,
      match_style,
      &snippet_match_indices,
    );

    if let Some((start, end)) = item.preview_col {
      highlight_char_range(
        buf,
        cursor_x,
        y,
        content_width as u16,
        start,
        end,
        match_style,
      );
    }
  }
}

fn software_cursor_style(theme: &the_lib::render::theme::Theme) -> Style {
  theme
    .try_get("ui.cursor.active")
    .or_else(|| theme.try_get("ui.cursor"))
    .map(lib_style_to_ratatui)
    .unwrap_or_else(|| Style::default().add_modifier(Modifier::REVERSED))
}

fn unfocused_pane_cursor_style(theme: &the_lib::render::theme::Theme) -> Style {
  theme
    .try_get("ui.cursor.match")
    .or_else(|| theme.try_get("ui.cursor"))
    .map(lib_style_to_ratatui)
    .unwrap_or_else(|| Style::default().add_modifier(Modifier::REVERSED))
}

fn draw_software_cursor_cell(buf: &mut Buffer, x: u16, y: u16, cursor_style: Style) {
  let cell = buf.get_mut(x, y);
  cell.set_style(cell.style().patch(cursor_style));
}

fn cursor_shape_color(cursor_style: Style, base_style: Style) -> Color {
  cursor_style
    .bg
    .or(cursor_style.fg)
    .or(base_style.fg)
    .unwrap_or(Color::White)
}

fn draw_buffer_cursor_cell(
  buf: &mut Buffer,
  x: u16,
  y: u16,
  kind: LibCursorKind,
  cursor_style: Style,
) {
  let cell = buf.get_mut(x, y);
  let base_style = cell.style();

  match kind {
    LibCursorKind::Hidden => {},
    LibCursorKind::Block => {
      cell.set_style(base_style.patch(cursor_style));
    },
    LibCursorKind::Underline => {
      let color = cursor_shape_color(cursor_style, base_style);
      let overlay = Style::default()
        .underline_color(color)
        .add_modifier(Modifier::UNDERLINED);
      cell.set_style(base_style.patch(overlay));
    },
    LibCursorKind::Bar => {
      let color = cursor_shape_color(cursor_style, base_style);
      cell.set_symbol("▏");
      cell.set_style(base_style.patch(Style::default().fg(color)));
    },
    LibCursorKind::Hollow => {
      let color = cursor_shape_color(cursor_style, base_style);
      cell.set_symbol("□");
      cell.set_style(base_style.patch(Style::default().fg(color)));
    },
  }
}

#[derive(Clone)]
struct StyledTextRun {
  text:  String,
  style: Style,
  kind:  DocsSemanticKind,
}

#[derive(Clone, Copy)]
struct CompletionDocsStyles {
  base:             Style,
  heading:          [Style; 6],
  bullet:           Style,
  quote:            Style,
  code:             Style,
  active_parameter: Style,
  link:             Style,
  rule:             Style,
}

impl CompletionDocsStyles {
  fn default(base: Style) -> Self {
    let heading = [
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
    ];
    Self {
      base,
      heading,
      bullet: base.add_modifier(Modifier::BOLD),
      quote: base.add_modifier(Modifier::DIM),
      code: base.add_modifier(Modifier::DIM),
      active_parameter: base
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED),
      link: base.add_modifier(Modifier::UNDERLINED),
      rule: base.add_modifier(Modifier::DIM),
    }
  }
}

fn theme_style_or(ctx: &Ctx, scope: &str, fallback: Style) -> Style {
  ctx
    .ui_theme
    .try_get(scope)
    .map(lib_style_to_ratatui)
    .map(|style| fallback.patch(style))
    .unwrap_or(fallback)
}

fn completion_docs_styles(ctx: &Ctx, base: Style) -> CompletionDocsStyles {
  let mut styles = CompletionDocsStyles::default(base);
  styles.heading = [
    theme_style_or(ctx, "markup.heading.1", styles.heading[0]),
    theme_style_or(ctx, "markup.heading.2", styles.heading[1]),
    theme_style_or(ctx, "markup.heading.3", styles.heading[2]),
    theme_style_or(ctx, "markup.heading.4", styles.heading[3]),
    theme_style_or(ctx, "markup.heading.5", styles.heading[4]),
    theme_style_or(ctx, "markup.heading.6", styles.heading[5]),
  ];
  styles.bullet = theme_style_or(ctx, "markup.list.unnumbered", styles.bullet);
  styles.quote = theme_style_or(ctx, "markup.quote", styles.quote);
  styles.code = theme_style_or(ctx, "markup.raw.inline", styles.code);
  styles.active_parameter = theme_style_or(
    ctx,
    "ui.selection.active",
    theme_style_or(ctx, "ui.selection", styles.active_parameter),
  );
  styles.link = theme_style_or(ctx, "markup.link.text", styles.link);
  styles.rule = theme_style_or(ctx, "punctuation.special", styles.rule);
  styles
}

fn push_styled_run(
  runs: &mut Vec<StyledTextRun>,
  text: String,
  style: Style,
  kind: DocsSemanticKind,
) {
  if text.is_empty() {
    return;
  }
  if let Some(last) = runs.last_mut()
    && last.style == style
    && last.kind == kind
  {
    last.text.push_str(&text);
    return;
  }
  runs.push(StyledTextRun { text, style, kind });
}

fn docs_runs_from_inline(
  inline_runs: &[DocsInlineRun],
  styles: &CompletionDocsStyles,
  base_style: Style,
  default_kind: DocsSemanticKind,
) -> Vec<StyledTextRun> {
  let mut runs = Vec::new();
  for inline in inline_runs {
    let (kind, mut style) = match inline.kind {
      DocsInlineKind::Text => (default_kind, base_style),
      DocsInlineKind::Link => (DocsSemanticKind::Link, base_style.patch(styles.link)),
      DocsInlineKind::InlineCode => (DocsSemanticKind::InlineCode, base_style.patch(styles.code)),
    };
    if inline.strong {
      style = style.add_modifier(Modifier::BOLD);
    }
    if inline.emphasis {
      style = style.add_modifier(Modifier::ITALIC);
    }
    push_styled_run(&mut runs, inline.text.clone(), style, kind);
  }
  runs
}

fn strip_signature_active_markers_from_line(
  line: &str,
) -> (String, Option<std::ops::Range<usize>>) {
  let mut cleaned = String::with_capacity(line.len());
  let mut idx = 0usize;
  let mut start = None;
  let mut end = None;

  while idx < line.len() {
    if line[idx..].starts_with(SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER) {
      if start.is_none() {
        start = Some(cleaned.len());
      }
      idx += SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER.len();
      continue;
    }
    if line[idx..].starts_with(SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER) {
      if start.is_some() && end.is_none() {
        end = Some(cleaned.len());
      }
      idx += SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER.len();
      continue;
    }

    let mut chars = line[idx..].chars();
    let Some(ch) = chars.next() else {
      break;
    };
    cleaned.push(ch);
    idx += ch.len_utf8();
  }

  let range = match (start, end) {
    (Some(start), Some(end)) if start < end => Some(start..end),
    (Some(start), None) if start < cleaned.len() => Some(start..cleaned.len()),
    _ => None,
  };
  (cleaned, range)
}

fn strip_signature_active_markers_from_lines(
  code_lines: &[String],
) -> (Vec<String>, Option<std::ops::Range<usize>>) {
  let mut cleaned_lines = Vec::with_capacity(code_lines.len());
  let mut active_range = None;
  let mut line_start = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let (cleaned, line_range) = strip_signature_active_markers_from_line(line);
    if active_range.is_none()
      && let Some(range) = line_range
    {
      active_range = Some((line_start + range.start)..(line_start + range.end));
    }
    line_start += cleaned.len();
    if idx + 1 < code_lines.len() {
      line_start += 1;
    }
    cleaned_lines.push(cleaned);
  }

  (cleaned_lines, active_range)
}

fn byte_range_overlaps_active(
  byte_start: usize,
  byte_end: usize,
  active_range: Option<&std::ops::Range<usize>>,
) -> bool {
  active_range.is_some_and(|active| byte_start < active.end && byte_end > active.start)
}

fn render_code_lines_with_active_style(
  code_lines: &[String],
  base_style: Style,
  active_parameter_style: Style,
  active_range: Option<&std::ops::Range<usize>>,
) -> Vec<Vec<StyledTextRun>> {
  let mut rendered = Vec::with_capacity(code_lines.len());
  let mut line_start_byte = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let mut runs = Vec::new();
    let mut piece = String::new();
    let mut run_style = base_style;
    let mut run_kind = DocsSemanticKind::Code;
    let mut byte_idx = line_start_byte;

    for ch in line.chars() {
      let byte_end = byte_idx.saturating_add(ch.len_utf8());
      let mut style = base_style;
      let mut kind = DocsSemanticKind::Code;
      if byte_range_overlaps_active(byte_idx, byte_end, active_range) {
        style = style.patch(active_parameter_style);
        kind = DocsSemanticKind::ActiveParameter;
      }
      if (style != run_style || kind != run_kind) && !piece.is_empty() {
        push_styled_run(&mut runs, std::mem::take(&mut piece), run_style, run_kind);
      }
      run_style = style;
      run_kind = kind;
      piece.push(ch);
      byte_idx = byte_end;
    }

    push_styled_run(&mut runs, piece, run_style, run_kind);
    if runs.is_empty() {
      runs.push(StyledTextRun {
        text:  String::new(),
        style: base_style,
        kind:  DocsSemanticKind::Code,
      });
    }
    rendered.push(runs);

    line_start_byte += line.len();
    if idx + 1 < code_lines.len() {
      line_start_byte += 1;
    }
  }

  rendered
}

fn highlighted_code_block_lines(
  code_lines: &[String],
  styles: &CompletionDocsStyles,
  ctx: Option<&Ctx>,
  language: Option<&str>,
) -> Vec<Vec<StyledTextRun>> {
  if code_lines.is_empty() {
    return vec![Vec::new()];
  }
  let (code_lines, active_range) = strip_signature_active_markers_from_lines(code_lines);
  if code_lines.is_empty() {
    return vec![Vec::new()];
  }

  let Some(ctx) = ctx else {
    return render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };
  let Some(loader) = ctx.loader.as_deref() else {
    return render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };
  let resolved_language = language.and_then(|marker| {
    let marker = marker.trim();
    let marker_lower = marker.to_ascii_lowercase();
    loader
      .language_for_name(marker)
      .or_else(|| loader.language_for_name(marker_lower.as_str()))
      .or_else(|| loader.language_for_scope(marker))
      .or_else(|| loader.language_for_scope(marker_lower.as_str()))
      .or_else(|| {
        language_filename_hints(marker)
          .into_iter()
          .find_map(|hint| loader.language_for_filename(Path::new(format!("tmp.{hint}").as_str())))
      })
  });
  let current_buffer_language = ctx
    .file_path
    .as_deref()
    .and_then(|path| loader.language_for_filename(path))
    .or_else(|| {
      ctx
        .lsp_document
        .as_ref()
        .and_then(|state| loader.language_for_name(state.language_id.as_str()))
    });
  let Some(language) = resolved_language.or(current_buffer_language) else {
    return render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };

  let joined = code_lines.join("\n");
  let rope = Rope::from_str(&joined);
  let Ok(syntax) = Syntax::new(rope.slice(..), language, loader) else {
    return render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };

  let mut highlights = syntax.collect_highlights(rope.slice(..), loader, 0..rope.len_bytes());
  highlights.sort_by_key(|(_highlight, range)| (range.start, std::cmp::Reverse(range.end)));

  let mut rendered = Vec::with_capacity(code_lines.len());
  let mut line_start_byte = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let mut runs = Vec::new();
    let mut piece = String::new();
    let mut active_style = styles.code;
    let mut active_kind = DocsSemanticKind::Code;
    let mut byte_idx = line_start_byte;

    for ch in line.chars() {
      let byte_end = byte_idx.saturating_add(ch.len_utf8());
      let mut style = preview_highlight_at(&highlights, byte_idx)
        .map(|highlight| {
          styles
            .code
            .patch(lib_style_to_ratatui(ctx.ui_theme.highlight(highlight)))
        })
        .unwrap_or(styles.code);
      let mut kind = DocsSemanticKind::Code;
      if byte_range_overlaps_active(byte_idx, byte_end, active_range.as_ref()) {
        style = style.patch(styles.active_parameter);
        kind = DocsSemanticKind::ActiveParameter;
      }
      if (style != active_style || kind != active_kind) && !piece.is_empty() {
        push_styled_run(
          &mut runs,
          std::mem::take(&mut piece),
          active_style,
          active_kind,
        );
      }
      active_style = style;
      active_kind = kind;
      piece.push(ch);
      byte_idx = byte_end;
    }
    push_styled_run(&mut runs, piece, active_style, active_kind);
    if runs.is_empty() {
      runs.push(StyledTextRun {
        text:  String::new(),
        style: styles.code,
        kind:  DocsSemanticKind::Code,
      });
    }
    rendered.push(runs);

    line_start_byte = line_start_byte.saturating_add(line.len());
    if idx + 1 < code_lines.len() {
      line_start_byte = line_start_byte.saturating_add(1);
    }
  }

  rendered
}

fn completion_docs_markdown_lines(
  markdown: &str,
  styles: &CompletionDocsStyles,
  ctx: Option<&Ctx>,
) -> Vec<Vec<StyledTextRun>> {
  let mut lines = Vec::new();
  for block in parse_markdown_blocks(markdown) {
    match block {
      DocsBlock::Paragraph(inline_runs) => {
        lines.push(docs_runs_from_inline(
          &inline_runs,
          styles,
          styles.base,
          DocsSemanticKind::Body,
        ));
      },
      DocsBlock::Heading { level, runs } => {
        let level_idx = level.saturating_sub(1).min(5) as usize;
        lines.push(docs_runs_from_inline(
          &runs,
          styles,
          styles.heading[level_idx],
          DocsSemanticKind::from_heading_level(level),
        ));
      },
      DocsBlock::ListItem {
        marker,
        runs: inline_runs,
      } => {
        let marker_text = match marker {
          DocsListMarker::Bullet => "• ".to_string(),
          DocsListMarker::Ordered(marker) => format!("{marker} "),
        };
        let mut runs = Vec::new();
        push_styled_run(
          &mut runs,
          marker_text,
          styles.bullet,
          DocsSemanticKind::ListMarker,
        );
        runs.extend(docs_runs_from_inline(
          &inline_runs,
          styles,
          styles.base,
          DocsSemanticKind::Body,
        ));
        lines.push(runs);
      },
      DocsBlock::Quote(inline_runs) => {
        let mut runs = Vec::new();
        push_styled_run(
          &mut runs,
          "│ ".to_string(),
          styles.quote,
          DocsSemanticKind::QuoteMarker,
        );
        runs.extend(docs_runs_from_inline(
          &inline_runs,
          styles,
          styles.quote,
          DocsSemanticKind::QuoteText,
        ));
        lines.push(runs);
      },
      DocsBlock::CodeFence {
        language,
        lines: code_lines,
      } => {
        lines.extend(highlighted_code_block_lines(
          &code_lines,
          styles,
          ctx,
          language.as_deref(),
        ));
      },
      DocsBlock::Rule => {
        lines.push(vec![StyledTextRun {
          text:  "───".to_string(),
          style: styles.rule,
          kind:  DocsSemanticKind::Rule,
        }]);
      },
      DocsBlock::BlankLine => lines.push(Vec::new()),
    }
  }

  if lines.is_empty() {
    lines.push(Vec::new());
  }
  lines
}

fn wrap_styled_runs(runs: &[StyledTextRun], width: usize) -> Vec<Vec<StyledTextRun>> {
  if width == 0 {
    return Vec::new();
  }
  if runs.is_empty() {
    return vec![Vec::new()];
  }

  let mut wrapped = Vec::new();
  let mut current = Vec::new();
  let mut col = 0usize;

  for run in runs {
    let mut piece = String::new();
    for ch in run.text.chars() {
      if col >= width {
        if !piece.is_empty() {
          push_styled_run(
            &mut current,
            std::mem::take(&mut piece),
            run.style,
            run.kind,
          );
        }
        wrapped.push(current);
        current = Vec::new();
        col = 0;
      }
      piece.push(ch);
      col += 1;
    }
    if !piece.is_empty() {
      push_styled_run(&mut current, piece, run.style, run.kind);
    }
  }

  if current.is_empty() {
    wrapped.push(Vec::new());
  } else {
    wrapped.push(current);
  }
  wrapped
}

fn completion_docs_rows_with_context(
  markdown: &str,
  styles: &CompletionDocsStyles,
  width: usize,
  ctx: Option<&Ctx>,
) -> Vec<Vec<StyledTextRun>> {
  let mut rows = Vec::new();
  for mut line in completion_docs_markdown_lines(markdown, styles, ctx) {
    if line.len() == 1 && width > 0 && matches!(line[0].kind, DocsSemanticKind::Rule) {
      line[0].text = "─".repeat(width);
    }
    rows.extend(wrap_styled_runs(&line, width));
  }
  if rows.is_empty() {
    rows.push(Vec::new());
  }
  rows
}

fn completion_docs_rows(
  markdown: &str,
  styles: &CompletionDocsStyles,
  width: usize,
) -> Vec<Vec<StyledTextRun>> {
  completion_docs_rows_with_context(markdown, styles, width, None)
}

#[derive(Debug, Clone, Copy)]
struct CompletionDocsRenderMetrics {
  content_width:  usize,
  total_rows:     usize,
  visible_rows:   usize,
  show_scrollbar: bool,
}

fn completion_docs_render_metrics(
  markdown: &str,
  styles: &CompletionDocsStyles,
  rect: Rect,
) -> CompletionDocsRenderMetrics {
  if rect.width == 0 || rect.height == 0 {
    return CompletionDocsRenderMetrics {
      content_width:  0,
      total_rows:     0,
      visible_rows:   0,
      show_scrollbar: false,
    };
  }

  let mut content_width = rect.width as usize;
  let mut rows = completion_docs_rows(markdown, styles, content_width);
  let mut show_scrollbar = rows.len() > rect.height as usize && rect.width > 1;
  if show_scrollbar {
    content_width = rect.width.saturating_sub(1) as usize;
    rows = completion_docs_rows(markdown, styles, content_width);
    show_scrollbar = rows.len() > rect.height as usize && rect.width > 1;
  }

  CompletionDocsRenderMetrics {
    content_width,
    total_rows: rows.len(),
    visible_rows: rect.height as usize,
    show_scrollbar,
  }
}

fn draw_styled_row(
  buf: &mut Buffer,
  x: u16,
  y: u16,
  width: usize,
  row: &[StyledTextRun],
  base_style: Style,
) {
  if width == 0 {
    return;
  }
  buf.set_string(x, y, " ".repeat(width), base_style);

  let mut col = 0usize;
  for run in row {
    for ch in run.text.chars() {
      if col >= width {
        return;
      }
      let mut symbol = [0u8; 4];
      let symbol = ch.encode_utf8(&mut symbol);
      buf.set_stringn(x + col as u16, y, symbol, 1, run.style);
      col += 1;
    }
  }
}

fn docs_scroll_for_source(ctx: &Ctx, source: DocsPanelSource) -> usize {
  match source {
    DocsPanelSource::Completion => ctx.completion_menu.docs_scroll,
    DocsPanelSource::Hover => ctx.hover_docs_scroll,
    DocsPanelSource::Signature => ctx.signature_help.docs_scroll,
    DocsPanelSource::CommandPalette => 0,
  }
}

fn draw_markdown_docs(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  markdown: &str,
  source: DocsPanelSource,
  base_style: Style,
  scrollbar_style: Style,
) -> Option<CompletionDocsLayout> {
  if rect.width == 0 || rect.height == 0 {
    return None;
  }

  let styles = completion_docs_styles(ctx, base_style);
  let metrics = completion_docs_render_metrics(markdown, &styles, rect);
  let content_width = metrics.content_width;
  let rows = completion_docs_rows_with_context(markdown, &styles, content_width, Some(ctx));
  let total_rows = metrics.total_rows;
  let visible_rows = metrics.visible_rows;
  let max_scroll = total_rows.saturating_sub(visible_rows);
  let scroll = docs_scroll_for_source(ctx, source).min(max_scroll);
  let scrollbar_track = metrics.show_scrollbar.then(|| {
    Rect::new(
      rect.x + rect.width.saturating_sub(1),
      rect.y,
      1,
      rect.height,
    )
  });
  let content = if scrollbar_track.is_some() {
    Rect::new(rect.x, rect.y, rect.width.saturating_sub(1), rect.height)
  } else {
    rect
  };

  for row_idx in 0..visible_rows {
    let y = content.y + row_idx as u16;
    if let Some(row) = rows.get(scroll + row_idx) {
      draw_styled_row(buf, content.x, y, content_width, row, base_style);
    } else {
      draw_styled_row(buf, content.x, y, content_width, &[], base_style);
    }
  }

  if let Some(track) = scrollbar_track
    && let Some(metrics) = compute_scrollbar_metrics(track, total_rows, visible_rows, scroll)
  {
    for row in 0..track.height {
      let is_thumb = row >= metrics.thumb_offset
        && row < metrics.thumb_offset.saturating_add(metrics.thumb_height);
      if is_thumb {
        buf.set_string(track.x, track.y + row, "█", scrollbar_style);
      }
    }
  }

  Some(CompletionDocsLayout {
    panel: rect,
    content,
    scrollbar_track,
    visible_rows,
    total_rows,
    source,
  })
}

fn completion_list_styles(ctx: &Ctx) -> (Style, Style, Style) {
  let menu = overlay_panel_styles(ctx, "ui.menu");
  let selected_fg = theme_scope_color(ctx, "ui.menu.selected", |style| style.fg)
    .or_else(|| theme_scope_color(ctx, "ui.text.focus", |style| style.fg))
    .or_else(|| theme_scope_color(ctx, "ui.text", |style| style.fg))
    .or(menu.text.fg);
  let selected_bg = theme_scope_color(ctx, "ui.menu.selected", |style| style.bg)
    .or_else(|| theme_scope_color(ctx, "ui.selection", |style| style.bg))
    .or_else(|| theme_scope_color(ctx, "ui.menu", |style| style.bg))
    .or(menu.fill.bg);
  let mut selected_style = menu.fill;
  if let Some(fg) = selected_fg {
    selected_style = selected_style.fg(fg);
  }
  if let Some(bg) = selected_bg {
    selected_style = selected_style.bg(bg);
  }
  (menu.text, menu.fill, selected_style)
}

fn completion_item_icon_text(icon: &str) -> Cow<'_, str> {
  if icon.chars().count() == 1 {
    Cow::Borrowed(icon)
  } else {
    Cow::Borrowed(file_picker_icon_glyph(icon, false))
  }
}

#[derive(Debug, Clone)]
struct OverlayListItem {
  title:         String,
  subtitle:      Option<String>,
  description:   Option<String>,
  badge:         Option<String>,
  leading_icon:  Option<String>,
  leading_color: Option<Color>,
  emphasis:      bool,
}

fn draw_completion_style_list(
  buf: &mut Buffer,
  rect: Rect,
  items: &[OverlayListItem],
  selected: Option<usize>,
  mut scroll: usize,
  max_visible: Option<usize>,
  text_style: Style,
  fill_style: Style,
  selected_style: Style,
  scroll_style: Style,
) {
  if rect.width == 0 || rect.height == 0 || items.is_empty() {
    return;
  }

  let base_text_color = text_style.fg;
  let selected_text_color = selected_style.fg.or(base_text_color);
  let selected_bg_color = selected_style.bg;
  let has_icons = items.iter().any(|item| item.leading_icon.is_some());
  let icon_col_width: u16 = if has_icons { 2 } else { 0 };
  const COMPLETION_MIN_LABEL_WIDTH: usize = 18;
  const COMPLETION_LABEL_TARGET_NUM: usize = 3;
  const COMPLETION_LABEL_TARGET_DEN: usize = 5;
  const COMPLETION_MIN_DETAIL_WIDTH: usize = 12;
  let visible_rows = rect.height as usize;
  let visible_items = max_visible
    .map(|max_visible| visible_rows.min(max_visible.max(1)))
    .unwrap_or(visible_rows);
  if visible_items == 0 {
    return;
  }

  scroll = scroll.min(items.len().saturating_sub(visible_items));
  if let Some(sel) = selected {
    if sel < scroll {
      scroll = sel;
    } else if sel >= scroll + visible_items {
      scroll = sel + 1 - visible_items;
    }
  }

  for (row_idx, item) in items.iter().enumerate().skip(scroll).take(visible_items) {
    let y = rect.y + (row_idx - scroll) as u16;
    let is_selected = selected == Some(row_idx);
    let row_right_padding = if items.len() > visible_items { 2 } else { 1 };

    if is_selected && let Some(bg_color) = selected_bg_color {
      fill_rect(
        buf,
        Rect::new(rect.x, y, rect.width, 1),
        reset_style().bg(bg_color),
      );
    } else {
      fill_rect(buf, Rect::new(rect.x, y, rect.width, 1), fill_style);
    }

    let mut row_style = Style::default();
    let row_color = if is_selected {
      selected_text_color
    } else {
      base_text_color
    };
    if let Some(color) = row_color {
      row_style = row_style.fg(color);
    }
    if let Some(bg) = fill_style.bg {
      row_style = row_style.bg(bg);
    }
    if is_selected && let Some(bg) = selected_bg_color {
      row_style = row_style.bg(bg);
    }
    if item.emphasis {
      row_style = row_style.add_modifier(Modifier::BOLD);
    }

    let base_content_x = rect.x;
    let label_x = base_content_x + icon_col_width;
    let label_available = rect
      .width
      .saturating_sub(icon_col_width + row_right_padding) as usize;

    if has_icons && let Some(icon) = item.leading_icon.as_deref() {
      let icon = completion_item_icon_text(icon);
      let icon_style = if is_selected {
        row_style
      } else if let Some(color) = item.leading_color {
        row_style.fg(color)
      } else {
        row_style
      };
      buf.set_string(base_content_x, y, icon.as_ref(), icon_style);
    }

    let mut title = item.title.clone();
    let detail = item
      .subtitle
      .as_deref()
      .filter(|detail| !detail.is_empty())
      .or_else(|| {
        item
          .description
          .as_deref()
          .filter(|detail| !detail.is_empty())
      });
    let badge_text = item.badge.as_deref().filter(|badge| !badge.is_empty());
    let has_right_content = detail.is_some() || badge_text.is_some();

    if has_right_content {
      let content_right = rect.x + rect.width.saturating_sub(row_right_padding);
      let reserved_label = ((label_available * COMPLETION_LABEL_TARGET_NUM)
        / COMPLETION_LABEL_TARGET_DEN)
        .max(COMPLETION_MIN_LABEL_WIDTH.min(label_available));
      let max_detail_width = label_available.saturating_sub(reserved_label.saturating_add(1));

      if max_detail_width >= COMPLETION_MIN_DETAIL_WIDTH {
        let badge_chars = badge_text.map(|text| text.chars().count()).unwrap_or(0);
        let badge_gap = if badge_chars > 0 && detail.is_some() {
          1
        } else {
          0
        };
        let badge_total = badge_chars + badge_gap;
        let detail_max = max_detail_width.saturating_sub(badge_total);

        let (detail_text, detail_char_count) = if let Some(detail) = detail {
          let mut detail_text = detail.to_string();
          truncate_in_place(&mut detail_text, detail_max);
          let count = detail_text.chars().count();
          (Some(detail_text), count)
        } else {
          (None, 0)
        };

        let right_total = detail_char_count + badge_total;
        let right_x = content_right.saturating_sub(right_total as u16);
        let mut title_width = right_x.saturating_sub(label_x).saturating_sub(1) as usize;
        if title_width == 0 {
          title_width = 1;
        }
        truncate_in_place(&mut title, title_width);
        buf.set_string(label_x, y, title, row_style);

        let detail_style = if is_selected {
          row_style
        } else {
          row_style.add_modifier(Modifier::DIM)
        };

        let mut cursor_x = right_x;
        if let Some(detail_text) = detail_text {
          if cursor_x > label_x {
            buf.set_string(cursor_x, y, detail_text.as_str(), detail_style);
            cursor_x = cursor_x.saturating_add(detail_char_count as u16);
          }
        }
        if let Some(badge_text) = badge_text {
          if cursor_x > label_x && detail_char_count > 0 {
            buf.set_string(cursor_x, y, " ", row_style);
            cursor_x = cursor_x.saturating_add(1);
          }
          if cursor_x > label_x {
            buf.set_string(cursor_x, y, badge_text, row_style);
          }
        }
      } else {
        truncate_in_place(&mut title, label_available);
        buf.set_string(label_x, y, title, row_style);
      }
    } else {
      truncate_in_place(&mut title, label_available);
      buf.set_string(label_x, y, title, row_style);
    }
  }

  if items.len() > visible_items
    && let Some(metrics) = compute_scrollbar_metrics(
      Rect::new(
        rect.x + rect.width.saturating_sub(1),
        rect.y,
        1,
        visible_items as u16,
      ),
      items.len(),
      visible_items,
      scroll,
    )
  {
    for row in 0..metrics.track.height {
      let is_thumb = row >= metrics.thumb_offset
        && row < metrics.thumb_offset.saturating_add(metrics.thumb_height);
      if is_thumb {
        buf.set_string(metrics.track.x, metrics.track.y + row, "█", scroll_style);
      }
    }
  }
}

fn completion_overlay_items(ctx: &Ctx) -> Vec<OverlayListItem> {
  ctx
    .completion_menu
    .items
    .iter()
    .map(|item| {
      OverlayListItem {
        title:         item.label.clone(),
        subtitle:      item.detail.clone(),
        description:   None,
        badge:         None,
        leading_icon:  item.kind_icon.clone(),
        leading_color: item.kind_color.map(lib_color_to_ratatui),
        emphasis:      false,
      }
    })
    .collect()
}

fn command_palette_overlay_items(ctx: &Ctx, indices: &[usize]) -> Vec<OverlayListItem> {
  indices
    .iter()
    .filter_map(|index| ctx.command_palette.items.get(*index))
    .map(|item| {
      OverlayListItem {
        title:         item.title.clone(),
        subtitle:      item.subtitle.clone().or_else(|| item.shortcut.clone()),
        description:   item.description.clone(),
        badge:         (!item.aliases.is_empty())
          .then(|| format!("(aliases: {})", item.aliases.join(", "))),
        leading_icon:  item.leading_icon.clone(),
        leading_color: item.leading_color.map(lib_color_to_ratatui),
        emphasis:      item.emphasis,
      }
    })
    .collect()
}

fn statusline_segment_style(base: Style, emphasis: StatuslineEmphasis) -> Style {
  match emphasis {
    StatuslineEmphasis::Normal => base,
    StatuslineEmphasis::Muted => base.add_modifier(Modifier::DIM),
    StatuslineEmphasis::Strong => base.add_modifier(Modifier::BOLD),
  }
}

fn draw_file_picker_panel(
  buf: &mut Buffer,
  area: Rect,
  ctx: &Ctx,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let picker = &ctx.file_picker;
  if !picker.active || area.width < 4 || area.height < 4 {
    return;
  }

  let Some(layout) = ctx
    .file_picker_layout
    .or_else(|| compute_file_picker_layout(area, picker))
  else {
    return;
  };
  if layout.panel.width == 0 || layout.panel.height == 0 {
    return;
  }

  let diagnostics_picker = file_picker_is_diagnostics(picker);
  let symbols_picker = file_picker_is_symbols(picker);
  let live_grep_picker = file_picker_is_live_grep(picker);
  let styles = file_picker_panel_styles(ctx);

  fill_rect(buf, layout.panel, styles.fill);

  if layout.panel_inner.width < 3 || layout.panel_inner.height < 3 {
    return;
  }

  draw_file_picker_list_pane(
    buf,
    &layout,
    picker,
    styles.text,
    styles.fill,
    styles.border,
    &ctx.ui_theme,
    cursor_out,
    diagnostics_picker,
    symbols_picker,
    live_grep_picker,
  );

  if layout.show_preview {
    draw_file_picker_preview_pane(
      buf,
      &layout,
      picker,
      styles.text,
      styles.fill,
      styles.border,
      &ctx.ui_theme,
      diagnostics_picker,
      symbols_picker,
      live_grep_picker,
    );
  }
}

fn draw_file_picker_list_pane(
  buf: &mut Buffer,
  layout: &FilePickerLayout,
  picker: &the_default::FilePickerState,
  text_style: Style,
  fill_style: Style,
  border_style: Style,
  theme: &the_lib::render::theme::Theme,
  cursor_out: &mut Option<(u16, u16)>,
  diagnostics_picker: bool,
  symbols_picker: bool,
  live_grep_picker: bool,
) {
  let rect = layout.list_pane;
  let title_style = text_style.add_modifier(Modifier::BOLD);
  let title = picker.title.as_str();
  let count = format!("{}/{}", picker.matched_count(), picker.total_count());
  let count_style = text_style.add_modifier(Modifier::DIM);

  let borders = if layout.show_preview {
    Borders::TOP | Borders::LEFT | Borders::BOTTOM
  } else {
    Borders::ALL
  };

  let block = Block::default()
    .title(Span::styled(format!(" {} ", title), title_style))
    .borders(borders)
    .border_type(BorderType::Rounded)
    .border_style(border_style)
    .style(fill_style);
  block.render(rect, buf);

  let inner = layout.list_inner;
  if inner.width == 0 || inner.height == 0 {
    return;
  }

  // Input row: render the search query and right-aligned count.
  let prompt_area = layout.list_prompt;
  if !picker.query.is_empty() {
    Paragraph::new(picker.query.clone())
      .style(text_style)
      .render(prompt_area, buf);
  }
  // Right-aligned match count on the prompt line.
  let count_len = count.chars().count() as u16;
  if count_len < prompt_area.width {
    let count_x = prompt_area.x + prompt_area.width.saturating_sub(count_len);
    buf.set_string(count_x, prompt_area.y, &count, count_style);
  }

  if cursor_out.is_none() {
    let cursor_col = picker.query[..picker.cursor.min(picker.query.len())]
      .chars()
      .count() as u16;
    let x = prompt_area
      .x
      .saturating_add(cursor_col)
      .min(prompt_area.x + prompt_area.width.saturating_sub(1));
    let cursor_style = software_cursor_style(theme);
    draw_software_cursor_cell(buf, x, prompt_area.y, cursor_style);
    *cursor_out = Some((x, prompt_area.y));
  }

  // Separator sits just below the prompt.
  let separator_y = prompt_area.y.saturating_add(1);
  if separator_y < inner.y.saturating_add(inner.height) {
    let sep_style = border_style;
    let separator = "─".repeat(inner.width as usize);
    buf.set_string(inner.x, separator_y, separator, sep_style);
    // Connect separator to borders with T-junction characters.
    buf
      .get_mut(rect.x, separator_y)
      .set_symbol("├")
      .set_style(sep_style);
    if layout.show_preview {
      if let Some(preview_pane) = layout.preview_pane {
        buf
          .get_mut(preview_pane.x, separator_y)
          .set_symbol("┤")
          .set_style(sep_style);
      }
    } else {
      buf
        .get_mut(rect.x + rect.width.saturating_sub(1), separator_y)
        .set_symbol("┤")
        .set_style(sep_style);
    }
  }

  if inner.height < 3 {
    return;
  }

  let list_area = layout.list_content;
  if list_area.width == 0 || list_area.height == 0 {
    return;
  }

  let total_matches = picker.matched_count();
  if total_matches == 0 {
    Paragraph::new("<No matches>")
      .style(text_style.add_modifier(Modifier::DIM))
      .render(list_area, buf);
    return;
  }

  let visible_rows = list_area.height as usize;
  if visible_rows == 0 {
    return;
  }
  let scroll_offset = layout.list_scroll_offset;
  let end = scroll_offset
    .saturating_add(visible_rows)
    .min(total_matches);
  let selected_scope = theme
    .try_get("ui.file_picker.list.selected")
    .or_else(|| theme.try_get("ui.menu.selected"));
  let selected_bg = selected_scope
    .and_then(|style| style.bg)
    .map(lib_color_to_ratatui);
  let selected_fg = selected_scope
    .and_then(|style| style.fg)
    .map(lib_color_to_ratatui)
    .or(text_style.fg);
  let scrollbar_style = border_style.add_modifier(Modifier::DIM);
  let fuzzy_highlight_style =
    lib_style_to_ratatui(theme.try_get("special").unwrap_or_default()).add_modifier(Modifier::BOLD);
  let mut match_indices = Vec::new();
  for row_idx in scroll_offset..end {
    let Some(item) = picker.matched_item_with_match_indices(row_idx, &mut match_indices) else {
      continue;
    };
    let y = list_area.y + (row_idx - scroll_offset) as u16;
    let is_selected = picker.selected == Some(row_idx);
    let is_hovered = picker.hovered == Some(row_idx);
    let mut style = text_style;
    if is_selected && let Some(bg) = selected_bg {
      fill_rect(
        buf,
        Rect::new(list_area.x, y, list_area.width, 1),
        reset_style().bg(bg),
      );
    }
    if is_selected && let Some(fg) = selected_fg {
      style = style.fg(fg);
    }
    if item.is_dir {
      style = style.add_modifier(Modifier::BOLD);
    }
    if is_hovered {
      style = style.add_modifier(Modifier::UNDERLINED);
    }

    if diagnostics_picker {
      draw_diagnostics_picker_row(
        buf,
        Rect::new(list_area.x, y, list_area.width, 1),
        y,
        item.as_ref(),
        style,
        theme,
        selected_fg,
        is_selected,
        is_hovered,
      );
      continue;
    }
    if symbols_picker {
      let next_item = if row_idx + 1 < total_matches {
        picker.matched_item(row_idx + 1)
      } else {
        None
      };
      draw_symbols_picker_row(
        buf,
        Rect::new(list_area.x, y, list_area.width, 1),
        y,
        item.as_ref(),
        next_item.as_deref(),
        style,
        selected_fg,
        fuzzy_highlight_style,
        is_selected,
        is_hovered,
        &match_indices,
      );
      continue;
    }
    if live_grep_picker {
      let previous_item = if row_idx > 0 {
        picker.matched_item(row_idx - 1)
      } else {
        None
      };
      draw_live_grep_picker_row(
        buf,
        Rect::new(list_area.x, y, list_area.width, 1),
        y,
        item.as_ref(),
        previous_item.as_deref(),
        style,
        theme,
        selected_fg,
        is_selected,
        is_hovered,
        &match_indices,
      );
      continue;
    }

    let icon = file_picker_icon_glyph(item.icon.as_str(), item.is_dir);
    let icon_x = list_area.x.saturating_add(1);
    buf.set_string(icon_x, y, icon, style);

    let icon_width = icon.chars().count() as u16;
    let text_x = icon_x.saturating_add(icon_width.saturating_add(1));
    let content_width = list_area
      .width
      .saturating_sub(1 + icon_width.saturating_add(1)) as usize;
    if content_width == 0 {
      continue;
    }

    let display = item.display.as_str();
    if !item.display_path {
      draw_fuzzy_match_line(
        buf,
        text_x,
        y,
        display,
        content_width,
        style,
        fuzzy_highlight_style,
        &match_indices,
      );
      continue;
    }

    // Split display path into filename + parent directory (like fff.nvim).
    let (dir_part, file_part) = match display.rfind('/') {
      Some(pos) => (&display[..pos], &display[pos + 1..]),
      None => ("", display),
    };
    let file_char_start: usize = display.chars().count() - file_part.chars().count();
    let file_len = file_part.chars().count();

    // Draw filename with fuzzy highlighting (remap indices from full path).
    let file_match_indices: Vec<usize> = match_indices
      .iter()
      .filter(|&&idx| idx >= file_char_start)
      .map(|&idx| idx - file_char_start)
      .collect();
    draw_fuzzy_match_line(
      buf,
      text_x,
      y,
      file_part,
      content_width,
      style,
      fuzzy_highlight_style,
      &file_match_indices,
    );

    // Draw directory dimmed after the filename.
    if !dir_part.is_empty() && file_len + 1 < content_width {
      let dir_x = text_x.saturating_add(file_len as u16 + 1);
      let dir_width = content_width.saturating_sub(file_len + 1);
      let dir_style = style.add_modifier(Modifier::DIM);
      let dir_match_indices: Vec<usize> = match_indices
        .iter()
        .filter(|&&idx| idx < file_char_start.saturating_sub(1))
        .copied()
        .collect();
      draw_fuzzy_match_line(
        buf,
        dir_x,
        y,
        dir_part,
        dir_width,
        dir_style,
        fuzzy_highlight_style,
        &dir_match_indices,
      );
    }
  }

  if let Some(track) = layout.list_scrollbar_track
    && let Some(metrics) =
      compute_scrollbar_metrics(track, total_matches, visible_rows, scroll_offset)
  {
    for idx in 0..track.height {
      let y = track.y + idx;
      let is_thumb = idx >= metrics.thumb_offset
        && idx < metrics.thumb_offset.saturating_add(metrics.thumb_height);
      if !is_thumb {
        continue;
      }
      buf.set_string(track.x, y, "█", scrollbar_style);
    }
  }
}

fn draw_file_picker_preview_pane(
  buf: &mut Buffer,
  layout: &FilePickerLayout,
  picker: &the_default::FilePickerState,
  text_style: Style,
  fill_style: Style,
  border_style: Style,
  theme: &the_lib::render::theme::Theme,
  diagnostics_picker: bool,
  symbols_picker: bool,
  live_grep_picker: bool,
) {
  let Some(rect) = layout.preview_pane else {
    return;
  };

  let focus_line = picker.preview_focus_line;
  let current_item = picker.current_item();
  let focus_severity = current_item
    .as_ref()
    .and_then(|item| diagnostic_severity_from_icon(item.icon.as_str()));
  let focus_kind_color = current_item.as_ref().and_then(|item| {
    symbols_picker.then(|| {
      let row = parse_symbols_picker_display(item.display.as_str());
      symbol_picker_kind_color(row.kind.as_str())
    })
  });
  let focus_search_color = live_grep_picker.then(|| {
    theme
      .try_get("search.match")
      .and_then(|scope| scope.fg)
      .or_else(|| theme.try_get("special").and_then(|scope| scope.fg))
      .map(lib_color_to_ratatui)
      .unwrap_or(Color::LightBlue)
  });
  let focus_accent = focus_severity
    .map(|severity| diagnostic_severity_color(theme, severity))
    .or(focus_kind_color)
    .or(focus_search_color);
  let mut preview_border_style = border_style;
  if (diagnostics_picker || symbols_picker)
    && let Some(accent) = focus_accent
  {
    preview_border_style = preview_border_style.fg(accent);
  }

  let mut block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(preview_border_style)
    .style(fill_style);
  if let Some(preview_path) = &picker.preview_path {
    let path_display = preview_path
      .strip_prefix(&picker.root)
      .unwrap_or(preview_path)
      .display()
      .to_string();
    let title = if let Some(focus_line) = focus_line {
      format!(" {}  Ln {} ", path_display, focus_line.saturating_add(1))
    } else {
      format!(" {} ", path_display)
    };
    block = block.title(Title::from(Span::styled(
      title,
      text_style.add_modifier(Modifier::DIM),
    )));
  }
  block.render(rect, buf);

  // Fix junction characters where preview's left border meets the top/bottom
  // borders.
  if rect.height > 0 {
    buf
      .get_mut(rect.x, rect.y)
      .set_symbol("┬")
      .set_style(preview_border_style);
    buf
      .get_mut(rect.x, rect.y + rect.height.saturating_sub(1))
      .set_symbol("┴")
      .set_style(preview_border_style);
  }

  let Some(content) = layout.preview_content else {
    return;
  };
  if content.width == 0 || content.height == 0 {
    return;
  }

  let scroll_offset = layout.preview_scroll_offset;
  let visible_rows = content.height as usize;
  let preview_window = file_picker_preview_window(picker, scroll_offset, visible_rows, 0);
  let total_lines = preview_window.total_virtual_rows;

  draw_file_picker_preview_window(
    buf,
    content,
    &preview_window,
    text_style,
    theme,
    focus_accent,
  );

  if let Some(track) = layout.preview_scrollbar
    && let Some(metrics) =
      compute_scrollbar_metrics(track, total_lines, visible_rows, scroll_offset)
  {
    let scrollbar_style = border_style.add_modifier(Modifier::DIM);
    for idx in 0..track.height {
      let y = track.y + idx;
      let is_thumb = idx >= metrics.thumb_offset
        && idx < metrics.thumb_offset.saturating_add(metrics.thumb_height);
      if !is_thumb {
        continue;
      }
      buf.set_string(track.x, y, "█", scrollbar_style);
    }
  }
}

fn draw_file_picker_preview_window(
  buf: &mut Buffer,
  area: Rect,
  window: &the_default::FilePickerPreviewWindow,
  text_style: Style,
  theme: &the_lib::render::theme::Theme,
  focus_accent: Option<Color>,
) {
  if area.width == 0 || area.height == 0 {
    return;
  }
  if window.kind == 0 {
    return;
  }

  let show_line_numbers = window.kind == 1;
  let line_number_width = window.total_virtual_rows.max(1).to_string().len();
  let (focus_fill_style, focus_marker_style) =
    file_picker_preview_focus_styles(theme, text_style, focus_accent);
  let gutter_style = text_style.add_modifier(Modifier::DIM);
  let match_style = file_picker_match_highlight_style(theme, text_style, focus_accent);

  for (row, line) in window.lines.iter().take(area.height as usize).enumerate() {
    let y = area.y + row as u16;
    match line.kind {
      FilePickerPreviewLineKind::TruncatedAbove | FilePickerPreviewLineKind::TruncatedBelow => {
        buf.set_stringn(
          area.x,
          y,
          line.marker.as_str(),
          area.width as usize,
          gutter_style,
        );
      },
      FilePickerPreviewLineKind::Content => {
        if line.focused {
          fill_rect(buf, Rect::new(area.x, y, area.width, 1), focus_fill_style);
        }

        let mut text_x = area.x;
        let mut text_width = area.width;

        if show_line_numbers {
          let line_number = line
            .line_number
            .unwrap_or(line.virtual_row.saturating_add(1));
          let marker = if line.focused { "▶" } else { " " };
          let gutter = format!("{marker}{line_number:>line_number_width$} ");
          let gutter_width = gutter.chars().count() as u16;
          let active_gutter_style = if line.focused {
            focus_marker_style
          } else {
            gutter_style
          };
          buf.set_stringn(area.x, y, &gutter, area.width as usize, active_gutter_style);
          if gutter_width >= area.width {
            continue;
          }
          text_x = area.x.saturating_add(gutter_width);
          text_width = area.width.saturating_sub(gutter_width);
        } else if line.focused && area.width > 0 {
          buf.set_stringn(area.x, y, "▶", 1, focus_marker_style);
          text_x = area.x.saturating_add(1);
          text_width = area.width.saturating_sub(1);
        }

        if text_width == 0 {
          continue;
        }

        let spans = preview_window_line_spans(line, text_style, match_style, theme);
        if spans.is_empty() {
          continue;
        }

        Paragraph::new(Line::from(spans)).render(Rect::new(text_x, y, text_width, 1), buf);
      },
    }
  }
}

fn preview_window_line_spans<'a>(
  line: &'a the_default::FilePickerPreviewWindowLine,
  text_style: Style,
  match_style: Style,
  theme: &the_lib::render::theme::Theme,
) -> Vec<Span<'a>> {
  if line.segments.is_empty() {
    return Vec::new();
  }

  let mut spans = Vec::with_capacity(line.segments.len());
  let base_text_style = if line.focused {
    text_style.add_modifier(Modifier::BOLD)
  } else {
    text_style
  };

  for segment in &line.segments {
    if segment.text.is_empty() {
      continue;
    }

    let mut style = base_text_style;
    if let Some(highlight_id) = segment.highlight_id {
      style = style.patch(lib_style_to_ratatui(
        theme.highlight(Highlight::new(highlight_id)),
      ));
    }
    if segment.is_match {
      style = style.patch(match_style);
    }
    spans.push(Span::styled(segment.text.as_str(), style));
  }

  spans
}

fn preview_highlight_at(
  highlights: &[(Highlight, std::ops::Range<usize>)],
  byte_idx: usize,
) -> Option<Highlight> {
  let mut active = None;
  for (highlight, range) in highlights {
    if byte_idx < range.start {
      break;
    }
    if byte_idx < range.end {
      active = Some(*highlight);
    }
  }
  active
}

fn selected_completion_docs_text(ctx: &Ctx) -> Option<&str> {
  ctx
    .completion_menu
    .selected
    .and_then(|idx| ctx.completion_menu.items.get(idx))
    .and_then(|item| item.documentation.as_deref())
    .map(str::trim)
    .filter(|docs| !docs.is_empty())
}

fn signature_help_panel_text(ctx: &Ctx) -> Option<String> {
  signature_help_markdown(&ctx.signature_help)
}

fn command_palette_prompt_query_and_cursor(ctx: &Ctx) -> (&str, usize) {
  let raw = ctx.command_prompt.input.as_str();
  if let Some(stripped) = raw.strip_prefix(':') {
    (stripped, ctx.command_prompt.cursor.saturating_sub(1))
  } else {
    (raw, ctx.command_prompt.cursor)
  }
}

fn term_command_palette_filtered_selection(
  state: &the_default::CommandPaletteState,
) -> Option<(Vec<usize>, Option<usize>)> {
  let filtered = command_palette_filtered_indices(state);
  if filtered.is_empty() {
    return None;
  }
  let selected = state
    .selected
    .and_then(|current| filtered.iter().position(|&idx| idx == current));
  Some((filtered, selected))
}
fn draw_statusline(buf: &mut Buffer, area: Rect, ctx: &mut Ctx) {
  if area.width == 0 || area.height == 0 {
    return;
  }
  let rect = Rect::new(
    area.x,
    area.y + area.height.saturating_sub(1),
    area.width,
    1,
  );
  let styles = statusline_panel_styles(ctx);
  fill_rect(buf, rect, styles.fill);
  let snapshot = build_statusline_snapshot(ctx);
  let mut left = snapshot.left;
  if let Some(icon_token) = snapshot.left_icon.as_deref() {
    let glyph = file_picker_icon_glyph(icon_token, false);
    left = match left.split_once("  ") {
      Some((mode, file)) if !file.is_empty() => format!("{mode}  {glyph}  {file}"),
      _ if left.is_empty() => glyph.to_string(),
      _ => format!("{glyph} {left}"),
    };
  }
  truncate_in_place(&mut left, rect.width as usize);
  let mut left_width = left.chars().count() as u16;

  let separator = "  ";
  let separator_width = separator.chars().count() as u16;
  let mut total_right = 0u16;
  for (index, segment) in snapshot.right_segments.iter().enumerate() {
    total_right = total_right.saturating_add(segment.text.chars().count() as u16);
    if index > 0 {
      total_right = total_right.saturating_add(separator_width);
    }
  }

  if left_width.saturating_add(total_right) >= rect.width {
    let available = rect.width.saturating_sub(total_right.saturating_add(1));
    truncate_in_place(&mut left, available as usize);
    left_width = left.chars().count() as u16;
  }

  buf.set_string(rect.x, rect.y, left, styles.text);

  let mut rx = rect.x.saturating_add(rect.width);
  for (index, segment) in snapshot.right_segments.iter().enumerate().rev() {
    let segment_style = statusline_segment_style(styles.text, segment.emphasis);
    let text_width = segment.text.chars().count() as u16;
    rx = rx.saturating_sub(text_width);
    if rx >= rect.x.saturating_add(left_width) {
      buf.set_string(rx, rect.y, segment.text.as_str(), segment_style);
    }
    if index > 0 {
      rx = rx.saturating_sub(separator_width);
      if rx >= rect.x.saturating_add(left_width) {
        buf.set_string(rx, rect.y, separator, styles.text);
      }
    }
  }
}

fn editor_top_chrome_rows(ctx: &Ctx, area: Rect) -> u16 {
  ctx
    .buffer_tabs_top_chrome_rows()
    .min(area.height.saturating_sub(1))
}

fn editor_bottom_chrome_rows(area: Rect) -> u16 {
  area.height.min(1)
}

fn overlay_area(area: Rect, ctx: &Ctx) -> Rect {
  let top = editor_top_chrome_rows(ctx, area);
  let bottom = editor_bottom_chrome_rows(area);
  Rect::new(
    area.x,
    area.y.saturating_add(top),
    area.width,
    area.height.saturating_sub(top.saturating_add(bottom)),
  )
}

fn apply_editor_viewport(ctx: &mut Ctx, area: Rect) {
  let top = editor_top_chrome_rows(ctx, area);
  let bottom = editor_bottom_chrome_rows(area);
  let viewport = the_lib::render::graphics::Rect::new(
    0,
    top,
    area.width.max(1),
    area
      .height
      .saturating_sub(top.saturating_add(bottom))
      .max(1),
  );
  if ctx.editor.layout_viewport() != viewport {
    ctx.editor.set_layout_viewport(viewport);
  }
}

fn ensure_file_tree_sidebar_width(ctx: &mut Ctx) {
  let Some(snapshot) = file_tree_snapshot(ctx) else {
    return;
  };
  let Some(tree_pane) = snapshot.attached_pane else {
    return;
  };
  if ctx.editor.pane_count() <= 1 {
    return;
  }

  let viewport = ctx.editor.layout_viewport();
  let Some(tree_rect) = ctx.editor.pane_rect(tree_pane) else {
    return;
  };
  if tree_rect.width == 0 || viewport.width <= 24 {
    return;
  }

  let max_sidebar = viewport.width.saturating_sub(20);
  let desired_width = ((viewport.width.saturating_mul(28)) / 100)
    .clamp(24, 36)
    .min(max_sidebar.max(12));
  if tree_rect.width.abs_diff(desired_width) <= 1 {
    return;
  }

  let tree_mid_y = tree_rect.y.saturating_add(tree_rect.height / 2);
  let target_line = tree_rect.x.saturating_add(desired_width.saturating_sub(1));
  let Some(separator) = ctx
    .editor
    .pane_separators(viewport)
    .into_iter()
    .filter(|separator| separator.axis == the_lib::split_tree::SplitAxis::Vertical)
    .filter(|separator| tree_mid_y >= separator.span_start && tree_mid_y < separator.span_end)
    .min_by_key(|separator| {
      separator.line.abs_diff(
        tree_rect
          .x
          .saturating_add(tree_rect.width.saturating_sub(1)),
      )
    })
  else {
    return;
  };

  let _ = ctx
    .editor
    .resize_split(separator.split_id, target_line, tree_mid_y);
}

fn render_panel_block(
  buf: &mut Buffer,
  rect: Rect,
  title: Option<String>,
  styles: PanelStyles,
) -> Rect {
  let mut block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(styles.border)
    .style(styles.fill);
  if let Some(title) = title {
    block = block.title(Title::from(Span::styled(
      title,
      styles.text.add_modifier(Modifier::BOLD),
    )));
  }
  let inner = block.inner(rect);
  block.render(rect, buf);
  inner
}

fn draw_flat_overlay_panel(
  buf: &mut Buffer,
  rect: Rect,
  styles: PanelStyles,
  padding: u16,
) -> Rect {
  fill_rect(buf, rect, styles.fill);
  Rect::new(
    rect.x.saturating_add(padding),
    rect.y.saturating_add(padding),
    rect.width.saturating_sub(padding.saturating_mul(2)),
    rect.height.saturating_sub(padding.saturating_mul(2)),
  )
}

fn completion_panel_rect(
  area: Rect,
  panel_width: u16,
  panel_height: u16,
  editor_cursor: Option<(u16, u16)>,
) -> Rect {
  let rect = default_completion_panel_rect(
    DefaultOverlayRect::new(area.x, area.y, area.width, area.height),
    panel_width,
    panel_height,
    editor_cursor,
  );
  Rect::new(rect.x, rect.y, rect.width, rect.height)
}

fn signature_help_panel_rect(
  area: Rect,
  panel_width: u16,
  panel_height: u16,
  editor_cursor: Option<(u16, u16)>,
) -> Rect {
  let rect = default_signature_help_panel_rect(
    DefaultOverlayRect::new(area.x, area.y, area.width, area.height),
    panel_width,
    panel_height,
    editor_cursor,
  );
  Rect::new(rect.x, rect.y, rect.width, rect.height)
}

fn completion_docs_panel_rect(
  area: Rect,
  panel_width: u16,
  panel_height: u16,
  completion_rect: Rect,
) -> Option<Rect> {
  let rect = default_completion_docs_panel_rect(
    DefaultOverlayRect::new(area.x, area.y, area.width, area.height),
    panel_width,
    panel_height,
    DefaultOverlayRect::new(
      completion_rect.x,
      completion_rect.y,
      completion_rect.width,
      completion_rect.height,
    ),
  )?;
  Some(Rect::new(rect.x, rect.y, rect.width, rect.height))
}

fn inline_completion_presentation_width(
  presentation: &the_default::InlineCompletionPresentation,
) -> u16 {
  let title_width = presentation.title.chars().count().saturating_add(4) as u16;
  let content_width = presentation
    .lines
    .iter()
    .map(|line| line.text.chars().count())
    .max()
    .unwrap_or(0)
    .saturating_add(2) as u16;
  title_width.max(content_width).clamp(22, 64)
}

fn inline_completion_presentation_height(
  presentation: &the_default::InlineCompletionPresentation,
) -> u16 {
  presentation.lines.len().saturating_add(2).clamp(3, 10) as u16
}

fn inline_completion_menu_popover_rect(
  overlay: Rect,
  anchor: Rect,
  width: u16,
  height: u16,
) -> Rect {
  let x = anchor.x.min(
    overlay
      .x
      .saturating_add(overlay.width.saturating_sub(width)),
  );
  let below_y = anchor.y.saturating_add(anchor.height).saturating_add(1);
  let y = if anchor.y >= overlay.y.saturating_add(height).saturating_add(1) {
    anchor.y.saturating_sub(height).saturating_sub(1)
  } else if below_y.saturating_add(height) <= overlay.y.saturating_add(overlay.height) {
    below_y
  } else {
    overlay
      .y
      .saturating_add(overlay.height.saturating_sub(height))
  };
  Rect::new(x, y, width.min(overlay.width), height.min(overlay.height))
}

fn inline_completion_presentation_styles(ctx: &Ctx) -> PanelStyles {
  overlay_panel_styles(ctx, "ui.menu")
}

fn inline_completion_line_style(
  ctx: &Ctx,
  styles: PanelStyles,
  kind: the_default::InlineCompletionPresentationLineKind,
) -> Style {
  match kind {
    the_default::InlineCompletionPresentationLineKind::Plain => styles.text,
    the_default::InlineCompletionPresentationLineKind::Dim => {
      styles.text.add_modifier(Modifier::DIM)
    },
    the_default::InlineCompletionPresentationLineKind::Addition => {
      styles
        .text
        .patch(lib_style_to_ratatui(
          render_diff_styles_from_theme(&ctx.ui_theme).added,
        ))
        .add_modifier(Modifier::BOLD)
    },
    the_default::InlineCompletionPresentationLineKind::Removal => {
      styles.text.patch(lib_style_to_ratatui(
        render_diff_styles_from_theme(&ctx.ui_theme).removed,
      ))
    },
  }
}

fn draw_inline_completion_presentation_lines(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  styles: PanelStyles,
  presentation: &the_default::InlineCompletionPresentation,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }

  for (row_idx, line) in presentation
    .lines
    .iter()
    .take(rect.height as usize)
    .enumerate()
  {
    let y = rect.y + row_idx as u16;
    fill_rect(buf, Rect::new(rect.x, y, rect.width, 1), styles.fill);
    buf.set_stringn(
      rect.x,
      y,
      line.text.as_str(),
      rect.width as usize,
      inline_completion_line_style(ctx, styles, line.kind),
    );
  }
}

fn draw_inline_completion_overlay(
  buf: &mut Buffer,
  area: Rect,
  ctx: &mut Ctx,
  editor_cursor: Option<(u16, u16)>,
  _active_plan: Option<&RenderPlan>,
  _active_pane_area: Option<Rect>,
) {
  let Some(presentation) = ctx.inline_completion.presentation.clone() else {
    return;
  };
  if ctx.file_picker.active || ctx.command_palette.is_open || ctx.search_prompt.active {
    return;
  }
  if !ctx.completion_menu.active && (ctx.hover_docs.is_some() || ctx.signature_help.active) {
    return;
  }

  let overlay = overlay_area(area, ctx);
  if overlay.width < 12 || overlay.height < 4 {
    return;
  }

  let width = inline_completion_presentation_width(&presentation)
    .min(overlay.width)
    .max(1);
  let height = inline_completion_presentation_height(&presentation)
    .min(overlay.height)
    .max(1);
  let rect = if ctx.completion_menu.active
    && presentation.kind == the_default::InlineCompletionPresentationKind::Menu
  {
    let visible = ctx.completion_menu.items.len().min(10);
    let panel_height = visible as u16;
    let panel_width = overlay
      .width
      .saturating_mul(2)
      .saturating_div(3)
      .min(64)
      .max(1);
    let completion_rect = completion_panel_rect(overlay, panel_width, panel_height, editor_cursor);
    inline_completion_menu_popover_rect(overlay, completion_rect, width, height)
  } else if presentation.kind == the_default::InlineCompletionPresentationKind::Menu {
    return;
  } else {
    completion_panel_rect(overlay, width, height, editor_cursor)
  };

  let styles = inline_completion_presentation_styles(ctx);
  let inner = render_panel_block(buf, rect, Some(presentation.title.clone()), styles);
  draw_inline_completion_presentation_lines(buf, inner, ctx, styles, &presentation);
}

fn draw_completion_overlay(
  buf: &mut Buffer,
  area: Rect,
  ctx: &mut Ctx,
  editor_cursor: Option<(u16, u16)>,
) {
  if !ctx.completion_menu.active || ctx.file_picker.active || ctx.command_palette.is_open {
    return;
  }
  let overlay = overlay_area(area, ctx);
  if overlay.width < 8 || overlay.height < 3 {
    return;
  }
  let items = &ctx.completion_menu.items;
  if items.is_empty() {
    return;
  }
  let overlay_items = completion_overlay_items(ctx);

  let visible = items.len().min(10);
  let panel_height = visible as u16;
  let panel_width = overlay
    .width
    .saturating_mul(2)
    .saturating_div(3)
    .min(64)
    .max(1);
  let panel_rect = completion_panel_rect(overlay, panel_width, panel_height, editor_cursor);
  let panel_styles = overlay_panel_styles(ctx, "ui.menu");
  let inner = draw_flat_overlay_panel(buf, panel_rect, panel_styles, 0);
  let (text_style, fill_style, selected_style) = completion_list_styles(ctx);
  draw_completion_style_list(
    buf,
    inner,
    overlay_items.as_slice(),
    ctx.completion_menu.selected,
    ctx.completion_menu.scroll,
    Some(10),
    text_style,
    fill_style,
    selected_style,
    panel_styles.border,
  );

  if let Some(docs) = selected_completion_docs_text(ctx) {
    let docs_width = overlay
      .width
      .saturating_mul(2)
      .saturating_div(3)
      .min(84)
      .max(28);
    if let Some(docs_rect) =
      completion_docs_panel_rect(overlay, docs_width, panel_rect.height, panel_rect)
    {
      let docs_styles = docs_panel_styles(ctx);
      let docs_inner = draw_flat_overlay_panel(buf, docs_rect, docs_styles, 1);
      ctx.completion_docs_layout = draw_markdown_docs(
        buf,
        docs_inner,
        ctx,
        docs,
        DocsPanelSource::Completion,
        docs_styles.text,
        docs_styles.border,
      );
    }
  }
}

fn draw_signature_help_overlay(
  buf: &mut Buffer,
  area: Rect,
  ctx: &mut Ctx,
  editor_cursor: Option<(u16, u16)>,
) {
  if !ctx.signature_help.active
    || ctx.file_picker.active
    || ctx.command_palette.is_open
    || ctx.completion_menu.active
  {
    return;
  }
  let Some(text) = signature_help_panel_text(ctx) else {
    return;
  };
  let overlay = overlay_area(area, ctx);
  let panel_styles = docs_panel_styles(ctx);
  let rect = signature_help_panel_rect(
    overlay,
    overlay
      .width
      .saturating_mul(2)
      .saturating_div(3)
      .min(72)
      .max(12),
    16,
    editor_cursor,
  );
  let inner = draw_flat_overlay_panel(buf, rect, panel_styles, 1);
  ctx.completion_docs_layout = draw_markdown_docs(
    buf,
    inner,
    ctx,
    text.as_str(),
    DocsPanelSource::Signature,
    panel_styles.text,
    panel_styles.border,
  );
}

fn draw_hover_overlay(
  buf: &mut Buffer,
  area: Rect,
  ctx: &mut Ctx,
  editor_cursor: Option<(u16, u16)>,
) {
  if ctx.file_picker.active
    || ctx.command_palette.is_open
    || ctx.search_prompt.active
    || ctx.completion_menu.active
  {
    return;
  }
  let Some(docs) = ctx
    .hover_docs
    .as_deref()
    .map(str::trim)
    .filter(|docs| !docs.is_empty())
  else {
    return;
  };
  let overlay = overlay_area(area, ctx);
  let panel_styles = docs_panel_styles(ctx);
  let rect = completion_panel_rect(
    overlay,
    overlay
      .width
      .saturating_mul(2)
      .saturating_div(3)
      .min(84)
      .max(28),
    18,
    editor_cursor,
  );
  let inner = draw_flat_overlay_panel(buf, rect, panel_styles, 1);
  ctx.completion_docs_layout = draw_markdown_docs(
    buf,
    inner,
    ctx,
    docs,
    DocsPanelSource::Hover,
    panel_styles.text,
    panel_styles.border,
  );
}

fn draw_command_palette_overlay(buf: &mut Buffer, area: Rect, ctx: &mut Ctx) {
  if !ctx.command_palette.is_open {
    return;
  }
  let overlay = overlay_area(area, ctx);
  let state = &ctx.command_palette;
  let mut filtered_state = state.clone();
  if !state.prefiltered {
    let (query, _) = command_palette_prompt_query_and_cursor(ctx);
    filtered_state.query = query.to_string();
  }
  let Some((filtered, selected)) = term_command_palette_filtered_selection(&filtered_state) else {
    return;
  };
  let overlay_items = command_palette_overlay_items(ctx, filtered.as_slice());
  let visible = filtered.len().min(10);
  let height = visible as u16;
  let rect = Rect::new(
    overlay.x,
    overlay.y + overlay.height.saturating_sub(height),
    overlay.width,
    height,
  );
  let styles = overlay_panel_styles(ctx, "ui.menu");
  let inner = draw_flat_overlay_panel(buf, rect, styles, 0);
  let (text_style, fill_style, selected_style) = completion_list_styles(ctx);
  draw_completion_style_list(
    buf,
    inner,
    overlay_items.as_slice(),
    selected,
    filtered_state.scroll_offset,
    Some(10),
    text_style,
    fill_style,
    selected_style,
    styles.border,
  );
}

fn draw_overlays(
  buf: &mut Buffer,
  area: Rect,
  ctx: &mut Ctx,
  editor_cursor: Option<(u16, u16)>,
  active_plan: Option<&RenderPlan>,
  active_pane_area: Option<Rect>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  ctx.completion_docs_layout = None;
  draw_command_palette_overlay(buf, area, ctx);
  draw_completion_overlay(buf, area, ctx, editor_cursor);
  draw_inline_completion_overlay(buf, area, ctx, editor_cursor, active_plan, active_pane_area);
  if ctx.completion_docs_layout.is_none() {
    draw_signature_help_overlay(buf, area, ctx, editor_cursor);
  }
  if ctx.completion_docs_layout.is_none() {
    draw_hover_overlay(buf, area, ctx, editor_cursor);
  }
  if ctx.file_picker.active {
    draw_file_picker_panel(buf, overlay_area(area, ctx), ctx, cursor_out);
  }
  if ctx.completion_docs_layout.is_none() {
    ctx.completion_docs_drag = None;
  }
}

pub fn build_render_plan(ctx: &mut Ctx) -> RenderPlan {
  let styles = render_styles_from_theme(ctx);
  build_render_plan_with_styles(ctx, styles)
}

pub fn build_render_plan_with_styles(ctx: &mut Ctx, styles: RenderStyles) -> RenderPlan {
  let active_pane_id = ctx.editor.active_pane_id();
  let previous_generation_state = ctx
    .frame_generation_state
    .pane_states
    .get(&active_pane_id)
    .cloned();
  let view = ctx.editor.view();
  let gutter_width = gutter_width_for_document(
    ctx.editor.document(),
    view.viewport.width,
    &ctx.gutter_config,
  );
  let diagnostics_by_line = active_diagnostics_by_line(ctx);
  let diagnostic_styles = render_diagnostic_styles_from_theme(&ctx.ui_theme);
  let diff_styles = render_diff_styles_from_theme(&ctx.ui_theme);
  let diff_signs = ctx.gutter_diff_signs.clone();

  // Set up text formatting
  ctx.text_format.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
  let text_fmt = &ctx.text_format;

  // Set up annotations
  let mut annotations = TextAnnotations::default();
  if !ctx.inline_annotations.is_empty() {
    let _ = annotations.add_inline_annotations(&ctx.inline_annotations, None);
  }
  if !ctx.overlay_annotations.is_empty() {
    let _ = annotations.add_overlay(&ctx.overlay_annotations, None);
  }
  let inline_completion_virtual_lines = ctx.inline_completion_annotations.virtual_lines().to_vec();
  if !ctx.inline_completion_annotations.is_empty() {
    let _ = ctx.inline_completion_annotations.clone().extend_into(
      &mut annotations,
      ctx.editor.document().text().slice(..),
      text_fmt.viewport_width.max(1),
      view.scroll.col,
    );
  }
  if !ctx.word_jump_inline_annotations.is_empty() {
    let _ = annotations.add_inline_annotations(&ctx.word_jump_inline_annotations, None);
  }
  if !ctx.word_jump_overlay_annotations.is_empty() {
    let jump_label_style = ctx.ui_theme.find_highlight("ui.virtual.jump-label");
    let _ = annotations.add_overlay(&ctx.word_jump_overlay_annotations, jump_label_style);
  }
  ctx.inline_diagnostic_lines.clear();
  let inline_diagnostics = active_inline_diagnostics(ctx);
  let inline_diag_count = inline_diagnostics.len();
  let first_inline_diag_start_idx = inline_diagnostics.first().map(|diag| diag.start_char_idx);
  let first_inline_diag = inline_diagnostics.first().map(|diag| {
    json!({
      "start_char_idx": diag.start_char_idx,
      "severity": format!("{:?}", diag.severity),
      "message_preview": diag.message.as_str().chars().take(120).collect::<String>(),
    })
  });
  let diagnostics_for_underlines = diagnostics_for_buffer(ctx, ctx.editor.active_buffer_id())
    .map(<[Diagnostic]>::to_vec)
    .unwrap_or_default();
  let selection_match_style = ctx
    .ui_theme
    .try_get("ui.selection.match")
    .unwrap_or_else(|| LibStyle::default().bg(LibColor::Rgb(47, 63, 116)));
  let enable_point_selection_match = ctx.mode() == Mode::Select;
  let lsp_diag_count = diagnostics_for_underlines.len();
  let mut inline_enable_cursor_line = false;
  let mut inline_config_snapshot: Option<InlineDiagnosticsConfig> = None;
  let mut cursor_line_idx = None;
  if !inline_diagnostics.is_empty() {
    inline_enable_cursor_line = ctx.mode() != Mode::Insert;
    let inline_config = inline_diagnostics_config()
      .prepare(text_fmt.viewport_width.max(1), inline_enable_cursor_line);
    inline_config_snapshot = Some(inline_config.clone());
    cursor_line_idx = active_cursor_line_idx(ctx);
  }

  let allow_cache_refresh = ctx.syntax_highlight_refresh_allowed();

  // Build the render plan (with or without syntax highlighting).
  let (mut plan, diagnostic_underlines, inline_lines, inline_render_trace) = {
    let (doc, render_cache) = ctx.editor.document_and_cache();
    let mut plan = if let (Some(loader), Some(syntax)) = (&ctx.loader, doc.syntax()) {
      // Calculate line range for highlighting
      let line_range = view.scroll.row..(view.scroll.row + view.viewport.height as usize);

      // Create syntax highlight adapter
      let mut adapter = SyntaxHighlightAdapter::new(
        doc.text().slice(..),
        syntax,
        loader.as_ref(),
        &mut ctx.highlight_cache,
        line_range,
        doc.version(),
        doc.syntax_version(),
        allow_cache_refresh,
      );

      build_plan(
        doc,
        view.clone(),
        text_fmt,
        &ctx.gutter_config,
        &mut annotations,
        &mut adapter,
        render_cache,
        styles,
      )
    } else {
      // No syntax highlighting available
      let mut highlights = NoHighlights;
      build_plan(
        doc,
        view.clone(),
        text_fmt,
        &ctx.gutter_config,
        &mut annotations,
        &mut highlights,
        render_cache,
        styles,
      )
    };
    if !inline_completion_virtual_lines.is_empty() {
      let layout = render_virtual_lines_for_viewport(
        &plan,
        text_fmt.viewport_width.max(1),
        view.scroll.col,
        &inline_completion_virtual_lines,
      );
      apply_virtual_lines_layout(&mut plan, &layout);
    }
    add_selection_match_highlights(
      &mut plan,
      doc,
      text_fmt,
      &mut annotations,
      view.clone(),
      selection_match_style,
      SelectionMatchHighlightOptions {
        enable_point_cursor_match: enable_point_selection_match,
        ..SelectionMatchHighlightOptions::default()
      },
    );

    let mut diagnostic_underlines = diagnostic_underlines_for_document(
      doc.text(),
      &diagnostics_for_underlines,
      &plan,
      text_fmt,
      &mut annotations,
    );
    let inline_layout = if let Some(inline_config) = inline_config_snapshot.as_ref() {
      render_inline_diagnostics_for_viewport(
        doc.text().slice(..),
        &plan,
        text_fmt,
        &mut annotations,
        &inline_diagnostics,
        cursor_line_idx,
        inline_config,
      )
    } else {
      InlineDiagnosticsViewportLayout::default()
    };
    apply_row_insertions_to_underlines(
      &mut diagnostic_underlines,
      &plan,
      &inline_layout.row_insertions,
    );
    apply_row_insertions(&mut plan, &inline_layout.row_insertions);
    (
      plan,
      diagnostic_underlines,
      inline_layout.lines,
      inline_layout.last_trace,
    )
  };

  ctx.diagnostic_underlines = diagnostic_underlines;
  ctx.inline_diagnostic_lines = inline_lines;
  drop(annotations);
  let row_hashes = build_render_layer_row_hashes(
    &plan,
    &ctx.inline_diagnostic_lines,
    &ctx.diagnostic_underlines,
  );
  let generation_state = finish_render_generations(
    &mut plan,
    previous_generation_state.as_ref(),
    ctx.render_theme_generation,
    row_hashes,
  );
  ctx
    .frame_generation_state
    .pane_states
    .insert(active_pane_id, generation_state);

  let should_trace_inline_diagnostics = inline_diagnostics_trace_enabled()
    || (lsp_diag_count > 0 && inline_diag_count > 0 && ctx.inline_diagnostic_lines.is_empty());
  if should_trace_inline_diagnostics {
    let cursor_char_idx = active_cursor_char_idx(ctx);
    let cursor_line_idx = active_cursor_line_idx(ctx);
    let first_diag_line_idx = first_inline_diag_start_idx.map(|start| {
      let text = ctx.editor.document().text();
      text.char_to_line(start.min(text.len_chars()))
    });
    let config_json = inline_config_snapshot.as_ref().map(|config| {
      json!({
        "cursor_line": inline_diagnostic_filter_label(config.cursor_line),
        "other_lines": inline_diagnostic_filter_label(config.other_lines),
        "min_diagnostic_width": config.min_diagnostic_width,
        "prefix_len": config.prefix_len,
        "max_wrap": config.max_wrap,
        "max_diagnostics": config.max_diagnostics,
      })
    });
    let first_render_line = ctx.inline_diagnostic_lines.first().map(|line| {
      json!({
        "row": line.row,
        "col": line.col,
        "severity": format!("{:?}", line.severity),
        "text_preview": line.text.as_str().chars().take(80).collect::<String>(),
      })
    });
    ctx.log_render_trace_value(
      "inline_diagnostics_render",
      json!({
        "mode": format!("{:?}", ctx.mode()),
        "enable_cursor_line": inline_enable_cursor_line,
        "viewport_width": text_fmt.viewport_width,
        "scroll_col": view.scroll.col,
        "lsp_diag_count": lsp_diag_count,
        "inline_diag_count": inline_diag_count,
        "render_line_count": ctx.inline_diagnostic_lines.len(),
        "cursor_char_idx": cursor_char_idx,
        "cursor_line": cursor_line_idx,
        "first_diag_line": first_diag_line_idx,
        "annotation_trace": inline_render_trace.as_ref().map(|trace| json!({
          "doc_line": trace.doc_line,
          "cursor_doc_line": trace.cursor_doc_line,
          "cursor_anchor_hit": trace.cursor_anchor_hit,
          "used_cursor_line_filter": trace.used_cursor_line_filter,
          "stack_len": trace.stack_len,
          "filtered_len": trace.filtered_len,
          "emitted_line_count": trace.emitted_line_count,
          "row_delta": trace.row_delta,
        })),
        "config": config_json,
        "first_inline_diag": first_inline_diag,
        "first_render_line": first_render_line,
      }),
    );
  }

  apply_diagnostic_gutter_markers(&mut plan, &diagnostics_by_line, diagnostic_styles);
  apply_diff_gutter_markers(&mut plan, &diff_signs, diff_styles);
  plan
}

#[derive(Default)]
struct PaneDiagnosticRenderData {
  raw_diagnostics:         Vec<Diagnostic>,
  inline_diagnostic_lines: Vec<InlineDiagnosticRenderLine>,
  diagnostic_underlines:   Vec<DiagnosticUnderlineRenderSpan>,
}

fn build_inactive_pane_plan_with_styles(
  ctx: &mut Ctx,
  pane_id: PaneId,
  buffer_id: BufferId,
  styles: RenderStyles,
) -> (RenderPlan, PaneDiagnosticRenderData) {
  let Some(view) = ctx.editor.pane_view(pane_id) else {
    return (RenderPlan::default(), PaneDiagnosticRenderData::default());
  };
  let allow_cache_refresh = ctx.syntax_highlight_refresh_allowed();
  let mut text_fmt = ctx.text_format.clone();
  let mut annotations = TextAnnotations::default();
  let mut local_highlight_cache = ctx
    .inactive_highlight_caches
    .remove(&buffer_id)
    .unwrap_or_default();
  let raw_diagnostics = diagnostics_for_buffer(ctx, buffer_id)
    .map(<[Diagnostic]>::to_vec)
    .unwrap_or_default();
  let diagnostics_by_line = diagnostics_by_line(&raw_diagnostics);
  let diagnostic_styles = render_diagnostic_styles_from_theme(&ctx.ui_theme);
  let diff_styles = render_diff_styles_from_theme(&ctx.ui_theme);
  let diff_signs = ctx.gutter_diff_signs.clone();

  let Some((doc, render_cache)) = ctx.editor.document_and_cache_at_mut(buffer_id) else {
    return (RenderPlan::default(), PaneDiagnosticRenderData::default());
  };
  let gutter_width = gutter_width_for_document(doc, view.viewport.width, &ctx.gutter_config);
  text_fmt.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);

  let mut plan = if let (Some(loader), Some(syntax)) = (&ctx.loader, doc.syntax()) {
    let line_range = view.scroll.row..(view.scroll.row + view.viewport.height as usize);
    let mut adapter = SyntaxHighlightAdapter::new(
      doc.text().slice(..),
      syntax,
      loader.as_ref(),
      &mut local_highlight_cache,
      line_range,
      doc.version(),
      doc.syntax_version(),
      allow_cache_refresh,
    );
    build_plan(
      doc,
      view,
      &text_fmt,
      &ctx.gutter_config,
      &mut annotations,
      &mut adapter,
      render_cache,
      styles,
    )
  } else {
    let mut highlights = NoHighlights;
    build_plan(
      doc,
      view,
      &text_fmt,
      &ctx.gutter_config,
      &mut annotations,
      &mut highlights,
      render_cache,
      styles,
    )
  };

  let inline_diagnostics = inline_diagnostics_from_document(doc.text(), &raw_diagnostics);
  let inline_config =
    InlineDiagnosticsConfig::default().prepare(text_fmt.viewport_width.max(1) as u16, false);
  let mut diagnostic_underlines = diagnostic_underlines_for_document(
    doc.text(),
    &raw_diagnostics,
    &plan,
    &text_fmt,
    &mut annotations,
  );
  let inline_layout = render_inline_diagnostics_for_viewport(
    doc.text().slice(..),
    &plan,
    &text_fmt,
    &mut annotations,
    &inline_diagnostics,
    None,
    &inline_config,
  );
  apply_row_insertions_to_underlines(
    &mut diagnostic_underlines,
    &plan,
    &inline_layout.row_insertions,
  );
  apply_row_insertions(&mut plan, &inline_layout.row_insertions);
  apply_diagnostic_gutter_markers(&mut plan, &diagnostics_by_line, diagnostic_styles);
  apply_diff_gutter_markers(&mut plan, &diff_signs, diff_styles);

  ctx
    .inactive_highlight_caches
    .insert(buffer_id, local_highlight_cache);
  (plan, PaneDiagnosticRenderData {
    raw_diagnostics,
    inline_diagnostic_lines: inline_layout.lines,
    diagnostic_underlines,
  })
}

pub fn build_frame_render_plan(ctx: &mut Ctx) -> FrameRenderPlan {
  let styles = render_styles_from_theme(ctx);
  build_frame_render_plan_with_styles(ctx, styles)
}

pub fn build_frame_render_plan_with_styles(ctx: &mut Ctx, styles: RenderStyles) -> FrameRenderPlan {
  let previous_frame_generation_state = ctx.frame_generation_state.clone();
  let viewport = ctx.editor.layout_viewport();
  let pane_snapshots = ctx.editor.frame_pane_snapshots(viewport);
  if pane_snapshots.is_empty() {
    ctx.inline_diagnostic_lines.clear();
    ctx.diagnostic_underlines.clear();
    ctx.frame_inline_diagnostic_lines.clear();
    ctx.frame_diagnostic_underlines.clear();
    ctx.frame_diagnostics.clear();
    ctx.frame_generation_state = FrameGenerationState::default();
    return FrameRenderPlan::empty();
  }

  for pane in &pane_snapshots {
    if let PaneContent::EditorBuffer { .. } = pane.content {
      let _ = ctx.editor.set_pane_viewport(pane.pane_id, pane.rect);
    }
  }

  let active_pane = ctx.editor.active_pane_id();
  let mut pane_generation_states = BTreeMap::new();
  ctx.frame_inline_diagnostic_lines.clear();
  ctx.frame_diagnostic_underlines.clear();
  ctx.frame_diagnostics.clear();
  let panes = pane_snapshots
    .into_iter()
    .map(|pane| {
      let (pane_kind, client_surface_id, plan) = match pane.content {
        PaneContent::EditorBuffer { buffer_id } => {
          let (mut plan, diagnostic_data) = if pane.is_active_pane {
            let plan = build_render_plan_with_styles(ctx, styles);
            (plan, PaneDiagnosticRenderData {
              raw_diagnostics:         diagnostics_for_buffer(ctx, buffer_id)
                .map(<[Diagnostic]>::to_vec)
                .unwrap_or_default(),
              inline_diagnostic_lines: ctx.inline_diagnostic_lines.clone(),
              diagnostic_underlines:   ctx.diagnostic_underlines.clone(),
            })
          } else {
            build_inactive_pane_plan_with_styles(ctx, pane.pane_id, buffer_id, styles)
          };
          ctx
            .frame_diagnostics
            .insert(pane.pane_id, diagnostic_data.raw_diagnostics.clone());
          ctx.frame_inline_diagnostic_lines.insert(
            pane.pane_id,
            diagnostic_data.inline_diagnostic_lines.clone(),
          );
          ctx
            .frame_diagnostic_underlines
            .insert(pane.pane_id, diagnostic_data.diagnostic_underlines.clone());
          let generation_state = if pane.is_active_pane {
            ctx
              .frame_generation_state
              .pane_states
              .get(&pane.pane_id)
              .cloned()
              .unwrap_or_else(|| {
                RenderGenerationState {
                  layout_generation:       plan.layout_generation,
                  text_generation:         plan.text_generation,
                  decoration_generation:   plan.decoration_generation,
                  cursor_generation:       plan.cursor_generation,
                  cursor_blink_generation: plan.cursor_blink_generation,
                  scroll_generation:       plan.scroll_generation,
                  theme_generation:        plan.theme_generation,
                  text_rows:               Vec::new(),
                  decoration_rows:         Vec::new(),
                  cursor_rows:             Vec::new(),
                }
              })
          } else {
            let previous = previous_frame_generation_state
              .pane_states
              .get(&pane.pane_id);
            let row_hashes = build_render_layer_row_hashes(
              &plan,
              &diagnostic_data.inline_diagnostic_lines,
              &diagnostic_data.diagnostic_underlines,
            );
            finish_render_generations(&mut plan, previous, ctx.render_theme_generation, row_hashes)
          };
          pane_generation_states.insert(pane.pane_id, generation_state);
          (the_lib::editor::PaneContentKind::EditorBuffer, None, plan)
        },
        PaneContent::ClientSurface { surface_id } => {
          pane_generation_states.insert(pane.pane_id, RenderGenerationState::default());
          (
            the_lib::editor::PaneContentKind::ClientSurface,
            Some(surface_id),
            RenderPlan::default(),
          )
        },
      };
      PaneRenderPlan {
        pane_id: pane.pane_id,
        rect: pane.rect,
        pane_kind,
        client_surface_id,
        plan,
      }
    })
    .collect();

  let mut frame = FrameRenderPlan {
    active_pane,
    panes,
    frame_generation: 0,
    pane_structure_generation: 0,
    changed_pane_ids: Vec::new(),
    damage_is_full: false,
    damage_reason: the_lib::render::RenderDamageReason::None,
  };
  ctx.frame_generation_state = finish_frame_generations(
    &mut frame,
    Some(&previous_frame_generation_state),
    pane_generation_states,
  );
  frame
}

fn pane_screen_rect(area: Rect, pane: the_lib::render::graphics::Rect) -> Rect {
  Rect::new(
    area.x.saturating_add(pane.x),
    area.y.saturating_add(pane.y),
    pane.width,
    pane.height,
  )
}

fn draw_pane_content(
  buf: &mut Buffer,
  ctx: &mut Ctx,
  pane_id: PaneId,
  pane_area: Rect,
  plan: &RenderPlan,
  base_text_style: Style,
  draw_active_annotations: bool,
  draw_inactive_cursors: bool,
) -> Option<(u16, u16)> {
  let content_x = pane_area.x.saturating_add(plan.content_offset_x);
  let editor_cursor = plan.cursors.first().map(|cursor| {
    (
      content_x + cursor.pos.col as u16,
      pane_area.y + cursor.pos.row as u16,
    )
  });

  if plan.content_offset_x > 0 {
    for line in &plan.gutter_lines {
      let y = pane_area.y + line.row;
      if y >= pane_area.y + pane_area.height {
        continue;
      }
      for span in &line.spans {
        let x = pane_area.x + span.col;
        if x >= content_x {
          continue;
        }
        let max_width = content_x.saturating_sub(x) as usize;
        if max_width == 0 {
          continue;
        }
        let text = if is_diff_gutter_marker(span.text.as_str()) {
          "▏"
        } else {
          span.text.as_str()
        };
        buf.set_stringn(x, y, text, max_width, lib_style_to_ratatui(span.style));
      }
    }
  }

  for selection in &plan.selections {
    let rect = Rect::new(
      content_x + selection.rect.x,
      pane_area.y + selection.rect.y,
      selection.rect.width,
      selection.rect.height,
    );
    fill_rect(buf, rect, lib_style_to_ratatui(selection.style));
  }

  for line in &plan.lines {
    let y = pane_area.y + line.row;
    if y >= pane_area.y + pane_area.height {
      continue;
    }
    for span in &line.spans {
      let x = content_x + span.col;
      if x >= pane_area.x + pane_area.width {
        continue;
      }
      let style = span
        .highlight
        .map(|highlight| {
          base_text_style.patch(lib_style_to_ratatui(ctx.ui_theme.highlight(highlight)))
        })
        .unwrap_or(base_text_style);
      let max_width = pane_area
        .x
        .saturating_add(pane_area.width)
        .saturating_sub(x) as usize;
      if max_width == 0 {
        continue;
      }
      buf.set_stringn(x, y, span.text.as_str(), max_width, style);
    }
  }

  if let Some(diagnostic_underlines) = ctx.frame_diagnostic_underlines.get(&pane_id) {
    draw_diagnostic_underlines(
      buf,
      pane_area,
      content_x,
      &ctx.ui_theme,
      diagnostic_underlines,
    );
  }
  let pane_diagnostics = ctx
    .frame_diagnostics
    .get(&pane_id)
    .cloned()
    .unwrap_or_default();
  if !pane_diagnostics.is_empty() {
    let cursor_doc_line = if draw_active_annotations {
      active_cursor_line_idx(ctx)
    } else {
      None
    };
    draw_end_of_line_diagnostics(
      buf,
      pane_area,
      content_x,
      plan,
      ctx,
      &pane_diagnostics,
      cursor_doc_line,
    );
  }
  if let Some(inline_diagnostic_lines) = ctx.frame_inline_diagnostic_lines.get(&pane_id) {
    draw_inline_diagnostic_lines(
      buf,
      pane_area,
      content_x,
      plan,
      &ctx.ui_theme,
      inline_diagnostic_lines,
    );
  }

  for overlay in &plan.overlays {
    match overlay {
      the_lib::render::OverlayNode::Rect(rect) => {
        let overlay_rect = Rect::new(
          content_x.saturating_add(rect.rect.x),
          pane_area.y.saturating_add(rect.rect.y),
          rect.rect.width,
          rect.rect.height,
        );
        fill_rect(buf, overlay_rect, lib_style_to_ratatui(rect.style));
      },
      the_lib::render::OverlayNode::Text(text) => {
        let x = content_x.saturating_add(text.pos.col as u16);
        let y = pane_area.y.saturating_add(text.pos.row as u16);
        if x >= pane_area.x.saturating_add(pane_area.width)
          || y >= pane_area.y.saturating_add(pane_area.height)
        {
          continue;
        }
        let max_width = pane_area
          .x
          .saturating_add(pane_area.width)
          .saturating_sub(x) as usize;
        if max_width == 0 {
          continue;
        }
        buf.set_stringn(
          x,
          y,
          text.text.as_str(),
          max_width,
          lib_style_to_ratatui(text.style),
        );
      },
    }
  }

  if ctx.file_picker.active {
    return editor_cursor;
  }

  for (index, cursor) in plan.cursors.iter().enumerate() {
    if !draw_active_annotations && !draw_inactive_cursors {
      continue;
    }
    let x = content_x + cursor.pos.col as u16;
    let y = pane_area.y + cursor.pos.row as u16;
    if x < pane_area.x + pane_area.width && y < pane_area.y + pane_area.height {
      let use_terminal_hardware_cursor = draw_active_annotations
        && index == 0
        && matches!(cursor.kind, LibCursorKind::Bar | LibCursorKind::Underline);
      if use_terminal_hardware_cursor {
        continue;
      }
      let style = if draw_active_annotations {
        lib_style_to_ratatui(cursor.style)
      } else {
        unfocused_pane_cursor_style(&ctx.ui_theme)
      };
      draw_buffer_cursor_cell(buf, x, y, cursor.kind, style);
    }
  }

  editor_cursor
}

fn draw_pane_separators(buf: &mut Buffer, area: Rect, frame: &FrameRenderPlan, ctx: &Ctx) {
  if frame.panes.len() <= 1 {
    return;
  }

  let window_style = lib_style_to_ratatui(ctx.ui_theme.try_get("ui.window").unwrap_or_default());
  let active_style = lib_style_to_ratatui(
    ctx
      .ui_theme
      .try_get("ui.window.active")
      .or_else(|| ctx.ui_theme.try_get("ui.cursor.match"))
      .or_else(|| ctx.ui_theme.try_get("ui.window"))
      .unwrap_or_default(),
  );

  let mut vertical_cells: BTreeMap<(u16, u16), bool> = BTreeMap::new();
  let mut horizontal_cells: BTreeMap<(u16, u16), bool> = BTreeMap::new();
  let Some(active_pane) = frame
    .panes
    .iter()
    .find(|pane| pane.pane_id == frame.active_pane)
  else {
    return;
  };
  let active_rect = active_pane.rect;

  for separator in ctx.editor.pane_separators(ctx.editor.layout_viewport()) {
    match separator.axis {
      SplitAxis::Vertical => {
        let x = area.x.saturating_add(separator.line.saturating_sub(1));
        let is_active = active_rect.x == separator.line
          || active_rect.x.saturating_add(active_rect.width) == separator.line;
        for y in separator.span_start..separator.span_end {
          vertical_cells
            .entry((x, area.y.saturating_add(y)))
            .and_modify(|active| *active |= is_active)
            .or_insert(is_active);
        }
      },
      SplitAxis::Horizontal => {
        let y = area.y.saturating_add(separator.line.saturating_sub(1));
        let is_active = active_rect.y == separator.line
          || active_rect.y.saturating_add(active_rect.height) == separator.line;
        for x in separator.span_start..separator.span_end {
          horizontal_cells
            .entry((area.x.saturating_add(x), y))
            .and_modify(|active| *active |= is_active)
            .or_insert(is_active);
        }
      },
    }
  }

  for (&pos, &active_h) in &horizontal_cells {
    let active_v = vertical_cells.get(&pos).copied().unwrap_or(false);
    let style = if active_h || active_v {
      active_style
    } else {
      window_style
    };
    let symbol = if active_v { "┼" } else { "─" };
    let cell = buf.get_mut(pos.0, pos.1);
    cell.set_symbol(symbol);
    cell.set_style(cell.style().patch(style));
  }

  for (&pos, &active) in &vertical_cells {
    if horizontal_cells.contains_key(&pos) {
      continue;
    }
    let style = if active { active_style } else { window_style };
    let cell = buf.get_mut(pos.0, pos.1);
    cell.set_symbol("│");
    cell.set_style(cell.style().patch(style));
  }
}

fn draw_file_tree_pane(buf: &mut Buffer, pane_area: Rect, ctx: &Ctx, snapshot: &FileTreeSnapshot) {
  if pane_area.width == 0 || pane_area.height == 0 {
    return;
  }

  let panel = file_picker_panel_styles(ctx);
  let selected_style = theme_style_or(
    ctx,
    "ui.file_picker.list.selected",
    theme_style_or(
      ctx,
      "ui.menu.selected",
      theme_style_or(
        ctx,
        "ui.selection",
        panel.fill.add_modifier(Modifier::REVERSED),
      ),
    ),
  );
  let current_style = panel.text.add_modifier(Modifier::BOLD);
  let guide_style = panel.text.add_modifier(Modifier::DIM);
  let header_style = panel.text.add_modifier(Modifier::BOLD);

  fill_rect(buf, pane_area, panel.fill);

  let header = snapshot.root.display().to_string();
  buf.set_stringn(
    pane_area.x,
    pane_area.y,
    header,
    pane_area.width as usize,
    header_style,
  );

  if pane_area.height <= 1 {
    return;
  }

  let visible_rows = pane_area.height.saturating_sub(1) as usize;
  let max_offset = snapshot.rows.len().saturating_sub(visible_rows);
  let offset = snapshot.scroll_offset.min(max_offset);

  for (visible_index, row) in snapshot
    .rows
    .iter()
    .skip(offset)
    .take(visible_rows)
    .enumerate()
  {
    let y = pane_area.y.saturating_add(1 + visible_index as u16);
    let is_selected = snapshot.selected == Some(offset + visible_index);
    let decorations = ctx
      .file_tree_decorations
      .get(&row.path)
      .copied()
      .unwrap_or_default();
    let content_style = if is_selected {
      selected_style
    } else if row.is_current_file {
      current_style
    } else {
      panel.text
    };
    let mut x = pane_area.x;
    let mut guide_prefix = String::new();
    for continues in &row.ancestor_branches {
      guide_prefix.push_str(if *continues { "│ " } else { "  " });
    }
    if row.depth > 0 {
      guide_prefix.push_str(if row.is_last_sibling { "└ " } else { "├ " });
    }
    let remaining = pane_area
      .x
      .saturating_add(pane_area.width)
      .saturating_sub(x) as usize;
    if remaining == 0 {
      continue;
    }
    if !guide_prefix.is_empty() {
      buf.set_stringn(x, y, guide_prefix.as_str(), remaining, guide_style);
      x = x.saturating_add(guide_prefix.chars().count() as u16);
    }

    let remaining = pane_area
      .x
      .saturating_add(pane_area.width)
      .saturating_sub(x) as usize;
    if remaining == 0 {
      continue;
    }
    let icon = format!("{} ", row.icon_glyph);
    buf.set_stringn(x, y, icon.as_str(), remaining, content_style);
    x = x.saturating_add(icon.chars().count() as u16);

    let badge_span = file_tree_badge_span(ctx, decorations, panel.fill);
    let badge_width = badge_span
      .iter()
      .map(|(text, _)| text.chars().count())
      .sum::<usize>();
    let gap_width = badge_span.len().saturating_sub(1);
    let badges_total_width = badge_width.saturating_add(gap_width);
    let badge_right_padding = if badge_span.is_empty() { 0 } else { 2 };
    let badge_left_padding = usize::from(!badge_span.is_empty());

    let mut badge_x = pane_area
      .x
      .saturating_add(pane_area.width)
      .saturating_sub(badge_right_padding as u16);
    for (index, (badge_text, badge_style)) in badge_span.iter().enumerate().rev() {
      let width = badge_text.chars().count() as u16;
      badge_x = badge_x.saturating_sub(width);
      buf.set_stringn(
        badge_x,
        y,
        badge_text.as_str(),
        width as usize,
        *badge_style,
      );
      if index > 0 {
        badge_x = badge_x.saturating_sub(1);
      }
    }

    let available_right = pane_area
      .x
      .saturating_add(pane_area.width)
      .saturating_sub((badges_total_width + badge_left_padding + badge_right_padding) as u16);
    let remaining = available_right.saturating_sub(x) as usize;
    if remaining == 0 {
      continue;
    }
    buf.set_stringn(x, y, row.display_name.as_str(), remaining, content_style);
  }
}

fn file_tree_badge_span(
  ctx: &Ctx,
  decorations: FileTreeDecorations,
  base_style: Style,
) -> Vec<(String, Style)> {
  let mut out = Vec::new();
  if let Some(vcs) = decorations.vcs {
    let glyph = file_picker_icon_glyph(file_tree_vcs_icon_name(vcs), false).to_string();
    out.push((glyph, base_style.patch(file_tree_vcs_style(ctx, vcs))));
  }
  if let Some(severity) = decorations.diagnostic {
    let glyph = file_picker_icon_glyph(file_tree_diagnostic_icon_name(severity), false).to_string();
    out.push((
      glyph,
      base_style.patch(lib_style_to_ratatui(diagnostic_theme_style(
        &ctx.ui_theme,
        severity,
      ))),
    ));
  }
  out
}

fn file_tree_vcs_icon_name(kind: FileTreeVcsKind) -> &'static str {
  match kind {
    FileTreeVcsKind::Conflict => "git_conflict",
    FileTreeVcsKind::Deleted => "git_deleted",
    FileTreeVcsKind::Modified => "git_modified",
    FileTreeVcsKind::Renamed => "git_renamed",
    FileTreeVcsKind::Untracked => "git_untracked",
  }
}

fn file_tree_vcs_style(ctx: &Ctx, kind: FileTreeVcsKind) -> Style {
  let severity = match kind {
    FileTreeVcsKind::Conflict | FileTreeVcsKind::Deleted => DiagnosticSeverity::Error,
    FileTreeVcsKind::Modified => DiagnosticSeverity::Warning,
    FileTreeVcsKind::Renamed => DiagnosticSeverity::Information,
    FileTreeVcsKind::Untracked => DiagnosticSeverity::Hint,
  };
  lib_style_to_ratatui(diagnostic_theme_style(&ctx.ui_theme, severity))
}

fn file_tree_diagnostic_icon_name(severity: DiagnosticSeverity) -> &'static str {
  match severity {
    DiagnosticSeverity::Error => "diagnostic_error",
    DiagnosticSeverity::Warning => "diagnostic_warning",
    DiagnosticSeverity::Information => "diagnostic_info",
    DiagnosticSeverity::Hint => "diagnostic_hint",
  }
}

fn sync_file_tree_layout(ctx: &mut Ctx, area: Rect) {
  ctx.file_tree_layout = None;

  let Some(snapshot) = file_tree_snapshot(ctx) else {
    return;
  };
  let Some(pane_id) = snapshot.attached_pane else {
    return;
  };
  let Some(pane_rect) = ctx.editor.pane_rect(pane_id) else {
    return;
  };

  let pane = pane_screen_rect(area, pane_rect);
  let header_height = pane.height.min(1);
  let header = Rect::new(pane.x, pane.y, pane.width, header_height);
  let list = Rect::new(
    pane.x,
    pane.y.saturating_add(header_height),
    pane.width,
    pane.height.saturating_sub(header_height),
  );

  set_file_tree_visible_rows(ctx, list.height as usize);
  let scroll_offset = ctx.file_tree.scroll_offset;
  ctx.file_tree_layout = Some(FileTreeLayout {
    pane_id,
    pane,
    header,
    list,
    visible_rows: list.height as usize,
    scroll_offset,
  });
}

fn draw_buffer_tabs_row(buf: &mut Buffer, area: Rect, ctx: &Ctx) {
  let top_rows = ctx.buffer_tabs_top_chrome_rows();
  if top_rows == 0 || area.width == 0 || area.height == 0 {
    return;
  }

  let row_rect = Rect::new(area.x, area.y, area.width, 1);
  let base = theme_style_or(
    ctx,
    "ui.buffer_tabs",
    theme_style_or(
      ctx,
      "ui.window",
      lib_style_to_ratatui(ctx.ui_theme.try_get("ui.background").unwrap_or_default()),
    ),
  );
  let inactive = theme_style_or(ctx, "ui.buffer_tabs.tab.inactive", base);
  let active = theme_style_or(
    ctx,
    "ui.buffer_tabs.tab.active",
    theme_style_or(
      ctx,
      "ui.window.active",
      inactive.add_modifier(Modifier::BOLD),
    ),
  );
  let modified_style = theme_style_or(
    ctx,
    "ui.buffer_tabs.tab.modified",
    inactive.fg(Color::Yellow).add_modifier(Modifier::BOLD),
  );
  let close_style = Style::default().add_modifier(Modifier::DIM);
  let close_hover_style = modified_style.add_modifier(Modifier::BOLD);
  let icon_style = Style::default().add_modifier(Modifier::DIM);
  fill_rect(buf, row_rect, base);

  let (snapshot, slots) = ctx.buffer_tab_layout_slots(area.width);
  for slot in &slots {
    let Some(tab) = snapshot.tabs.get(slot.tab_index) else {
      continue;
    };
    let slot_rect = Rect::new(
      area.x.saturating_add(slot.x),
      row_rect.y,
      slot.width,
      row_rect.height,
    );
    let tab_style = if tab.is_active { active } else { inactive };
    fill_rect(buf, slot_rect, tab_style);

    let left_pad = if slot.width > 2 { 1 } else { 0 };
    let text_x = slot_rect.x.saturating_add(left_pad);
    let text_width = slot.width.saturating_sub(left_pad);
    if text_width == 0 {
      continue;
    }

    let mut title = tab.title.clone();
    let icon = tab
      .file_path
      .as_deref()
      .map(|path| file_picker_icon_glyph(file_picker_icon_name_for_path(path), false))
      .unwrap_or_else(|| file_picker_icon_glyph("file_generic", false));
    let icon_width = icon.chars().count() as u16;
    let icon_extra = if text_width > icon_width {
      icon_width.saturating_add(1)
    } else {
      0
    };
    let marker_text = if tab.modified { "● " } else { "" };
    let marker_width = marker_text.chars().count() as u16;
    let close_text = if slot.close_x.is_some() { "×" } else { "" };
    let close_width = close_text.chars().count() as u16;
    let close_pad_width = if close_width > 0 && text_width > close_width {
      1
    } else {
      0
    };
    let close_trailing_pad_width = if close_width > 0 { 1 } else { 0 };
    let title_width = text_width
      .saturating_sub(icon_extra)
      .saturating_sub(marker_width)
      .saturating_sub(close_pad_width)
      .saturating_sub(close_trailing_pad_width)
      .saturating_sub(close_width);
    if title_width == 0 {
      truncate_with_ellipsis_in_place(&mut title, text_width as usize);
      buf.set_string(text_x, slot_rect.y, title, tab_style);
    } else {
      let mut cursor_x = text_x;
      let mut remaining_text_width = text_width;
      if icon_extra > 0 && icon_width <= remaining_text_width {
        buf.set_string(cursor_x, slot_rect.y, icon, tab_style.patch(icon_style));
        cursor_x = cursor_x.saturating_add(icon_width.saturating_add(1));
        remaining_text_width = remaining_text_width.saturating_sub(icon_extra);
      }
      if tab.modified && marker_width <= remaining_text_width {
        buf.set_string(
          cursor_x,
          slot_rect.y,
          marker_text,
          tab_style.patch(modified_style),
        );
        cursor_x = cursor_x.saturating_add(marker_width.min(remaining_text_width));
      }
      truncate_with_ellipsis_in_place(&mut title, title_width as usize);
      buf.set_string(cursor_x, slot_rect.y, title, tab_style);
      if close_width > 0
        && let Some(close_x) = slot.close_x
      {
        let close_is_hovered = ctx
          .buffer_tab_hover
          .is_some_and(|hover| hover.buffer_id == tab.buffer_id && hover.over_close);
        buf.set_string(
          area.x.saturating_add(close_x),
          slot_rect.y,
          close_text,
          tab_style.patch(if close_is_hovered {
            close_hover_style
          } else {
            close_style
          }),
        );
      }
    }
  }

  if let Some(drag) = ctx.buffer_tab_drag
    && let Some(slot) = slots.iter().find(|slot| slot.buffer_id == drag.buffer_id)
    && let Some(tab) = snapshot.tabs.get(slot.tab_index)
  {
    let ghost_width = slot.width.min(area.width).max(1);
    let ghost_left = drag
      .pointer_x
      .saturating_sub(drag.grab_offset)
      .min(area.width.saturating_sub(ghost_width));
    let ghost_rect = Rect::new(
      area.x.saturating_add(ghost_left),
      row_rect.y,
      ghost_width,
      row_rect.height,
    );
    let ghost_style = active.add_modifier(Modifier::BOLD);
    let ghost_border = ghost_style.add_modifier(Modifier::REVERSED);
    fill_rect(buf, ghost_rect, ghost_border);

    let left_pad = if ghost_rect.width > 2 { 1 } else { 0 };
    let text_x = ghost_rect.x.saturating_add(left_pad);
    let text_width = ghost_rect.width.saturating_sub(left_pad);
    if text_width > 0 {
      let mut title = tab.title.clone();
      let icon = tab
        .file_path
        .as_deref()
        .map(|path| file_picker_icon_glyph(file_picker_icon_name_for_path(path), false))
        .unwrap_or_else(|| file_picker_icon_glyph("file_generic", false));
      let icon_width = icon.chars().count() as u16;
      let icon_extra = if text_width > icon_width {
        icon_width.saturating_add(1)
      } else {
        0
      };
      let marker_text = if tab.modified { "● " } else { "" };
      let marker_width = marker_text.chars().count() as u16;
      let close_text = if slot.close_x.is_some() && ghost_rect.width >= 12 {
        "×"
      } else {
        ""
      };
      let close_width = close_text.chars().count() as u16;
      let close_pad_width = if close_width > 0 && text_width > close_width {
        1
      } else {
        0
      };
      let close_trailing_pad_width = if close_width > 0 { 1 } else { 0 };
      let title_width = text_width
        .saturating_sub(icon_extra)
        .saturating_sub(marker_width)
        .saturating_sub(close_pad_width)
        .saturating_sub(close_trailing_pad_width)
        .saturating_sub(close_width);

      if title_width == 0 {
        truncate_with_ellipsis_in_place(&mut title, text_width as usize);
        buf.set_string(text_x, ghost_rect.y, title, ghost_style);
      } else {
        let mut cursor_x = text_x;
        let mut remaining_text_width = text_width;
        if icon_extra > 0 && icon_width <= remaining_text_width {
          buf.set_string(cursor_x, ghost_rect.y, icon, ghost_style.patch(icon_style));
          cursor_x = cursor_x.saturating_add(icon_width.saturating_add(1));
          remaining_text_width = remaining_text_width.saturating_sub(icon_extra);
        }
        if tab.modified && marker_width <= remaining_text_width {
          buf.set_string(
            cursor_x,
            ghost_rect.y,
            marker_text,
            ghost_style.patch(modified_style),
          );
          cursor_x = cursor_x.saturating_add(marker_width.min(remaining_text_width));
        }
        truncate_with_ellipsis_in_place(&mut title, title_width as usize);
        buf.set_string(cursor_x, ghost_rect.y, title, ghost_style);
        if close_width > 0 {
          let close_x = ghost_rect.x.saturating_add(
            ghost_rect
              .width
              .saturating_sub(close_width.saturating_add(1)),
          );
          let close_is_hovered = ctx
            .buffer_tab_hover
            .is_some_and(|hover| hover.buffer_id == tab.buffer_id && hover.over_close);
          buf.set_string(
            close_x,
            ghost_rect.y,
            close_text,
            ghost_style.patch(close_style),
          );
          if close_is_hovered {
            buf.set_string(
              close_x,
              ghost_rect.y,
              close_text,
              ghost_style.patch(close_hover_style),
            );
          }
        }
      }
    }
  }
}

/// Render the current document state to the terminal.
pub fn render(f: &mut Frame, ctx: &mut Ctx) {
  let perf_enabled = term_render_perf_config().is_some();
  let perf_seq = if perf_enabled {
    TERM_RENDER_PERF_SEQ.fetch_add(1, Ordering::Relaxed) + 1
  } else {
    0
  };
  let perf_start = perf_enabled.then(Instant::now);
  let area = f.size();
  sync_file_picker_viewport(ctx, area);
  let perf_after_picker = perf_enabled.then(Instant::now);
  apply_editor_viewport(ctx, f.size());
  ensure_file_tree_sidebar_width(ctx);
  sync_file_tree_layout(ctx, area);
  let perf_after_ui = perf_enabled.then(Instant::now);
  if !ctx.mouse_selection_drag_active && !ctx.mouse_viewport_detached {
    ensure_cursor_visible(ctx);
  }
  let perf_after_visibility = perf_enabled.then(Instant::now);
  let frame_plan = frame_render_plan(ctx);
  let perf_after_plan = perf_enabled.then(Instant::now);

  f.render_widget(Clear, area);
  let perf_after_clear = perf_enabled.then(Instant::now);

  let (
    ui_cursor,
    active_editor_cursor,
    active_editor_cursor_kind,
    pane_draw_ms,
    ui_draw_ms,
    active_line_count,
    active_span_count,
  ) = {
    let buf = f.buffer_mut();
    let mut cursor_out = None;
    let mut editor_cursor = None;
    let mut editor_cursor_kind = None;
    let mut active_line_count = 0usize;
    let mut active_span_count = 0usize;
    let base_text_style = lib_style_to_ratatui(ctx.ui_theme.try_get("ui.text").unwrap_or_default());
    if let Some(bg) = ctx
      .ui_theme
      .try_get("ui.background")
      .and_then(|style| style.bg)
    {
      fill_rect(buf, area, Style::default().bg(lib_color_to_ratatui(bg)));
    }

    draw_buffer_tabs_row(buf, area, ctx);

    let pane_draw_start = perf_enabled.then(Instant::now);
    let active_pane_is_client_surface = frame_plan
      .panes
      .iter()
      .find(|pane| pane.pane_id == frame_plan.active_pane)
      .is_some_and(|pane| {
        matches!(
          pane.pane_kind,
          the_lib::editor::PaneContentKind::ClientSurface
        )
      });
    let mut active_pane_area = None;
    for pane in &frame_plan.panes {
      let pane_area = pane_screen_rect(area, pane.rect);
      let is_active = pane.pane_id == frame_plan.active_pane;
      if let Some(surface_id) = pane.client_surface_id
        && let Some(snapshot) = file_tree_snapshot(ctx)
        && snapshot.surface_id == surface_id
      {
        draw_file_tree_pane(buf, pane_area, ctx, &snapshot);
        if is_active {
          editor_cursor = None;
          editor_cursor_kind = None;
          active_line_count = 0;
          active_span_count = 0;
          active_pane_area = Some(pane_area);
        }
      } else {
        let pane_cursor = draw_pane_content(
          buf,
          ctx,
          pane.pane_id,
          pane_area,
          &pane.plan,
          base_text_style,
          is_active,
          !active_pane_is_client_surface,
        );
        if is_active {
          editor_cursor = pane_cursor;
          editor_cursor_kind = pane.plan.cursors.first().map(|cursor| cursor.kind);
          active_line_count = pane.plan.lines.len();
          active_span_count = pane.plan.lines.iter().map(|line| line.spans.len()).sum();
          active_pane_area = Some(pane_area);
        }
      }
    }
    let pane_draw_ms = pane_draw_start.map_or(0.0, |start| start.elapsed().as_secs_f64() * 1000.0);

    draw_pane_separators(buf, area, &frame_plan, ctx);

    let ui_draw_start = perf_enabled.then(Instant::now);
    draw_overlays(
      buf,
      area,
      ctx,
      editor_cursor,
      frame_plan.active_plan(),
      active_pane_area,
      &mut cursor_out,
    );
    draw_statusline(buf, area, ctx);
    let ui_draw_ms = ui_draw_start.map_or(0.0, |start| start.elapsed().as_secs_f64() * 1000.0);
    (
      cursor_out,
      editor_cursor,
      editor_cursor_kind,
      pane_draw_ms,
      ui_draw_ms,
      active_line_count,
      active_span_count,
    )
  };

  let active_pane_is_file_tree = file_tree_snapshot(ctx)
    .is_some_and(|snapshot| snapshot.attached_pane == Some(frame_plan.active_pane));
  ctx.term_cursor_mode = resolve_term_cursor_mode(
    active_pane_is_file_tree,
    ui_cursor,
    active_editor_cursor,
    active_editor_cursor_kind,
  );

  if let Some(perf_start) = perf_start {
    let total_ms = perf_start.elapsed().as_secs_f64() * 1000.0;
    if term_render_perf_should_log(total_ms) {
      let perf_after_picker_ms = perf_after_picker
        .map(|instant| instant.duration_since(perf_start).as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
      let perf_after_ui_ms = perf_after_ui
        .map(|instant| instant.duration_since(perf_start).as_secs_f64() * 1000.0)
        .unwrap_or(perf_after_picker_ms);
      let perf_after_visibility_ms = perf_after_visibility
        .map(|instant| instant.duration_since(perf_start).as_secs_f64() * 1000.0)
        .unwrap_or(perf_after_ui_ms);
      let perf_after_plan_ms = perf_after_plan
        .map(|instant| instant.duration_since(perf_start).as_secs_f64() * 1000.0)
        .unwrap_or(perf_after_visibility_ms);
      let perf_after_clear_ms = perf_after_clear
        .map(|instant| instant.duration_since(perf_start).as_secs_f64() * 1000.0)
        .unwrap_or(perf_after_plan_ms);
      let view = ctx.editor.view();
      let scroll_changed = term_render_perf_scroll_changed(view.scroll.row, view.scroll.col);
      term_render_perf_write(format!(
        "kind=render seq={perf_seq} total={total_ms:.2}ms picker={picker_ms:.2}ms ui={ui_ms:.2}ms \
         ensure_visible={ensure_ms:.2}ms plan={plan_ms:.2}ms clear={clear_ms:.2}ms \
         panes={pane_draw_ms:.2}ms overlays={ui_draw_ms:.2}ms pane_count={} active_lines={} \
         active_spans={} scroll={}:{} scroll_changed={} viewport={}x{}",
        frame_plan.panes.len(),
        active_line_count,
        active_span_count,
        view.scroll.row,
        view.scroll.col,
        if scroll_changed { 1 } else { 0 },
        view.viewport.width,
        view.viewport.height,
        picker_ms = perf_after_picker_ms,
        ui_ms = perf_after_ui_ms - perf_after_picker_ms,
        ensure_ms = perf_after_visibility_ms - perf_after_ui_ms,
        plan_ms = perf_after_plan_ms - perf_after_visibility_ms,
        clear_ms = perf_after_clear_ms - perf_after_plan_ms,
      ));
    }
  }
}

fn is_diff_gutter_marker(text: &str) -> bool {
  matches!(text.trim(), "+" | "~" | "-")
}

fn sync_file_picker_viewport(ctx: &mut Ctx, area: Rect) {
  if !ctx.file_picker.active {
    ctx.file_picker_layout = None;
    ctx.file_picker_drag = None;
    return;
  }

  let Some(layout) = compute_file_picker_layout(area, &ctx.file_picker) else {
    set_picker_visible_rows(&mut ctx.file_picker, 1);
    ctx.file_picker.clamp_preview_scroll(1);
    ctx.file_picker_layout = None;
    ctx.file_picker_drag = None;
    return;
  };

  set_picker_visible_rows(&mut ctx.file_picker, layout.list_visible_rows());
  ctx
    .file_picker
    .clamp_preview_scroll(layout.preview_visible_rows());
  ctx.file_picker_layout = compute_file_picker_layout(area, &ctx.file_picker);
  if ctx.file_picker_layout.is_none() {
    ctx.file_picker_drag = None;
  }
}

/// Ensure cursor is visible by adjusting scroll if needed.
pub fn ensure_cursor_visible(ctx: &mut Ctx) {
  let Some(viewport_pane) = ctx.visible_editor_pane_for_viewport() else {
    return;
  };
  let (cursor_pos, cursor_line, cursor_col) = {
    let doc = ctx.editor.document();
    let text = doc.text();
    let max = text.len_chars();

    // Get the selected cursor position (active cursor if available).
    let selection = doc.selection();
    let Some(active_view) = ctx.editor.pane_view(viewport_pane) else {
      return;
    };
    let range = if let Some(active_cursor) = active_view.active_cursor {
      selection.range_by_id(active_cursor).copied()
    } else {
      selection.ranges().first().copied()
    };
    let Some(range) = range else {
      return;
    };
    let clamped = Range::new(range.anchor.min(max), range.head.min(max));
    let cursor_pos = clamped.cursor(text.slice(..));
    let cursor_line = text.char_to_line(cursor_pos);
    let cursor_col = cursor_pos - text.line_to_char(cursor_line);
    (cursor_pos, cursor_line, cursor_col)
  };

  let Some(view) = ctx.editor.pane_view(viewport_pane) else {
    return;
  };
  let doc = ctx.editor.document();
  let viewport_height = view.viewport.height as usize;
  let gutter_width = gutter_width_for_document(doc, view.viewport.width, &ctx.gutter_config);
  let viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1) as usize;

  if ctx.text_format.soft_wrap {
    let mut changed = false;
    let mut new_scroll = view.scroll;
    let cursor_visual_row = {
      let mut text_format = ctx.text_format.clone();
      text_format.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
      let mut annotations = ctx.text_annotations();
      visual_pos_at_char(
        doc.text().slice(..),
        &text_format,
        &mut annotations,
        cursor_pos,
      )
      .map(|pos| pos.row)
      .unwrap_or(cursor_line)
    };

    if let Some(new_row) = the_lib::view::scroll_row_to_keep_visible(
      cursor_visual_row,
      view.scroll.row,
      viewport_height,
      ctx.scrolloff,
    ) {
      new_scroll.row = new_row;
      changed = true;
    }

    if view.scroll.col != 0 {
      new_scroll.col = 0;
      changed = true;
    }

    if changed {
      if let Some(view) = ctx.editor.pane_view_mut(viewport_pane) {
        view.scroll = new_scroll;
      }
    }
    return;
  }

  if let Some(new_scroll) = the_lib::view::scroll_to_keep_visible(
    cursor_line,
    cursor_col,
    view.scroll,
    viewport_height,
    viewport_width,
    ctx.scrolloff,
  ) {
    if let Some(view) = ctx.editor.pane_view_mut(viewport_pane) {
      view.scroll = new_scroll;
    }
  }
}

#[cfg(test)]
mod tests {
  use ratatui::buffer::Cell;
  use the_lib::{
    render::VirtualLineSpec,
    transaction::Transaction,
  };

  use super::*;

  fn seed_underlined_cells(buf: &mut Buffer, rect: Rect) {
    let style = Style::reset()
      .fg(Color::Gray)
      .bg(Color::Black)
      .underline_color(Color::LightRed)
      .add_modifier(Modifier::UNDERLINED);
    for y in rect.y..rect.y.saturating_add(rect.height) {
      for x in rect.x..rect.x.saturating_add(rect.width) {
        buf.get_mut(x, y).set_symbol("~").set_style(style);
      }
    }
  }

  fn assert_cell_has_no_inherited_underline(cell: &Cell) {
    assert!(!cell.modifier.contains(Modifier::UNDERLINED));
    assert_eq!(cell.underline_color, Color::Reset);
  }

  fn render_line_text(plan: &RenderPlan, row: u16) -> Option<String> {
    plan.lines.iter().find(|line| line.row == row).map(|line| {
      line
        .spans
        .iter()
        .map(|span| span.text.to_string())
        .collect()
    })
  }

  fn buffer_contains_text(buf: &Buffer, rect: Rect, needle: &str) -> bool {
    (rect.y..rect.y.saturating_add(rect.height)).any(|y| {
      (rect.x..rect.x.saturating_add(rect.width))
        .map(|x| buf.get(x, y).symbol())
        .collect::<String>()
        .contains(needle)
    })
  }

  #[test]
  fn file_tree_active_forces_hidden_term_cursor() {
    let mode = resolve_term_cursor_mode(true, None, Some((12, 4)), Some(LibCursorKind::Underline));

    assert!(matches!(mode, TermCursorMode::Hidden));
  }

  #[test]
  fn editor_cursor_keeps_hardware_bar_and_underline_modes() {
    let mode = resolve_term_cursor_mode(false, None, Some((7, 3)), Some(LibCursorKind::Bar));

    assert!(matches!(
      mode,
      TermCursorMode::Hardware(TermHardwareCursor {
        x:    7,
        y:    3,
        kind: LibCursorKind::Bar,
      })
    ));
  }

  #[test]
  fn file_tree_rows_render_soft_selection_and_icon_prefix() {
    use std::num::NonZeroUsize;

    use the_default::FileTreeRow;
    use the_lib::editor::ClientSurfaceId;

    let ctx = Ctx::new(None).expect("ctx");
    let pane = Rect::new(0, 0, 24, 3);
    let mut buf = Buffer::empty(pane);
    let snapshot = FileTreeSnapshot {
      surface_id:     ClientSurfaceId::new(NonZeroUsize::new(1).expect("surface id")),
      root:           "/tmp".into(),
      rows:           vec![FileTreeRow {
        path:              "/tmp/the-core".into(),
        display_name:      "the-core".to_string(),
        depth:             0,
        ancestor_branches: Vec::new(),
        is_last_sibling:   true,
        has_children:      true,
        is_dir:            true,
        is_expanded:       false,
        is_current_file:   false,
        icon_name:         "folder".to_string(),
        icon_glyph:        "",
      }],
      selected:       Some(0),
      scroll_offset:  0,
      show_hidden:    false,
      follow_current: false,
      attached_pane:  None,
      active:         true,
    };

    draw_file_tree_pane(&mut buf, pane, &ctx, &snapshot);

    let panel = file_picker_panel_styles(&ctx);

    assert_eq!(buf.get(0, 1).symbol(), "");
    assert_eq!(buf.get(1, 1).symbol(), " ");
    assert_eq!(buf.get(2, 1).symbol(), "t");
    assert_eq!(buf.get(23, 1).bg, panel.fill.bg.unwrap_or(Color::Reset));
  }

  #[test]
  fn nested_file_tree_rows_render_unicode_guides_and_file_icons() {
    use std::num::NonZeroUsize;

    use the_default::FileTreeRow;
    use the_lib::editor::ClientSurfaceId;

    let ctx = Ctx::new(None).expect("ctx");
    let pane = Rect::new(0, 0, 24, 3);
    let mut buf = Buffer::empty(pane);
    let snapshot = FileTreeSnapshot {
      surface_id:     ClientSurfaceId::new(NonZeroUsize::new(1).expect("surface id")),
      root:           "/tmp".into(),
      rows:           vec![FileTreeRow {
        path:              "/tmp/src/the-core".into(),
        display_name:      "the-core".to_string(),
        depth:             1,
        ancestor_branches: vec![true],
        is_last_sibling:   true,
        has_children:      false,
        is_dir:            false,
        is_expanded:       false,
        is_current_file:   false,
        icon_name:         "file".to_string(),
        icon_glyph:        "f",
      }],
      selected:       Some(0),
      scroll_offset:  0,
      show_hidden:    false,
      follow_current: false,
      attached_pane:  None,
      active:         true,
    };

    draw_file_tree_pane(&mut buf, pane, &ctx, &snapshot);

    assert_eq!(buf.get(0, 1).symbol(), "│");
    assert_eq!(buf.get(2, 1).symbol(), "└");
    assert_eq!(buf.get(4, 1).symbol(), "f");
    assert_eq!(buf.get(6, 1).symbol(), "t");
  }

  #[test]
  fn file_tree_rows_render_right_aligned_vcs_and_diagnostic_badges() {
    use std::num::NonZeroUsize;

    use the_default::FileTreeRow;
    use the_lib::editor::ClientSurfaceId;

    let mut ctx = Ctx::new(None).expect("ctx");
    let path: std::path::PathBuf = "/tmp/the-core".into();
    ctx
      .file_tree_decorations
      .insert(path.clone(), crate::ctx::FileTreeDecorations {
        vcs:        Some(crate::ctx::FileTreeVcsKind::Modified),
        diagnostic: Some(DiagnosticSeverity::Warning),
      });
    let pane = Rect::new(0, 0, 24, 3);
    let mut buf = Buffer::empty(pane);
    let snapshot = FileTreeSnapshot {
      surface_id:     ClientSurfaceId::new(NonZeroUsize::new(1).expect("surface id")),
      root:           "/tmp".into(),
      rows:           vec![FileTreeRow {
        path,
        display_name: "the-core".to_string(),
        depth: 0,
        ancestor_branches: Vec::new(),
        is_last_sibling: true,
        has_children: true,
        is_dir: true,
        is_expanded: false,
        is_current_file: false,
        icon_name: "folder".to_string(),
        icon_glyph: "",
      }],
      selected:       Some(0),
      scroll_offset:  0,
      show_hidden:    false,
      follow_current: false,
      attached_pane:  None,
      active:         true,
    };

    draw_file_tree_pane(&mut buf, pane, &ctx, &snapshot);

    let row_symbols = (0..pane.width)
      .map(|x| buf.get(x, 1).symbol().to_string())
      .collect::<Vec<_>>();
    assert!(row_symbols.iter().any(|symbol| symbol == ""));
    assert!(row_symbols.iter().any(|symbol| symbol == ""));
  }

  #[test]
  fn file_picker_overlay_clears_inherited_underlines_without_reflowing_layout() {
    use the_default::{
      FilePickerItem,
      FilePickerItemAction,
      open_custom_picker,
    };

    let mut ctx = Ctx::new(None).expect("ctx");
    let item = FilePickerItem {
      absolute:     "/tmp/demo.rs".into(),
      display:      "demo.rs".to_string(),
      icon:         "file_rust".to_string(),
      is_dir:       false,
      display_path: false,
      action:       FilePickerItemAction::OpenFile("/tmp/demo.rs".into()),
      preview_path: Some("/tmp/demo.rs".into()),
      preview_line: None,
      preview_col:  None,
      row_data:     None,
      preview:      None,
      payload:      None,
    };
    open_custom_picker(&mut ctx, "File Picker", "/tmp".into(), None, vec![item], 0);

    let area = Rect::new(0, 0, 100, 24);
    let layout = compute_file_picker_layout(area, &ctx.file_picker).expect("layout");
    assert!(layout.show_preview);

    let mut buf = Buffer::empty(area);
    seed_underlined_cells(&mut buf, area);

    let mut cursor_out = None;
    draw_file_picker_panel(&mut buf, area, &ctx, &mut cursor_out);

    assert_cell_has_no_inherited_underline(buf.get(layout.list_prompt.x, layout.list_prompt.y));
    assert_cell_has_no_inherited_underline(buf.get(
      layout.list_content.x.saturating_add(1),
      layout.list_content.y,
    ));

    let preview = layout.preview_content.expect("preview content");
    assert_cell_has_no_inherited_underline(buf.get(preview.x, preview.y));
  }

  #[test]
  fn command_palette_overlay_clears_inherited_underlines() {
    use the_default::{
      CommandPaletteItem,
      CommandPaletteSource,
      CommandPaletteState,
    };

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.command_palette = CommandPaletteState {
      is_open:       true,
      source:        CommandPaletteSource::ActionPalette,
      source_mode:   Mode::Normal,
      query:         String::new(),
      selected:      Some(0),
      items:         vec![CommandPaletteItem::new("file-tree-move")],
      max_results:   usize::MAX,
      prefiltered:   true,
      scroll_offset: 0,
      prompt_text:   None,
    };

    let area = Rect::new(0, 0, 80, 12);
    let overlay = overlay_area(area, &ctx);
    let row_y = overlay.y.saturating_add(overlay.height.saturating_sub(1));
    let mut buf = Buffer::empty(area);
    seed_underlined_cells(&mut buf, area);

    draw_command_palette_overlay(&mut buf, area, &mut ctx);

    assert_cell_has_no_inherited_underline(buf.get(overlay.x, row_y));
    assert_cell_has_no_inherited_underline(
      buf.get(overlay.x + overlay.width.saturating_sub(1), row_y),
    );
  }

  #[test]
  fn hover_docs_overlay_clears_inherited_underlines() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs = Some("hover docs".to_string());

    let area = Rect::new(0, 0, 100, 24);
    let overlay = overlay_area(area, &ctx);
    let rect = completion_panel_rect(
      overlay,
      overlay
        .width
        .saturating_mul(2)
        .saturating_div(3)
        .min(84)
        .max(28),
      18,
      None,
    );
    let mut buf = Buffer::empty(area);
    seed_underlined_cells(&mut buf, area);

    draw_hover_overlay(&mut buf, area, &mut ctx, None);

    assert_cell_has_no_inherited_underline(buf.get(rect.x, rect.y));
    assert_cell_has_no_inherited_underline(
      buf.get(rect.x.saturating_add(1), rect.y.saturating_add(1)),
    );
  }

  #[test]
  fn active_render_plan_includes_inline_completion_annotations() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(40, 8);
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("abc\n".into()))),
    )
    .expect("seed text");
    assert!(<Ctx as DefaultContext>::apply_transaction(&mut ctx, &tx));

    let highlight = ctx.ui_theme.find_highlight("ui.virtual.inline");
    let mut owned = the_default::OwnedTextAnnotations::default();
    let _ = owned.add_inline_text(3, " ghost-inline", highlight);
    let _ = owned.add_virtual_line(VirtualLineSpec::after(0).text("ghost-line").single_line());
    ctx.inline_completion_annotations = owned;

    let plan = build_render_plan(&mut ctx);

    assert!(
      render_line_text(&plan, 0)
        .as_deref()
        .is_some_and(|text| text.contains("ghost-inline"))
    );
    assert!(plan.lines.iter().any(|line| {
      line
        .spans
        .iter()
        .map(|span| span.text.to_string())
        .collect::<String>()
        .contains("ghost-line")
    }));
  }

  #[test]
  fn completion_menu_renders_inline_provider_item() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.completion_menu.active = true;
    ctx.completion_menu.items = vec![
      the_default::CompletionMenuItem::new("printf(\"hello world\");")
        .detail("Copilot")
        .documentation("printf(\"hello world\");")
        .kind(
          "copilot",
          the_lib::render::graphics::Color::Rgb(0x8B, 0xB6, 0xFF),
        ),
    ];
    ctx.completion_menu.selected = Some(0);

    let area = Rect::new(0, 0, 100, 24);
    let mut buf = Buffer::empty(area);

    draw_completion_overlay(&mut buf, area, &mut ctx, Some((12, 12)));

    assert!(buffer_contains_text(
      &buf,
      area,
      file_picker_icon_glyph("copilot", false)
    ));
    assert!(buffer_contains_text(&buf, area, "printf(\"hello world\");"));
    assert!(!buffer_contains_text(&buf, area, "Copilot Prediction"));
  }

  #[test]
  fn nearby_remote_inline_completion_ghost_text_renders_at_target_line() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(60, 8);
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("line 1\nline 2\nline 3\n".into()))),
    )
    .expect("seed text");
    assert!(<Ctx as DefaultContext>::apply_transaction(&mut ctx, &tx));

    let highlight = ctx.ui_theme.find_highlight("ui.virtual.inline");
    let mut owned = the_default::OwnedTextAnnotations::default();
    let insertion = ctx.editor.document().text().line_to_char(1);
    let _ = owned.add_inline_text(insertion, "printf(\"hello world\");", highlight);
    ctx.inline_completion_annotations = owned;

    let plan = build_render_plan(&mut ctx);
    assert!(
      render_line_text(&plan, 1)
        .as_deref()
        .is_some_and(|text| text.contains("printf(\"hello world\");line 2"))
    );
  }
}
