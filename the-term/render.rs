//! Rendering - converts RenderPlan to ratatui draw calls.

use std::{
  cell::RefCell,
  collections::BTreeMap,
  env,
  path::Path,
  rc::Rc,
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
  FilePickerPreview,
  Mode,
  OverlayRect as DefaultOverlayRect,
  SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER,
  SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER,
  command_palette_filtered_indices,
  completion_docs_panel_rect as default_completion_docs_panel_rect,
  completion_panel_rect as default_completion_panel_rect,
  file_picker_icon_glyph,
  render_plan,
  set_picker_visible_rows,
  signature_help_markdown,
  signature_help_panel_rect as default_signature_help_panel_rect,
  ui_tree,
};
use the_lib::{
  docs_markdown::{
    DocsBlock,
    DocsInlineKind,
    DocsInlineRun,
    DocsListMarker,
    DocsSemanticKind,
    language_filename_hints,
    parse_markdown_blocks,
  },
  diagnostics::{
    Diagnostic,
    DiagnosticSeverity,
  },
  render::{
    InlineDiagnostic,
    InlineDiagnosticFilter,
    InlineDiagnosticsConfig,
    InlineDiagnosticsLineAnnotation,
    LayoutIntent,
    NoHighlights,
    RenderDiagnosticGutterStyles,
    RenderDiffGutterStyles,
    RenderPlan,
    RenderStyles,
    SharedInlineDiagnosticsRenderData,
    SyntaxHighlightAdapter,
    UiAlign,
    UiAlignPair,
    UiAxis,
    UiColor,
    UiColorToken,
    UiConstraints,
    UiContainer,
    UiEmphasis,
    UiInput,
    UiInsets,
    UiLayer,
    UiLayout,
    UiList,
    UiListItem,
    UiNode,
    UiPanel,
    UiStatusBar,
    UiStyle,
    UiStyledSpan,
    UiText,
    UiTooltip,
    UiTree,
    apply_diagnostic_gutter_markers,
    apply_diff_gutter_markers,
    build_plan,
    graphics::{
      Modifier as LibModifier,
      Style as LibStyle,
      UnderlineStyle as LibUnderlineStyle,
    },
    gutter_width_for_document,
    text_annotations::TextAnnotations,
    ui_theme::resolve_ui_tree,
    visual_pos_at_char,
  },
  selection::Range,
  syntax::{
    Highlight,
    Syntax,
  },
};
use the_lsp::text_sync::utf16_position_to_char_idx;

use crate::{
  Ctx,
  ctx::DiagnosticUnderlineRenderSpan,
  docs_panel::{
    DocsPanelConfig,
    DocsPanelSource,
    build_docs_panel,
    docs_panel_source_from_panel,
    docs_panel_source_from_text,
  },
  picker_layout::{
    CompletionDocsLayout,
    FilePickerLayout,
    compute_file_picker_layout,
    compute_scrollbar_metrics,
  },
};

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

fn render_styles_from_theme(theme: &the_lib::render::theme::Theme) -> RenderStyles {
  let selection = theme.try_get("ui.selection").unwrap_or_default();
  let cursor = theme.try_get("ui.cursor").unwrap_or_default();
  let active_cursor = theme
    .try_get("ui.cursor.primary")
    .or_else(|| theme.try_get("ui.cursor"))
    .unwrap_or_default();
  RenderStyles {
    selection,
    cursor,
    active_cursor,
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

fn active_diagnostics_by_line(ctx: &Ctx) -> BTreeMap<usize, DiagnosticSeverity> {
  let Some(state) = ctx.lsp_document.as_ref().filter(|state| state.opened) else {
    return BTreeMap::new();
  };
  let Some(document) = ctx.diagnostics.document(&state.uri) else {
    return BTreeMap::new();
  };

  let mut out = BTreeMap::new();
  for diagnostic in &document.diagnostics {
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

fn active_inline_diagnostics(ctx: &Ctx) -> Vec<InlineDiagnostic> {
  let Some(state) = ctx.lsp_document.as_ref().filter(|state| state.opened) else {
    return Vec::new();
  };
  let Some(document_diagnostics) = ctx.diagnostics.document(&state.uri) else {
    return Vec::new();
  };

  inline_diagnostics_from_document(
    ctx.editor.document().text(),
    &document_diagnostics.diagnostics,
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

fn primary_cursor_char_idx(ctx: &Ctx) -> Option<usize> {
  let doc = ctx.editor.document();
  let range = doc.selection().ranges().first().copied()?;
  Some(range.cursor(doc.text().slice(..)))
}

fn primary_cursor_line_idx(ctx: &Ctx) -> Option<usize> {
  let doc = ctx.editor.document();
  let range = doc.selection().ranges().first().copied()?;
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
  ctx: &Ctx,
) {
  let row_start = plan.scroll.row;
  let row_end = row_start.saturating_add(plan.viewport.height as usize);
  let content_width = plan.content_width();
  if content_width == 0 {
    return;
  }

  for line in &ctx.inline_diagnostic_lines {
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

    let style = inline_diagnostic_text_style(&ctx.ui_theme, line.severity);
    let max_width = content_width.saturating_sub(visible_col);
    buf.set_stringn(x, y, line.text.as_str(), max_width, style);
  }
}

fn draw_diagnostic_underlines(buf: &mut Buffer, area: Rect, content_x: u16, ctx: &Ctx) {
  for underline in &ctx.diagnostic_underlines {
    let y = area.y.saturating_add(underline.row);
    if y >= area.y + area.height {
      continue;
    }

    let x_start = content_x.saturating_add(underline.start_col);
    let x_end = content_x.saturating_add(underline.end_col);
    if x_start >= area.x + area.width || x_start >= x_end {
      continue;
    }

    let style = lib_style_to_ratatui(diagnostic_underline_theme_style(
      &ctx.ui_theme,
      underline.severity,
    ));
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
) {
  let content_width = plan.content_width();
  if content_width == 0 {
    return;
  }

  let Some(lsp_state) = ctx.lsp_document.as_ref().filter(|state| state.opened) else {
    return;
  };
  let Some(document_diagnostics) = ctx.diagnostics.document(&lsp_state.uri) else {
    return;
  };
  if document_diagnostics.diagnostics.is_empty() {
    return;
  }

  let eol_filter = end_of_line_diagnostics_filter();
  if matches!(eol_filter, InlineDiagnosticFilter::Disable) {
    return;
  }

  let inline_config =
    inline_diagnostics_config().prepare(content_width.max(1) as u16, ctx.mode() != Mode::Insert);
  let cursor_doc_line = primary_cursor_line_idx(ctx);
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

    let diagnostics_on_row = document_diagnostics
      .diagnostics
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
      document_diagnostics
        .diagnostics
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
        "doc_diagnostic_count": document_diagnostics.diagnostics.len(),
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

fn inset_rect(rect: Rect, insets: the_lib::render::UiInsets) -> Rect {
  let x = rect.x.saturating_add(insets.left);
  let y = rect.y.saturating_add(insets.top);
  let width = rect
    .width
    .saturating_sub(insets.left.saturating_add(insets.right));
  let height = rect
    .height
    .saturating_sub(insets.top.saturating_add(insets.bottom));
  Rect::new(x, y, width, height)
}

fn align_horizontal(rect: Rect, child_width: u16, align: the_lib::render::UiAlign) -> (u16, u16) {
  let width = match align {
    the_lib::render::UiAlign::Stretch => rect.width,
    _ => child_width.min(rect.width).max(1),
  };
  let x = match align {
    the_lib::render::UiAlign::Center => rect.x + (rect.width.saturating_sub(width)) / 2,
    the_lib::render::UiAlign::End => rect.x + rect.width.saturating_sub(width),
    _ => rect.x,
  };
  (x, width)
}

fn align_vertical(rect: Rect, child_height: u16, align: the_lib::render::UiAlign) -> (u16, u16) {
  let height = match align {
    the_lib::render::UiAlign::Stretch => rect.height,
    _ => child_height.min(rect.height).max(1),
  };
  let y = match align {
    the_lib::render::UiAlign::Center => rect.y + (rect.height.saturating_sub(height)) / 2,
    the_lib::render::UiAlign::End => rect.y + rect.height.saturating_sub(height),
    _ => rect.y,
  };
  (y, height)
}

fn resolve_ui_color(color: &the_lib::render::UiColor) -> Option<Color> {
  match color {
    the_lib::render::UiColor::Value(value) => Some(lib_color_to_ratatui(*value)),
    the_lib::render::UiColor::Token(_) => None,
  }
}

fn ui_style_colors(style: &UiStyle) -> (Style, Style, Style) {
  let text_color = style.fg.as_ref().and_then(resolve_ui_color);
  let bg_color = style.bg.as_ref().and_then(resolve_ui_color);
  let border_color = style.border.as_ref().and_then(resolve_ui_color);

  let mut text_style = Style::default();
  if let Some(color) = text_color {
    text_style = text_style.fg(color);
  }

  let mut fill_style = Style::default();
  if let Some(color) = bg_color {
    fill_style = fill_style.bg(color);
  }

  let mut border_style = Style::default();
  if let Some(color) = border_color {
    border_style = border_style.fg(color);
  }

  (text_style, fill_style, border_style)
}

fn software_cursor_style(theme: &the_lib::render::theme::Theme) -> Style {
  theme
    .try_get("ui.cursor.primary")
    .or_else(|| theme.try_get("ui.cursor"))
    .map(lib_style_to_ratatui)
    .unwrap_or_else(|| Style::default().add_modifier(Modifier::REVERSED))
}

fn draw_software_cursor_cell(buf: &mut Buffer, x: u16, y: u16, cursor_style: Style) {
  let cell = buf.get_mut(x, y);
  cell.set_style(cell.style().patch(cursor_style));
}

fn apply_ui_emphasis(style: Style, emphasis: UiEmphasis) -> Style {
  match emphasis {
    UiEmphasis::Muted => style.add_modifier(Modifier::DIM),
    UiEmphasis::Strong => style.add_modifier(Modifier::BOLD),
    UiEmphasis::Normal => style,
  }
}

fn apply_constraints(
  mut width: u16,
  mut height: u16,
  constraints: &the_lib::render::UiConstraints,
  max_width: u16,
  max_height: u16,
) -> (u16, u16) {
  if let Some(min_w) = constraints.min_width {
    width = width.max(min_w);
  }
  if let Some(max_w) = constraints.max_width {
    width = width.min(max_w);
  }
  width = width.min(max_width).max(1);

  if let Some(min_h) = constraints.min_height {
    height = height.max(min_h);
  }
  if let Some(max_h) = constraints.max_height {
    height = height.min(max_h);
  }
  height = height.min(max_height).max(1);

  (width, height)
}

fn measure_node(node: &UiNode, max_width: u16) -> (u16, u16) {
  match node {
    UiNode::Text(text) => {
      let mut width = 0u16;
      let mut height = 0u16;
      let max_lines = text.max_lines.unwrap_or(u16::MAX) as usize;
      for line in text.content.lines() {
        let line_len = line.chars().count() as u16;
        if text.clip {
          width = width.max(line_len);
          height = height.saturating_add(1);
        } else if max_width > 0 {
          width = width.max(max_width);
          let wrapped = ((line_len as usize + max_width as usize - 1) / max_width as usize).max(1);
          height = height.saturating_add(wrapped as u16);
        } else {
          width = width.max(line_len);
          height = height.saturating_add(1);
        }
        if height as usize >= max_lines {
          height = max_lines as u16;
          break;
        }
      }
      (width.min(max_width), height.max(1))
    },
    UiNode::Input(input) => {
      let mut width = input.value.chars().count();
      if let Some(placeholder) = input.placeholder.as_ref() {
        width = width.max(placeholder.chars().count());
      }
      (width.min(max_width as usize) as u16, 1)
    },
    UiNode::Divider(_) => (max_width, 1),
    UiNode::Spacer(spacer) => (max_width, spacer.size.max(1)),
    UiNode::List(list) => {
      let mut width: usize = 0;
      let is_completion_list = list.style.role.as_deref() == Some("completion");
      let has_icons =
        is_completion_list && list.items.iter().any(|item| item.leading_icon.is_some());
      let icon_width: usize = if has_icons { 2 } else { 0 };
      let mut has_detail = false;
      for item in &list.items {
        let mut w = item.title.chars().count() + icon_width;
        if let Some(shortcut) = item.shortcut.as_ref() {
          w = w.saturating_add(shortcut.chars().count() + 3);
        }
        if let Some(detail) = item
          .subtitle
          .as_deref()
          .filter(|s| !s.is_empty())
          .or_else(|| item.description.as_deref().filter(|s| !s.is_empty()))
        {
          if is_completion_list {
            w = w.saturating_add(detail.chars().count() + 2);
          } else {
            has_detail = true;
            w = w.max(detail.chars().count());
          }
        }
        width = width.max(w);
      }
      let width = if list.fill_width {
        max_width
      } else {
        width.min(max_width as usize).max(1) as u16
      };
      let base_height = if is_completion_list {
        1
      } else if has_detail {
        2
      } else {
        1
      };
      let row_height = base_height;
      let mut count = list.items.len().max(1);
      if let Some(max_visible) = list.max_visible {
        count = count.min(max_visible.max(1));
      }
      let count = count as u16;
      let total_height = count.saturating_mul(row_height as u16);
      (width, total_height)
    },
    UiNode::Container(container) => {
      match &container.layout {
        UiLayout::Stack { axis, gap } => {
          match axis {
            UiAxis::Vertical => {
              let mut height = 0u16;
              let mut width = 0u16;
              for (idx, child) in container.children.iter().enumerate() {
                let (cw, ch) = measure_node(child, max_width);
                width = width.max(cw);
                height = height.saturating_add(ch);
                if idx + 1 < container.children.len() {
                  height = height.saturating_add(*gap);
                }
              }
              let width = width.saturating_add(container.constraints.padding.horizontal());
              let height = height.saturating_add(container.constraints.padding.vertical());
              apply_constraints(
                width,
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
            },
            UiAxis::Horizontal => {
              let mut width = 0u16;
              let mut height = 0u16;
              for (idx, child) in container.children.iter().enumerate() {
                let (cw, ch) = measure_node(child, max_width);
                width = width.saturating_add(cw);
                height = height.max(ch);
                if idx + 1 < container.children.len() {
                  width = width.saturating_add(*gap);
                }
              }
              let width = width.saturating_add(container.constraints.padding.horizontal());
              let height = height.saturating_add(container.constraints.padding.vertical());
              apply_constraints(
                width.max(1),
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
            },
          }
        },
        UiLayout::Split { axis, .. } => {
          match axis {
            UiAxis::Vertical => {
              let width = max_width.saturating_add(container.constraints.padding.horizontal());
              let height =
                container.children.len().max(1) as u16 + container.constraints.padding.vertical();
              apply_constraints(
                width.max(1),
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
            },
            UiAxis::Horizontal => {
              let width = max_width.saturating_add(container.constraints.padding.horizontal());
              let height = 1 + container.constraints.padding.vertical();
              apply_constraints(
                width.max(1),
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
            },
          }
        },
      }
    },
    UiNode::Panel(panel) => {
      let max_width =
        max_content_width_for_intent(panel.intent.clone(), Rect::new(0, 0, max_width, 1), 0, 0);
      let (child_w, child_h) = measure_node(&panel.child, max_width);
      let width = child_w.saturating_add(panel.constraints.padding.horizontal());
      let height = child_h.saturating_add(panel.constraints.padding.vertical());
      apply_constraints(
        width.max(1),
        height.max(1),
        &panel.constraints,
        max_width,
        u16::MAX,
      )
    },
    UiNode::Tooltip(tooltip) => {
      let width = tooltip
        .content
        .chars()
        .count()
        .saturating_add(2)
        .min(max_width as usize) as u16;
      (width.max(2), 3)
    },
    UiNode::StatusBar(_) => (max_width, 1),
  }
}

fn layout_children<'a>(container: &'a UiContainer, rect: Rect) -> Vec<(Rect, &'a UiNode)> {
  let mut placements = Vec::new();
  let rect = inset_rect(rect, container.constraints.padding);

  match &container.layout {
    UiLayout::Stack { axis, gap } => {
      match axis {
        UiAxis::Vertical => {
          let mut y = rect.y;
          for child in &container.children {
            let (child_w, h) = measure_node(child, rect.width);
            let height = h
              .min(rect.height.saturating_sub(y.saturating_sub(rect.y)))
              .max(1);
            if height == 0 {
              break;
            }
            let (x, width) =
              align_horizontal(rect, child_w, container.constraints.align.horizontal);
            let child_rect = Rect::new(x, y, width, height);
            placements.push((child_rect, child));
            y = y.saturating_add(height).saturating_add(*gap);
            if y >= rect.y + rect.height {
              break;
            }
          }
        },
        UiAxis::Horizontal => {
          let mut x = rect.x;
          for child in &container.children {
            let (w, child_h) = measure_node(child, rect.width);
            let width = w
              .min(rect.width.saturating_sub(x.saturating_sub(rect.x)))
              .max(1);
            if width == 0 {
              break;
            }
            let (y, height) = align_vertical(rect, child_h, container.constraints.align.vertical);
            let child_rect = Rect::new(x, y, width, height);
            placements.push((child_rect, child));
            x = x.saturating_add(width).saturating_add(*gap);
            if x >= rect.x + rect.width {
              break;
            }
          }
        },
      }
    },
    UiLayout::Split { axis, ratios } => {
      let count = container.children.len().max(1);
      let mut ratios = ratios.clone();
      if ratios.len() < count {
        ratios.resize(count, 1);
      }
      let total: u16 = ratios.iter().sum();
      let total = if total == 0 { count as u16 } else { total };

      match axis {
        UiAxis::Vertical => {
          let mut y = rect.y;
          for (child, ratio) in container.children.iter().zip(ratios.iter()) {
            let height = rect
              .height
              .saturating_mul(*ratio)
              .saturating_div(total)
              .max(1);
            let (x, width) =
              align_horizontal(rect, rect.width, container.constraints.align.horizontal);
            let child_rect = Rect::new(x, y, width, height);
            placements.push((child_rect, child));
            y = y.saturating_add(height);
          }
        },
        UiAxis::Horizontal => {
          let mut x = rect.x;
          for (child, ratio) in container.children.iter().zip(ratios.iter()) {
            let width = rect
              .width
              .saturating_mul(*ratio)
              .saturating_div(total)
              .max(1);
            let (y, height) =
              align_vertical(rect, rect.height, container.constraints.align.vertical);
            let child_rect = Rect::new(x, y, width, height);
            placements.push((child_rect, child));
            x = x.saturating_add(width);
          }
        },
      }
    },
  }

  placements
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
    "ui.selection.primary",
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
    if line.len() == 1
      && width > 0
      && matches!(line[0].kind, DocsSemanticKind::Rule)
    {
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

fn draw_completion_docs_text(buf: &mut Buffer, rect: Rect, ctx: &Ctx, text: &UiText) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }

  let (text_style, ..) = ui_style_colors(&text.style);
  let base_style = apply_ui_emphasis(text_style, text.style.emphasis);
  let styles = completion_docs_styles(ctx, base_style);
  let metrics = completion_docs_render_metrics(&text.content, &styles, rect);
  let content_width = metrics.content_width;
  let rows = completion_docs_rows_with_context(&text.content, &styles, content_width, Some(ctx));
  let total_rows = metrics.total_rows;
  let visible_rows = metrics.visible_rows;
  let max_scroll = total_rows.saturating_sub(visible_rows);
  let docs_scroll = match docs_panel_source_from_text(text).unwrap_or(DocsPanelSource::Completion) {
    DocsPanelSource::Completion => ctx.completion_menu.docs_scroll,
    DocsPanelSource::Hover => ctx.hover_docs_scroll,
    DocsPanelSource::Signature => ctx.signature_help.docs_scroll,
    DocsPanelSource::CommandPalette => 0,
  };
  let scroll = docs_scroll.min(max_scroll);

  for row_idx in 0..visible_rows {
    let y = rect.y + row_idx as u16;
    if let Some(row) = rows.get(scroll + row_idx) {
      draw_styled_row(buf, rect.x, y, content_width, row, base_style);
    } else {
      draw_styled_row(buf, rect.x, y, content_width, &[], base_style);
    }
  }

  if metrics.show_scrollbar {
    let track_x = rect.x + rect.width - 1;
    let track_height = rect.height;
    let thumb_height = ((visible_rows as f32 / total_rows as f32) * track_height as f32)
      .round()
      .clamp(1.0, track_height as f32) as u16;
    let max_thumb_offset = track_height.saturating_sub(thumb_height);
    let thumb_offset = if max_scroll == 0 || max_thumb_offset == 0 {
      0
    } else {
      ((scroll as f32 / max_scroll as f32) * max_thumb_offset as f32).round() as u16
    };
    let scroll_color = text
      .style
      .border
      .as_ref()
      .and_then(resolve_ui_color)
      .or_else(|| text.style.accent.as_ref().and_then(resolve_ui_color))
      .or(base_style.fg);

    for row in 0..track_height {
      let is_thumb = row >= thumb_offset && row < thumb_offset + thumb_height;
      if !is_thumb {
        continue;
      }
      let mut style = Style::default();
      if let Some(color) = scroll_color {
        style = style.fg(color);
      }
      buf.set_string(track_x, rect.y + row, "█", style);
    }
  }
}

fn draw_ui_text(buf: &mut Buffer, rect: Rect, ctx: &Ctx, text: &UiText) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  if docs_panel_source_from_text(text).is_some()
    || text.style.role.as_deref() == Some("completion_docs")
  {
    draw_completion_docs_text(buf, rect, ctx, text);
    return;
  }
  let (text_style, ..) = ui_style_colors(&text.style);
  let style = apply_ui_emphasis(text_style, text.style.emphasis);
  let max_lines = text.max_lines.unwrap_or(u16::MAX) as usize;
  let mut drawn = 0usize;

  for line in text.content.lines() {
    if drawn >= max_lines {
      break;
    }

    if text.clip || rect.width == 0 {
      let y = rect.y + drawn as u16;
      if y >= rect.y + rect.height {
        break;
      }
      let mut truncated = line.to_string();
      truncate_in_place(&mut truncated, rect.width as usize);
      buf.set_string(rect.x, y, truncated, style);
      drawn += 1;
    } else {
      let mut chunk = String::new();
      for ch in line.chars() {
        if chunk.chars().count() >= rect.width as usize {
          let y = rect.y + drawn as u16;
          if y >= rect.y + rect.height {
            break;
          }
          buf.set_string(rect.x, y, chunk.clone(), style);
          drawn += 1;
          chunk.clear();
          if drawn >= max_lines {
            break;
          }
        }
        chunk.push(ch);
      }
      if !chunk.is_empty() && drawn < max_lines {
        let y = rect.y + drawn as u16;
        if y >= rect.y + rect.height {
          break;
        }
        buf.set_string(rect.x, y, chunk, style);
        drawn += 1;
      }
    }
  }
}

fn draw_ui_input(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  input: &UiInput,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, ..) = ui_style_colors(&input.style);
  let placeholder_color = input
    .style
    .accent
    .as_ref()
    .and_then(resolve_ui_color)
    .or(text_style.fg);
  let (value, style) = if input.value.is_empty() {
    let placeholder = input.placeholder.as_deref().unwrap_or("...");
    let mut style = Style::default();
    if let Some(color) = placeholder_color {
      style = style.fg(color);
    }
    (placeholder.to_string(), style)
  } else {
    (input.value.clone(), text_style)
  };
  let mut truncated = value;
  truncate_in_place(&mut truncated, rect.width as usize);
  buf.set_string(rect.x, rect.y, truncated, style);

  let is_focused = focus.map(|f| f.id == input.id).unwrap_or(focus.is_none());
  if is_focused && cursor_out.is_none() {
    let cursor_pos = focus.and_then(|f| f.cursor).unwrap_or(input.cursor);
    let cursor_x = rect
      .x
      .saturating_add(cursor_pos as u16)
      .min(rect.x + rect.width - 1);
    let cursor_style = software_cursor_style(&ctx.ui_theme);
    draw_software_cursor_cell(buf, cursor_x, rect.y, cursor_style);
    *cursor_out = Some((cursor_x, rect.y));
  }
}

fn draw_ui_list(buf: &mut Buffer, rect: Rect, list: &UiList, _cursor_out: &mut Option<(u16, u16)>) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, ..) = ui_style_colors(&list.style);
  let base_text_color = text_style.fg;
  let selected_text_color = list
    .style
    .border
    .as_ref()
    .and_then(resolve_ui_color)
    .or(base_text_color);
  let selected_bg_color = list.style.accent.as_ref().and_then(resolve_ui_color);
  let scroll_color = list
    .style
    .border
    .as_ref()
    .and_then(resolve_ui_color)
    .or(base_text_color);
  let is_completion_list = list.style.role.as_deref() == Some("completion");
  let has_icons = is_completion_list && list.items.iter().any(|item| item.leading_icon.is_some());
  let icon_col_width: u16 = if has_icons { 2 } else { 0 };
  // Keep completion labels legible even when detail/signature text is very long.
  // This mirrors column-based completion UIs (e.g. blink.cmp): label gets
  // priority, detail only uses remaining space.
  const COMPLETION_MIN_LABEL_WIDTH: usize = 18;
  const COMPLETION_LABEL_TARGET_NUM: usize = 3; // 60%
  const COMPLETION_LABEL_TARGET_DEN: usize = 5;
  const COMPLETION_MIN_DETAIL_WIDTH: usize = 12;
  let has_detail = list.items.iter().any(|item| {
    item.subtitle.as_ref().map_or(false, |s| !s.is_empty())
      || item.description.as_ref().map_or(false, |s| !s.is_empty())
  });
  let base_height: usize = if is_completion_list {
    1
  } else if has_detail {
    2
  } else {
    1
  };
  let row_gap: usize = 0;
  let row_height: usize = base_height + row_gap;
  let visible_rows = rect.height as usize;
  let mut visible_items = visible_rows / row_height;
  if let Some(max_visible) = list.max_visible {
    visible_items = visible_items.min(max_visible.max(1));
  }
  if visible_items == 0 {
    return;
  }
  let total_items = list.virtual_total.unwrap_or(list.items.len());
  let virtual_mode = list.virtual_total.is_some();
  let mut scroll_offset = if virtual_mode {
    list
      .virtual_start
      .min(total_items.saturating_sub(visible_items))
  } else {
    list.scroll.min(total_items.saturating_sub(visible_items))
  };
  let selected = list.selected;
  if !virtual_mode {
    if let Some(sel) = selected {
      if sel < scroll_offset {
        scroll_offset = sel;
      } else if sel >= scroll_offset + visible_items {
        scroll_offset = sel + 1 - visible_items;
      }
    }
  }
  let mut draw_item = |row_idx: usize, absolute_idx: usize, item: &the_lib::render::UiListItem| {
    let y = rect.y + (row_idx * row_height) as u16;
    let is_selected = selected == Some(absolute_idx);
    let row_right_padding = if total_items > visible_items { 2 } else { 1 };

    if is_selected {
      if let Some(bg_color) = selected_bg_color {
        fill_rect(
          buf,
          Rect::new(rect.x, y, rect.width, base_height as u16),
          Style::default().bg(bg_color),
        );
      }
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
    if item.emphasis {
      row_style = row_style.add_modifier(Modifier::BOLD);
    }

    let mut title = item.title.clone();
    let shortcut = item.shortcut.clone().unwrap_or_default();
    let leading_pad = if is_completion_list { 0 } else { 1 };
    let base_content_x = rect.x + leading_pad;
    let available_width = rect.width.saturating_sub(leading_pad + row_right_padding) as usize;
    if !shortcut.is_empty() && shortcut.len() + 2 < available_width {
      let shortcut_width = shortcut.len() + 1;
      truncate_in_place(&mut title, available_width.saturating_sub(shortcut_width));
      let shortcut_x = rect.x
        + rect
          .width
          .saturating_sub(shortcut.len() as u16 + row_right_padding);
      buf.set_string(shortcut_x, y, shortcut, row_style);
    } else {
      truncate_in_place(&mut title, available_width);
    }
    if is_completion_list {
      if has_icons {
        if let Some(icon) = item.leading_icon.as_deref() {
          let icon_style = if is_selected {
            row_style
          } else if let Some(ref color) = item.leading_color {
            resolve_ui_color(color)
              .map(|c| Style::default().fg(c))
              .unwrap_or(row_style)
          } else {
            row_style
          };
          buf.set_string(base_content_x, y, icon, icon_style);
        }
      }

      let label_x = base_content_x + icon_col_width;
      let label_available = rect
        .width
        .saturating_sub(leading_pad + icon_col_width + row_right_padding)
        as usize;

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
      if let Some(detail) = detail {
        let content_right = rect.x + rect.width.saturating_sub(row_right_padding);
        let reserved_label = ((label_available * COMPLETION_LABEL_TARGET_NUM)
          / COMPLETION_LABEL_TARGET_DEN)
          .max(COMPLETION_MIN_LABEL_WIDTH.min(label_available));
        let max_detail_width = label_available.saturating_sub(reserved_label.saturating_add(1));

        if max_detail_width >= COMPLETION_MIN_DETAIL_WIDTH {
          let mut detail_text = detail.to_string();
          truncate_in_place(&mut detail_text, max_detail_width);
          let detail_width = detail_text.chars().count() as u16;
          let detail_x = content_right.saturating_sub(detail_width);
          let mut title_width = detail_x.saturating_sub(label_x).saturating_sub(1) as usize;
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
          if detail_x > label_x {
            buf.set_string(detail_x, y, detail_text, detail_style);
          }
        } else {
          truncate_in_place(&mut title, label_available);
          buf.set_string(label_x, y, title, row_style);
        }
      } else {
        truncate_in_place(&mut title, label_available);
        buf.set_string(label_x, y, title, row_style);
      }
    } else {
      buf.set_string(base_content_x, y, title, row_style);
    }

    if !is_completion_list && base_height > 1 {
      let detail = item
        .subtitle
        .as_deref()
        .filter(|detail: &&str| !detail.is_empty())
        .or_else(|| {
          item
            .description
            .as_deref()
            .filter(|detail: &&str| !detail.is_empty())
        });
      if let Some(detail) = detail {
        let mut detail_text = detail.to_string();
        truncate_in_place(&mut detail_text, available_width);
        let mut detail_style = row_style;
        if !is_selected {
          detail_style = detail_style.add_modifier(Modifier::DIM);
        }
        buf.set_string(rect.x + 1, y + 1, detail_text, detail_style);
      }
    }
  };

  if virtual_mode {
    for (row_idx, item) in list.items.iter().take(visible_items).enumerate() {
      draw_item(row_idx, scroll_offset + row_idx, item);
    }
  } else {
    for (row_idx, item) in list
      .items
      .iter()
      .skip(scroll_offset)
      .take(visible_items)
      .enumerate()
    {
      draw_item(row_idx, row_idx + scroll_offset, item);
    }
  }

  let selected_track_row = selected.and_then(|sel| {
    if sel < scroll_offset || sel >= scroll_offset + visible_items {
      return None;
    }
    let visible_row = sel - scroll_offset;
    let row_start = (visible_row * row_height) as u16;
    let row_end = row_start.saturating_add(base_height as u16);
    Some((row_start, row_end))
  });

  if total_items > visible_items {
    let track_x = rect.x + rect.width - 1;
    let track_height = rect.height;
    let thumb_height = ((visible_items as f32 / total_items as f32) * track_height as f32)
      .ceil()
      .max(1.0) as u16;
    let max_scroll = total_items.saturating_sub(visible_items);
    let mut thumb_offset = if max_scroll == 0 {
      0
    } else {
      ((scroll_offset as f32 / max_scroll as f32) * (track_height - thumb_height) as f32).round()
        as u16
    };
    if let Some((selected_start, selected_end)) = selected_track_row {
      let max_thumb_offset = track_height.saturating_sub(thumb_height);
      let overlaps_selected = |offset: u16| {
        let thumb_end = offset.saturating_add(thumb_height);
        offset < selected_end && thumb_end > selected_start
      };
      if overlaps_selected(thumb_offset) && thumb_height < track_height {
        let mut moved = false;
        for candidate in (thumb_offset + 1)..=max_thumb_offset {
          if !overlaps_selected(candidate) {
            thumb_offset = candidate;
            moved = true;
            break;
          }
        }
        if !moved && thumb_offset > 0 {
          for candidate in (0..thumb_offset).rev() {
            if !overlaps_selected(candidate) {
              thumb_offset = candidate;
              break;
            }
          }
        }
      }
    }
    for i in 0..track_height {
      let y = rect.y + i;
      let is_thumb = i >= thumb_offset && i < thumb_offset + thumb_height;
      if !is_thumb {
        continue;
      }
      let mut style = Style::default();
      if let Some(color) = scroll_color {
        style = style.fg(color);
      }
      buf.set_string(track_x, y, "█", style);
    }
  }
}

fn draw_ui_node(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  node: &UiNode,
  focus: Option<&the_lib::render::UiFocus>,
  editor_cursor: Option<(u16, u16)>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  match node {
    UiNode::Text(text) => draw_ui_text(buf, rect, ctx, text),
    UiNode::Input(input) => draw_ui_input(buf, rect, ctx, input, focus, cursor_out),
    UiNode::List(list) => draw_ui_list(buf, rect, list, cursor_out),
    UiNode::Divider(_) => {
      let line = "─".repeat(rect.width as usize);
      let style = Style::default().add_modifier(Modifier::DIM);
      buf.set_string(rect.x, rect.y, line, style);
    },
    UiNode::Spacer(_) => {},
    UiNode::Container(container) => {
      let placements = layout_children(container, rect);
      for (child_rect, child) in placements {
        draw_ui_node(
          buf,
          child_rect,
          ctx,
          child,
          focus,
          editor_cursor,
          cursor_out,
        );
      }
    },
    UiNode::Panel(panel) => {
      draw_ui_panel(buf, rect, ctx, panel, focus, editor_cursor, cursor_out);
    },
    UiNode::Tooltip(tooltip) => {
      draw_ui_tooltip(buf, rect, ctx, tooltip);
    },
    UiNode::StatusBar(status) => {
      draw_ui_status_bar(buf, rect, ctx, status);
    },
  }
}

fn draw_file_picker_panel(
  buf: &mut Buffer,
  area: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  focus: Option<&the_lib::render::UiFocus>,
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

  let (text_style, mut fill_style, border_style) = ui_style_colors(&panel.style);
  if fill_style.bg.is_none() {
    let fallback_bg = ctx
      .ui_theme
      .try_get("ui.file_picker")
      .and_then(|style| style.bg)
      .or_else(|| {
        ctx
          .ui_theme
          .try_get("ui.background")
          .and_then(|style| style.bg)
      })
      .map(lib_color_to_ratatui);
    if let Some(bg) = fallback_bg {
      fill_style = fill_style.bg(bg);
    }
  }
  if fill_style.bg.is_none() {
    fill_style = fill_style.bg(Color::Reset);
  }

  fill_rect(buf, layout.panel, fill_style);

  if layout.panel_inner.width < 3 || layout.panel_inner.height < 3 {
    return;
  }

  draw_file_picker_list_pane(
    buf,
    &layout,
    picker,
    text_style,
    fill_style,
    border_style,
    &ctx.ui_theme,
    focus,
    cursor_out,
  );

  if layout.show_preview {
    draw_file_picker_preview_pane(
      buf,
      &layout,
      picker,
      text_style,
      fill_style,
      border_style,
      &ctx.ui_theme,
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
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let rect = layout.list_pane;
  let title_style = text_style.add_modifier(Modifier::BOLD);
  let count = format!("{}/{}", picker.matched_count(), picker.total_count());
  let count_style = text_style.add_modifier(Modifier::DIM);

  let borders = if layout.show_preview {
    Borders::TOP | Borders::LEFT | Borders::BOTTOM
  } else {
    Borders::ALL
  };

  let block = Block::default()
    .title(Span::styled(" Find File ", title_style))
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

  let is_focused = focus
    .map(|focus| focus.id == "file_picker_input")
    .unwrap_or(true);
  if is_focused && cursor_out.is_none() {
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
    buf.get_mut(rect.x, separator_y).set_symbol("├").set_style(sep_style);
    if layout.show_preview {
      if let Some(preview_pane) = layout.preview_pane {
        buf.get_mut(preview_pane.x, separator_y).set_symbol("┤").set_style(sep_style);
      }
    } else {
      buf.get_mut(rect.x + rect.width.saturating_sub(1), separator_y)
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
        Style::default().bg(bg),
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

    // Split display path into filename + parent directory (like fff.nvim).
    let display = item.display.as_str();
    let (dir_part, file_part) = match display.rfind('/') {
      Some(pos) => (&display[..pos], &display[pos + 1..]),
      None => ("", display),
    };
    let file_char_start: usize = display.chars().count() - file_part.chars().count();

    // Draw filename with fuzzy highlighting (remap indices from full path).
    let file_len = file_part.chars().count();
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
) {
  let Some(rect) = layout.preview_pane else {
    return;
  };

  let mut block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(border_style)
    .style(fill_style);
  if let Some(preview_path) = &picker.preview_path {
    let path_display = preview_path
      .strip_prefix(&picker.root)
      .unwrap_or(preview_path)
      .display()
      .to_string();
    block = block.title(
      Title::from(Span::styled(
        format!(" {} ", path_display),
        text_style.add_modifier(Modifier::DIM),
      )),
    );
  }
  block.render(rect, buf);

  // Fix junction characters where preview's left border meets the top/bottom borders.
  if rect.height > 0 {
    buf.get_mut(rect.x, rect.y).set_symbol("┬").set_style(border_style);
    buf
      .get_mut(rect.x, rect.y + rect.height.saturating_sub(1))
      .set_symbol("┴")
      .set_style(border_style);
  }

  let Some(content) = layout.preview_content else {
    return;
  };
  if content.width == 0 || content.height == 0 {
    return;
  }

  let scroll_offset = layout.preview_scroll_offset;
  let visible_rows = content.height as usize;
  let total_lines = picker.preview_line_count();

  match &picker.preview {
    FilePickerPreview::Empty => {},
    FilePickerPreview::Source(source) => {
      draw_file_picker_source_preview(buf, content, source, text_style, theme, scroll_offset);
    },
    FilePickerPreview::Text(text) | FilePickerPreview::Message(text) => {
      draw_file_picker_plain_preview(buf, content, text, text_style, scroll_offset);
    },
  }

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

fn draw_file_picker_source_preview(
  buf: &mut Buffer,
  area: Rect,
  source: &the_default::FilePickerSourcePreview,
  text_style: Style,
  theme: &the_lib::render::theme::Theme,
  scroll_offset: usize,
) {
  if area.width == 0 || area.height == 0 {
    return;
  }

  let lines_len = source.lines.len().max(1);
  let line_number_width = lines_len.to_string().len();
  let gutter_style = text_style.add_modifier(Modifier::DIM);

  for row in 0..area.height as usize {
    let y = area.y + row as u16;
    let line_idx = scroll_offset.saturating_add(row);
    if line_idx >= source.lines.len() {
      if source.truncated && line_idx == source.lines.len() {
        buf.set_stringn(area.x, y, "…", area.width as usize, gutter_style);
      }
      continue;
    }

    let line_number = line_idx + 1;
    let gutter = format!("{line_number:>line_number_width$} ");
    let gutter_width = gutter.chars().count() as u16;
    buf.set_stringn(area.x, y, &gutter, area.width as usize, gutter_style);

    if gutter_width >= area.width {
      continue;
    }

    let line = &source.lines[line_idx];
    if line.is_empty() {
      continue;
    }

    let line_start = source.line_starts[line_idx];
    let line_spans = preview_line_spans(line, line_start, &source.highlights, text_style, theme);

    Paragraph::new(Line::from(line_spans)).render(
      Rect::new(
        area.x + gutter_width,
        y,
        area.width.saturating_sub(gutter_width),
        1,
      ),
      buf,
    );
  }
}

fn draw_file_picker_plain_preview(
  buf: &mut Buffer,
  area: Rect,
  text: &str,
  text_style: Style,
  scroll_offset: usize,
) {
  if area.width == 0 || area.height == 0 {
    return;
  }

  for (row, line) in text
    .lines()
    .skip(scroll_offset)
    .take(area.height as usize)
    .enumerate()
  {
    buf.set_stringn(
      area.x,
      area.y + row as u16,
      line,
      area.width as usize,
      text_style,
    );
  }
}

fn preview_line_spans<'a>(
  line: &'a str,
  line_start: usize,
  highlights: &[(Highlight, std::ops::Range<usize>)],
  text_style: Style,
  theme: &the_lib::render::theme::Theme,
) -> Vec<Span<'a>> {
  if line.is_empty() {
    return Vec::new();
  }

  if highlights.is_empty() {
    return vec![Span::styled(line, text_style)];
  }

  let line_end = line_start.saturating_add(line.len());
  let mut boundaries = vec![line_start, line_end];
  for (_highlight, range) in highlights {
    if range.end <= line_start || range.start >= line_end {
      continue;
    }
    boundaries.push(range.start.max(line_start));
    boundaries.push(range.end.min(line_end));
  }
  boundaries.sort_unstable();
  boundaries.dedup();

  let mut spans = Vec::new();
  for pair in boundaries.windows(2) {
    let absolute_start = pair[0];
    let absolute_end = pair[1];
    if absolute_end <= absolute_start {
      continue;
    }

    let local_start = clamp_boundary(line, absolute_start.saturating_sub(line_start), false);
    let local_end = clamp_boundary(line, absolute_end.saturating_sub(line_start), true);
    if local_end <= local_start {
      continue;
    }

    let sample_byte = absolute_start + (absolute_end - absolute_start) / 2;
    let style = preview_highlight_at(highlights, sample_byte)
      .map(|highlight| text_style.patch(lib_style_to_ratatui(theme.highlight(highlight))))
      .unwrap_or(text_style);

    spans.push(Span::styled(&line[local_start..local_end], style));
  }

  if spans.is_empty() {
    spans.push(Span::styled(line, text_style));
  }
  spans
}

fn clamp_boundary(text: &str, idx: usize, round_up: bool) -> usize {
  let mut idx = idx.min(text.len());
  if text.is_char_boundary(idx) {
    return idx;
  }
  if round_up {
    while idx < text.len() && !text.is_char_boundary(idx) {
      idx += 1;
    }
    return idx;
  }
  while idx > 0 && !text.is_char_boundary(idx) {
    idx -= 1;
  }
  idx
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

fn max_content_width_for_intent(
  intent: LayoutIntent,
  area: Rect,
  border: u16,
  padding_h: u16,
) -> u16 {
  let full = area.width.saturating_sub(border * 2 + padding_h).max(1);
  match intent {
    LayoutIntent::Floating | LayoutIntent::Custom(_) => {
      let cap = area.width.saturating_mul(2) / 3;
      full.min(cap.max(20))
    },
    _ => full,
  }
}

fn panel_is_completion(panel: &UiPanel) -> bool {
  panel.id == "completion" || panel.style.role.as_deref() == Some("completion")
}

fn panel_is_completion_docs(panel: &UiPanel) -> bool {
  matches!(
    docs_panel_source_from_panel(panel),
    Some(DocsPanelSource::Completion)
  )
}

fn panel_is_hover(panel: &UiPanel) -> bool {
  matches!(
    docs_panel_source_from_panel(panel),
    Some(DocsPanelSource::Hover)
  )
}

fn panel_is_signature_help(panel: &UiPanel) -> bool {
  matches!(
    docs_panel_source_from_panel(panel),
    Some(DocsPanelSource::Signature)
  ) || panel.id == "signature_help"
}

fn panel_is_term_command_palette_list(panel: &UiPanel) -> bool {
  panel.id == "term_command_palette_list"
}

fn panel_is_term_command_palette_docs(panel: &UiPanel) -> bool {
  panel.id == "term_command_palette_docs"
}

fn term_command_palette_panel_rect(area: Rect, panel_width: u16, panel_height: u16) -> Rect {
  let width = panel_width.min(area.width).max(1);
  let height = panel_height.min(area.height).max(1);
  let max_y = area.y.saturating_add(area.height.saturating_sub(height));
  let y = max_y;
  Rect::new(area.x, y, width, height)
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

fn panel_box_size(panel: &UiPanel, area: Rect) -> (u16, u16) {
  let boxed = panel.style.border.is_some();
  let border: u16 = if boxed { 1 } else { 0 };
  let padding = panel.constraints.padding;
  let padding_h = padding.horizontal();
  let padding_v = padding.vertical();
  let title_height = panel.title.is_some() as u16;

  let (min_panel_width, min_panel_height) = if panel_is_completion(panel) {
    (1, 1)
  } else {
    (10, 3)
  };

  let max_content_width =
    max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
  let (child_w, child_h) = measure_node(&panel.child, max_content_width);
  let panel_width = child_w
    .saturating_add(border * 2 + padding_h)
    .min(area.width)
    .max(min_panel_width);
  let panel_height = child_h
    .saturating_add(border * 2 + padding_v + title_height)
    .min(area.height)
    .max(min_panel_height);

  apply_constraints(
    panel_width,
    panel_height,
    &panel.constraints,
    area.width,
    area.height,
  )
}

fn panel_content_rect(rect: Rect, panel: &UiPanel) -> Rect {
  let mut content = inner_rect(rect);
  if panel.title.is_some() {
    content = Rect::new(
      content.x,
      content.y.saturating_add(1),
      content.width,
      content.height.saturating_sub(1),
    );
  }
  inset_rect(content, panel.constraints.padding)
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

fn completion_docs_layout_for_panel(
  ctx: &Ctx,
  panel: &UiPanel,
  panel_rect: Rect,
  docs: &str,
  source: DocsPanelSource,
) -> Option<CompletionDocsLayout> {
  let content = panel_content_rect(panel_rect, panel);
  if content.width == 0 || content.height == 0 {
    return None;
  }

  let base_style = ui_style_colors(&panel.style).0;
  let styles = completion_docs_styles(ctx, base_style);
  let metrics = completion_docs_render_metrics(docs, &styles, content);
  let scrollbar_track = metrics.show_scrollbar.then(|| {
    Rect::new(
      content.x + content.width.saturating_sub(1),
      content.y,
      1,
      content.height,
    )
  });

  Some(CompletionDocsLayout {
    panel: panel_rect,
    content: if scrollbar_track.is_some() {
      Rect::new(
        content.x,
        content.y,
        content.width.saturating_sub(1),
        content.height,
      )
    } else {
      content
    },
    scrollbar_track,
    visible_rows: metrics.visible_rows,
    total_rows: metrics.total_rows,
    source,
  })
}

fn draw_ui_panel(
  buf: &mut Buffer,
  area: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  focus: Option<&the_lib::render::UiFocus>,
  editor_cursor: Option<(u16, u16)>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if panel.id == "file_picker" || panel.style.role.as_deref() == Some("file_picker") {
    draw_file_picker_panel(buf, area, ctx, panel, focus, cursor_out);
    return;
  }

  let boxed = panel.style.border.is_some();
  let border: u16 = if boxed { 1 } else { 0 };
  let padding_h = panel.constraints.padding.horizontal();
  let padding_v = panel.constraints.padding.vertical();
  let max_content_width =
    max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
  let (_, child_h) = measure_node(&panel.child, max_content_width);
  let (mut panel_width, panel_height) = panel_box_size(panel, area);

  if panel_is_completion(panel)
    && matches!(
      panel.intent,
      LayoutIntent::Custom(_) | LayoutIntent::Floating
    )
  {
    let rect = completion_panel_rect(area, panel_width, panel_height, editor_cursor);
    draw_panel_in_rect(
      buf,
      rect,
      ctx,
      panel,
      BorderEdge::Top,
      false,
      focus,
      cursor_out,
    );
    return;
  }

  match panel.intent.clone() {
    LayoutIntent::Bottom => {
      let mut height = if boxed {
        panel_height.min(area.height).max(4)
      } else {
        let divider = flat_panel_divider(panel);
        let mut height = child_h.saturating_add(padding_v + divider).min(area.height);
        if panel_is_statusline(panel) {
          height = height.max(1);
        } else {
          height = height.max(3);
        }
        height
      };
      if panel_is_statusline(panel) {
        height = height.min(area.height).max(1);
      } else {
        height = height.min(area.height).max(2);
      }
      let rect = Rect::new(area.x, area.y + area.height - height, area.width, height);
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        !panel_is_statusline(panel),
        focus,
        cursor_out,
      );
    },
    LayoutIntent::Top => {
      let mut height = if boxed {
        panel_height.min(area.height).max(4)
      } else {
        let divider = flat_panel_divider(panel);
        let mut height = child_h.saturating_add(padding_v + divider).min(area.height);
        if panel_is_statusline(panel) {
          height = height.max(1);
        } else {
          height = height.max(3);
        }
        height
      };
      if panel_is_statusline(panel) {
        height = height.min(area.height).max(1);
      } else {
        height = height.min(area.height).max(2);
      }
      let rect = Rect::new(area.x, area.y, area.width, height);
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Bottom,
        !panel_is_statusline(panel),
        focus,
        cursor_out,
      );
    },
    LayoutIntent::SidebarLeft => {
      panel_width = (area.width / 3).max(panel_width.min(area.width));
      let rect = Rect::new(area.x, area.y, panel_width, area.height);
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
    },
    LayoutIntent::SidebarRight => {
      panel_width = (area.width / 3).max(panel_width.min(area.width));
      let rect = Rect::new(
        area.x + area.width - panel_width,
        area.y,
        panel_width,
        area.height,
      );
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
    },
    LayoutIntent::Fullscreen => {
      let rect = area;
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
    },
    LayoutIntent::Custom(_) | LayoutIntent::Floating => {
      let (x, width) = align_horizontal(area, panel_width, panel.constraints.align.horizontal);
      let (y, height) = align_vertical(area, panel_height, panel.constraints.align.vertical);
      let rect = Rect::new(x, y, width, height);
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
    },
  }
}

fn draw_box_with_title(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let (text_style, fill_style, border_style) = ui_style_colors(&panel.style);
  draw_box(buf, rect, border_style, fill_style);

  let mut content = inner_rect(rect);
  if let Some(title) = panel.title.as_ref() {
    let mut truncated = title.clone();
    truncate_in_place(&mut truncated, content.width as usize);
    buf.set_string(content.x, content.y, truncated, text_style);
    content = Rect::new(
      content.x,
      content.y + 1,
      content.width,
      content.height.saturating_sub(1),
    );
  }

  let content = inset_rect(content, panel.constraints.padding);
  draw_ui_node(buf, content, ctx, &panel.child, focus, None, cursor_out);
}

#[derive(Clone, Copy)]
enum BorderEdge {
  Top,
  Bottom,
}

fn panel_is_statusline(panel: &UiPanel) -> bool {
  panel.id == "statusline" || panel.style.role.as_deref() == Some("statusline")
}

fn flat_panel_divider(panel: &UiPanel) -> u16 {
  if panel_is_statusline(panel) {
    return 0;
  }
  match panel.intent {
    LayoutIntent::Top | LayoutIntent::Bottom => 1,
    _ => 0,
  }
}

fn draw_flat_panel(
  buf: &mut Buffer,
  rect: Rect,
  _ctx: &Ctx,
  panel: &UiPanel,
  edge: BorderEdge,
  draw_divider: bool,
) -> Rect {
  let (_, fill_style, border_style) = ui_style_colors(&panel.style);
  fill_rect(buf, rect, fill_style);

  let content = if draw_divider {
    let line = "─".repeat(rect.width as usize);
    let border_y = match edge {
      BorderEdge::Top => rect.y,
      BorderEdge::Bottom => rect.y + rect.height.saturating_sub(1),
    };
    buf.set_string(rect.x, border_y, &line, border_style);
    match edge {
      BorderEdge::Top => {
        Rect::new(
          rect.x,
          rect.y + 1,
          rect.width,
          rect.height.saturating_sub(1),
        )
      },
      BorderEdge::Bottom => Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(1)),
    }
  } else {
    rect
  };
  inset_rect(content, panel.constraints.padding)
}

fn node_layer(node: &UiNode) -> UiLayer {
  match node {
    UiNode::Panel(panel) => panel.layer,
    UiNode::Tooltip(_) => UiLayer::Tooltip,
    _ => UiLayer::Overlay,
  }
}

fn apply_ui_viewport(ctx: &mut Ctx, ui: &UiTree, area: Rect) {
  let mut reserved_bottom: u16 = 0;
  for node in &ui.overlays {
    let UiNode::Panel(panel) = node else {
      continue;
    };
    if panel.intent != LayoutIntent::Bottom || panel.layer == UiLayer::Tooltip {
      continue;
    }
    let available = area.height.saturating_sub(reserved_bottom);
    if available == 0 {
      break;
    }
    let rect_area = Rect::new(area.x, area.y, area.width, available);
    let height = panel_height_for_area(panel, rect_area);
    reserved_bottom = reserved_bottom.saturating_add(height);
  }

  let height = area.height.saturating_sub(reserved_bottom).max(1);
  let width = area.width.max(1);
  let view = ctx.editor.view_mut();
  if view.viewport.width != width || view.viewport.height != height {
    view.viewport = the_lib::render::graphics::Rect::new(0, 0, width, height);
  }
}

fn draw_ui_tooltip(buf: &mut Buffer, area: Rect, _ctx: &Ctx, tooltip: &UiTooltip) {
  if area.width == 0 || area.height == 0 {
    return;
  }
  let (text_style, fill_style, border_style) = ui_style_colors(&tooltip.style);
  let mut text = tooltip.content.clone();
  let max_width = area.width.saturating_sub(2).max(1) as usize;
  truncate_in_place(&mut text, max_width);
  let width = (text.chars().count() as u16)
    .saturating_add(2)
    .min(area.width)
    .max(2);
  let height = 3u16.min(area.height).max(1);

  let rect = match tooltip.placement.clone() {
    LayoutIntent::Bottom => Rect::new(area.x, area.y + area.height - height, width, height),
    LayoutIntent::Top => Rect::new(area.x, area.y, width, height),
    LayoutIntent::SidebarLeft => Rect::new(area.x, area.y, width, height),
    LayoutIntent::SidebarRight => Rect::new(area.x + area.width - width, area.y, width, height),
    LayoutIntent::Fullscreen => Rect::new(area.x, area.y, width, height),
    LayoutIntent::Custom(_) | LayoutIntent::Floating => {
      Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
      )
    },
  };

  draw_box(buf, rect, border_style, fill_style);
  let inner = inner_rect(rect);
  buf.set_string(inner.x, inner.y, text, text_style);
}

fn draw_ui_status_bar(buf: &mut Buffer, rect: Rect, _ctx: &Ctx, status: &UiStatusBar) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, fill_style, _) = ui_style_colors(&status.style);
  fill_rect(buf, Rect::new(rect.x, rect.y, rect.width, 1), fill_style);

  let mut left = status.left.clone();
  if let Some(icon_token) = status.left_icon.as_deref() {
    let glyph = file_picker_icon_glyph(icon_token, false);
    left = match left.split_once("  ") {
      Some((mode, file)) if !file.is_empty() => format!("{mode}  {glyph}  {file}"),
      _ if left.is_empty() => glyph.to_string(),
      _ => format!("{glyph} {left}"),
    };
  }
  truncate_in_place(&mut left, rect.width as usize);
  let left_width = left.chars().count() as u16;

  if !status.right_segments.is_empty() {
    // Styled segments path: render each segment with its own style.
    let separator = "  ";
    let sep_len = separator.chars().count() as u16;

    // Compute total right width.
    let mut total_right: u16 = 0;
    for (i, seg) in status.right_segments.iter().enumerate() {
      total_right = total_right.saturating_add(seg.text.chars().count() as u16);
      if i > 0 {
        total_right = total_right.saturating_add(sep_len);
      }
    }

    // Collision: if left + right >= width, cap left.
    let left_width = if left_width.saturating_add(total_right) >= rect.width {
      let available = rect.width.saturating_sub(total_right.saturating_add(1));
      truncate_in_place(&mut left, available as usize);
      left.chars().count() as u16
    } else {
      left_width
    };

    buf.set_string(rect.x, rect.y, &left, text_style);

    // Render segments right-to-left.
    let mut rx = rect.x.saturating_add(rect.width);
    for (i, seg) in status.right_segments.iter().enumerate().rev() {
      let seg_style = styled_span_style(seg, text_style);
      let text_w = seg.text.chars().count() as u16;
      rx = rx.saturating_sub(text_w);
      if rx >= rect.x.saturating_add(left_width) {
        buf.set_string(rx, rect.y, &seg.text, seg_style);
      }
      if i > 0 {
        rx = rx.saturating_sub(sep_len);
        if rx >= rect.x.saturating_add(left_width) {
          buf.set_string(rx, rect.y, separator, text_style);
        }
      }
    }
  } else {
    // Fallback: plain text path (backward compat).
    let mut center = status.center.clone();
    let mut right = status.right.clone();
    truncate_in_place(&mut right, rect.width as usize);
    truncate_in_place(&mut center, rect.width as usize);

    let mut left_width = left_width;
    let mut right_width = right.chars().count() as u16;
    if left_width.saturating_add(right_width) >= rect.width {
      let available_right = rect.width.saturating_sub(left_width.saturating_add(1));
      truncate_in_place(&mut right, available_right as usize);
      right_width = right.chars().count() as u16;
    }
    if left_width.saturating_add(right_width) >= rect.width {
      let available_left = rect.width.saturating_sub(right_width.saturating_add(1));
      truncate_in_place(&mut left, available_left as usize);
      left_width = left.chars().count() as u16;
    }

    buf.set_string(rect.x, rect.y, &left, text_style);
    if !right.is_empty() {
      let rx = rect.x + rect.width.saturating_sub(right_width);
      buf.set_string(rx, rect.y, right, text_style);
    }
    if !center.is_empty() {
      let center_start = rect.x + left_width.saturating_add(1);
      let center_end = rect
        .x
        .saturating_add(rect.width)
        .saturating_sub(right_width.saturating_add(1));
      if center_end > center_start {
        let center_width = center_end.saturating_sub(center_start);
        truncate_in_place(&mut center, center_width as usize);
        let center_text_width = center.chars().count() as u16;
        let cx = center_start + center_width.saturating_sub(center_text_width) / 2;
        buf.set_string(cx, rect.y, center, text_style);
      }
    }
  }
}

fn styled_span_style(span: &UiStyledSpan, base: Style) -> Style {
  match &span.style {
    None => base,
    Some(s) => {
      let mut style = base;
      if let Some(ref fg) = s.fg {
        if let Some(color) = resolve_ui_color(fg) {
          style = style.fg(color);
        }
      }
      apply_ui_emphasis(style, s.emphasis)
    },
  }
}

fn panel_height_for_area(panel: &UiPanel, area: Rect) -> u16 {
  let boxed = panel.style.border.is_some();
  if boxed {
    let border: u16 = 1;
    let padding = panel.constraints.padding;
    let padding_h = padding.horizontal();
    let padding_v = padding.vertical();
    let title_height = panel.title.is_some() as u16;
    let max_content_width =
      max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
    let (_, child_h) = measure_node(&panel.child, max_content_width);
    child_h
      .saturating_add(border * 2 + padding_v + title_height)
      .min(area.height)
      .max(4)
  } else {
    let padding_v = panel.constraints.padding.vertical();
    let max_content_width = max_content_width_for_intent(panel.intent.clone(), area, 0, 0);
    let (_, child_h) = measure_node(&panel.child, max_content_width);
    let divider = flat_panel_divider(panel);
    let height = child_h.saturating_add(divider + padding_v).min(area.height);
    if panel_is_statusline(panel) {
      height.max(1)
    } else {
      height.max(2)
    }
  }
}

fn draw_panel_in_rect(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  edge: BorderEdge,
  draw_divider: bool,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  if panel.style.border.is_some() {
    draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
  } else {
    let content = draw_flat_panel(buf, rect, ctx, panel, edge, draw_divider);
    draw_ui_node(buf, content, ctx, &panel.child, focus, None, cursor_out);
  }
}

fn draw_ui_overlays(
  buf: &mut Buffer,
  area: Rect,
  ctx: &mut Ctx,
  ui: &UiTree,
  editor_cursor: Option<(u16, u16)>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  ctx.completion_docs_layout = None;
  let mut top_offset: u16 = 0;
  let mut bottom_offset: u16 = 0;
  let focus = ui.focus.as_ref();
  let layers = [
    the_lib::render::UiLayer::Background,
    the_lib::render::UiLayer::Overlay,
    the_lib::render::UiLayer::Tooltip,
  ];
  for layer in layers {
    let layer_nodes: Vec<&UiNode> = ui
      .overlays
      .iter()
      .filter(|node| node_layer(node) == layer)
      .collect();
    let mut index = 0usize;
    while index < layer_nodes.len() {
      let node = layer_nodes[index];
      match node {
        UiNode::Panel(panel) => {
          let term_command_docs_pair = layer_nodes.get(index + 1).and_then(|next| {
            match *next {
              UiNode::Panel(next_panel) if panel_is_term_command_palette_docs(next_panel) => {
                Some(next_panel)
              },
              _ => None,
            }
          });
          let completion_docs_pair = layer_nodes.get(index + 1).and_then(|next| {
            match *next {
              UiNode::Panel(next_panel) if panel_is_completion_docs(next_panel) => Some(next_panel),
              _ => None,
            }
          });
          if panel_is_term_command_palette_list(panel) {
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height > 0 {
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let (list_width, list_height) = panel_box_size(panel, overlay_area);
              let list_rect =
                term_command_palette_panel_rect(overlay_area, list_width, list_height);
              draw_panel_in_rect(
                buf,
                list_rect,
                ctx,
                panel,
                BorderEdge::Top,
                false,
                focus,
                cursor_out,
              );

              if let Some(docs_panel) = term_command_docs_pair {
                let (docs_width, docs_height) = panel_box_size(docs_panel, overlay_area);
                let docs_rect =
                  completion_docs_panel_rect(overlay_area, docs_width, docs_height, list_rect);
                if let Some(docs_rect) = docs_rect {
                  draw_panel_in_rect(
                    buf,
                    docs_rect,
                    ctx,
                    docs_panel,
                    BorderEdge::Top,
                    false,
                    focus,
                    cursor_out,
                  );
                }
              }
            }
            index += if term_command_docs_pair.is_some() {
              2
            } else {
              1
            };
            continue;
          }
          if panel_is_completion(panel)
            && matches!(
              panel.intent,
              LayoutIntent::Custom(_) | LayoutIntent::Floating
            )
            && completion_docs_pair.is_some()
          {
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height > 0 {
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let (completion_width, completion_height) = panel_box_size(panel, overlay_area);
              let completion_rect = completion_panel_rect(
                overlay_area,
                completion_width,
                completion_height,
                editor_cursor,
              );
              draw_panel_in_rect(
                buf,
                completion_rect,
                ctx,
                panel,
                BorderEdge::Top,
                false,
                focus,
                cursor_out,
              );

              if let Some(docs_panel) = completion_docs_pair {
                let (docs_width, docs_height) = panel_box_size(docs_panel, overlay_area);
                let docs_rect = completion_docs_panel_rect(
                  overlay_area,
                  docs_width,
                  docs_height,
                  completion_rect,
                );
                if let Some(docs_rect) = docs_rect {
                  draw_panel_in_rect(
                    buf,
                    docs_rect,
                    ctx,
                    docs_panel,
                    BorderEdge::Top,
                    false,
                    focus,
                    cursor_out,
                  );
                  if let (Some(docs), Some(source)) = (
                    selected_completion_docs_text(ctx),
                    docs_panel_source_from_panel(docs_panel),
                  ) {
                    ctx.completion_docs_layout =
                      completion_docs_layout_for_panel(ctx, docs_panel, docs_rect, docs, source);
                  }
                }
              }
            }
            index += 2;
            continue;
          }
          if panel_is_hover(panel)
            && matches!(
              panel.intent,
              LayoutIntent::Custom(_) | LayoutIntent::Floating
            )
          {
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height > 0 {
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let (hover_width, hover_height) = panel_box_size(panel, overlay_area);
              let hover_rect =
                completion_panel_rect(overlay_area, hover_width, hover_height, editor_cursor);
              draw_panel_in_rect(
                buf,
                hover_rect,
                ctx,
                panel,
                BorderEdge::Top,
                false,
                focus,
                cursor_out,
              );
              if let (Some(docs), Some(source)) = (
                ctx
                  .hover_docs
                  .as_deref()
                  .map(str::trim)
                  .filter(|text| !text.is_empty()),
                docs_panel_source_from_panel(panel),
              ) {
                ctx.completion_docs_layout =
                  completion_docs_layout_for_panel(ctx, panel, hover_rect, docs, source);
              }
            }
            index += 1;
            continue;
          }
          if panel_is_signature_help(panel)
            && matches!(
              panel.intent,
              LayoutIntent::Custom(_) | LayoutIntent::Floating
            )
          {
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height > 0 {
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let (popup_width, popup_height) = panel_box_size(panel, overlay_area);
              let popup_rect =
                signature_help_panel_rect(overlay_area, popup_width, popup_height, editor_cursor);
              draw_panel_in_rect(
                buf,
                popup_rect,
                ctx,
                panel,
                BorderEdge::Top,
                false,
                focus,
                cursor_out,
              );
              if let (Some(text), Some(source)) = (
                signature_help_panel_text(ctx),
                docs_panel_source_from_panel(panel),
              ) {
                ctx.completion_docs_layout =
                  completion_docs_layout_for_panel(ctx, panel, popup_rect, &text, source);
              }
            }
            index += 1;
            continue;
          }

          match panel.intent.clone() {
            LayoutIntent::Bottom => {
              if matches!(layer, the_lib::render::UiLayer::Tooltip) {
                draw_ui_panel(buf, area, ctx, panel, focus, editor_cursor, cursor_out);
                index += 1;
                continue;
              }
              let available_height = area.height.saturating_sub(top_offset + bottom_offset);
              if available_height == 0 {
                index += 1;
                continue;
              }
              let rect_area = Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let panel_height = panel_height_for_area(panel, rect_area);
              let rect = Rect::new(
                area.x,
                area.y + area.height - bottom_offset - panel_height,
                area.width,
                panel_height,
              );
              bottom_offset = bottom_offset.saturating_add(panel_height);
              draw_panel_in_rect(
                buf,
                rect,
                ctx,
                panel,
                BorderEdge::Top,
                !panel_is_statusline(panel),
                focus,
                cursor_out,
              );
            },
            LayoutIntent::Top => {
              if matches!(layer, the_lib::render::UiLayer::Tooltip) {
                draw_ui_panel(buf, area, ctx, panel, focus, editor_cursor, cursor_out);
                index += 1;
                continue;
              }
              let available_height = area.height.saturating_sub(top_offset + bottom_offset);
              if available_height == 0 {
                index += 1;
                continue;
              }
              let rect_area = Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let panel_height = panel_height_for_area(panel, rect_area);
              let rect = Rect::new(area.x, area.y + top_offset, area.width, panel_height);
              top_offset = top_offset.saturating_add(panel_height);
              draw_panel_in_rect(
                buf,
                rect,
                ctx,
                panel,
                BorderEdge::Bottom,
                !panel_is_statusline(panel),
                focus,
                cursor_out,
              );
            },
            _ => {
              let available_height = area.height.saturating_sub(top_offset + bottom_offset);
              if available_height == 0 {
                continue;
              }
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              draw_ui_panel(
                buf,
                overlay_area,
                ctx,
                panel,
                focus,
                editor_cursor,
                cursor_out,
              )
            },
          }
        },
        _ => draw_ui_node(buf, area, ctx, node, focus, editor_cursor, cursor_out),
      }
      index += 1;
    }
  }
  if ctx.completion_docs_layout.is_none() {
    ctx.completion_docs_drag = None;
  }
}

fn is_search_prompt_overlay(node: &UiNode) -> bool {
  matches!(node, UiNode::Panel(panel) if panel.id.starts_with("search_prompt"))
}

fn is_command_palette_overlay(node: &UiNode) -> bool {
  matches!(node, UiNode::Panel(panel) if panel.id.starts_with("command_palette"))
}

fn is_hover_overlay(node: &UiNode) -> bool {
  matches!(
    node,
    UiNode::Panel(panel) if matches!(docs_panel_source_from_panel(panel), Some(DocsPanelSource::Hover))
  )
}

fn status_bar_from_overlay_mut(node: &mut UiNode) -> Option<&mut UiStatusBar> {
  match node {
    UiNode::Panel(panel) if panel.id == "statusline" => {
      if let UiNode::StatusBar(status) = panel.child.as_mut() {
        Some(status)
      } else {
        None
      }
    },
    UiNode::StatusBar(status) if status.id.as_deref() == Some("statusline") => Some(status),
    _ => None,
  }
}

fn command_palette_prompt_query_and_cursor(ctx: &Ctx) -> (&str, usize) {
  let raw = ctx.command_prompt.input.as_str();
  if let Some(stripped) = raw.strip_prefix(':') {
    (stripped, ctx.command_prompt.cursor.saturating_sub(1))
  } else {
    (raw, ctx.command_prompt.cursor)
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

fn search_statusline_text(
  kind: the_default::SearchPromptKind,
  query: &str,
  cursor: usize,
) -> String {
  let mut cursor = cursor.min(query.len());
  while cursor > 0 && !query.is_char_boundary(cursor) {
    cursor -= 1;
  }
  if !query.is_char_boundary(cursor) {
    cursor = 0;
  }
  let (before, after) = query.split_at(cursor);
  let prefix = match kind {
    the_default::SearchPromptKind::Search => "FIND",
    the_default::SearchPromptKind::SelectRegex => "SELECT",
  };
  format!("{prefix} {before}█{after}")
}

fn build_term_command_palette_list_overlay(ctx: &Ctx) -> Option<UiNode> {
  let state = &ctx.command_palette;
  if !state.is_open {
    return None;
  }

  let (query, _) = command_palette_prompt_query_and_cursor(ctx);
  let mut filtered_state = state.clone();
  filtered_state.query = query.to_string();
  let (filtered, selected) = term_command_palette_filtered_selection(&filtered_state)?;
  const MAX_VISIBLE_ITEMS: usize = 10;
  let items: Vec<UiListItem> = filtered
    .iter()
    .filter_map(|index| state.items.get(*index))
    .map(|item| {
      UiListItem {
        title:         item.title.clone(),
        subtitle:      item.subtitle.clone().or_else(|| item.shortcut.clone()),
        description:   None,
        shortcut:      None,
        badge:         item.badge.clone(),
        leading_icon:  item.leading_icon.clone(),
        leading_color: item.leading_color.map(UiColor::Value),
        symbols:       item.symbols.clone(),
        match_indices: None,
        emphasis:      item.emphasis,
        action:        None,
      }
    })
    .collect();
  if items.is_empty() {
    return None;
  }

  let mut list = UiList::new("command_palette_list", items);
  list.fill_width = false;
  list.selected = selected;
  list.scroll = 0;
  list.max_visible = Some(MAX_VISIBLE_ITEMS);
  list.style = list.style.with_role("completion");
  list.style.accent = Some(UiColor::Token(UiColorToken::SelectedBg));
  list.style.border = Some(UiColor::Token(UiColorToken::SelectedText));

  let mut container = UiContainer::column("term_command_palette_container", 0, vec![UiNode::List(
    list,
  )]);
  container.style = container.style.with_role("completion");

  let mut panel = UiPanel::new(
    "term_command_palette_list",
    LayoutIntent::Custom("term_command_palette_list".to_string()),
    UiNode::Container(container),
  );
  panel.style = panel.style.with_role("completion");
  panel.style.border = None;
  panel.layer = UiLayer::Overlay;
  panel.constraints = UiConstraints {
    min_width: None,
    max_width: Some(64),
    min_height: Some(1),
    max_height: Some((MAX_VISIBLE_ITEMS as u16).saturating_add(4)),
    padding: UiInsets {
      left:   1,
      right:  1,
      top:    0,
      bottom: 0,
    },
    align: UiAlignPair {
      horizontal: UiAlign::Start,
      vertical:   UiAlign::End,
    },
    ..UiConstraints::default()
  };

  Some(UiNode::Panel(panel))
}

fn build_term_command_palette_docs_overlay(ctx: &Ctx) -> Option<UiNode> {
  let state = &ctx.command_palette;
  if !state.is_open {
    return None;
  }
  let (query, _) = command_palette_prompt_query_and_cursor(ctx);
  let mut filtered_state = state.clone();
  filtered_state.query = query.to_string();
  let (filtered, selected_filtered) = term_command_palette_filtered_selection(&filtered_state)?;
  let selected_index = *filtered.get(selected_filtered.unwrap_or(0))?;
  let item = state.items.get(selected_index)?;

  let mut docs = String::new();
  if let Some(description) = item.description.as_deref().map(str::trim)
    && !description.is_empty()
  {
    docs.push_str(description);
  }
  if !item.aliases.is_empty() {
    if !docs.is_empty() {
      docs.push_str("\n\n");
    }
    docs.push_str("aliases: ");
    docs.push_str(item.aliases.join(", ").as_str());
  }
  if docs.is_empty() {
    return None;
  }

  Some(build_docs_panel(
    DocsPanelConfig::command_palette_docs(
      "term_command_palette_docs",
      "term_command_palette_docs_text",
      LayoutIntent::Custom("term_command_palette_docs".to_string()),
    ),
    docs,
  ))
}

fn build_lsp_hover_overlay(ctx: &Ctx) -> Option<UiNode> {
  let docs = ctx
    .hover_docs
    .as_deref()
    .map(str::trim)
    .filter(|text| !text.is_empty())?;
  let config = DocsPanelConfig::hover_docs(
    "lsp_hover",
    "lsp_hover_text",
    LayoutIntent::Custom("lsp_hover".to_string()),
  );
  Some(build_docs_panel(config, docs.to_string()))
}

fn adapt_ui_tree_for_term(ctx: &Ctx, ui: &mut UiTree) {
  ui.overlays.retain(|node| !is_hover_overlay(node));
  if ctx.command_palette.is_open {
    ui.overlays.retain(|node| !is_command_palette_overlay(node));
    let (query, cursor) = command_palette_prompt_query_and_cursor(ctx);
    if let Some(status) = ui.overlays.iter_mut().find_map(status_bar_from_overlay_mut) {
      status.left = command_palette_statusline_text(query, cursor);
      status.left_icon = None;
    }
    if let Some(list_overlay) = build_term_command_palette_list_overlay(ctx) {
      ui.overlays.push(list_overlay);
      if let Some(docs_overlay) = build_term_command_palette_docs_overlay(ctx) {
        ui.overlays.push(docs_overlay);
      }
    }
    return;
  }

  if !ctx.search_prompt.active {
    if ctx.completion_menu.active {
      return;
    }
    if let Some(hover_overlay) = build_lsp_hover_overlay(ctx) {
      ui.overlays.push(hover_overlay);
    }
    return;
  }

  ui.overlays.retain(|node| !is_search_prompt_overlay(node));
  if ui
    .focus
    .as_ref()
    .is_some_and(|focus| focus.id.starts_with("search_prompt"))
  {
    ui.focus = None;
  }

  if let Some(status) = ui.overlays.iter_mut().find_map(status_bar_from_overlay_mut) {
    status.left = search_statusline_text(
      ctx.search_prompt.kind,
      ctx.search_prompt.query.as_str(),
      ctx.search_prompt.cursor,
    );
    status.left_icon = None;
  }

  if let Some(hover_overlay) = build_lsp_hover_overlay(ctx) {
    ui.overlays.push(hover_overlay);
  }
}

pub fn build_render_plan(ctx: &mut Ctx) -> RenderPlan {
  let styles = render_styles_from_theme(&ctx.ui_theme);
  build_render_plan_with_styles(ctx, styles)
}

pub fn build_render_plan_with_styles(ctx: &mut Ctx, styles: RenderStyles) -> RenderPlan {
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
  ctx.inline_diagnostic_lines.clear();
  let inline_diagnostic_render_data: SharedInlineDiagnosticsRenderData =
    Rc::new(RefCell::new(Default::default()));
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
  let diagnostics_for_underlines = ctx
    .lsp_document
    .as_ref()
    .filter(|state| state.opened)
    .and_then(|state| ctx.diagnostics.document(&state.uri))
    .map(|doc| doc.diagnostics.clone())
    .unwrap_or_default();
  let lsp_diag_count = diagnostics_for_underlines.len();
  let mut inline_enable_cursor_line = false;
  let mut inline_config_snapshot: Option<InlineDiagnosticsConfig> = None;
  if !inline_diagnostics.is_empty() {
    inline_enable_cursor_line = ctx.mode() != Mode::Insert;
    let inline_config = inline_diagnostics_config()
      .prepare(text_fmt.viewport_width.max(1), inline_enable_cursor_line);
    inline_config_snapshot = Some(inline_config.clone());
    if !inline_config.disabled() {
      let cursor_char_idx = primary_cursor_char_idx(ctx).unwrap_or(0);
      let cursor_line_idx = primary_cursor_line_idx(ctx);
      let _ = annotations.add_line_annotation(Box::new(InlineDiagnosticsLineAnnotation::new(
        inline_diagnostics,
        cursor_char_idx,
        cursor_line_idx,
        text_fmt.viewport_width.max(1),
        view.scroll.col,
        inline_config,
        inline_diagnostic_render_data.clone(),
      )));
    }
  }

  let allow_cache_refresh = ctx.syntax_highlight_refresh_allowed();

  // Build the render plan (with or without syntax highlighting).
  let (mut plan, diagnostic_underlines) = {
    let (doc, render_cache) = ctx.editor.document_and_cache();
    let plan = if let (Some(loader), Some(syntax)) = (&ctx.loader, doc.syntax()) {
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
        view,
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
        view,
        text_fmt,
        &ctx.gutter_config,
        &mut annotations,
        &mut highlights,
        render_cache,
        styles,
      )
    };
    let diagnostic_underlines = diagnostic_underlines_for_document(
      doc.text(),
      &diagnostics_for_underlines,
      &plan,
      text_fmt,
      &mut annotations,
    );
    (plan, diagnostic_underlines)
  };

  ctx.diagnostic_underlines = diagnostic_underlines;
  let inline_render_trace = inline_diagnostic_render_data.borrow().last_trace.clone();
  ctx.inline_diagnostic_lines = inline_diagnostic_render_data.borrow().lines.clone();
  drop(annotations);

  let should_trace_inline_diagnostics = inline_diagnostics_trace_enabled()
    || (lsp_diag_count > 0 && inline_diag_count > 0 && ctx.inline_diagnostic_lines.is_empty());
  if should_trace_inline_diagnostics {
    let cursor_char_idx = primary_cursor_char_idx(ctx);
    let cursor_line_idx = primary_cursor_line_idx(ctx);
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

/// Render the current document state to the terminal.
pub fn render(f: &mut Frame, ctx: &mut Ctx) {
  let area = f.size();
  sync_file_picker_viewport(ctx, area);
  let mut ui = ui_tree(ctx);
  adapt_ui_tree_for_term(ctx, &mut ui);
  resolve_ui_tree(&mut ui, &ctx.ui_theme);
  apply_ui_viewport(ctx, &ui, f.size());
  ensure_cursor_visible(ctx);
  let plan = render_plan(ctx);

  f.render_widget(Clear, area);

  let _ui_cursor = {
    let buf = f.buffer_mut();
    let mut cursor_out = None;
    let content_x = area.x.saturating_add(plan.content_offset_x);
    let editor_cursor = plan.cursors.first().map(|cursor| {
      (
        content_x + cursor.pos.col as u16,
        area.y + cursor.pos.row as u16,
      )
    });
    let base_text_style = lib_style_to_ratatui(ctx.ui_theme.try_get("ui.text").unwrap_or_default());
    if let Some(bg) = ctx
      .ui_theme
      .try_get("ui.background")
      .and_then(|style| style.bg)
    {
      fill_rect(buf, area, Style::default().bg(lib_color_to_ratatui(bg)));
    }

    if plan.content_offset_x > 0 {
      for line in &plan.gutter_lines {
        let y = area.y + line.row;
        if y >= area.y + area.height {
          continue;
        }
        for span in &line.spans {
          let x = area.x + span.col;
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
        area.y + selection.rect.y,
        selection.rect.width,
        selection.rect.height,
      );
      fill_rect(buf, rect, lib_style_to_ratatui(selection.style));
    }

    // Draw text lines with syntax colors
    for line in &plan.lines {
      let y = area.y + line.row;
      if y >= area.y + area.height {
        continue;
      }
      for span in &line.spans {
        let x = content_x + span.col;
        if x >= area.x + area.width {
          continue;
        }
        let style = span
          .highlight
          .map(|highlight| {
            base_text_style.patch(lib_style_to_ratatui(ctx.ui_theme.highlight(highlight)))
          })
          .unwrap_or(base_text_style);
        buf.set_string(x, y, span.text.as_str(), style);
      }
    }

    draw_diagnostic_underlines(buf, area, content_x, ctx);
    draw_end_of_line_diagnostics(buf, area, content_x, &plan, ctx);
    draw_inline_diagnostic_lines(buf, area, content_x, &plan, ctx);

    // Draw cursors
    for cursor in &plan.cursors {
      let x = content_x + cursor.pos.col as u16;
      let y = area.y + cursor.pos.row as u16;
      if x < area.x + area.width && y < area.y + area.height {
        let style = lib_style_to_ratatui(cursor.style);
        let cell = buf.get_mut(x, y);
        let merged = cell.style().patch(style);
        cell.set_style(merged);
      }
    }

    // Draw UI root and overlays.
    draw_ui_node(
      buf,
      area,
      ctx,
      &ui.root,
      ui.focus.as_ref(),
      editor_cursor,
      &mut cursor_out,
    );
    draw_ui_overlays(buf, area, ctx, &ui, editor_cursor, &mut cursor_out);
    cursor_out
  };
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
  let doc = ctx.editor.document();
  let text = doc.text();
  let max = text.len_chars();

  // Get primary cursor position
  let Some(range) = doc.selection().ranges().get(0).copied() else {
    return;
  };
  let clamped = Range::new(range.anchor.min(max), range.head.min(max));
  let cursor_pos = clamped.cursor(text.slice(..));
  let cursor_line = text.char_to_line(cursor_pos);
  let cursor_col = cursor_pos - text.line_to_char(cursor_line);

  let view = ctx.editor.view();
  let viewport_height = view.viewport.height as usize;
  let gutter_width = gutter_width_for_document(doc, view.viewport.width, &ctx.gutter_config);
  let viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1) as usize;

  if ctx.text_format.soft_wrap {
    let mut changed = false;
    let mut new_scroll = view.scroll;

    if let Some(new_row) = the_lib::view::scroll_row_to_keep_visible(
      cursor_line,
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
      ctx.editor.view_mut().scroll = new_scroll;
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
    ctx.editor.view_mut().scroll = new_scroll;
  }
}

#[cfg(test)]
mod tests {
  use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
  };
  use ropey::Rope;
  use the_default::{
    CommandPaletteItem,
    CommandPaletteState,
  };
  use the_lib::{
    diagnostics::{
      Diagnostic,
      DiagnosticPosition,
      DiagnosticRange,
      DiagnosticSeverity,
    },
    render::{
      InlineDiagnosticFilter,
      LayoutIntent,
      UiConstraints,
      UiContainer,
      UiInsets,
      UiList,
      UiListItem,
      UiNode,
      UiPanel,
      UiText,
    },
  };

  use super::{
    CompletionDocsStyles,
    DocsBlock,
    StyledTextRun,
    build_lsp_hover_overlay,
    completion_docs_panel_rect,
    completion_docs_rows,
    completion_panel_rect,
    draw_ui_list,
    draw_ui_text,
    inline_diagnostics_from_document,
    language_filename_hints,
    max_content_width_for_intent,
    panel_box_size,
    parse_inline_diagnostic_filter,
    parse_markdown_blocks,
    select_end_of_line_diagnostic,
    term_command_palette_filtered_selection,
  };
  use crate::Ctx;

  fn flatten_rows(rows: &[Vec<StyledTextRun>]) -> Vec<String> {
    rows
      .iter()
      .map(|row| {
        row
          .iter()
          .map(|run| run.text.as_str())
          .collect::<Vec<_>>()
          .join("")
      })
      .collect()
  }

  fn buffer_row_text(buf: &Buffer, rect: Rect, row: u16) -> String {
    (rect.x..rect.x + rect.width)
      .map(|x| buf.get(x, row).symbol())
      .collect::<String>()
  }

  #[test]
  fn completion_panel_rect_places_below_cursor_when_space_exists() {
    let area = Rect::new(0, 0, 100, 30);
    let rect = completion_panel_rect(area, 32, 8, Some((40, 10)));
    assert_eq!(rect.y, 11);
    assert_eq!(rect.width, 32);
    assert_eq!(rect.height, 8);
  }

  #[test]
  fn completion_panel_rect_flips_above_when_below_is_tight() {
    let area = Rect::new(0, 0, 80, 12);
    let rect = completion_panel_rect(area, 30, 8, Some((20, 10)));
    assert!(rect.y < 10);
    assert_eq!(rect.height, 8);
  }

  #[test]
  fn completion_panel_rect_clamps_to_viewport_bounds() {
    let area = Rect::new(5, 3, 20, 10);
    let rect = completion_panel_rect(area, 18, 9, Some((500, 500)));
    assert!(rect.x >= area.x);
    assert!(rect.y >= area.y);
    assert!(rect.x + rect.width <= area.x + area.width);
    assert!(rect.y + rect.height <= area.y + area.height);
  }

  #[test]
  fn parse_inline_diagnostic_filter_parses_expected_values() {
    assert_eq!(
      parse_inline_diagnostic_filter("warning"),
      Some(the_lib::render::InlineDiagnosticFilter::Enable(
        DiagnosticSeverity::Warning
      ))
    );
    assert_eq!(
      parse_inline_diagnostic_filter("disable"),
      Some(the_lib::render::InlineDiagnosticFilter::Disable)
    );
    assert_eq!(parse_inline_diagnostic_filter("unknown"), None);
  }

  #[test]
  fn inline_diagnostics_from_document_maps_utf16_positions() {
    let text = Rope::from("a🧪b\\n");
    let diagnostics = vec![Diagnostic {
      range:    DiagnosticRange {
        start: DiagnosticPosition {
          line:      0,
          character: 3,
        },
        end:   DiagnosticPosition {
          line:      0,
          character: 4,
        },
      },
      severity: Some(DiagnosticSeverity::Warning),
      code:     None,
      source:   None,
      message:  "mapped".to_string(),
    }];

    let mapped = inline_diagnostics_from_document(&text, &diagnostics);
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].start_char_idx, 2);
  }

  fn diagnostic_with_severity(severity: DiagnosticSeverity) -> Diagnostic {
    Diagnostic {
      range:    DiagnosticRange {
        start: DiagnosticPosition {
          line:      0,
          character: 0,
        },
        end:   DiagnosticPosition {
          line:      0,
          character: 1,
        },
      },
      severity: Some(severity),
      code:     None,
      source:   None,
      message:  format!("{severity:?}"),
    }
  }

  #[test]
  fn end_of_line_diagnostic_selects_hidden_diagnostic_when_inline_filters_it_out() {
    let diagnostics = vec![
      diagnostic_with_severity(DiagnosticSeverity::Error),
      diagnostic_with_severity(DiagnosticSeverity::Hint),
    ];

    let selected = select_end_of_line_diagnostic(
      diagnostics.iter(),
      InlineDiagnosticFilter::Enable(DiagnosticSeverity::Warning),
      InlineDiagnosticFilter::Enable(DiagnosticSeverity::Hint),
    )
    .expect("expected hidden diagnostic");

    assert_eq!(selected.severity, Some(DiagnosticSeverity::Hint));
  }

  #[test]
  fn end_of_line_diagnostic_uses_highest_severity_when_inline_disabled() {
    let diagnostics = vec![
      diagnostic_with_severity(DiagnosticSeverity::Information),
      diagnostic_with_severity(DiagnosticSeverity::Error),
      diagnostic_with_severity(DiagnosticSeverity::Warning),
    ];

    let selected = select_end_of_line_diagnostic(
      diagnostics.iter(),
      InlineDiagnosticFilter::Disable,
      InlineDiagnosticFilter::Enable(DiagnosticSeverity::Hint),
    )
    .expect("expected diagnostic");

    assert_eq!(selected.severity, Some(DiagnosticSeverity::Error));
  }

  #[test]
  fn completion_docs_panel_rect_prefers_right_side() {
    let area = Rect::new(0, 0, 100, 30);
    let completion_rect = Rect::new(20, 9, 30, 8);
    let docs_rect = completion_docs_panel_rect(area, 24, 10, completion_rect).expect("docs rect");
    assert_eq!(docs_rect.x, 51);
    assert_eq!(docs_rect.y, completion_rect.y);
  }

  #[test]
  fn completion_docs_panel_rect_flips_left_when_right_is_tight() {
    let area = Rect::new(0, 0, 70, 20);
    let completion_rect = Rect::new(45, 4, 24, 8);
    let docs_rect = completion_docs_panel_rect(area, 20, 8, completion_rect).expect("docs rect");
    assert_eq!(docs_rect.x, 24);
    assert_eq!(docs_rect.y, completion_rect.y);
  }

  #[test]
  fn completion_docs_panel_rect_hides_when_viewport_is_narrow() {
    let area = Rect::new(0, 0, 72, 22);
    let completion_rect = Rect::new(4, 5, 46, 10);
    let docs_rect = completion_docs_panel_rect(area, 40, 9, completion_rect);
    assert!(docs_rect.is_none());
  }

  #[test]
  fn completion_docs_panel_rect_hides_when_side_space_is_unavailable() {
    let area = Rect::new(0, 0, 80, 24);
    let completion_rect = Rect::new(2, 6, 76, 10);
    let docs_rect = completion_docs_panel_rect(area, 36, 9, completion_rect);
    assert!(docs_rect.is_none());
  }

  #[test]
  fn lsp_hover_overlay_builds_completion_docs_panel() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs = Some("```rust\nfn hover() {}\n```\n\nhover docs".to_string());

    let Some(UiNode::Panel(panel)) = build_lsp_hover_overlay(&ctx) else {
      panic!("expected hover panel overlay");
    };
    assert_eq!(panel.id, "lsp_hover");
    assert_eq!(panel.style.role.as_deref(), Some("completion_docs"));
    assert_eq!(panel.source.as_deref(), Some("hover"));
    assert_eq!(panel.layer, the_lib::render::UiLayer::Tooltip);
  }

  #[test]
  fn hover_docs_text_uses_hover_scroll_source_without_canonical_text_id() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs_scroll = 1;
    ctx.completion_menu.docs_scroll = 0;

    let mut text = UiText::new("shared_docs_text", "a0\nb1\nc2");
    text.source = Some("hover".to_string());
    text.style = text.style.with_role("completion_docs");
    text.clip = false;

    let rect = Rect::new(0, 0, 8, 1);
    let mut buf = Buffer::empty(rect);
    draw_ui_text(&mut buf, rect, &ctx, &text);
    assert_eq!(buf.get(0, 0).symbol(), "b");
  }

  #[test]
  fn lsp_hover_overlay_omits_empty_docs() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.hover_docs = Some("   ".to_string());
    assert!(build_lsp_hover_overlay(&ctx).is_none());
  }

  #[test]
  fn term_command_palette_selection_stays_empty_without_explicit_selection() {
    let state = CommandPaletteState {
      is_open:     true,
      query:       String::new(),
      selected:    None,
      items:       vec![
        CommandPaletteItem::new("help"),
        CommandPaletteItem::new("quit"),
      ],
      max_results: 10,
    };

    let (filtered, selected) =
      term_command_palette_filtered_selection(&state).expect("filtered selection");
    assert_eq!(filtered, vec![0, 1]);
    assert_eq!(selected, None);
  }

  #[test]
  fn completion_list_scrollbar_preserves_selected_row_fill() {
    let items = (0..10)
      .map(|idx| UiListItem::new(format!("item-{idx}")))
      .collect();
    let mut list = UiList::new("completion_list", items);
    list.style = list.style.with_role("completion");
    list.selected = Some(0);
    list.scroll = 0;
    list.max_visible = Some(3);

    let rect = Rect::new(0, 0, 24, 3);
    let mut buf = Buffer::empty(rect);
    let mut cursor = None;
    draw_ui_list(&mut buf, rect, &list, &mut cursor);

    let track_x = rect.x + rect.width - 1;
    let selected_row_cell = buf.get(track_x, rect.y);
    assert_eq!(selected_row_cell.symbol(), " ");

    let next_row_cell = buf.get(track_x, rect.y + 1);
    assert_eq!(next_row_cell.symbol(), "█");
  }

  #[test]
  fn completion_list_keeps_function_label_visible_with_long_signature_detail() {
    let mut item = UiListItem::new("install_test_watch_state");
    item.leading_icon = Some("f".to_string());
    item.subtitle =
      Some("fn(&mut App, EditorId, &Path) -> Sender<Vec<PathEvent, Global>>".to_string());
    let mut list = UiList::new("completion_list", vec![item]);
    list.style = list.style.with_role("completion");
    list.selected = Some(0);

    let rect = Rect::new(0, 0, 64, 1);
    let mut buf = Buffer::empty(rect);
    let mut cursor = None;
    draw_ui_list(&mut buf, rect, &list, &mut cursor);

    let row = buffer_row_text(&buf, rect, rect.y);
    assert!(
      row.contains("install_test_watch_state"),
      "completion row should preserve the label text, got: {row:?}"
    );
  }

  #[test]
  fn completion_panel_size_uses_fixed_viewport_width_and_single_row_height() {
    let mut list = UiList::new("completion_list", vec![UiListItem::new("std")]);
    list.style = list.style.with_role("completion");

    let mut container = UiContainer::column("completion_container", 0, vec![UiNode::List(list)]);
    container.style = container.style.with_role("completion");

    let mut panel = UiPanel::new(
      "completion",
      LayoutIntent::Custom("completion".to_string()),
      UiNode::Container(container),
    );
    panel.style = panel.style.with_role("completion");
    panel.style.border = None;
    panel.constraints = UiConstraints::panel();
    panel.constraints.padding = UiInsets {
      left:   0,
      right:  0,
      top:    0,
      bottom: 0,
    };
    panel.constraints.min_width = None;

    let area = Rect::new(0, 0, 80, 20);
    let (width, height) = panel_box_size(&panel, area);
    let expected_width = max_content_width_for_intent(panel.intent.clone(), area, 0, 0)
      .min(panel.constraints.max_width.unwrap_or(u16::MAX));

    assert_eq!(width, expected_width);
    assert_eq!(height, 1);
  }

  #[test]
  fn completion_docs_rows_parse_markdown_basics() {
    let styles = CompletionDocsStyles::default(Style::default());
    let rows = completion_docs_rows(
      "# Title\n- item\n[Result](https://example.com)\n```rs\nfn test() {}\n```",
      &styles,
      80,
    );
    let non_empty: Vec<_> = flatten_rows(&rows)
      .into_iter()
      .filter(|line| !line.trim().is_empty())
      .collect();
    assert_eq!(non_empty, vec![
      "Title".to_string(),
      "• item Result".to_string(),
      "fn test() {}".to_string(),
    ]);
  }

  #[test]
  fn completion_docs_rows_strip_signature_active_parameter_markers() {
    let styles = CompletionDocsStyles::default(Style::default());
    let markdown = format!(
      "```c\nadd(int x, {}int y{}) -> int\n```",
      the_default::SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER,
      the_default::SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER
    );
    let rows = completion_docs_rows(&markdown, &styles, 120);
    let non_empty: Vec<_> = flatten_rows(&rows)
      .into_iter()
      .filter(|line| !line.trim().is_empty())
      .collect();
    assert_eq!(non_empty, vec!["add(int x, int y) -> int".to_string()]);
  }

  #[test]
  fn completion_docs_rows_wrap_long_lines() {
    let styles = CompletionDocsStyles::default(Style::default());
    let rows = completion_docs_rows("abcdef", &styles, 3);
    assert_eq!(flatten_rows(&rows), vec![
      "abc".to_string(),
      "def".to_string()
    ]);
  }

  #[test]
  fn markdown_fence_language_normalizes_case() {
    let blocks = parse_markdown_blocks("```Rust\nfn main() {}\n```");
    assert!(blocks
      .iter()
      .any(|block| matches!(block, DocsBlock::CodeFence { language: Some(language), .. } if language == "rust")));
  }

  #[test]
  fn language_hints_include_rust_extension_alias() {
    let hints = language_filename_hints("rust");
    assert!(hints.iter().any(|hint| hint == "rs"));
  }
}
