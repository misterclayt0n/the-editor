use std::{
  collections::HashMap,
  time::Instant,
};

use once_cell::sync::Lazy;
use the_editor_renderer::{
  Color,
  TextSection,
};

use crate::{
  Editor,
  core::{
    animation::breathing::BreathingAnimation,
    diagnostics::Severity,
    document::Document,
    graphics::{
      Rect,
      Style,
    },
    indent::IndentStyle,
    line_ending::LineEnding,
    view::View,
  },
  editor::{
    ModeConfig,
    StatusLineConfig,
    StatusLineElement,
  },
  keymap::Mode,
  lsp::LanguageServerId,
  theme::Theme,
  ui::{
    compositor::{
      Component,
      Context,
      Surface,
    },
    theme_color_to_renderer_color,
  },
};

/// Nix icon SVG data for statusline indicator
const NIX_ICON: &[u8] = include_bytes!("../../../assets/icons/nix.svg");

/// Check if we're running inside a Nix shell (cached at startup)
static IN_NIX_SHELL: Lazy<bool> =
  Lazy::new(|| the_editor_stdx::env::env_var_is_set("IN_NIX_SHELL"));

/// Formats a model ID for display in the statusline.
fn format_model_display(model_id: &str) -> String {
  let model_part = model_id.split('/').last().unwrap_or(model_id);

  if model_part.contains("haiku") {
    return "haiku".to_string();
  }
  if model_part.contains("sonnet") {
    if let Some(base) = model_part.strip_suffix("-20250514") {
      return base.to_string();
    }
    if model_part.contains("sonnet-4") {
      return "sonnet-4".to_string();
    }
    return "sonnet".to_string();
  }
  if model_part.contains("opus") {
    return "opus".to_string();
  }

  let without_date = model_part
    .split('-')
    .take_while(|part| part.parse::<u32>().is_err() || part.len() < 8)
    .collect::<Vec<_>>()
    .join("-");

  if without_date.is_empty() {
    model_part.to_string()
  } else {
    without_date
  }
}

// Visual constants
const STATUS_BAR_HEIGHT: f32 = 28.0;
const SEGMENT_PADDING_X: f32 = 12.0;
const FONT_SIZE: f32 = 13.0;
const SECTION_SPACING: f32 = 8.0; // Space between left/center/right sections

/// A rendered element with its text content and optional custom style
struct RenderedElement {
  text:  String,
  style: Option<Style>,
  width: f32,
}

impl RenderedElement {
  fn new(text: String) -> Self {
    let width = measure_text(&text);
    Self {
      text,
      style: None,
      width,
    }
  }

  fn with_style(text: String, style: Style) -> Self {
    let width = measure_text(&text);
    Self {
      text,
      style: Some(style),
      width,
    }
  }
}

/// Measure text width without drawing
fn measure_text(text: &str) -> f32 {
  let est_char_w = FONT_SIZE * 0.6;
  est_char_w * (text.chars().count() as f32)
}

/// StatusLine component with Helix-compatible configuration
pub struct StatusLine {
  visible:             bool,
  target_visible:      bool,
  anim_t:              f32,
  status_bar_y:        f32,
  slide_offset:        f32,
  should_slide:        bool,
  slide_anim_t:        f32,
  status_msg_anim_t:   f32,
  status_msg_slide_x:  f32,
  last_status_msg:     Option<String>,
  lsp_breathing_anims: HashMap<LanguageServerId, (BreathingAnimation, Instant)>,
  acp_breathing_anim:  Option<BreathingAnimation>,
}

impl StatusLine {
  pub fn new() -> Self {
    Self {
      visible:             true,
      target_visible:      true,
      anim_t:              1.0,
      status_bar_y:        0.0,
      slide_offset:        0.0,
      should_slide:        false,
      slide_anim_t:        1.0,
      status_msg_anim_t:   1.0, // Start completed to avoid infinite redraw loop when no status msg
      status_msg_slide_x:  0.0,
      last_status_msg:     None,
      lsp_breathing_anims: HashMap::new(),
      acp_breathing_anim:  None,
    }
  }

  pub fn toggle(&mut self) {
    self.target_visible = !self.target_visible;
    self.anim_t = 0.0;
  }

  pub fn is_visible(&self) -> bool {
    self.visible
  }

  pub fn slide_for_prompt(&mut self, slide: bool) {
    self.should_slide = slide;
    self.slide_anim_t = 0.0;
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Element Renderers
  // ─────────────────────────────────────────────────────────────────────────────

  fn render_mode(
    editor: &Editor,
    mode_config: &ModeConfig,
    focused: bool,
  ) -> Option<RenderedElement> {
    if !focused {
      // Unfocused views show empty space to prevent layout shift
      let width = measure_text(&mode_config.normal);
      return Some(RenderedElement {
        text: " ".repeat((width / (FONT_SIZE * 0.6)) as usize),
        style: None,
        width,
      });
    }

    let mode = editor.mode();
    let text = if let Some(ref custom) = editor.custom_mode_str {
      custom.clone()
    } else {
      match mode {
        Mode::Normal => mode_config.normal.clone(),
        Mode::Insert => mode_config.insert.clone(),
        Mode::Select => mode_config.select.clone(),
        Mode::Command => "CMD".to_string(),
      }
    };

    // Apply mode-specific style if color_modes is enabled
    let style = if editor.config().color_modes {
      let theme_key = match mode {
        Mode::Normal => "ui.statusline.normal",
        Mode::Insert => "ui.statusline.insert",
        Mode::Select => "ui.statusline.select",
        Mode::Command => "ui.statusline.normal",
      };
      Some(editor.theme.get(theme_key))
    } else {
      None
    };

    if let Some(style) = style {
      Some(RenderedElement::with_style(text, style))
    } else {
      Some(RenderedElement::new(text))
    }
  }

  fn render_spinner(editor: &Editor, doc: Option<&Document>) -> Option<RenderedElement> {
    const SPINNER_FRAMES: &[char] = &['⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷'];

    let doc = doc?;

    // Check if any language server is progressing
    let is_progressing = doc
      .language_servers
      .iter()
      .any(|(_, client)| editor.lsp_progress.is_progressing(client.id()));

    if is_progressing {
      // Calculate spinner frame based on current time (100ms per frame)
      let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
      let frame_idx = (millis / 100) as usize % SPINNER_FRAMES.len();
      let frame = SPINNER_FRAMES[frame_idx];
      return Some(RenderedElement::new(format!(" {} ", frame)));
    }

    None
  }

  fn render_file_name(editor: &Editor, doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let text = if let Some(path) = doc.path() {
      // Try to get workspace root for relative path
      if let Some(workspace_root) = editor.diff_providers.get_workspace_root(path) {
        if let Ok(rel_path) = path.strip_prefix(&workspace_root) {
          format!(" {} ", rel_path.display())
        } else {
          let folded = the_editor_stdx::path::fold_home_dir(path);
          format!(" {} ", folded.display())
        }
      } else {
        let folded = the_editor_stdx::path::fold_home_dir(path);
        format!(" {} ", folded.display())
      }
    } else {
      " [scratch] ".to_string()
    };
    Some(RenderedElement::new(text))
  }

  fn render_file_absolute_path(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let text = if let Some(path) = doc.path() {
      format!(" {} ", path.display())
    } else {
      " [scratch] ".to_string()
    };
    Some(RenderedElement::new(text))
  }

  fn render_file_base_name(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let text = if let Some(path) = doc.path() {
      let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("[No Name]");
      format!(" {} ", name)
    } else {
      " [scratch] ".to_string()
    };
    Some(RenderedElement::new(text))
  }

  fn render_file_modification_indicator(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    if doc.is_modified() {
      Some(RenderedElement::new("[+]".to_string()))
    } else {
      None
    }
  }

  fn render_read_only_indicator(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    if doc.readonly {
      Some(RenderedElement::new("[readonly]".to_string()))
    } else {
      None
    }
  }

  fn render_file_encoding(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let encoding = doc.encoding();
    // Only show if not UTF-8 (Helix behavior)
    if encoding.name() != "UTF-8" {
      Some(RenderedElement::new(format!(" {} ", encoding.name())))
    } else {
      None
    }
  }

  fn render_file_line_ending(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let text = match doc.line_ending {
      LineEnding::Crlf => " CRLF ",
      LineEnding::LF => " LF ",
      #[cfg(feature = "unicode-lines")]
      LineEnding::CR => " CR ",
      #[cfg(feature = "unicode-lines")]
      LineEnding::FF => " FF ",
      #[cfg(feature = "unicode-lines")]
      LineEnding::Nel => " NEL ",
      #[cfg(feature = "unicode-lines")]
      _ => " LF ",
    };
    Some(RenderedElement::new(text.to_string()))
  }

  fn render_file_indent_style(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let indent = doc.indent_style;
    let text = match indent {
      IndentStyle::Tabs => " tabs ".to_string(),
      IndentStyle::Spaces(n) => format!(" {} spaces ", n),
    };
    Some(RenderedElement::new(text))
  }

  fn render_file_type(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let lang = doc.language_name().unwrap_or("text");
    Some(RenderedElement::new(format!(" {} ", lang)))
  }

  fn render_diagnostics(
    _editor: &Editor,
    doc: Option<&Document>,
    config: &StatusLineConfig,
  ) -> Option<RenderedElement> {
    let doc = doc?;
    let diagnostics = doc.diagnostics();

    let mut counts: HashMap<Severity, usize> = HashMap::new();
    for diag in diagnostics {
      if let Some(severity) = diag.severity {
        *counts.entry(severity).or_insert(0) += 1;
      }
    }

    let mut parts = Vec::new();
    for severity in &config.diagnostics {
      if let Some(&count) = counts.get(severity) {
        if count > 0 {
          let symbol = match severity {
            Severity::Error => "●",
            Severity::Warning => "●",
            Severity::Info => "●",
            Severity::Hint => "●",
          };
          parts.push(format!("{} {}", symbol, count));
        }
      }
    }

    if parts.is_empty() {
      None
    } else {
      // Get colors for diagnostics
      let text = parts.join(" ");
      Some(RenderedElement::new(text))
    }
  }

  fn render_workspace_diagnostics(
    editor: &Editor,
    config: &StatusLineConfig,
  ) -> Option<RenderedElement> {
    let mut counts: HashMap<Severity, usize> = HashMap::new();

    for doc in editor.documents.values() {
      for diag in doc.diagnostics() {
        if let Some(severity) = diag.severity {
          *counts.entry(severity).or_insert(0) += 1;
        }
      }
    }

    let mut parts = Vec::new();
    for severity in &config.workspace_diagnostics {
      if let Some(&count) = counts.get(severity) {
        if count > 0 {
          let symbol = match severity {
            Severity::Error => "●",
            Severity::Warning => "●",
            Severity::Info => "●",
            Severity::Hint => "●",
          };
          parts.push(format!("{} {}", symbol, count));
        }
      }
    }

    if parts.is_empty() {
      None
    } else {
      let text = format!("W {}", parts.join(" "));
      Some(RenderedElement::new(text))
    }
  }

  fn render_selections(doc: Option<&Document>, view: Option<&View>) -> Option<RenderedElement> {
    let doc = doc?;
    let view = view?;
    let selection = doc.selection(view.id);
    let count = selection.ranges().len();

    let text = if count == 1 {
      " 1 sel ".to_string()
    } else {
      format!(" {}/{} sels ", selection.primary_index() + 1, count)
    };
    Some(RenderedElement::new(text))
  }

  fn render_primary_selection_length(
    doc: Option<&Document>,
    view: Option<&View>,
  ) -> Option<RenderedElement> {
    let doc = doc?;
    let view = view?;
    let selection = doc.selection(view.id);
    let primary = selection.primary();
    let len = primary.to() - primary.from();

    if len > 0 {
      Some(RenderedElement::new(format!(" {} chars ", len)))
    } else {
      None
    }
  }

  fn render_position(doc: Option<&Document>, view: Option<&View>) -> Option<RenderedElement> {
    let doc = doc?;
    let view = view?;
    let text = doc.text();
    let selection = doc.selection(view.id);
    let cursor = selection.primary().cursor(text.slice(..));
    let line = text.char_to_line(cursor);
    let col = cursor - text.line_to_char(line);

    // 1-indexed for display
    Some(RenderedElement::new(format!(" {}:{} ", line + 1, col + 1)))
  }

  fn render_position_percentage(
    doc: Option<&Document>,
    view: Option<&View>,
  ) -> Option<RenderedElement> {
    let doc = doc?;
    let view = view?;
    let text = doc.text();
    let selection = doc.selection(view.id);
    let cursor = selection.primary().cursor(text.slice(..));
    let line = text.char_to_line(cursor);
    let total_lines = text.len_lines().saturating_sub(1);

    let text = if total_lines == 0 {
      "All".to_string()
    } else if line == 0 {
      "Top".to_string()
    } else if line >= total_lines {
      "Bot".to_string()
    } else {
      format!("{}%", (line * 100) / total_lines)
    };
    Some(RenderedElement::new(text))
  }

  fn render_total_line_numbers(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    let total = doc.text().len_lines();
    Some(RenderedElement::new(format!(" {} ", total)))
  }

  fn render_separator(config: &StatusLineConfig) -> Option<RenderedElement> {
    Some(RenderedElement::new(config.separator.clone()))
  }

  fn render_spacer() -> Option<RenderedElement> {
    Some(RenderedElement::new(" ".to_string()))
  }

  fn render_version_control(doc: Option<&Document>) -> Option<RenderedElement> {
    let doc = doc?;
    if let Some(branch) = doc.version_control_head() {
      Some(RenderedElement::new(format!(" {} ", branch.as_ref())))
    } else {
      None
    }
  }

  fn render_register(editor: &Editor) -> Option<RenderedElement> {
    if let Some(reg) = editor.selected_register {
      Some(RenderedElement::new(format!(" reg={} ", reg)))
    } else {
      None
    }
  }

  fn render_current_working_directory() -> Option<RenderedElement> {
    if let Ok(cwd) = std::env::current_dir() {
      if let Some(name) = cwd.file_name().and_then(|n| n.to_str()) {
        return Some(RenderedElement::new(format!(" {} ", name)));
      }
    }
    None
  }

  // === the-editor extension element renderers ===

  fn render_acp_status_element(editor: &Editor) -> Option<RenderedElement> {
    let acp = editor.acp.as_ref().filter(|acp| acp.is_connected())?;

    let text = if let Some(model_state) = acp.model_state() {
      let model_id_str = model_state.current_model_id.to_string();
      let model_display = format_model_display(&model_id_str);
      format!(" ACP:{} ", model_display)
    } else {
      " ACP ".to_string()
    };

    Some(RenderedElement::new(text))
  }

  fn render_nix_shell_element() -> Option<RenderedElement> {
    if *IN_NIX_SHELL {
      Some(RenderedElement::new(" nix ".to_string()))
    } else {
      None
    }
  }

  fn render_lsp_servers_element(
    _editor: &Editor,
    doc: Option<&Document>,
  ) -> Option<RenderedElement> {
    let doc = doc?;
    if doc.language_servers.is_empty() {
      return None;
    }

    let server_names: Vec<&str> = doc
      .language_servers
      .iter()
      .map(|(name, _)| name.as_str())
      .collect();

    let text = format!(" {} ", server_names.join(", "));
    Some(RenderedElement::new(text))
  }

  /// Render a single statusline element
  fn render_element(
    &self,
    element: &StatusLineElement,
    editor: &Editor,
    doc: Option<&Document>,
    view: Option<&View>,
    config: &StatusLineConfig,
    focused: bool,
  ) -> Option<RenderedElement> {
    match element {
      StatusLineElement::Mode => Self::render_mode(editor, &config.mode, focused),
      StatusLineElement::Spinner => Self::render_spinner(editor, doc),
      StatusLineElement::FileName => Self::render_file_name(editor, doc),
      StatusLineElement::FileAbsolutePath => Self::render_file_absolute_path(doc),
      StatusLineElement::FileBaseName => Self::render_file_base_name(doc),
      StatusLineElement::FileModificationIndicator => Self::render_file_modification_indicator(doc),
      StatusLineElement::ReadOnlyIndicator => Self::render_read_only_indicator(doc),
      StatusLineElement::FileEncoding => Self::render_file_encoding(doc),
      StatusLineElement::FileLineEnding => Self::render_file_line_ending(doc),
      StatusLineElement::FileIndentStyle => Self::render_file_indent_style(doc),
      StatusLineElement::FileType => Self::render_file_type(doc),
      StatusLineElement::Diagnostics => Self::render_diagnostics(editor, doc, config),
      StatusLineElement::WorkspaceDiagnostics => Self::render_workspace_diagnostics(editor, config),
      StatusLineElement::Selections => Self::render_selections(doc, view),
      StatusLineElement::PrimarySelectionLength => Self::render_primary_selection_length(doc, view),
      StatusLineElement::Position => Self::render_position(doc, view),
      StatusLineElement::PositionPercentage => Self::render_position_percentage(doc, view),
      StatusLineElement::TotalLineNumbers => Self::render_total_line_numbers(doc),
      StatusLineElement::Separator => Self::render_separator(config),
      StatusLineElement::Spacer => Self::render_spacer(),
      StatusLineElement::VersionControl => Self::render_version_control(doc),
      StatusLineElement::Register => Self::render_register(editor),
      StatusLineElement::CurrentWorkingDirectory => Self::render_current_working_directory(),
      // the-editor extensions
      StatusLineElement::AcpStatus => Self::render_acp_status_element(editor),
      StatusLineElement::NixShell => Self::render_nix_shell_element(),
      StatusLineElement::LspServers => Self::render_lsp_servers_element(editor, doc),
    }
  }

  /// Render a section (left, center, or right) and return total width
  fn render_section(
    &self,
    elements: &[StatusLineElement],
    editor: &Editor,
    doc: Option<&Document>,
    view: Option<&View>,
    config: &StatusLineConfig,
    focused: bool,
  ) -> Vec<RenderedElement> {
    let mut rendered = Vec::new();

    for element in elements.iter() {
      if let Some(elem) = self.render_element(element, editor, doc, view, config, focused) {
        rendered.push(elem);
      }
    }

    rendered
  }

  /// Draw rendered elements starting at x position, returns ending x
  fn draw_elements(
    surface: &mut Surface,
    elements: &[RenderedElement],
    x: f32,
    y: f32,
    base_color: Color,
    _theme: &Theme,
  ) -> f32 {
    let mut current_x = x;
    let text_y = y + (STATUS_BAR_HEIGHT - FONT_SIZE) * 0.5;

    for elem in elements {
      let color = if let Some(ref style) = elem.style {
        // Draw background if style has one
        if let Some(bg) = style.bg {
          let bg_color = theme_color_to_renderer_color(bg);
          surface.draw_rect(current_x, y, elem.width, STATUS_BAR_HEIGHT, bg_color);
        }
        style
          .fg
          .map(theme_color_to_renderer_color)
          .unwrap_or(base_color)
      } else {
        base_color
      };

      surface.draw_text(TextSection::simple(
        current_x, text_y, &elem.text, FONT_SIZE, color,
      ));
      current_x += elem.width;
    }

    current_x
  }

  /// Calculate total width of rendered elements
  fn section_width(elements: &[RenderedElement]) -> f32 {
    elements.iter().map(|e| e.width).sum()
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // the-editor Extensions (Bonus Features)
  // ─────────────────────────────────────────────────────────────────────────────

  fn render_nix_indicator(&self, surface: &mut Surface, x: f32, y: f32, color: Color) -> f32 {
    if !*IN_NIX_SHELL {
      return 0.0;
    }

    const NIX_ICON_SIZE: u32 = 18;
    let icon_y = y + (STATUS_BAR_HEIGHT - NIX_ICON_SIZE as f32) * 0.5;
    surface.draw_svg_icon(NIX_ICON, x, icon_y, NIX_ICON_SIZE, NIX_ICON_SIZE, color);
    NIX_ICON_SIZE as f32 + 8.0 // icon + spacing
  }

  fn render_lsp_status(
    &mut self,
    surface: &mut Surface,
    x: f32,
    y: f32,
    doc: Option<&Document>,
    editor: &Editor,
    base_color: Color,
  ) -> f32 {
    let doc = match doc {
      Some(d) if !d.language_servers.is_empty() => d,
      _ => return 0.0,
    };

    let now = Instant::now();
    const ANIMATION_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(500);

    let lsp_servers: Vec<_> = doc
      .language_servers
      .iter()
      .map(|(name, client)| (name.as_str(), client.id()))
      .collect();

    // Update breathing animations
    for (_server_name, server_id) in &lsp_servers {
      if editor.lsp_progress.is_progressing(*server_id) {
        self
          .lsp_breathing_anims
          .entry(*server_id)
          .and_modify(|(_, last_seen)| *last_seen = now)
          .or_insert_with(|| (BreathingAnimation::new(), now));
      }
    }

    let current_server_ids: std::collections::HashSet<_> =
      lsp_servers.iter().map(|(_, id)| *id).collect();
    self.lsp_breathing_anims.retain(|id, (_, last_seen)| {
      current_server_ids.contains(id)
        && now.saturating_duration_since(*last_seen) < ANIMATION_GRACE_PERIOD
    });

    let lsp_text = lsp_servers
      .iter()
      .map(|(name, _)| *name)
      .collect::<Vec<_>>()
      .join(",");
    let lsp_width = measure_text(&lsp_text);

    let lsp_color = if lsp_servers
      .iter()
      .any(|(_, id)| self.lsp_breathing_anims.contains_key(id))
    {
      let (anim, _) = lsp_servers
        .iter()
        .find_map(|(_, id)| self.lsp_breathing_anims.get(id))
        .unwrap();

      let loading_style = editor.theme.get("ui.statusline.lsp.loading");
      let color = loading_style
        .fg
        .or_else(|| editor.theme.get("ui.statusline").fg)
        .map(theme_color_to_renderer_color)
        .unwrap_or(base_color);
      anim.apply_to_color(color, now)
    } else {
      base_color
    };

    let text_y = y + (STATUS_BAR_HEIGHT - FONT_SIZE) * 0.5;
    surface.draw_text(TextSection::simple(
      x, text_y, &lsp_text, FONT_SIZE, lsp_color,
    ));

    lsp_width + 8.0 // text + spacing
  }

  fn render_acp_status(
    &mut self,
    surface: &mut Surface,
    x: f32,
    y: f32,
    editor: &Editor,
    base_color: Color,
  ) -> f32 {
    let acp = match &editor.acp {
      Some(acp) if acp.is_connected() => acp,
      _ => return 0.0,
    };

    let now = Instant::now();
    let has_pending_permissions = editor.acp_permissions.pending_count() > 0;

    if has_pending_permissions {
      if self.acp_breathing_anim.is_none() {
        self.acp_breathing_anim = Some(BreathingAnimation::new());
      }
    } else {
      self.acp_breathing_anim = None;
    }

    let acp_text = if let Some(model_state) = acp.model_state() {
      let model_id_str = model_state.current_model_id.to_string();
      let model_display = format_model_display(&model_id_str);
      format!("ACP:{}", model_display)
    } else {
      "ACP".to_string()
    };

    let acp_width = measure_text(&acp_text);

    let acp_color = if let Some(ref anim) = self.acp_breathing_anim {
      let pending_style = editor.theme.get("ui.statusline.acp.pending");
      let color = pending_style
        .fg
        .or_else(|| editor.theme.get("warning").fg)
        .map(theme_color_to_renderer_color)
        .unwrap_or(Color::new(0.9, 0.7, 0.3, 1.0));
      anim.apply_to_color(color, now)
    } else {
      base_color
    };

    let text_y = y + (STATUS_BAR_HEIGHT - FONT_SIZE) * 0.5;
    surface.draw_text(TextSection::simple(
      x, text_y, &acp_text, FONT_SIZE, acp_color,
    ));

    acp_width + 8.0
  }

  fn render_status_message(
    &mut self,
    surface: &mut Surface,
    x: f32,
    y: f32,
    editor: &Editor,
    dt: f32,
  ) -> f32 {
    let (status_msg, severity) = match editor.get_status() {
      Some(s) => s,
      None => {
        if self.last_status_msg.is_some() {
          self.last_status_msg = None;
          self.status_msg_anim_t = 0.0;
          self.status_msg_slide_x = 0.0;
        }
        return 0.0;
      },
    };

    let anim_enabled = editor.config().status_msg_anim_enabled;

    let current_msg = status_msg.to_string();
    if self.last_status_msg.as_ref() != Some(&current_msg) {
      self.last_status_msg = Some(current_msg);
      self.status_msg_anim_t = 0.0;
      self.status_msg_slide_x = if anim_enabled { -30.0 } else { 0.0 };
    }

    const STATUS_ANIM_SPEED: f32 = 0.15;
    if self.status_msg_anim_t < 1.0 {
      let speed = STATUS_ANIM_SPEED * 420.0;
      self.status_msg_anim_t = (self.status_msg_anim_t + speed * dt).min(1.0);
    }

    let eased = 1.0 - (1.0 - self.status_msg_anim_t) * (1.0 - self.status_msg_anim_t);

    if anim_enabled {
      let lerp_t = 1.0 - 0.75_f32.powf(dt * 420.0);
      self.status_msg_slide_x += (0.0 - self.status_msg_slide_x) * lerp_t;
    } else {
      self.status_msg_slide_x = 0.0;
    }

    let msg_color = match severity {
      Severity::Error => {
        editor
          .theme
          .get("error")
          .fg
          .map(theme_color_to_renderer_color)
          .unwrap_or(Color::new(0.9, 0.3, 0.3, 1.0))
      },
      Severity::Warning => {
        editor
          .theme
          .get("warning")
          .fg
          .map(theme_color_to_renderer_color)
          .unwrap_or(Color::new(0.9, 0.7, 0.3, 1.0))
      },
      Severity::Info => {
        editor
          .theme
          .get("info")
          .fg
          .map(theme_color_to_renderer_color)
          .unwrap_or(Color::new(0.4, 0.7, 0.9, 1.0))
      },
      Severity::Hint => {
        editor
          .theme
          .get("hint")
          .fg
          .map(theme_color_to_renderer_color)
          .unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0))
      },
    };

    let animated_color = Color::new(msg_color.r, msg_color.g, msg_color.b, msg_color.a * eased);

    let anim_x = x + self.status_msg_slide_x;
    let text_y = y + (STATUS_BAR_HEIGHT - FONT_SIZE) * 0.5;
    surface.draw_text(TextSection::simple(
      anim_x,
      text_y,
      status_msg.as_ref(),
      FONT_SIZE,
      animated_color,
    ));

    measure_text(status_msg.as_ref())
  }
}

impl Default for StatusLine {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for StatusLine {
  fn render(&mut self, _area: Rect, surface: &mut Surface, cx: &mut Context) {
    // Save font state and configure UI font
    let saved_font = surface.save_font_state();
    let ui_font_family = surface.current_font_family().to_owned();
    surface.configure_font(&ui_font_family, FONT_SIZE);

    let theme = &cx.editor.theme;
    let config = cx.editor.config();
    let statusline_config = &config.statusline;

    // Get base statusline style
    let statusline_style = theme.get("ui.statusline");
    let bg_color = statusline_style
      .bg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.13, 1.0));
    let text_color = statusline_style
      .fg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.85, 0.85, 0.9, 1.0));

    // Update vertical animation
    const ANIM_SPEED: f32 = 0.12;
    if self.anim_t < 1.0 {
      let speed = ANIM_SPEED * 420.0;
      self.anim_t = (self.anim_t + speed * cx.dt).min(1.0);
    }
    let eased_t = 1.0 - (1.0 - self.anim_t) * (1.0 - self.anim_t);

    // Update horizontal slide animation
    const PROMPT_WIDTH_PERCENT: f32 = 0.25;
    let target_offset = if self.should_slide {
      surface.width() as f32 * PROMPT_WIDTH_PERCENT + 16.0
    } else {
      0.0
    };

    const SLIDE_SPEED: f32 = 0.15;
    if self.slide_anim_t < 1.0 {
      let speed = SLIDE_SPEED * 420.0;
      self.slide_anim_t = (self.slide_anim_t + speed * cx.dt).min(1.0);
    }
    let eased_slide = 1.0 - (1.0 - self.slide_anim_t) * (1.0 - self.slide_anim_t);
    self.slide_offset += (target_offset - self.slide_offset) * eased_slide;

    // Calculate Y position
    let base_y = surface.height() as f32 - STATUS_BAR_HEIGHT;
    let hidden_y = surface.height() as f32;
    let bar_y = if self.target_visible {
      hidden_y + (base_y - hidden_y) * eased_t
    } else {
      base_y + (hidden_y - base_y) * eased_t
    };
    self.status_bar_y = bar_y;

    // Early exit if fully hidden
    if !self.target_visible && self.anim_t >= 1.0 {
      self.visible = false;
      surface.restore_font_state(saved_font);
      return;
    }
    self.visible = true;

    // Draw background
    let viewport_width = surface.width() as f32;
    surface.draw_rect(0.0, bar_y, viewport_width, STATUS_BAR_HEIGHT, bg_color);

    // Render in overlay mode
    surface.with_overlay_region(0.0, bar_y, viewport_width, STATUS_BAR_HEIGHT, |surface| {
      let focus_id = cx.editor.tree.focus;
      let view = cx.editor.tree.get(focus_id);
      let focused = true; // Main view is always focused in this context

      let (doc, view_ref) = if let Some(doc_id) = view.doc() {
        let doc = cx.editor.documents.get(&doc_id);
        (doc, Some(view))
      } else {
        (None, None)
      };

      // Render left section
      let left_elements = self.render_section(
        &statusline_config.left,
        &cx.editor,
        doc,
        view_ref,
        statusline_config,
        focused,
      );
      let left_width = Self::section_width(&left_elements);

      // Render right section
      let right_elements = self.render_section(
        &statusline_config.right,
        &cx.editor,
        doc,
        view_ref,
        statusline_config,
        focused,
      );
      let right_width = Self::section_width(&right_elements);

      // Render center section
      let center_elements = self.render_section(
        &statusline_config.center,
        &cx.editor,
        doc,
        view_ref,
        statusline_config,
        focused,
      );
      let center_width = Self::section_width(&center_elements);

      // Calculate positions using Helix's layout algorithm
      let left_x = SEGMENT_PADDING_X + self.slide_offset;

      // Draw right config elements
      let right_x = viewport_width - SEGMENT_PADDING_X - right_width + self.slide_offset;
      Self::draw_elements(surface, &right_elements, right_x, bar_y, text_color, theme);

      // Draw left elements
      let left_end = Self::draw_elements(surface, &left_elements, left_x, bar_y, text_color, theme);

      // Draw status message after left elements
      self.render_status_message(surface, left_end + 16.0, bar_y, &cx.editor, cx.dt);

      // Draw center elements (if any)
      if !center_elements.is_empty() {
        let center_x = (viewport_width - center_width) / 2.0 + self.slide_offset;
        // Only draw if there's space (not overlapping left/right)
        let left_boundary = left_x + left_width + SECTION_SPACING;
        let right_boundary = right_x - SECTION_SPACING;
        if center_x >= left_boundary && center_x + center_width <= right_boundary {
          Self::draw_elements(
            surface,
            &center_elements,
            center_x,
            bar_y,
            text_color,
            theme,
          );
        }
      }
    });

    surface.restore_font_state(saved_font);
  }

  fn should_update(&self) -> bool {
    self.anim_t < 1.0
      || self.slide_anim_t < 1.0
      || self.status_msg_anim_t < 1.0
      || !self.lsp_breathing_anims.is_empty()
      || self.acp_breathing_anim.is_some()
  }
}
