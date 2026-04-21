use std::{
  collections::{
    HashMap,
    HashSet,
  },
  env,
  ffi::OsString,
  fs::OpenOptions,
  io::{
    BufRead,
    BufReader,
    BufWriter,
    Read,
    Write,
  },
  path::{
    Path,
    PathBuf,
  },
  process::{
    Child,
    ChildStdin,
    ChildStdout,
    Command as ProcessCommand,
    Stdio,
  },
  sync::{
    Mutex,
    OnceLock,
    mpsc::{
      self,
      Receiver,
      RecvTimeoutError,
      Sender,
      TryRecvError,
    },
  },
  thread,
  time::{
    Duration,
    Instant,
    SystemTime,
  },
};

use ropey::Rope;
use serde::{
  Deserialize,
  Serialize,
};
use serde_json::{
  Value,
  json,
};
use the_lib::{
  editor::BufferId,
  indent::IndentStyle,
  render::{
    OwnedTextAnnotations,
    VirtualLineSpec,
    graphics::Color,
  },
  selection::{
    CursorId,
    Selection,
  },
  transaction::Transaction,
};
use the_lsp::text_sync::{
  char_idx_to_utf16_position,
  file_uri_for_path,
  utf16_position_to_char_idx,
};

use crate::{
  Completion,
  CompletionMenuItem,
  DefaultContext,
  Key,
  KeyEvent,
  Mode,
  RenderWaker,
};

const SOURCE: &str = "inline";
const DEBOUNCE: Duration = Duration::from_millis(75);
const RETRY_AFTER_ERROR: Duration = Duration::from_secs(10);
const EDITOR_NAME: &str = "the-editor";
const EDITOR_PLUGIN_NAME: &str = "the-editor-inline-completion";
const SUPERMAVEN_QUERY_TIMEOUT: Duration = Duration::from_millis(350);
const INLINE_COMPLETION_TRACE_ENV: &str = "THE_EDITOR_INLINE_COMPLETION_TRACE_LOG";
const INLINE_COMPLETION_TRACE_DEFAULT: &str = "/tmp/the-editor-inline-completion.log";
const MAX_INLINE_COMPLETION_PREVIEW_REMAINING_LINES: usize = 5;
const MAX_INLINE_COMPLETION_SIMPLE_FIRST_LINE_CHARS: usize = 96;
const MAX_INLINE_COMPLETION_SIMPLE_TOTAL_CHARS: usize = 180;
const MAX_INLINE_COMPLETION_SIMPLE_TOTAL_LINES: usize = 3;
const MAX_INLINE_COMPLETION_PRESENTATION_LINES: usize = 6;
const MAX_INLINE_COMPLETION_PRESENTATION_LINE_CHARS: usize = 96;
static INLINE_COMPLETION_TRACE_WRITER: OnceLock<Option<Mutex<BufWriter<std::fs::File>>>> =
  OnceLock::new();

pub fn resolve_inline_completion_trace_log_path() -> Option<PathBuf> {
  match env::var(INLINE_COMPLETION_TRACE_ENV) {
    Ok(path) => {
      let path = path.trim();
      if path.is_empty() || path.eq_ignore_ascii_case("off") || path.eq_ignore_ascii_case("none") {
        None
      } else {
        Some(PathBuf::from(path))
      }
    },
    Err(_) => Some(PathBuf::from(INLINE_COMPLETION_TRACE_DEFAULT)),
  }
}

pub fn trace_event(event: &str, data: Value) {
  inline_completion_trace(event, data);
}

fn inline_completion_trace_writer() -> Option<&'static Mutex<BufWriter<std::fs::File>>> {
  INLINE_COMPLETION_TRACE_WRITER
    .get_or_init(|| {
      let path = resolve_inline_completion_trace_log_path()?;
      if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
      {
        eprintln!(
          "Warning: failed to create inline completion trace directory '{}': {err}",
          parent.display()
        );
        return None;
      }

      match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => Some(Mutex::new(BufWriter::new(file))),
        Err(err) => {
          eprintln!(
            "Warning: failed to open inline completion trace log '{}': {err}",
            path.display()
          );
          None
        },
      }
    })
    .as_ref()
}

fn inline_completion_trace(event: &str, data: Value) {
  tracing::trace!(target: "the_default::inline_completion", event = event, data = %data);

  let Some(writer) = inline_completion_trace_writer() else {
    return;
  };

  let ts_ms = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .map(|duration| duration.as_millis() as u64)
    .unwrap_or(0);
  let line = match serde_json::to_string(&json!({
    "ts_ms": ts_ms,
    "event": event,
    "data": data,
  })) {
    Ok(line) => line,
    Err(_) => return,
  };

  let mut writer = match writer.lock() {
    Ok(writer) => writer,
    Err(poisoned) => poisoned.into_inner(),
  };
  let _ = writeln!(writer, "{line}");
  let _ = writer.flush();
}

fn inline_completion_trace_text_preview(text: &str) -> String {
  let mut preview = text.replace('\n', "\\n").replace('\r', "\\r");
  if preview.chars().count() > 80 {
    preview = preview.chars().take(77).collect::<String>();
    preview.push_str("...");
  }
  preview
}

fn inline_completion_trace_query_key(key: &QueryKey) -> Value {
  json!({
    "buffer_id": format!("{:?}", key.buffer_id),
    "doc_version": key.doc_version,
    "cursor_char": key.cursor_char,
    "file_path": key.file_path,
  })
}

fn inline_completion_trace_request(request: &ProviderQuery) -> Value {
  match request {
    ProviderQuery::Copilot(request) => {
      json!({
        "provider": "copilot",
        "uri": request.uri.as_str(),
        "language_id": request.language_id.as_str(),
        "version": request.version,
        "line": request.line,
        "character": request.character,
        "workspace_root": request.workspace_root.display().to_string(),
      })
    },
    ProviderQuery::Supermaven(request) => {
      json!({
        "provider": "supermaven",
        "file_path": request.file_path.display().to_string(),
        "cursor_char": request.cursor_char,
        "line_before": inline_completion_trace_text_preview(&request.line_before),
        "line_after": inline_completion_trace_text_preview(&request.line_after),
      })
    },
  }
}

fn inline_completion_trace_range(range: Option<&JsonRange>) -> Value {
  match range {
    Some(range) => {
      json!({
        "start": {
          "line": range.start.line,
          "character": range.start.character,
        },
        "end": {
          "line": range.end.line,
          "character": range.end.character,
        },
      })
    },
    None => Value::Null,
  }
}

fn inline_completion_trace_worker_suggestion(suggestion: &WorkerSuggestion) -> Value {
  json!({
    "from": suggestion.from,
    "to": suggestion.to,
    "text_chars": suggestion.text.chars().count(),
    "text_preview": inline_completion_trace_text_preview(&suggestion.text),
    "source": suggestion.source.label(),
    "has_command": suggestion.command.is_some(),
    "display_range": inline_completion_trace_range(suggestion.display_range.as_ref()),
  })
}

fn inline_completion_trace_suggestion(suggestion: &InlineSuggestion) -> Value {
  json!({
    "key": inline_completion_trace_query_key(&suggestion.key),
    "from": suggestion.from,
    "to": suggestion.to,
    "text_chars": suggestion.text.chars().count(),
    "text_preview": inline_completion_trace_text_preview(&suggestion.text),
    "source": suggestion.source.label(),
    "has_command": suggestion.command.is_some(),
    "display_range": inline_completion_trace_range(suggestion.display_range.as_ref()),
    "reported_to_provider": suggestion.reported_to_provider,
  })
}

fn inline_completion_trace_display_kind(kind: SuggestionDisplayKind) -> Value {
  match kind {
    SuggestionDisplayKind::Ghost { anchor_char } => {
      json!({
        "kind": "ghost",
        "anchor_char": anchor_char,
      })
    },
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionProvider {
  None,
  Copilot,
  Supermaven,
}

impl InlineCompletionProvider {
  pub fn parse(raw: &str) -> Option<Self> {
    match raw.trim().to_ascii_lowercase().as_str() {
      "none" | "off" => Some(Self::None),
      "copilot" => Some(Self::Copilot),
      "supermaven" => Some(Self::Supermaven),
      _ => None,
    }
  }

  pub const fn label(self) -> &'static str {
    match self {
      Self::None => "none",
      Self::Copilot => "copilot",
      Self::Supermaven => "supermaven",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionBackendStatus {
  Idle,
  Starting,
  Ready,
  Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCompletionDefaults {
  pub enabled:  bool,
  pub provider: InlineCompletionProvider,
}

impl Default for InlineCompletionDefaults {
  fn default() -> Self {
    Self {
      enabled:  true,
      provider: InlineCompletionProvider::None,
    }
  }
}

impl InlineCompletionDefaults {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn enabled(mut self, enabled: bool) -> Self {
    self.enabled = enabled;
    self
  }

  pub fn provider(mut self, provider: InlineCompletionProvider) -> Self {
    self.provider = provider;
    self
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptKind {
  Full,
  Word,
  Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionPresentationKind {
  Menu,
  Popover,
  Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionPresentationLineKind {
  Plain,
  Addition,
  Removal,
  Dim,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCompletionPresentationLine {
  pub kind: InlineCompletionPresentationLineKind,
  pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCompletionPresentation {
  pub kind:         InlineCompletionPresentationKind,
  pub title:        String,
  pub lines:        Vec<InlineCompletionPresentationLine>,
  pub target_char:  Option<usize>,
  pub target_line:  Option<usize>,
  pub accept_label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlinePredictedEditDisplayMode {
  InlineGhost,
  InlineGhostWithPopover,
  DiffPopover,
}

impl InlinePredictedEditDisplayMode {
  const fn label(self) -> &'static str {
    match self {
      Self::InlineGhost => "inline_ghost",
      Self::InlineGhostWithPopover => "inline_ghost_with_popover",
      Self::DiffPopover => "diff_popover",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlinePredictedEditSpanKind {
  Equal,
  Addition,
  Removal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlinePredictedEditSpan {
  kind: InlinePredictedEditSpanKind,
  text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineGhostPreview {
  preview_char:               usize,
  replacement_first_line_len: usize,
  first_line:                 String,
  remaining_text:             String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlinePredictedEdit {
  source:         InlineSuggestionSource,
  replace_from:   usize,
  replace_to:     usize,
  anchor_char:    usize,
  preview_char:   usize,
  inserted_text:  String,
  replaced_text:  String,
  spans:          Vec<InlinePredictedEditSpan>,
  display_mode:   InlinePredictedEditDisplayMode,
  inline_preview: Option<InlineGhostPreview>,
  presentation:   Option<InlineCompletionPresentation>,
}

impl InlinePredictedEdit {
  const fn is_pure_insertion(&self) -> bool {
    self.replace_from == self.replace_to
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryKey {
  buffer_id:   BufferId,
  doc_version: u64,
  cursor_char: usize,
  file_path:   PathBuf,
}

#[derive(Debug, Clone)]
struct PreparedQuery {
  key:     QueryKey,
  request: ProviderQuery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedQueryIneligibility {
  NotInsertMode,
  NoProvider,
  NoFilePath,
  InvalidSelection,
  NonEmptySelection,
}

impl PreparedQueryIneligibility {
  const fn label(self) -> &'static str {
    match self {
      Self::NotInsertMode => "not_insert_mode",
      Self::NoProvider => "no_provider",
      Self::NoFilePath => "no_file_path",
      Self::InvalidSelection => "invalid_selection",
      Self::NonEmptySelection => "non_empty_selection",
    }
  }
}

#[derive(Debug, Clone)]
struct ScheduledQuery {
  prepared: PreparedQuery,
  ready_at: Instant,
}

#[derive(Debug, Clone)]
struct InFlightQuery {
  request_id: u64,
  key:        QueryKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineSuggestionSource {
  CopilotInlineCompletion,
  Supermaven,
}

impl InlineSuggestionSource {
  const fn is_copilot(self) -> bool {
    matches!(self, Self::CopilotInlineCompletion)
  }

  const fn label(self) -> &'static str {
    match self {
      Self::CopilotInlineCompletion => "copilot_inline_completion",
      Self::Supermaven => "supermaven",
    }
  }
}

#[derive(Debug, Clone)]
struct InlineSuggestion {
  key:                  QueryKey,
  from:                 usize,
  to:                   usize,
  text:                 String,
  source:               InlineSuggestionSource,
  command:              Option<JsonCommand>,
  display_range:        Option<JsonRange>,
  reported_to_provider: bool,
}

#[derive(Debug)]
struct InlineCompletionTransport {
  provider: InlineCompletionProvider,
  tx:       Sender<WorkerCommand>,
  rx:       Receiver<WorkerEvent>,
}

#[derive(Debug)]
pub struct InlineCompletionState {
  pub enabled:         bool,
  pub provider:        InlineCompletionProvider,
  pub status:          InlineCompletionBackendStatus,
  pub auth_prompt:     Option<String>,
  pub activation_url:  Option<String>,
  pub account_user:    Option<String>,
  pub service_tier:    Option<String>,
  pub presentation:    Option<InlineCompletionPresentation>,
  transport:           Option<InlineCompletionTransport>,
  scheduled:           Option<ScheduledQuery>,
  in_flight:           Option<InFlightQuery>,
  suggestion:          Option<InlineSuggestion>,
  last_completed_key:  Option<QueryKey>,
  pub last_error:      Option<String>,
  last_reported_error: Option<String>,
  retry_at:            Option<Instant>,
  next_request_id:     u64,
}

impl Default for InlineCompletionState {
  fn default() -> Self {
    Self::from_defaults(InlineCompletionDefaults::default())
  }
}

impl InlineCompletionState {
  pub fn from_defaults(defaults: InlineCompletionDefaults) -> Self {
    Self {
      enabled:             defaults.enabled,
      provider:            defaults.provider,
      status:              InlineCompletionBackendStatus::Idle,
      auth_prompt:         None,
      activation_url:      None,
      account_user:        None,
      service_tier:        None,
      presentation:        None,
      transport:           None,
      scheduled:           None,
      in_flight:           None,
      suggestion:          None,
      last_completed_key:  None,
      last_error:          None,
      last_reported_error: None,
      retry_at:            None,
      next_request_id:     1,
    }
  }

  pub fn apply_defaults(&mut self, defaults: &InlineCompletionDefaults) {
    self.enabled = defaults.enabled;
    if self.provider != defaults.provider {
      self.provider = defaults.provider;
      reset_backend_runtime(self);
    }
  }
}

fn reset_backend_runtime(state: &mut InlineCompletionState) {
  state.transport = None;
  state.status = InlineCompletionBackendStatus::Idle;
  state.auth_prompt = None;
  state.activation_url = None;
  state.account_user = None;
  state.service_tier = None;
  state.presentation = None;
  state.scheduled = None;
  state.in_flight = None;
  state.suggestion = None;
  state.last_completed_key = None;
  state.last_error = None;
  state.last_reported_error = None;
  state.retry_at = None;
}

pub fn complete_inline_provider(query: &str) -> Vec<Completion> {
  let query = query.trim().to_ascii_lowercase();
  [
    InlineCompletionProvider::None,
    InlineCompletionProvider::Copilot,
    InlineCompletionProvider::Supermaven,
  ]
  .into_iter()
  .filter(|provider| provider.label().starts_with(&query))
  .map(|provider| {
    Completion {
      range: 0..,
      text:  provider.label().to_string(),
      doc:   Some(format!("Use {} for inline completions", provider.label())),
    }
  })
  .collect()
}

pub fn handle_pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if ctx.mode() != Mode::Insert {
    return false;
  }

  match key.key {
    Key::Tab if key.modifiers.is_empty() && !ctx.completion_menu().active => {
      accept_inline_completion_kind(ctx, AcceptKind::Full)
    },
    Key::Escape if !ctx.completion_menu().active => {
      let _ = dismiss_inline_completion(ctx);
      false
    },
    _ => false,
  }
}

pub fn pre_render<Ctx: DefaultContext>(ctx: &mut Ctx) {
  drain_worker_events(ctx);
  schedule_or_send_query(ctx);
  sync_inline_completion_annotations(ctx);
}

pub fn set_inline_provider_command<Ctx: DefaultContext>(ctx: &mut Ctx, arg: Option<&str>) {
  let Some(raw) = arg else {
    ctx.push_error(
      SOURCE,
      "missing provider. Use :inline-provider none|copilot|supermaven",
    );
    return;
  };
  let Some(provider) = InlineCompletionProvider::parse(raw) else {
    ctx.push_error(
      SOURCE,
      format!("unknown inline provider `{raw}`. Use none, copilot, or supermaven."),
    );
    return;
  };
  set_active_provider(ctx, provider, true);
}

pub fn toggle_inline<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let enabled = {
    let state = ctx.inline_completion_mut();
    state.enabled = !state.enabled;
    if !state.enabled {
      state.auth_prompt = None;
      state.scheduled = None;
      state.in_flight = None;
      state.suggestion = None;
      state.last_completed_key = None;
    } else {
      state.retry_at = None;
      state.last_error = None;
    }
    state.enabled
  };

  if enabled {
    ctx.push_info(SOURCE, "Inline completions enabled");
  } else {
    clear_inline_completion_surface(ctx);
    ctx.push_info(SOURCE, "Inline completions disabled");
  }
}

pub fn start_copilot_sign_in<Ctx: DefaultContext>(ctx: &mut Ctx) {
  set_active_provider(ctx, InlineCompletionProvider::Copilot, false);
  if ensure_transport(ctx).is_err() {
    return;
  }

  let workspace_root = ctx.workspace_root();
  let send_result = {
    let state = ctx.inline_completion_mut();
    state.status = InlineCompletionBackendStatus::Starting;
    state.last_error = None;
    state.auth_prompt = None;
    match state.transport.as_ref() {
      Some(transport) => {
        transport
          .tx
          .send(WorkerCommand::CopilotSignIn { workspace_root })
          .map_err(|error| format!("failed to send Copilot sign-in request to worker: {error}"))
      },
      None => Err("Copilot transport is unavailable".to_string()),
    }
  };

  match send_result {
    Ok(()) => {
      ctx.push_info(
        SOURCE,
        "Starting Copilot sign-in. If your browser does not open, wait for the device code and visit https://github.com/login/device.",
      );
      ctx.request_render();
    },
    Err(error) => apply_worker_event(ctx, WorkerEvent::Error(error)),
  }
}

pub fn supermaven_use_free<Ctx: DefaultContext>(ctx: &mut Ctx) {
  set_active_provider(ctx, InlineCompletionProvider::Supermaven, false);
  if ensure_transport(ctx).is_err() {
    return;
  }
  let workspace_root = ctx.workspace_root();

  let send_result = {
    let state = ctx.inline_completion_mut();
    state.status = InlineCompletionBackendStatus::Starting;
    state.last_error = None;
    match state.transport.as_ref() {
      Some(transport) => {
        transport
          .tx
          .send(WorkerCommand::SupermavenUseFree { workspace_root })
          .map_err(|error| format!("failed to send Supermaven free-tier request: {error}"))
      },
      None => Err("Supermaven transport is unavailable".to_string()),
    }
  };

  match send_result {
    Ok(()) => {
      ctx.push_info(SOURCE, "Requested Supermaven free tier.");
      ctx.request_render();
    },
    Err(error) => apply_worker_event(ctx, WorkerEvent::Error(error)),
  }
}

pub fn supermaven_use_pro<Ctx: DefaultContext>(ctx: &mut Ctx) {
  set_active_provider(ctx, InlineCompletionProvider::Supermaven, false);
  if ensure_transport(ctx).is_err() {
    return;
  }
  let existing_url = ctx.inline_completion().activation_url.clone();
  if let Some(url) = existing_url.filter(|url| !url.trim().is_empty()) {
    ctx.push_info(SOURCE, format!("Supermaven activation URL: {url}"));
    return;
  }
  let workspace_root = ctx.workspace_root();

  let send_result = {
    let state = ctx.inline_completion_mut();
    match state.transport.as_ref() {
      Some(transport) => {
        transport
          .tx
          .send(WorkerCommand::SupermavenUsePro { workspace_root })
          .map_err(|error| format!("failed to request Supermaven activation link: {error}"))
      },
      None => Err("Supermaven transport is unavailable".to_string()),
    }
  };

  match send_result {
    Ok(()) => ctx.request_render(),
    Err(error) => apply_worker_event(ctx, WorkerEvent::Error(error)),
  }
}

pub fn supermaven_logout<Ctx: DefaultContext>(ctx: &mut Ctx) {
  set_active_provider(ctx, InlineCompletionProvider::Supermaven, false);
  if ensure_transport(ctx).is_err() {
    return;
  }
  let workspace_root = ctx.workspace_root();

  let send_result = {
    let state = ctx.inline_completion_mut();
    match state.transport.as_ref() {
      Some(transport) => {
        transport
          .tx
          .send(WorkerCommand::SupermavenLogout { workspace_root })
          .map_err(|error| format!("failed to send Supermaven logout request: {error}"))
      },
      None => Err("Supermaven transport is unavailable".to_string()),
    }
  };

  match send_result {
    Ok(()) => {
      let state = ctx.inline_completion_mut();
      state.account_user = None;
      state.service_tier = None;
      state.auth_prompt = None;
      state.activation_url = None;
      state.suggestion = None;
      state.last_completed_key = None;
      state.presentation = None;
      ctx.clear_inline_completion_annotations();
      ctx.push_info(SOURCE, "Logged out of Supermaven");
    },
    Err(error) => apply_worker_event(ctx, WorkerEvent::Error(error)),
  }
}

pub fn retry_inline_completion<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let state = ctx.inline_completion_mut();
  state.retry_at = None;
  state.last_error = None;
  state.auth_prompt = None;
  state.activation_url = None;
  state.last_completed_key = None;
  state.scheduled = None;
  state.in_flight = None;
  state.suggestion = None;
  state.presentation = None;
  state.status = if state.transport.is_some() {
    InlineCompletionBackendStatus::Ready
  } else {
    InlineCompletionBackendStatus::Idle
  };
  clear_inline_completion_surface(ctx);
  ctx.push_info(SOURCE, "Inline backend retry cleared");
}

pub fn dismiss_inline_completion<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let dismissed = {
    let state = ctx.inline_completion_mut();
    let dismissed = state.suggestion.take().is_some() || state.presentation.take().is_some();
    if dismissed {
      state.last_completed_key = None;
    }
    dismissed
  };
  if dismissed {
    clear_inline_completion_surface(ctx);
    ctx.request_render();
  }
  dismissed
}

pub fn accept_inline_completion<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  accept_inline_completion_kind(ctx, AcceptKind::Full)
}

pub fn accept_inline_completion_word<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  accept_inline_completion_kind(ctx, AcceptKind::Word)
}

pub fn accept_inline_completion_line<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  accept_inline_completion_kind(ctx, AcceptKind::Line)
}

fn maybe_report_copilot_display<Ctx: DefaultContext>(ctx: &mut Ctx, suggestion: &InlineSuggestion) {
  if !suggestion.source.is_copilot() || suggestion.reported_to_provider {
    return;
  }
  let Some(range) = suggestion.display_range.clone() else {
    return;
  };

  let send_result = {
    let state = ctx.inline_completion_mut();
    let Some(transport) = state.transport.as_ref() else {
      return;
    };
    transport
      .tx
      .send(WorkerCommand::CopilotDidShow {
        source: suggestion.source,
        text: suggestion.text.clone(),
        range,
        command: suggestion.command.clone(),
      })
      .map_err(|error| format!("failed to send Copilot display notification: {error}"))
  };

  match send_result {
    Ok(()) => {
      inline_completion_trace(
        "copilot_did_show_sent",
        json!({
          "suggestion": inline_completion_trace_suggestion(suggestion),
        }),
      );
      if let Some(active) = ctx.inline_completion_mut().suggestion.as_mut()
        && suggestions_match_identity(active, suggestion)
      {
        active.reported_to_provider = true;
      }
    },
    Err(error) => apply_worker_event(ctx, WorkerEvent::Error(error)),
  }
}

fn maybe_report_copilot_accept<Ctx: DefaultContext>(ctx: &mut Ctx, suggestion: &InlineSuggestion) {
  if !suggestion.source.is_copilot() {
    return;
  }
  let Some(command) = suggestion.command.clone() else {
    return;
  };

  let send_result = {
    let state = ctx.inline_completion_mut();
    let Some(transport) = state.transport.as_ref() else {
      return;
    };
    transport
      .tx
      .send(WorkerCommand::CopilotAccept { command })
      .map_err(|error| format!("failed to send Copilot accept notification: {error}"))
  };

  if let Err(error) = send_result {
    apply_worker_event(ctx, WorkerEvent::Error(error));
  }
}

fn accept_inline_completion_kind<Ctx: DefaultContext>(ctx: &mut Ctx, kind: AcceptKind) -> bool {
  let suggestion = ctx.inline_completion().suggestion.clone();
  let Some(suggestion) = suggestion else {
    return false;
  };

  if ctx.completion_menu().active {
    crate::close_completion_menu(ctx);
  }

  let accepted = match kind {
    AcceptKind::Full => suggestion.text.clone(),
    AcceptKind::Word => next_word_fragment(&suggestion.text),
    AcceptKind::Line => next_line_fragment(&suggestion.text),
  };
  if accepted.is_empty() {
    return false;
  }

  let cursor = suggestion.from.saturating_add(accepted.chars().count());
  let tx = match Transaction::change(ctx.editor_ref().document().text(), vec![(
    suggestion.from,
    suggestion.to,
    Some(accepted.into()),
  )]) {
    Ok(tx) => tx.with_selection(Selection::point(cursor)),
    Err(error) => {
      ctx.push_error(
        SOURCE,
        format!("failed to build accept transaction: {error}"),
      );
      return false;
    },
  };

  if !ctx.apply_transaction(&tx) {
    ctx.push_error(SOURCE, "failed to apply inline suggestion");
    return false;
  }

  let _ = ctx.editor().document_mut().commit();
  if kind == AcceptKind::Full {
    maybe_report_copilot_accept(ctx, &suggestion);
  }
  {
    let state = ctx.inline_completion_mut();
    state.suggestion = None;
    state.presentation = None;
    state.last_completed_key = None;
  }
  clear_inline_completion_surface(ctx);
  ctx.request_render();
  true
}

fn inline_status_message_for_state(state: &InlineCompletionState) -> String {
  if !state.enabled {
    "Inline completions are disabled".to_string()
  } else if state.provider == InlineCompletionProvider::None {
    "No inline provider selected. Use :inline-provider copilot or :inline-provider supermaven."
      .to_string()
  } else if let Some(error) = state.last_error.as_ref() {
    format!("{} backend error: {error}", state.provider.label())
  } else if let Some(prompt) = state.auth_prompt.as_ref() {
    format!("{} setup required: {prompt}", state.provider.label())
  } else {
    let mut base = match (state.provider, state.status, state.suggestion.as_ref()) {
      (InlineCompletionProvider::Copilot, InlineCompletionBackendStatus::Starting, _) => {
        "Copilot backend is starting".to_string()
      },
      (InlineCompletionProvider::Supermaven, InlineCompletionBackendStatus::Starting, _) => {
        "Supermaven backend is starting".to_string()
      },
      (_, InlineCompletionBackendStatus::Ready, Some(suggestion)) => {
        format!(
          "{} ready: active suggestion {} chars",
          state.provider.label(),
          suggestion.text.chars().count()
        )
      },
      (_, InlineCompletionBackendStatus::Ready, None) => {
        format!("{} ready: no active suggestion", state.provider.label())
      },
      (InlineCompletionProvider::Copilot, InlineCompletionBackendStatus::Idle, _) => {
        "Copilot idle. Run :copilot-sign-in to connect GitHub Copilot.".to_string()
      },
      (InlineCompletionProvider::Supermaven, InlineCompletionBackendStatus::Idle, _) => {
        "Supermaven idle. Run :supermaven-use-free or :supermaven-use-pro.".to_string()
      },
      (_, InlineCompletionBackendStatus::Error, _) => {
        format!("{} backend is in an error state", state.provider.label())
      },
      (InlineCompletionProvider::None, ..) => "No inline provider selected.".to_string(),
    };
    if state.provider == InlineCompletionProvider::Supermaven
      && let Some(url) = state
        .activation_url
        .as_ref()
        .filter(|url| !url.trim().is_empty())
    {
      base.push_str(&format!(" (pro activation URL available: {url})"));
    }
    if let Some(user) = state.account_user.as_ref() {
      base.push_str(&format!(" (user: {user})"));
    }
    if let Some(tier) = state.service_tier.as_ref() {
      base.push_str(&format!(" [{tier}]"));
    }
    base
  }
}

pub fn has_active_suggestion(state: &InlineCompletionState) -> bool {
  state.suggestion.is_some()
}

pub fn debug_summary(
  state: &InlineCompletionState,
  annotations_visible: bool,
  has_multiline: bool,
) -> String {
  let presentation_kind = state.presentation.as_ref().map(|presentation| {
    match presentation.kind {
      InlineCompletionPresentationKind::Menu => "menu",
      InlineCompletionPresentationKind::Popover => "popover",
      InlineCompletionPresentationKind::Diff => "diff",
    }
  });
  let presentation_line_count = state
    .presentation
    .as_ref()
    .map(|presentation| presentation.lines.len())
    .unwrap_or(0);
  let mut parts = Vec::new();
  parts.push(format!("enabled={}", state.enabled));
  parts.push(format!("provider={}", state.provider.label()));
  parts.push(format!("status={:?}", state.status).to_ascii_lowercase());
  parts.push(format!("suggestion={}", state.suggestion.is_some()));
  parts.push(format!("presentation={}", state.presentation.is_some()));
  parts.push(format!(
    "presentation_kind={}",
    presentation_kind.unwrap_or("none")
  ));
  parts.push(format!("presentation_lines={presentation_line_count}"));
  parts.push(format!("annotations={}", annotations_visible));
  parts.push(format!("multiline={}", has_multiline));
  if let Some(user) = state.account_user.as_ref() {
    parts.push(format!("user={user}"));
  }
  if let Some(tier) = state.service_tier.as_ref() {
    parts.push(format!("tier={tier}"));
  }
  let headline = parts.join(" · ");
  format!("{}\n{}", headline, inline_status_message_for_state(state))
}

pub fn debug_context_summary<Ctx: DefaultContext>(
  ctx: &Ctx,
  annotations_visible: bool,
  has_multiline: bool,
) -> String {
  let state = ctx.inline_completion();
  let presentation_kind = state.presentation.as_ref().map(|presentation| {
    match presentation.kind {
      InlineCompletionPresentationKind::Menu => "menu",
      InlineCompletionPresentationKind::Popover => "popover",
      InlineCompletionPresentationKind::Diff => "diff",
    }
  });
  let presentation_line_count = state
    .presentation
    .as_ref()
    .map(|presentation| presentation.lines.len())
    .unwrap_or(0);
  let mut parts = Vec::new();
  parts.push(format!("enabled={}", state.enabled));
  parts.push(format!("provider={}", state.provider.label()));
  parts.push(format!("status={:?}", state.status).to_ascii_lowercase());
  parts.push(format!("suggestion={}", state.suggestion.is_some()));
  parts.push(format!("presentation={}", state.presentation.is_some()));
  parts.push(format!(
    "presentation_kind={}",
    presentation_kind.unwrap_or("none")
  ));
  parts.push(format!("presentation_lines={presentation_line_count}"));
  parts.push(format!("annotations={}", annotations_visible));
  parts.push(format!("multiline={}", has_multiline));
  parts.push(format!("mode={:?}", ctx.mode()).to_ascii_lowercase());
  parts.push(format!("file={}", ctx.file_path().is_some()));
  let editor = ctx.editor_ref();
  parts.push(format!(
    "active_cursor={}",
    editor.view().active_cursor.is_some()
  ));
  let selection = editor.document().selection();
  parts.push(format!("ranges={}", selection.ranges().len()));
  match current_prepared_query_result(ctx) {
    Ok(prepared) => {
      parts.push("eligible=true".to_string());
      parts.push(format!("cursor_char={}", prepared.key.cursor_char));
      parts.push(format!("doc_version={}", prepared.key.doc_version));
    },
    Err(reason) => {
      parts.push("eligible=false".to_string());
      parts.push(format!("reason={}", reason.label()));
    },
  }
  parts.push(format!("scheduled={}", state.scheduled.is_some()));
  parts.push(format!("in_flight={}", state.in_flight.is_some()));
  parts.push(format!("completed={}", state.last_completed_key.is_some()));
  if let Some(user) = state.account_user.as_ref() {
    parts.push(format!("user={user}"));
  }
  if let Some(tier) = state.service_tier.as_ref() {
    parts.push(format!("tier={tier}"));
  }
  let headline = parts.join(" · ");
  format!("{}\n{}", headline, inline_status_message_for_state(state))
}

pub fn show_inline_status<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let state = ctx.inline_completion();
  ctx.push_info(SOURCE, inline_status_message_for_state(state));
}

fn set_active_provider<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  provider: InlineCompletionProvider,
  announce: bool,
) {
  let changed = {
    let state = ctx.inline_completion_mut();
    if state.provider == provider {
      false
    } else {
      state.provider = provider;
      reset_backend_runtime(state);
      true
    }
  };

  clear_inline_completion_surface(ctx);
  if announce {
    if changed {
      ctx.push_info(
        SOURCE,
        format!("Inline completion provider set to {}", provider.label()),
      );
    } else {
      ctx.push_info(
        SOURCE,
        format!("Inline completion provider is already {}", provider.label()),
      );
    }
  }
}

fn drain_worker_events<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let mut events = Vec::new();
  {
    let state = ctx.inline_completion();
    let Some(transport) = state.transport.as_ref() else {
      return;
    };
    loop {
      match transport.rx.try_recv() {
        Ok(event) => events.push(event),
        Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
      }
    }
  }

  for event in events {
    apply_worker_event(ctx, event);
  }
}

fn apply_worker_event<Ctx: DefaultContext>(ctx: &mut Ctx, event: WorkerEvent) {
  match event {
    WorkerEvent::Ready => {
      inline_completion_trace("worker_ready", json!({}));
      let state = ctx.inline_completion_mut();
      state.status = InlineCompletionBackendStatus::Ready;
      state.last_error = None;
      state.retry_at = None;
    },
    WorkerEvent::AuthPrompt { message } => {
      inline_completion_trace(
        "worker_auth_prompt",
        json!({
          "message": &message,
        }),
      );
      {
        let state = ctx.inline_completion_mut();
        state.status = InlineCompletionBackendStatus::Starting;
        state.auth_prompt = Some(message.clone());
        state.last_error = None;
        state.retry_at = None;
      }
      ctx.push_info(SOURCE, message);
    },
    WorkerEvent::ActivationUrl(url) => {
      inline_completion_trace(
        "worker_activation_url",
        json!({
          "url": &url,
        }),
      );
      let state = ctx.inline_completion_mut();
      state.activation_url = Some(url.clone());
      ctx.push_info(SOURCE, format!("Supermaven activation URL: {url}"));
    },
    WorkerEvent::Authenticated { user } => {
      inline_completion_trace(
        "worker_authenticated",
        json!({
          "user": user.as_ref(),
        }),
      );
      {
        let state = ctx.inline_completion_mut();
        state.status = InlineCompletionBackendStatus::Ready;
        state.auth_prompt = None;
        state.activation_url = None;
        state.account_user = user.clone();
        state.last_error = None;
        state.retry_at = None;
      }
      match user {
        Some(user) => ctx.push_info(SOURCE, format!("Connected as {user}")),
        None => ctx.push_info(SOURCE, "Inline backend connected"),
      };
    },
    WorkerEvent::Status(message) => {
      inline_completion_trace(
        "worker_status",
        json!({
          "message": &message,
        }),
      );
      let state = ctx.inline_completion_mut();
      state.service_tier = Some(message.clone());
      state.activation_url = None;
      ctx.push_info(SOURCE, message);
    },
    WorkerEvent::Error(error) => {
      inline_completion_trace(
        "worker_error",
        json!({
          "error": &error,
        }),
      );
      let mut report = None;
      {
        let state = ctx.inline_completion_mut();
        state.status = InlineCompletionBackendStatus::Error;
        state.last_error = Some(error.clone());
        state.in_flight = None;
        state.suggestion = None;
        state.presentation = None;
        state.retry_at = Some(Instant::now() + RETRY_AFTER_ERROR);
        if state.last_reported_error.as_deref() != Some(error.as_str()) {
          state.last_reported_error = Some(error.clone());
          report = Some(error);
        }
      }
      clear_inline_completion_surface(ctx);
      ctx.render_waker().wake_after(RETRY_AFTER_ERROR);
      if let Some(error) = report {
        ctx.push_error(SOURCE, error);
      }
    },
    WorkerEvent::QueryResult { request_id, result } => {
      let ticket = {
        let state = ctx.inline_completion_mut();
        match state.in_flight.take() {
          Some(ticket) if ticket.request_id == request_id => {
            state.last_completed_key = Some(ticket.key.clone());
            Some(ticket)
          },
          Some(ticket) => {
            state.in_flight = Some(ticket);
            None
          },
          None => None,
        }
      };
      let Some(ticket) = ticket else {
        inline_completion_trace(
          "query_result_ignored_no_ticket",
          json!({
            "request_id": request_id,
          }),
        );
        return;
      };

      match result {
        Ok(worker_suggestion) => {
          inline_completion_trace(
            "query_result_received",
            json!({
              "request_id": request_id,
              "ticket": inline_completion_trace_query_key(&ticket.key),
              "suggestion": worker_suggestion
                .as_ref()
                .map(inline_completion_trace_worker_suggestion)
                .unwrap_or(Value::Null),
            }),
          );
          let next = worker_suggestion.and_then(|raw| {
            let prepared = match current_prepared_query_result(ctx) {
              Ok(prepared) => prepared,
              Err(reason) => {
                inline_completion_trace(
                  "query_result_dropped_ineligible",
                  json!({
                    "request_id": request_id,
                    "ticket": inline_completion_trace_query_key(&ticket.key),
                    "reason": reason.label(),
                    "suggestion": inline_completion_trace_worker_suggestion(&raw),
                  }),
                );
                return None;
              },
            };
            if prepared.key != ticket.key {
              inline_completion_trace(
                "query_result_dropped_stale",
                json!({
                  "request_id": request_id,
                  "ticket": inline_completion_trace_query_key(&ticket.key),
                  "current": inline_completion_trace_query_key(&prepared.key),
                  "suggestion": inline_completion_trace_worker_suggestion(&raw),
                }),
              );
              return None;
            }
            if !suggestion_is_cursor_local(ticket.key.cursor_char, raw.from, raw.to) {
              inline_completion_trace(
                "query_result_dropped_non_local_inline_completion",
                json!({
                  "request_id": request_id,
                  "ticket": inline_completion_trace_query_key(&ticket.key),
                  "suggestion": inline_completion_trace_worker_suggestion(&raw),
                }),
              );
              return None;
            }
            Some(InlineSuggestion {
              key:                  ticket.key.clone(),
              from:                 raw.from,
              to:                   raw.to,
              text:                 raw.text,
              source:               raw.source,
              command:              raw.command,
              display_range:        raw.display_range,
              reported_to_provider: false,
            })
          });
          let state = ctx.inline_completion_mut();
          state.status = InlineCompletionBackendStatus::Ready;
          state.suggestion = next;
          if state.suggestion.is_none() {
            state.presentation = None;
          }
          inline_completion_trace(
            "query_result_applied",
            json!({
              "request_id": request_id,
              "ticket": inline_completion_trace_query_key(&ticket.key),
              "suggestion": state
                .suggestion
                .as_ref()
                .map(inline_completion_trace_suggestion)
                .unwrap_or(Value::Null),
            }),
          );
        },
        Err(error) => apply_worker_event(ctx, WorkerEvent::Error(error)),
      }
    },
  }
}

fn schedule_or_send_query<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let now = Instant::now();
  let prepared = current_prepared_query(ctx);
  let mut to_send = None;
  let mut wake_after = None;
  let mut blocked_by_retry = false;

  {
    let state = ctx.inline_completion_mut();
    if !state.enabled {
      state.scheduled = None;
      state.in_flight = None;
      state.suggestion = None;
      state.presentation = None;
      ctx.clear_inline_completion_annotations();
      return;
    }

    if let Some(retry_at) = state.retry_at
      && now < retry_at
    {
      wake_after = Some(retry_at.saturating_duration_since(now));
      blocked_by_retry = true;
    } else {
      let Some(prepared) = prepared else {
        state.scheduled = None;
        state.suggestion = None;
        state.presentation = None;
        state.last_completed_key = None;
        return;
      };

      if state
        .suggestion
        .as_ref()
        .is_some_and(|suggestion| suggestion.key != prepared.key)
      {
        state.suggestion = None;
        state.presentation = None;
      }

      if state
        .in_flight
        .as_ref()
        .is_some_and(|ticket| ticket.key == prepared.key)
      {
      } else if state
        .suggestion
        .as_ref()
        .is_some_and(|suggestion| suggestion.key == prepared.key)
      {
        return;
      } else if state
        .last_completed_key
        .as_ref()
        .is_some_and(|key| key == &prepared.key)
      {
        return;
      } else if state
        .scheduled
        .as_ref()
        .is_some_and(|scheduled| scheduled.prepared.key == prepared.key)
      {
        if state
          .scheduled
          .as_ref()
          .is_some_and(|scheduled| scheduled.ready_at <= now)
          && state.in_flight.is_none()
        {
          to_send = state.scheduled.take().map(|scheduled| scheduled.prepared);
        } else if let Some(scheduled) = state.scheduled.as_ref() {
          wake_after = Some(scheduled.ready_at.saturating_duration_since(now));
        }
      } else {
        state.scheduled = Some(ScheduledQuery {
          prepared,
          ready_at: now + DEBOUNCE,
        });
        wake_after = Some(DEBOUNCE);
      }
    }
  }

  if blocked_by_retry {
    if let Some(delay) = wake_after
      && !delay.is_zero()
    {
      ctx.render_waker().wake_after(delay);
    }
    return;
  }

  if let Some(prepared) = to_send {
    send_query(ctx, prepared);
  }

  if let Some(delay) = wake_after
    && !delay.is_zero()
  {
    ctx.render_waker().wake_after(delay);
  }
}

fn send_query<Ctx: DefaultContext>(ctx: &mut Ctx, prepared: PreparedQuery) {
  if ensure_transport(ctx).is_err() {
    return;
  }

  let request_id = {
    let state = ctx.inline_completion_mut();
    let request_id = state.next_request_id;
    state.next_request_id = state.next_request_id.saturating_add(1);
    state.in_flight = Some(InFlightQuery {
      request_id,
      key: prepared.key.clone(),
    });
    request_id
  };

  inline_completion_trace(
    "query_send",
    json!({
      "request_id": request_id,
      "key": inline_completion_trace_query_key(&prepared.key),
      "request": inline_completion_trace_request(&prepared.request),
    }),
  );

  let send_result = {
    let state = ctx.inline_completion_mut();
    match state.transport.as_ref() {
      Some(transport) => {
        transport
          .tx
          .send(WorkerCommand::Query {
            request_id,
            request: prepared.request,
          })
          .map_err(|error| {
            format!(
              "failed to send {} query to worker: {error}",
              transport.provider.label()
            )
          })
      },
      None => Err("inline transport is unavailable".to_string()),
    }
  };

  if let Err(error) = send_result {
    apply_worker_event(ctx, WorkerEvent::Error(error));
  }
}

fn ensure_transport<Ctx: DefaultContext>(ctx: &mut Ctx) -> Result<(), ()> {
  let selected_provider = ctx.inline_completion().provider;
  if selected_provider == InlineCompletionProvider::None {
    return Err(());
  }

  let already_started = ctx
    .inline_completion()
    .transport
    .as_ref()
    .is_some_and(|transport| transport.provider == selected_provider);
  if already_started {
    return Ok(());
  }

  {
    let state = ctx.inline_completion_mut();
    state.transport = None;
  }

  let (tx, rx) = mpsc::channel();
  let (event_tx, event_rx) = mpsc::channel();
  let waker = ctx.render_waker();
  let thread_name = format!("the-editor-{}", selected_provider.label());
  let spawn_result = thread::Builder::new()
    .name(thread_name)
    .spawn(move || worker_main(selected_provider, rx, event_tx, waker));

  let handle = match spawn_result {
    Ok(handle) => handle,
    Err(error) => {
      apply_worker_event(
        ctx,
        WorkerEvent::Error(format!(
          "failed to spawn {} worker thread: {error}",
          selected_provider.label()
        )),
      );
      return Err(());
    },
  };
  std::mem::forget(handle);

  let state = ctx.inline_completion_mut();
  state.transport = Some(InlineCompletionTransport {
    provider: selected_provider,
    tx,
    rx: event_rx,
  });
  state.status = InlineCompletionBackendStatus::Starting;
  Ok(())
}

fn current_prepared_query<Ctx: DefaultContext>(ctx: &Ctx) -> Option<PreparedQuery> {
  current_prepared_query_result(ctx).ok()
}

fn current_prepared_query_result<Ctx: DefaultContext>(
  ctx: &Ctx,
) -> Result<PreparedQuery, PreparedQueryIneligibility> {
  if ctx.mode() != Mode::Insert {
    return Err(PreparedQueryIneligibility::NotInsertMode);
  }

  let selected_provider = ctx.inline_completion().provider;
  if selected_provider == InlineCompletionProvider::None {
    return Err(PreparedQueryIneligibility::NoProvider);
  }

  let path = ctx
    .file_path()
    .map(Path::to_path_buf)
    .ok_or(PreparedQueryIneligibility::NoFilePath)?;
  let editor = ctx.editor_ref();
  let document = editor.document();
  let selection = document.selection();
  let range = single_active_range(selection, editor.view().active_cursor)
    .ok_or(PreparedQueryIneligibility::InvalidSelection)?;
  if !range.is_empty() {
    return Err(PreparedQueryIneligibility::NonEmptySelection);
  }

  let cursor_char = range.head;
  let text = document.text();
  let key = QueryKey {
    buffer_id: editor.active_buffer_id(),
    doc_version: document.version(),
    cursor_char,
    file_path: path.clone(),
  };

  let request = match selected_provider {
    InlineCompletionProvider::None => return Err(PreparedQueryIneligibility::NoProvider),
    InlineCompletionProvider::Copilot => {
      let uri = file_uri_for_path(&path).ok_or(PreparedQueryIneligibility::NoFilePath)?;
      let (line, character) = char_idx_to_utf16_position(text, cursor_char);
      let (tab_size, insert_spaces) = formatting_options(document.indent_style());
      let language_id = language_id_for_path(ctx, &path);
      ProviderQuery::Copilot(CopilotQuery {
        workspace_root: ctx.workspace_root(),
        uri,
        language_id,
        version: document.version().min(i32::MAX as u64) as i32,
        text: text.to_string(),
        line,
        character,
        tab_size,
        insert_spaces,
      })
    },
    InlineCompletionProvider::Supermaven => {
      let cursor_line = text.char_to_line(cursor_char);
      let line_start = text.line_to_char(cursor_line);
      let line_end = line_start + line_char_len_without_newline(text.line(cursor_line));
      let line_text = text.line(cursor_line).to_string();
      let line_without_newline = line_text.trim_end_matches(['\r', '\n']).to_string();
      let cursor_col = cursor_char.saturating_sub(line_start);
      let line_before = line_without_newline
        .chars()
        .take(cursor_col)
        .collect::<String>();
      let line_after = line_without_newline
        .chars()
        .skip(cursor_col)
        .collect::<String>();
      let following_lines = (cursor_line + 1..text.len_lines())
        .take(64)
        .map(|idx| {
          text
            .line(idx)
            .to_string()
            .trim_end_matches(['\r', '\n'])
            .to_string()
        })
        .collect();
      ProviderQuery::Supermaven(SupermavenQuery {
        workspace_root: ctx.workspace_root(),
        file_path: path,
        text: text.to_string(),
        prefix: text.slice(..cursor_char).to_string(),
        cursor_char,
        line_before,
        line_after,
        following_lines,
        line_end_char: line_end,
      })
    },
  };

  Ok(PreparedQuery { key, request })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuggestionDisplayKind {
  Ghost { anchor_char: usize },
}

fn clear_inline_completion_surface<Ctx: DefaultContext>(ctx: &mut Ctx) {
  ctx.clear_inline_completion_annotations();
  ctx.inline_completion_mut().presentation = None;
}

fn suggestions_match_identity(left: &InlineSuggestion, right: &InlineSuggestion) -> bool {
  left.key == right.key
    && left.from == right.from
    && left.to == right.to
    && left.text == right.text
    && left.source == right.source
}

fn suggestion_is_cursor_local(cursor_char: usize, from: usize, to: usize) -> bool {
  cursor_char >= from && cursor_char <= to.max(from)
}

fn classify_suggestion<Ctx: DefaultContext>(
  ctx: &Ctx,
  suggestion: &InlineSuggestion,
) -> Option<SuggestionDisplayKind> {
  let editor = ctx.editor_ref();
  let document = editor.document();
  let selection = document.selection();
  let range = single_active_range(selection, editor.view().active_cursor)?;
  if !range.is_empty() || range.head != suggestion.key.cursor_char {
    return None;
  }

  let cursor_char = range.head;
  let max_char = document.text().len_chars();
  let from = suggestion.from.min(max_char);
  let to = suggestion.to.min(max_char);
  if suggestion_is_cursor_local(cursor_char, from, to) {
    Some(SuggestionDisplayKind::Ghost {
      anchor_char: cursor_char,
    })
  } else {
    None
  }
}

fn inline_completion_provider_display_name(source: InlineSuggestionSource) -> &'static str {
  match source {
    InlineSuggestionSource::CopilotInlineCompletion => "Copilot",
    InlineSuggestionSource::Supermaven => "Supermaven",
  }
}

fn inline_completion_provider_icon(source: InlineSuggestionSource) -> &'static str {
  match source {
    InlineSuggestionSource::CopilotInlineCompletion => "copilot",
    InlineSuggestionSource::Supermaven => "supermaven",
  }
}

fn inline_completion_provider_color(source: InlineSuggestionSource) -> Color {
  match source {
    InlineSuggestionSource::CopilotInlineCompletion => Color::Rgb(0x8B, 0xB6, 0xFF),
    InlineSuggestionSource::Supermaven => Color::Rgb(0xF2, 0xC1, 0x63),
  }
}

fn completion_menu_summary_label(text: &str) -> Option<String> {
  let first = text.lines().find(|line| !line.trim().is_empty())?;
  Some(truncate_completion_menu_label(first))
}

fn truncate_completion_menu_label(text: &str) -> String {
  let mut out = text.trim().to_string();
  if out.chars().count() > 52 {
    out = out.chars().take(49).collect::<String>();
    out.push_str("...");
  }
  out
}

pub fn completion_menu_inline_item<Ctx: DefaultContext>(ctx: &Ctx) -> Option<CompletionMenuItem> {
  let suggestion = ctx.inline_completion().suggestion.as_ref()?;
  classify_suggestion(ctx, suggestion)?;
  let label = completion_menu_summary_label(&suggestion.text)?;
  Some(
    CompletionMenuItem::new(label)
      .detail(inline_completion_provider_display_name(suggestion.source))
      .documentation(suggestion.text.clone())
      .kind(
        inline_completion_provider_icon(suggestion.source),
        inline_completion_provider_color(suggestion.source),
      ),
  )
}

fn sync_inline_completion_annotations<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if ctx.mode() != Mode::Insert {
    clear_inline_completion_surface(ctx);
    return;
  }

  let suggestion = ctx.inline_completion().suggestion.clone();
  let Some(suggestion) = suggestion else {
    clear_inline_completion_surface(ctx);
    return;
  };

  let Some(display_kind) = classify_suggestion(ctx, &suggestion) else {
    inline_completion_trace(
      "surface_clear_no_display_kind",
      json!({
        "suggestion": inline_completion_trace_suggestion(&suggestion),
      }),
    );
    clear_inline_completion_surface(ctx);
    return;
  };
  maybe_report_copilot_display(ctx, &suggestion);

  let SuggestionDisplayKind::Ghost { anchor_char } = display_kind;
  let Some(predicted_edit) = predicted_edit_for_suggestion(ctx, &suggestion, anchor_char) else {
    inline_completion_trace(
      "surface_ghost_skipped",
      json!({
        "display": inline_completion_trace_display_kind(display_kind),
        "suggestion": inline_completion_trace_suggestion(&suggestion),
        "reason": "predicted_edit_none",
      }),
    );
    clear_inline_completion_surface(ctx);
    return;
  };

  let mut annotations = preview_annotations_for_predicted_edit(ctx, &predicted_edit);
  let virtual_line_count = annotations.virtual_lines().len();
  let annotations_visible = !annotations.is_empty();
  let presentation = predicted_edit.presentation.clone();
  let presentation_kind = presentation.as_ref().map(|presentation| {
    match presentation.kind {
      InlineCompletionPresentationKind::Menu => "menu",
      InlineCompletionPresentationKind::Popover => "popover",
      InlineCompletionPresentationKind::Diff => "diff",
    }
  });
  let presentation_line_count = presentation
    .as_ref()
    .map(|presentation| presentation.lines.len())
    .unwrap_or(0);

  if annotations_visible {
    ctx.set_inline_completion_annotations(std::mem::take(&mut annotations));
  } else {
    ctx.clear_inline_completion_annotations();
  }
  ctx.inline_completion_mut().presentation = presentation;

  if !annotations_visible && presentation_line_count == 0 {
    inline_completion_trace(
      "surface_ghost_skipped",
      json!({
        "display": inline_completion_trace_display_kind(display_kind),
        "suggestion": inline_completion_trace_suggestion(&suggestion),
        "reason": "surface_empty",
        "display_mode": predicted_edit.display_mode.label(),
      }),
    );
    clear_inline_completion_surface(ctx);
    return;
  }

  inline_completion_trace(
    "surface_prediction_updated",
    json!({
      "display": inline_completion_trace_display_kind(display_kind),
      "suggestion": inline_completion_trace_suggestion(&suggestion),
      "source": predicted_edit.source.label(),
      "anchor_char": predicted_edit.anchor_char,
      "preview_char": predicted_edit.preview_char,
      "display_mode": predicted_edit.display_mode.label(),
      "is_pure_insertion": predicted_edit.is_pure_insertion(),
      "inserted_text_chars": predicted_edit.inserted_text.chars().count(),
      "replaced_text_chars": predicted_edit.replaced_text.chars().count(),
      "span_count": predicted_edit.spans.len(),
      "virtual_lines": virtual_line_count,
      "presentation_kind": presentation_kind,
      "presentation_line_count": presentation_line_count,
    }),
  );
}

fn predicted_edit_for_suggestion<Ctx: DefaultContext>(
  ctx: &Ctx,
  suggestion: &InlineSuggestion,
  anchor_char: usize,
) -> Option<InlinePredictedEdit> {
  let editor = ctx.editor_ref();
  let document = editor.document();
  let range = single_active_range(document.selection(), editor.view().active_cursor)?;
  if !range.is_empty() || range.head != suggestion.key.cursor_char {
    inline_completion_trace(
      "preview_annotations_skip_active_range",
      json!({
        "suggestion": inline_completion_trace_suggestion(suggestion),
        "selection_head": range.head,
        "selection_empty": range.is_empty(),
        "anchor_char": anchor_char,
      }),
    );
    return None;
  }

  let text = document.text().slice(..);
  let max_char = text.len_chars();
  let mut replace_from = suggestion.from.min(max_char);
  let mut replace_to = suggestion.to.min(max_char);
  if replace_from > replace_to {
    std::mem::swap(&mut replace_from, &mut replace_to);
  }

  let preview_char = anchor_char.min(max_char);
  let existing_prefix = if preview_char > replace_from {
    text
      .slice(replace_from..preview_char.min(max_char))
      .to_string()
  } else {
    String::new()
  };
  let preview_text = preview_text_for_cursor(existing_prefix, preview_char, suggestion);
  if preview_text.is_empty() {
    inline_completion_trace(
      "preview_annotations_skip_empty_text",
      json!({
        "suggestion": inline_completion_trace_suggestion(suggestion),
        "anchor_char": anchor_char,
        "preview_char": preview_char,
      }),
    );
    return None;
  }

  let replaced_text = text.slice(replace_from..replace_to).to_string();
  let spans = build_predicted_edit_spans(&replaced_text, &suggestion.text);
  let (first_line, remaining_raw) = split_preview_lines(&preview_text);
  let remaining_compacted = compact_preview_remaining_lines(&remaining_raw);
  let preview_total_chars = preview_text.chars().count();
  let preview_total_lines = preview_line_count(&preview_text);
  let can_inline_first_line = first_line.is_empty()
    || first_line.chars().count() <= MAX_INLINE_COMPLETION_SIMPLE_FIRST_LINE_CHARS;
  let display_mode = if replace_from == replace_to {
    if can_inline_first_line
      && preview_total_lines <= MAX_INLINE_COMPLETION_SIMPLE_TOTAL_LINES
      && preview_total_chars <= MAX_INLINE_COMPLETION_SIMPLE_TOTAL_CHARS
    {
      InlinePredictedEditDisplayMode::InlineGhost
    } else if can_inline_first_line {
      InlinePredictedEditDisplayMode::InlineGhostWithPopover
    } else {
      InlinePredictedEditDisplayMode::DiffPopover
    }
  } else {
    InlinePredictedEditDisplayMode::DiffPopover
  };

  let (remaining_inline_text, remaining_truncated_line_count) =
    if matches!(display_mode, InlinePredictedEditDisplayMode::InlineGhost) {
      clamp_preview_remaining_lines(
        &remaining_compacted,
        MAX_INLINE_COMPLETION_PREVIEW_REMAINING_LINES,
      )
    } else {
      (String::new(), 0)
    };

  let replacement_text = text.slice(preview_char..replace_to).to_string();
  let replacement_first_line_len = replacement_first_line_len(replacement_text.as_str());
  let inline_preview = match display_mode {
    InlinePredictedEditDisplayMode::InlineGhost => {
      if first_line.is_empty() && remaining_inline_text.is_empty() {
        None
      } else {
        Some(InlineGhostPreview {
          preview_char,
          replacement_first_line_len,
          first_line: first_line.clone(),
          remaining_text: remaining_inline_text.clone(),
        })
      }
    },
    InlinePredictedEditDisplayMode::InlineGhostWithPopover => {
      if first_line.is_empty() {
        None
      } else {
        Some(InlineGhostPreview {
          preview_char,
          replacement_first_line_len,
          first_line: first_line.clone(),
          remaining_text: String::new(),
        })
      }
    },
    InlinePredictedEditDisplayMode::DiffPopover => None,
  };

  let target_line = text.char_to_line(anchor_char.min(max_char));
  let target_char = anchor_char
    .min(max_char)
    .saturating_sub(text.line_to_char(target_line));
  let first_line_rendered = inline_preview
    .as_ref()
    .is_some_and(|preview| !preview.first_line.is_empty());
  let presentation = match display_mode {
    InlinePredictedEditDisplayMode::InlineGhost => None,
    InlinePredictedEditDisplayMode::InlineGhostWithPopover => {
      build_insert_presentation(
        suggestion.source,
        &preview_text,
        first_line_rendered,
        target_line,
        target_char,
      )
    },
    InlinePredictedEditDisplayMode::DiffPopover if replace_from == replace_to => {
      build_insert_presentation(
        suggestion.source,
        &preview_text,
        false,
        target_line,
        target_char,
      )
    },
    InlinePredictedEditDisplayMode::DiffPopover => {
      build_diff_presentation(
        suggestion.source,
        &replaced_text,
        &suggestion.text,
        target_line,
        target_char,
      )
    },
  };

  inline_completion_trace(
    "preview_annotations_built",
    json!({
      "suggestion": inline_completion_trace_suggestion(suggestion),
      "anchor_char": anchor_char,
      "preview_char": preview_char,
      "replace_from": replace_from,
      "replace_to": replace_to,
      "display_mode": display_mode.label(),
      "span_count": spans.len(),
      "preview_line": text.char_to_line(preview_char),
      "first_line_preview": inline_completion_trace_text_preview(&first_line),
      "has_remaining_lines": !remaining_inline_text.is_empty(),
      "remaining_raw_line_count": preview_line_count(&remaining_raw),
      "remaining_compacted_line_count": preview_line_count(&remaining_compacted),
      "remaining_displayed_line_count": preview_line_count(&remaining_inline_text),
      "remaining_blank_line_count": preview_blank_line_count(&remaining_raw),
      "remaining_truncated_line_count": remaining_truncated_line_count,
      "presentation_kind": presentation.as_ref().map(|presentation| match presentation.kind {
        InlineCompletionPresentationKind::Menu => "menu",
        InlineCompletionPresentationKind::Popover => "popover",
        InlineCompletionPresentationKind::Diff => "diff",
      }),
      "presentation_line_count": presentation
        .as_ref()
        .map(|presentation| presentation.lines.len())
        .unwrap_or(0),
    }),
  );

  Some(InlinePredictedEdit {
    source: suggestion.source,
    replace_from,
    replace_to,
    anchor_char,
    preview_char,
    inserted_text: suggestion.text.clone(),
    replaced_text,
    spans,
    display_mode,
    inline_preview,
    presentation,
  })
}

fn build_predicted_edit_spans(
  replaced_text: &str,
  inserted_text: &str,
) -> Vec<InlinePredictedEditSpan> {
  if replaced_text.is_empty() {
    return vec![InlinePredictedEditSpan {
      kind: InlinePredictedEditSpanKind::Addition,
      text: inserted_text.to_string(),
    }];
  }
  if inserted_text.is_empty() {
    return vec![InlinePredictedEditSpan {
      kind: InlinePredictedEditSpanKind::Removal,
      text: replaced_text.to_string(),
    }];
  }

  let (prefix_chars, prefix_bytes) = shared_prefix(replaced_text, inserted_text);
  let removed_remaining = &replaced_text[prefix_bytes..];
  let inserted_remaining = &inserted_text[prefix_bytes..];
  let (suffix_chars, suffix_bytes) = shared_suffix(removed_remaining, inserted_remaining);
  let removed_mid_end = replaced_text.len().saturating_sub(suffix_bytes);
  let inserted_mid_end = inserted_text.len().saturating_sub(suffix_bytes);

  let mut spans = Vec::new();
  if prefix_chars > 0 {
    spans.push(InlinePredictedEditSpan {
      kind: InlinePredictedEditSpanKind::Equal,
      text: replaced_text[..prefix_bytes].to_string(),
    });
  }
  if removed_mid_end > prefix_bytes {
    spans.push(InlinePredictedEditSpan {
      kind: InlinePredictedEditSpanKind::Removal,
      text: replaced_text[prefix_bytes..removed_mid_end].to_string(),
    });
  }
  if inserted_mid_end > prefix_bytes {
    spans.push(InlinePredictedEditSpan {
      kind: InlinePredictedEditSpanKind::Addition,
      text: inserted_text[prefix_bytes..inserted_mid_end].to_string(),
    });
  }
  if suffix_chars > 0 {
    let suffix_start = replaced_text.len().saturating_sub(suffix_bytes);
    spans.push(InlinePredictedEditSpan {
      kind: InlinePredictedEditSpanKind::Equal,
      text: replaced_text[suffix_start..].to_string(),
    });
  }
  if spans.is_empty() {
    spans.push(InlinePredictedEditSpan {
      kind: InlinePredictedEditSpanKind::Addition,
      text: inserted_text.to_string(),
    });
  }
  spans
}

fn preview_annotations_for_predicted_edit<Ctx: DefaultContext>(
  ctx: &Ctx,
  predicted_edit: &InlinePredictedEdit,
) -> OwnedTextAnnotations {
  let Some(preview) = predicted_edit.inline_preview.as_ref() else {
    return OwnedTextAnnotations::default();
  };

  let highlight = ctx.ui_theme().find_highlight("ui.virtual.inline");
  let text = ctx.editor_ref().document().text().slice(..);
  let preview_line = text.char_to_line(preview.preview_char);
  let mut annotations = OwnedTextAnnotations::default();

  if !preview.first_line.is_empty() {
    render_first_line_preview(
      &mut annotations,
      preview.preview_char,
      preview.replacement_first_line_len,
      &preview.first_line,
      highlight,
    );
  }

  if !preview.remaining_text.is_empty() {
    let _ = annotations.add_virtual_line(
      VirtualLineSpec::after(preview_line)
        .text(&preview.remaining_text)
        .highlight(highlight)
        .wrap_to_viewport(),
    );
  }

  annotations
}

fn build_insert_presentation(
  source: InlineSuggestionSource,
  preview_text: &str,
  first_line_rendered: bool,
  target_line: usize,
  target_char: usize,
) -> Option<InlineCompletionPresentation> {
  let text = if first_line_rendered {
    let (_, remaining) = split_preview_lines(preview_text);
    remaining
  } else {
    preview_text.to_string()
  };
  let mut lines =
    presentation_lines_for_text(&text, InlineCompletionPresentationLineKind::Addition);
  clamp_presentation_lines(&mut lines, MAX_INLINE_COMPLETION_PRESENTATION_LINES);
  if lines.is_empty() {
    return None;
  }

  Some(InlineCompletionPresentation {
    kind: InlineCompletionPresentationKind::Popover,
    title: format!(
      "{} {}",
      inline_completion_provider_display_name(source),
      if first_line_rendered {
        "continuation"
      } else {
        "completion"
      }
    ),
    lines,
    target_char: Some(target_char),
    target_line: Some(target_line),
    accept_label: Some("Tab".to_string()),
  })
}

fn build_diff_presentation(
  source: InlineSuggestionSource,
  replaced_text: &str,
  inserted_text: &str,
  target_line: usize,
  target_char: usize,
) -> Option<InlineCompletionPresentation> {
  let mut lines =
    presentation_lines_for_text(replaced_text, InlineCompletionPresentationLineKind::Removal);
  lines.extend(presentation_lines_for_text(
    inserted_text,
    InlineCompletionPresentationLineKind::Addition,
  ));
  clamp_presentation_lines(&mut lines, MAX_INLINE_COMPLETION_PRESENTATION_LINES);
  if lines.is_empty() {
    return None;
  }

  Some(InlineCompletionPresentation {
    kind: InlineCompletionPresentationKind::Diff,
    title: format!("{} edit", inline_completion_provider_display_name(source)),
    lines,
    target_char: Some(target_char),
    target_line: Some(target_line),
    accept_label: Some("Tab".to_string()),
  })
}

fn presentation_lines_for_text(
  text: &str,
  kind: InlineCompletionPresentationLineKind,
) -> Vec<InlineCompletionPresentationLine> {
  normalized_presentation_lines(text)
    .into_iter()
    .map(|line| {
      InlineCompletionPresentationLine {
        kind,
        text: truncate_presentation_line_text(&line, MAX_INLINE_COMPLETION_PRESENTATION_LINE_CHARS),
      }
    })
    .collect()
}

fn normalized_presentation_lines(text: &str) -> Vec<String> {
  if text.is_empty() {
    return Vec::new();
  }

  let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
  let mut lines = normalized
    .split('\n')
    .map(str::to_string)
    .collect::<Vec<_>>();
  while lines.last().is_some_and(|line| line.is_empty()) {
    lines.pop();
  }
  lines
}

fn truncate_presentation_line_text(text: &str, max_chars: usize) -> String {
  if max_chars == 0 {
    return String::new();
  }
  if text.chars().count() <= max_chars {
    return text.to_string();
  }
  let mut out = text
    .chars()
    .take(max_chars.saturating_sub(1))
    .collect::<String>();
  out.push('…');
  out
}

fn clamp_presentation_lines(lines: &mut Vec<InlineCompletionPresentationLine>, max_lines: usize) {
  if max_lines == 0 {
    lines.clear();
    return;
  }
  if lines.len() <= max_lines {
    return;
  }

  let visible_lines = max_lines.saturating_sub(1);
  let hidden_lines = lines.len().saturating_sub(visible_lines);
  lines.truncate(visible_lines);
  lines.push(InlineCompletionPresentationLine {
    kind: InlineCompletionPresentationLineKind::Dim,
    text: presentation_hidden_lines_text(hidden_lines),
  });
}

fn presentation_hidden_lines_text(hidden_lines: usize) -> String {
  format!(
    "… {} more line{}",
    hidden_lines,
    if hidden_lines == 1 { "" } else { "s" }
  )
}

fn single_active_range(
  selection: &Selection,
  active: Option<CursorId>,
) -> Option<the_lib::selection::Range> {
  if selection.ranges().len() != 1 {
    return None;
  }
  match active {
    Some(cursor_id) => {
      selection
        .range_by_id(cursor_id)
        .copied()
        .or_else(|| selection.ranges().first().copied())
    },
    None => selection.ranges().first().copied(),
  }
}

fn formatting_options(indent: IndentStyle) -> (u32, bool) {
  match indent {
    IndentStyle::Tabs => (4, false),
    IndentStyle::Spaces(width) => (u32::from(width.max(1)), true),
  }
}

fn language_id_for_path<Ctx: DefaultContext>(ctx: &Ctx, path: &Path) -> String {
  ctx
    .syntax_loader()
    .and_then(|loader| {
      let language = loader.language_for_filename(path)?;
      let config = loader.language(language).config();
      config
        .services
        .language_server_language_id
        .clone()
        .or_else(|| Some(config.language_id().to_string()))
    })
    .or_else(|| {
      path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_string)
    })
    .unwrap_or_else(|| "plaintext".to_string())
}

fn render_first_line_preview(
  annotations: &mut OwnedTextAnnotations,
  cursor_char: usize,
  replacement_first_line_len: usize,
  first_line: &str,
  highlight: Option<the_lib::syntax::Highlight>,
) {
  let preview_chars = first_line.chars().collect::<Vec<_>>();
  let overlay_len = preview_chars.len().min(replacement_first_line_len);

  for (idx, ch) in preview_chars.iter().take(overlay_len).enumerate() {
    let _ = annotations.add_overlay_grapheme(cursor_char + idx, ch.to_string(), highlight);
  }

  for idx in overlay_len..replacement_first_line_len {
    let _ = annotations.add_overlay_grapheme(cursor_char + idx, " ", highlight);
  }

  if preview_chars.len() > replacement_first_line_len {
    let tail = preview_chars[replacement_first_line_len..]
      .iter()
      .collect::<String>();
    let _ = annotations.add_inline_text(cursor_char + replacement_first_line_len, tail, highlight);
  }
}

fn split_preview_lines(text: &str) -> (String, String) {
  match text.split_once('\n') {
    Some((first, remaining)) => (first.to_string(), remaining.to_string()),
    None => (text.to_string(), String::new()),
  }
}

fn preview_line_count(text: &str) -> usize {
  if text.is_empty() {
    0
  } else {
    text.lines().count()
  }
}

fn preview_blank_line_count(text: &str) -> usize {
  text.lines().filter(|line| line.trim().is_empty()).count()
}

fn compact_preview_remaining_lines(text: &str) -> String {
  if text.is_empty() {
    return String::new();
  }

  let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
  let mut lines = normalized
    .split('\n')
    .map(str::to_string)
    .collect::<Vec<_>>();
  while lines.last().is_some_and(|line| line.trim().is_empty()) {
    lines.pop();
  }

  lines
    .into_iter()
    .filter(|line| !line.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn clamp_preview_remaining_lines(text: &str, max_lines: usize) -> (String, usize) {
  if text.is_empty() || max_lines == 0 {
    return (String::new(), 0);
  }

  let lines = text.lines().collect::<Vec<_>>();
  if lines.len() <= max_lines {
    return (text.to_string(), 0);
  }

  let visible_content_lines = max_lines.saturating_sub(1);
  let mut out = lines
    .iter()
    .take(visible_content_lines)
    .map(|line| (*line).to_string())
    .collect::<Vec<_>>();
  let hidden_line = lines.get(visible_content_lines).copied().unwrap_or("");
  out.push(preview_ellipsis_line(hidden_line));
  (
    out.join("\n"),
    lines.len().saturating_sub(visible_content_lines),
  )
}

fn preview_ellipsis_line(hidden_line: &str) -> String {
  let indent = hidden_line
    .chars()
    .take_while(|ch| ch.is_whitespace())
    .collect::<String>();
  format!("{indent}…")
}

fn replacement_first_line_len(text: &str) -> usize {
  text
    .chars()
    .take_while(|ch| *ch != '\n' && *ch != '\r')
    .count()
}

fn preview_text_for_cursor(
  existing_prefix: String,
  cursor_char: usize,
  suggestion: &InlineSuggestion,
) -> String {
  if cursor_char <= suggestion.from {
    return suggestion.text.clone();
  }

  let (_, prefix_bytes) = shared_prefix(&existing_prefix, &suggestion.text);
  suggestion.text[prefix_bytes..].to_string()
}

#[derive(Debug, Clone)]
struct CopilotQuery {
  workspace_root: PathBuf,
  uri:            String,
  language_id:    String,
  version:        i32,
  text:           String,
  line:           u32,
  character:      u32,
  tab_size:       u32,
  insert_spaces:  bool,
}

#[derive(Debug, Clone)]
struct SupermavenQuery {
  workspace_root:  PathBuf,
  file_path:       PathBuf,
  text:            String,
  prefix:          String,
  cursor_char:     usize,
  line_before:     String,
  line_after:      String,
  following_lines: Vec<String>,
  line_end_char:   usize,
}

#[derive(Debug, Clone)]
enum ProviderQuery {
  Copilot(CopilotQuery),
  Supermaven(SupermavenQuery),
}

#[derive(Debug)]
enum WorkerCommand {
  Query {
    request_id: u64,
    request:    ProviderQuery,
  },
  CopilotDidShow {
    source:  InlineSuggestionSource,
    text:    String,
    range:   JsonRange,
    command: Option<JsonCommand>,
  },
  CopilotAccept {
    command: JsonCommand,
  },
  CopilotSignIn {
    workspace_root: PathBuf,
  },
  SupermavenUseFree {
    workspace_root: PathBuf,
  },
  SupermavenUsePro {
    workspace_root: PathBuf,
  },
  SupermavenLogout {
    workspace_root: PathBuf,
  },
}

#[derive(Debug)]
enum WorkerEvent {
  Ready,
  AuthPrompt {
    message: String,
  },
  ActivationUrl(String),
  Authenticated {
    user: Option<String>,
  },
  Status(String),
  Error(String),
  QueryResult {
    request_id: u64,
    result:     Result<Option<WorkerSuggestion>, String>,
  },
}

#[derive(Debug, Clone)]
struct WorkerSuggestion {
  from:          usize,
  to:            usize,
  text:          String,
  source:        InlineSuggestionSource,
  command:       Option<JsonCommand>,
  display_range: Option<JsonRange>,
}

fn emit_worker_event(event_tx: &Sender<WorkerEvent>, waker: &RenderWaker, event: WorkerEvent) {
  let _ = event_tx.send(event);
  waker.wake();
}

fn worker_main(
  provider: InlineCompletionProvider,
  rx: Receiver<WorkerCommand>,
  event_tx: Sender<WorkerEvent>,
  waker: RenderWaker,
) {
  match provider {
    InlineCompletionProvider::None => {},
    InlineCompletionProvider::Copilot => copilot_worker_main(rx, event_tx, waker),
    InlineCompletionProvider::Supermaven => supermaven_worker_main(rx, event_tx, waker),
  }
}

fn copilot_worker_main(
  rx: Receiver<WorkerCommand>,
  event_tx: Sender<WorkerEvent>,
  waker: RenderWaker,
) {
  let mut server = None;
  while let Ok(command) = rx.recv() {
    match command {
      WorkerCommand::Query {
        request_id,
        request,
      } => {
        let ProviderQuery::Copilot(request) = request else {
          continue;
        };
        if server.is_none() {
          match CopilotServer::start(&request.workspace_root) {
            Ok(next) => {
              emit_worker_event(&event_tx, &waker, WorkerEvent::Ready);
              server = Some(next);
            },
            Err(error) => {
              emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
              continue;
            },
          }
        }

        let result = match server.as_mut() {
          Some(server) => server.inline_completion(&request),
          None => Err("Copilot server failed to start".to_string()),
        };
        if result.is_err() {
          server = None;
        }
        emit_worker_event(&event_tx, &waker, WorkerEvent::QueryResult {
          request_id,
          result,
        });
      },
      WorkerCommand::CopilotDidShow {
        source,
        text,
        range,
        command,
      } => {
        if let Some(active_server) = server.as_mut()
          && let Err(error) = active_server.report_shown_completion(source, &text, &range, command)
        {
          server = None;
          emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
        }
      },
      WorkerCommand::CopilotAccept { command } => {
        if let Some(active_server) = server.as_mut()
          && let Err(error) = active_server.execute_command(command)
        {
          server = None;
          emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
        }
      },
      WorkerCommand::CopilotSignIn { workspace_root } => {
        if server.is_none() {
          match CopilotServer::start(&workspace_root) {
            Ok(next) => {
              server = Some(next);
            },
            Err(error) => {
              emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
              continue;
            },
          }
        }

        let result = match server.as_mut() {
          Some(server) => server.sign_in(&event_tx, &waker),
          None => Err("Copilot server failed to start".to_string()),
        };

        match result {
          Ok(user) => {
            emit_worker_event(&event_tx, &waker, WorkerEvent::Authenticated { user });
            emit_worker_event(&event_tx, &waker, WorkerEvent::Ready);
          },
          Err(error) => {
            server = None;
            emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
          },
        }
      },
      WorkerCommand::SupermavenUseFree { .. }
      | WorkerCommand::SupermavenUsePro { .. }
      | WorkerCommand::SupermavenLogout { .. } => {},
    }
  }
}

fn supermaven_worker_main(
  rx: Receiver<WorkerCommand>,
  event_tx: Sender<WorkerEvent>,
  waker: RenderWaker,
) {
  let mut server = None;
  while let Ok(command) = rx.recv() {
    match command {
      WorkerCommand::Query {
        request_id,
        request,
      } => {
        let ProviderQuery::Supermaven(request) = request else {
          continue;
        };
        if server.is_none() {
          match SupermavenServer::start(&request.workspace_root) {
            Ok(mut next) => {
              let _ = next.pump_until(
                Instant::now() + Duration::from_millis(150),
                &event_tx,
                &waker,
                None,
                None,
              );
              emit_worker_event(&event_tx, &waker, WorkerEvent::Ready);
              server = Some(next);
            },
            Err(error) => {
              emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
              continue;
            },
          }
        }

        let result = match server.as_mut() {
          Some(server) => server.inline_completion(&request, &event_tx, &waker),
          None => Err("Supermaven server failed to start".to_string()),
        };
        if result.is_err() {
          server = None;
        }
        emit_worker_event(&event_tx, &waker, WorkerEvent::QueryResult {
          request_id,
          result,
        });
      },
      WorkerCommand::SupermavenUseFree { workspace_root } => {
        if server.is_none() {
          match SupermavenServer::start(&workspace_root) {
            Ok(mut next) => {
              let _ = next.pump_until(
                Instant::now() + Duration::from_millis(150),
                &event_tx,
                &waker,
                None,
                None,
              );
              emit_worker_event(&event_tx, &waker, WorkerEvent::Ready);
              server = Some(next);
            },
            Err(error) => {
              emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
              continue;
            },
          }
        }
        let mut reset_server = false;
        if let Some(active_server) = server.as_mut()
          && let Err(error) = active_server.use_free(&event_tx, &waker)
        {
          emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
          reset_server = true;
        }
        if reset_server {
          server = None;
        }
      },
      WorkerCommand::SupermavenUsePro { workspace_root } => {
        if server.is_none() {
          match SupermavenServer::start(&workspace_root) {
            Ok(mut next) => {
              let _ = next.pump_until(
                Instant::now() + Duration::from_millis(150),
                &event_tx,
                &waker,
                None,
                None,
              );
              emit_worker_event(&event_tx, &waker, WorkerEvent::Ready);
              server = Some(next);
            },
            Err(error) => {
              emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
              continue;
            },
          }
        }
        if let Some(server) = server.as_mut()
          && let Err(error) = server.use_pro(&event_tx, &waker)
        {
          emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
        }
      },
      WorkerCommand::SupermavenLogout { workspace_root } => {
        if server.is_none() {
          match SupermavenServer::start(&workspace_root) {
            Ok(mut next) => {
              let _ = next.pump_until(
                Instant::now() + Duration::from_millis(150),
                &event_tx,
                &waker,
                None,
                None,
              );
              emit_worker_event(&event_tx, &waker, WorkerEvent::Ready);
              server = Some(next);
            },
            Err(error) => {
              emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
              continue;
            },
          }
        }
        let mut reset_server = false;
        if let Some(active_server) = server.as_mut()
          && let Err(error) = active_server.logout(&event_tx, &waker)
        {
          emit_worker_event(&event_tx, &waker, WorkerEvent::Error(error));
          reset_server = true;
        }
        if reset_server {
          server = None;
        }
      },
      WorkerCommand::CopilotDidShow { .. }
      | WorkerCommand::CopilotAccept { .. }
      | WorkerCommand::CopilotSignIn { .. } => {},
    }
  }
}

#[derive(Debug, Clone)]
enum SupermavenResponseItem {
  Text(String),
  Dedent(String),
  Delete { verify: String },
  End,
  Barrier,
  FinishEdit,
  Jump,
  Skip,
}

#[derive(Debug)]
enum SupermavenMessage {
  Response {
    state_id: u64,
    items:    Vec<SupermavenResponseItem>,
  },
  Metadata {
    dust_strings: Vec<String>,
  },
  ActivationRequest {
    activate_url: String,
  },
  ActivationSuccess,
  ServiceTier {
    display: Option<String>,
  },
}

#[derive(Debug)]
struct SupermavenDerivedSuggestion {
  text:               String,
  prior_delete_chars: usize,
  is_incomplete:      bool,
}

struct SupermavenServer {
  child:          Child,
  stdin:          BufWriter<ChildStdin>,
  rx:             Receiver<SupermavenMessage>,
  next_state_id:  u64,
  dust_strings:   Vec<String>,
  activation_url: Option<String>,
}

impl SupermavenServer {
  fn start(workspace_root: &Path) -> Result<Self, String> {
    let binary_path = resolve_supermaven_binary_path(workspace_root)?;
    let mut child = ProcessCommand::new(binary_path)
      .args(["stdio"])
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|error| format!("failed to spawn Supermaven agent: {error}"))?;

    let stdin = child
      .stdin
      .take()
      .ok_or_else(|| "failed to capture Supermaven stdin".to_string())?;
    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| "failed to capture Supermaven stdout".to_string())?;
    let (tx, rx) = mpsc::channel();
    let reader = thread::Builder::new()
      .name("the-editor-supermaven-reader".to_string())
      .spawn(move || supermaven_reader_main(stdout, tx))
      .map_err(|error| format!("failed to spawn Supermaven reader thread: {error}"))?;
    std::mem::forget(reader);

    let mut server = Self {
      child,
      stdin: BufWriter::new(stdin),
      rx,
      next_state_id: 0,
      dust_strings: Vec::new(),
      activation_url: None,
    };
    server.send_json_line(&json!({
      "kind": "greeting",
      "allowGitignore": false,
    }))?;
    Ok(server)
  }

  fn inline_completion(
    &mut self,
    request: &SupermavenQuery,
    event_tx: &Sender<WorkerEvent>,
    waker: &RenderWaker,
  ) -> Result<Option<WorkerSuggestion>, String> {
    let state_id = self.submit_query(request)?;
    let deadline = Instant::now() + SUPERMAVEN_QUERY_TIMEOUT;
    let mut items = Vec::new();
    let mut best = None;

    while Instant::now() < deadline {
      self.pump_until(deadline, event_tx, waker, Some(state_id), Some(&mut items))?;
      if let Some(next) = derive_supermaven_suggestion(request, &items, &self.dust_strings) {
        best = Some(next);
        if best.as_ref().is_some_and(|derived| !derived.is_incomplete) {
          break;
        }
      } else if !items.is_empty() {
        break;
      }
    }

    Ok(best.map(|derived| {
      WorkerSuggestion {
        from:          request
          .cursor_char
          .saturating_sub(derived.prior_delete_chars),
        to:            request.line_end_char,
        text:          derived.text,
        source:        InlineSuggestionSource::Supermaven,
        command:       None,
        display_range: None,
      }
    }))
  }

  fn use_free(
    &mut self,
    event_tx: &Sender<WorkerEvent>,
    waker: &RenderWaker,
  ) -> Result<(), String> {
    self.send_json_line(&json!({
      "kind": "use_free_version",
    }))?;
    self.pump_until(
      Instant::now() + Duration::from_millis(150),
      event_tx,
      waker,
      None,
      None,
    )
  }

  fn use_pro(&mut self, event_tx: &Sender<WorkerEvent>, waker: &RenderWaker) -> Result<(), String> {
    self.pump_until(
      Instant::now() + Duration::from_millis(150),
      event_tx,
      waker,
      None,
      None,
    )?;
    match self
      .activation_url
      .as_ref()
      .map(|url| url.trim())
      .filter(|url| !url.is_empty())
    {
      Some(url) => {
        emit_worker_event(event_tx, waker, WorkerEvent::ActivationUrl(url.to_string()));
        Ok(())
      },
      None => {
        Err(
          "Supermaven did not provide an activation URL yet. Run :supermaven-use-free first and \
           wait for the service tier message."
            .to_string(),
        )
      },
    }
  }

  fn logout(&mut self, event_tx: &Sender<WorkerEvent>, waker: &RenderWaker) -> Result<(), String> {
    self.send_json_line(&json!({
      "kind": "logout",
    }))?;
    self.pump_until(
      Instant::now() + Duration::from_millis(150),
      event_tx,
      waker,
      None,
      None,
    )
  }

  fn submit_query(&mut self, request: &SupermavenQuery) -> Result<u64, String> {
    self.send_json_line(&json!({
      "kind": "inform_file_changed",
      "path": request.file_path,
    }))?;
    self.next_state_id = self.next_state_id.saturating_add(1);
    self.send_json_line(&json!({
      "kind": "state_update",
      "newId": self.next_state_id.to_string(),
      "updates": [
        {
          "kind": "cursor_update",
          "path": request.file_path,
          "offset": request.prefix.len(),
        },
        {
          "kind": "file_update",
          "path": request.file_path,
          "content": request.text,
        }
      ]
    }))?;
    Ok(self.next_state_id)
  }

  fn pump_until(
    &mut self,
    deadline: Instant,
    event_tx: &Sender<WorkerEvent>,
    waker: &RenderWaker,
    target_state_id: Option<u64>,
    mut items_out: Option<&mut Vec<SupermavenResponseItem>>,
  ) -> Result<(), String> {
    while Instant::now() < deadline {
      let timeout = deadline
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(25));
      match self.rx.recv_timeout(timeout) {
        Ok(message) => {
          self.process_message(
            message,
            event_tx,
            waker,
            target_state_id,
            items_out.as_deref_mut(),
          )
        },
        Err(RecvTimeoutError::Timeout) => break,
        Err(RecvTimeoutError::Disconnected) => {
          return Err("Supermaven agent closed the connection".to_string());
        },
      }
    }
    Ok(())
  }

  fn process_message(
    &mut self,
    message: SupermavenMessage,
    event_tx: &Sender<WorkerEvent>,
    waker: &RenderWaker,
    target_state_id: Option<u64>,
    items_out: Option<&mut Vec<SupermavenResponseItem>>,
  ) {
    match message {
      SupermavenMessage::Response { state_id, items } => {
        if target_state_id.is_some_and(|target| target == state_id)
          && let Some(items_out) = items_out
        {
          items_out.extend(items);
        }
      },
      SupermavenMessage::Metadata { dust_strings } => {
        self.dust_strings = dust_strings;
      },
      SupermavenMessage::ActivationRequest { activate_url } => {
        let activate_url = activate_url.trim().to_string();
        if !activate_url.is_empty() {
          self.activation_url = Some(activate_url);
        }
      },
      SupermavenMessage::ActivationSuccess => {
        emit_worker_event(event_tx, waker, WorkerEvent::Authenticated { user: None });
      },
      SupermavenMessage::ServiceTier { display } => {
        if let Some(display) = display {
          emit_worker_event(
            event_tx,
            waker,
            WorkerEvent::Status(format!("Supermaven {display} is running")),
          );
        }
      },
    }
  }

  fn send_json_line(&mut self, value: &Value) -> Result<(), String> {
    let line = serde_json::to_string(value)
      .map_err(|error| format!("failed to serialize Supermaven message: {error}"))?;
    writeln!(self.stdin, "{line}")
      .map_err(|error| format!("failed to write Supermaven message: {error}"))?;
    self
      .stdin
      .flush()
      .map_err(|error| format!("failed to flush Supermaven message: {error}"))
  }
}

impl Drop for SupermavenServer {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

fn supermaven_reader_main(stdout: ChildStdout, tx: Sender<SupermavenMessage>) {
  let mut reader = BufReader::new(stdout);
  loop {
    let mut line = String::new();
    match reader.read_line(&mut line) {
      Ok(0) | Err(_) => break,
      Ok(_) => {
        let line = line.trim_end_matches(['\r', '\n']);
        if !line.starts_with("SM-MESSAGE ") {
          continue;
        }
        let payload = &line["SM-MESSAGE ".len()..];
        let Ok(value) = serde_json::from_str::<Value>(payload) else {
          continue;
        };
        if let Some(message) = parse_supermaven_message(value) {
          let _ = tx.send(message);
        }
      },
    }
  }
}

fn parse_supermaven_message(value: Value) -> Option<SupermavenMessage> {
  let kind = value.get("kind")?.as_str()?;
  match kind {
    "response" => {
      let state_id = value.get("stateId")?.as_str()?.parse::<u64>().ok()?;
      let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
          items
            .iter()
            .filter_map(parse_supermaven_response_item)
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
      Some(SupermavenMessage::Response { state_id, items })
    },
    "metadata" => {
      Some(SupermavenMessage::Metadata {
        dust_strings: value
          .get("dustStrings")
          .and_then(Value::as_array)
          .map(|items| {
            items
              .iter()
              .filter_map(|item| item.as_str().map(str::to_string))
              .collect::<Vec<_>>()
          })
          .unwrap_or_default(),
      })
    },
    "activation_request" => {
      Some(SupermavenMessage::ActivationRequest {
        activate_url: value.get("activateUrl")?.as_str()?.to_string(),
      })
    },
    "activation_success" => Some(SupermavenMessage::ActivationSuccess),
    "service_tier" => {
      Some(SupermavenMessage::ServiceTier {
        display: value
          .get("display")
          .and_then(Value::as_str)
          .map(str::to_string),
      })
    },
    "passthrough" => {
      value
        .get("passthrough")
        .cloned()
        .and_then(parse_supermaven_message)
    },
    _ => None,
  }
}

fn parse_supermaven_response_item(value: &Value) -> Option<SupermavenResponseItem> {
  let kind = value.get("kind")?.as_str()?;
  match kind {
    "text" => {
      Some(SupermavenResponseItem::Text(
        value.get("text")?.as_str()?.to_string(),
      ))
    },
    "dedent" => {
      Some(SupermavenResponseItem::Dedent(
        value.get("text")?.as_str()?.to_string(),
      ))
    },
    "delete" => {
      Some(SupermavenResponseItem::Delete {
        verify: value.get("verify")?.as_str()?.to_string(),
      })
    },
    "end" => Some(SupermavenResponseItem::End),
    "barrier" => Some(SupermavenResponseItem::Barrier),
    "finish_edit" => Some(SupermavenResponseItem::FinishEdit),
    "jump" => Some(SupermavenResponseItem::Jump),
    "skip" => Some(SupermavenResponseItem::Skip),
    _ => None,
  }
}

fn derive_supermaven_suggestion(
  request: &SupermavenQuery,
  items: &[SupermavenResponseItem],
  _dust_strings: &[String],
) -> Option<SupermavenDerivedSuggestion> {
  let mut output = String::new();
  let mut dedent = String::new();
  let mut complete = false;

  for item in items {
    match item {
      SupermavenResponseItem::Text(text) => output.push_str(text),
      SupermavenResponseItem::Dedent(text) => dedent.push_str(text),
      SupermavenResponseItem::Barrier
      | SupermavenResponseItem::FinishEdit
      | SupermavenResponseItem::End => {
        complete = true;
        break;
      },
      SupermavenResponseItem::Delete { verify } => {
        if request
          .following_lines
          .first()
          .is_some_and(|line| trim_end_whitespace(line) == trim_end_whitespace(verify))
        {
          complete = true;
          break;
        }
      },
      SupermavenResponseItem::Jump | SupermavenResponseItem::Skip => return None,
    }
  }

  if !dedent.is_empty() && !request.line_before.ends_with(&dedent) {
    return None;
  }

  while !dedent.is_empty() && !output.is_empty() {
    let Some(dedent_first) = dedent.chars().next() else {
      break;
    };
    let Some(output_first) = output.chars().next() else {
      break;
    };
    if dedent_first != output_first {
      break;
    }
    dedent = dedent[dedent_first.len_utf8()..].to_string();
    output = output[output_first.len_utf8()..].to_string();
  }

  let output = trim_end_whitespace(&output).to_string();
  if output.trim().is_empty() {
    return None;
  }

  Some(SupermavenDerivedSuggestion {
    text:               output,
    prior_delete_chars: dedent.chars().count(),
    is_incomplete:      !complete && request.line_after.trim().is_empty(),
  })
}

fn trim_end_whitespace(text: &str) -> &str {
  text.trim_end_matches(char::is_whitespace)
}

struct CopilotContentChange {
  range:      JsonRange,
  text:       String,
  start_char: usize,
  end_char:   usize,
}

fn copilot_incremental_change(old_text: &str, new_text: &str) -> Option<CopilotContentChange> {
  if old_text == new_text {
    return None;
  }

  let old_rope = Rope::from_str(old_text);
  let old_total_chars = old_text.chars().count();
  let (prefix_chars, prefix_bytes) = shared_prefix(old_text, new_text);
  let old_remaining = &old_text[prefix_bytes..];
  let new_remaining = &new_text[prefix_bytes..];
  let (suffix_chars, suffix_bytes) = shared_suffix(old_remaining, new_remaining);
  let end_char = old_total_chars.saturating_sub(suffix_chars);
  let new_end_byte = new_text.len().saturating_sub(suffix_bytes);
  let replacement = new_text[prefix_bytes..new_end_byte].to_string();
  let (start_line, start_character) = char_idx_to_utf16_position(&old_rope, prefix_chars);
  let (end_line, end_character) = char_idx_to_utf16_position(&old_rope, end_char);

  Some(CopilotContentChange {
    range: JsonRange {
      start: JsonPosition {
        line:      start_line,
        character: start_character,
      },
      end:   JsonPosition {
        line:      end_line,
        character: end_character,
      },
    },
    text: replacement,
    start_char: prefix_chars,
    end_char,
  })
}

struct CopilotDocumentState {
  uri:         String,
  language_id: String,
  text:        String,
  version:     i32,
}

struct CopilotServer {
  child:                Child,
  stdin:                BufWriter<ChildStdin>,
  stdout:               BufReader<ChildStdout>,
  next_id:              u64,
  document:             Option<CopilotDocumentState>,
  pending_responses:    HashMap<u64, Value>,
  ignored_response_ids: HashSet<u64>,
}

impl CopilotServer {
  fn start(workspace_root: &Path) -> Result<Self, String> {
    let (program, args) = resolve_server_command(workspace_root)?;
    let mut child = ProcessCommand::new(program)
      .args(args)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|error| format!("failed to spawn Copilot language server: {error}"))?;

    let stdin = child
      .stdin
      .take()
      .ok_or_else(|| "failed to capture Copilot server stdin".to_string())?;
    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| "failed to capture Copilot server stdout".to_string())?;

    let mut server = Self {
      child,
      stdin: BufWriter::new(stdin),
      stdout: BufReader::new(stdout),
      next_id: 1,
      document: None,
      pending_responses: HashMap::new(),
      ignored_response_ids: HashSet::new(),
    };
    server.initialize(workspace_root)?;
    Ok(server)
  }

  fn initialize(&mut self, workspace_root: &Path) -> Result<(), String> {
    let root_uri = file_uri_for_path(workspace_root);
    let initialize_id = self.next_request_id();
    let workspace_folders = if let Some(uri) = root_uri.as_ref() {
      vec![json!({
        "uri": uri,
        "name": workspace_root.file_name().and_then(|name| name.to_str()).unwrap_or("workspace"),
      })]
    } else {
      Vec::new()
    };
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "id": initialize_id,
      "method": "initialize",
      "params": {
        "processId": Value::Null,
        "rootUri": root_uri,
        "capabilities": {
          "window": {
            "showDocument": {
              "support": true,
            },
          },
        },
        "clientInfo": {
          "name": EDITOR_NAME,
          "version": env!("CARGO_PKG_VERSION"),
        },
        "initializationOptions": {
          "editorInfo": {
            "name": EDITOR_NAME,
            "version": env!("CARGO_PKG_VERSION"),
          },
          "editorPluginInfo": {
            "name": EDITOR_PLUGIN_NAME,
            "version": env!("CARGO_PKG_VERSION"),
          },
        },
        "workspaceFolders": workspace_folders,
      }
    }))?;
    inline_completion_trace(
      "copilot_initialize_sent",
      json!({
        "workspace_root": workspace_root.display().to_string(),
        "root_uri": root_uri,
        "workspace_folder_count": workspace_folders.len(),
      }),
    );
    self.read_response(initialize_id)?;
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "method": "initialized",
      "params": {}
    }))?;
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "method": "workspace/didChangeConfiguration",
      "params": {
        "settings": {},
      }
    }))?;
    inline_completion_trace("copilot_initialized", json!({}));
    Ok(())
  }

  fn inline_completion(
    &mut self,
    request: &CopilotQuery,
  ) -> Result<Option<WorkerSuggestion>, String> {
    self.ensure_signed_in_for_queries()?;
    let synced_version = self.sync_document(request)?;
    let inline_request_id = self.request_inline_completion(request, synced_version)?;
    let (_, message) = self.read_any_response(&[inline_request_id])?;
    match self.parse_inline_completion_response(request, message) {
      Ok(Some(suggestion)) => {
        inline_completion_trace(
          "copilot_query_selected_inline_completion",
          json!({
            "uri": request.uri.as_str(),
            "editor_version": request.version,
            "provider_version": synced_version,
            "suggestion": inline_completion_trace_worker_suggestion(&suggestion),
          }),
        );
        Ok(Some(suggestion))
      },
      Ok(None) => {
        inline_completion_trace(
          "copilot_query_no_suggestion",
          json!({
            "uri": request.uri.as_str(),
            "editor_version": request.version,
            "provider_version": synced_version,
          }),
        );
        Ok(None)
      },
      Err(error) => {
        inline_completion_trace(
          "copilot_query_error",
          json!({
            "uri": request.uri.as_str(),
            "editor_version": request.version,
            "provider_version": synced_version,
            "error": &error,
          }),
        );
        Err(error)
      },
    }
  }

  fn request_inline_completion(
    &mut self,
    request: &CopilotQuery,
    version: i32,
  ) -> Result<u64, String> {
    let request_id = self.next_request_id();
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "id": request_id,
      "method": "textDocument/inlineCompletion",
      "params": {
        "textDocument": {
          "uri": request.uri,
          "version": version,
        },
        "position": {
          "line": request.line,
          "character": request.character,
        },
        "context": {
          "triggerKind": 2,
        },
        "formattingOptions": {
          "tabSize": request.tab_size,
          "insertSpaces": request.insert_spaces,
        }
      }
    }))?;
    Ok(request_id)
  }

  fn parse_inline_completion_response(
    &mut self,
    request: &CopilotQuery,
    message: Value,
  ) -> Result<Option<WorkerSuggestion>, String> {
    let value = self.parse_response_message(message)?;
    let result: InlineCompletionResult = serde_json::from_value(value)
      .map_err(|error| format!("failed to parse Copilot inline completion response: {error}"))?;
    let item_count = result.items.len();
    let suggestion = result
      .items
      .into_iter()
      .next()
      .and_then(|item| normalize_copilot_suggestion(request, item));
    inline_completion_trace(
      "copilot_inline_completion_response",
      json!({
        "uri": request.uri.as_str(),
        "version": request.version,
        "item_count": item_count,
        "suggestion": suggestion
          .as_ref()
          .map(inline_completion_trace_worker_suggestion)
          .unwrap_or(Value::Null),
      }),
    );
    Ok(suggestion)
  }

  fn sign_in(
    &mut self,
    event_tx: &Sender<WorkerEvent>,
    waker: &RenderWaker,
  ) -> Result<Option<String>, String> {
    let status = self.check_status()?;
    if is_authorized(&status) {
      return Ok(authorized_user(&status));
    }

    let request_id = self.next_request_id();
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "id": request_id,
      "method": "signIn",
      "params": {}
    }))?;
    let prompt: PromptUserDeviceFlow = serde_json::from_value(self.read_response(request_id)?)
      .map_err(|error| format!("failed to parse Copilot device-flow prompt: {error}"))?;
    emit_worker_event(event_tx, waker, WorkerEvent::AuthPrompt {
      message: format!(
        "Copilot sign-in code: {}. Open https://github.com/login/device if the browser does not \
         open automatically.",
        prompt.user_code
      ),
    });
    let status = self.finish_device_flow(prompt.command)?;
    if is_authorized(&status) {
      return Ok(authorized_user(&status));
    }

    Err(sign_in_status_message(&status))
  }

  fn ensure_signed_in_for_queries(&mut self) -> Result<(), String> {
    let status = self.check_status()?;
    if is_authorized(&status) {
      return Ok(());
    }

    Err(sign_in_status_message(&status))
  }

  fn check_status(&mut self) -> Result<SignInStatus, String> {
    let status_id = self.next_request_id();
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "id": status_id,
      "method": "checkStatus",
      "params": {
        "localChecksOnly": false
      }
    }))?;
    serde_json::from_value(self.read_response(status_id)?)
      .map_err(|error| format!("failed to parse Copilot auth status: {error}"))
  }

  fn finish_device_flow(&mut self, command: JsonCommand) -> Result<SignInStatus, String> {
    let result = self.execute_command(command)?;
    if let Ok(status) = serde_json::from_value::<SignInStatus>(result.clone()) {
      return Ok(status);
    }

    for _ in 0..300 {
      thread::sleep(Duration::from_secs(1));
      let status = self.check_status()?;
      match status {
        SignInStatus::Ok { .. }
        | SignInStatus::MaybeOk { .. }
        | SignInStatus::AlreadySignedIn { .. } => return Ok(status),
        SignInStatus::NotAuthorized { .. } | SignInStatus::NotSignedIn => continue,
      }
    }

    Err(
      "Copilot sign-in timed out. Visit https://github.com/login/device, complete the device \
       flow, then run :copilot-sign-in again."
        .to_string(),
    )
  }

  fn execute_command(&mut self, command: JsonCommand) -> Result<Value, String> {
    let request_id = self.next_request_id();
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "id": request_id,
      "method": "workspace/executeCommand",
      "params": {
        "command": command.command,
        "arguments": command.arguments.unwrap_or_default(),
      }
    }))?;
    self.read_response(request_id)
  }

  fn report_shown_completion(
    &mut self,
    source: InlineSuggestionSource,
    text: &str,
    range: &JsonRange,
    command: Option<JsonCommand>,
  ) -> Result<(), String> {
    match source {
      InlineSuggestionSource::CopilotInlineCompletion => {
        self.write_message(&json!({
          "jsonrpc": "2.0",
          "method": "textDocument/didShowCompletion",
          "params": {
            "item": {
              "insertText": text,
              "range": range,
              "command": command,
            }
          }
        }))
      },
      InlineSuggestionSource::Supermaven => Ok(()),
    }
  }

  fn sync_document(&mut self, request: &CopilotQuery) -> Result<i32, String> {
    let needs_reopen = self.document.as_ref().is_none_or(|document| {
      document.uri != request.uri || document.language_id != request.language_id
    });
    if needs_reopen {
      if let Some(document) = self.document.take() {
        self.write_message(&json!({
          "jsonrpc": "2.0",
          "method": "textDocument/didClose",
          "params": {
            "textDocument": {
              "uri": document.uri,
            }
          }
        }))?;
        inline_completion_trace(
          "copilot_document_close",
          json!({
            "uri": document.uri,
            "version": document.version,
          }),
        );
      }

      self.write_message(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
          "textDocument": {
            "uri": request.uri,
            "languageId": request.language_id,
            "version": 0,
            "text": request.text,
          }
        }
      }))?;
      self.write_message(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didFocus",
        "params": {
          "uri": request.uri,
        }
      }))?;
      inline_completion_trace(
        "copilot_document_open",
        json!({
          "uri": request.uri.as_str(),
          "language_id": request.language_id.as_str(),
          "version": 0,
          "text_chars": request.text.chars().count(),
        }),
      );
      inline_completion_trace(
        "copilot_document_focus",
        json!({
          "uri": request.uri.as_str(),
        }),
      );
      self.document = Some(CopilotDocumentState {
        uri:         request.uri.clone(),
        language_id: request.language_id.clone(),
        text:        request.text.clone(),
        version:     0,
      });
      return Ok(0);
    }

    let (document_text, document_version) = {
      let document = self.document.as_ref().expect("document is open");
      (document.text.clone(), document.version)
    };
    if let Some(change) = copilot_incremental_change(&document_text, &request.text) {
      let next_version = document_version.saturating_add(1);
      self.write_message(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
          "textDocument": {
            "uri": request.uri,
            "version": next_version,
          },
          "contentChanges": [
            {
              "range": change.range,
              "text": change.text,
            }
          ]
        }
      }))?;
      inline_completion_trace(
        "copilot_document_change",
        json!({
          "uri": request.uri.as_str(),
          "version": next_version,
          "start_char": change.start_char,
          "end_char": change.end_char,
          "text_chars": change.text.chars().count(),
          "text_preview": inline_completion_trace_text_preview(&change.text),
        }),
      );
      if let Some(document) = self.document.as_mut() {
        document.text = request.text.clone();
        document.version = next_version;
      }
      return Ok(next_version);
    }

    Ok(document_version)
  }

  fn next_request_id(&mut self) -> u64 {
    let id = self.next_id;
    self.next_id = self.next_id.saturating_add(1);
    id
  }

  fn write_message(&mut self, value: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(value)
      .map_err(|error| format!("failed to serialize Copilot JSON-RPC message: {error}"))?;
    write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len())
      .map_err(|error| format!("failed to write Copilot JSON-RPC header: {error}"))?;
    self
      .stdin
      .write_all(&body)
      .map_err(|error| format!("failed to write Copilot JSON-RPC body: {error}"))?;
    self
      .stdin
      .flush()
      .map_err(|error| format!("failed to flush Copilot JSON-RPC message: {error}"))
  }

  fn read_response(&mut self, expected_id: u64) -> Result<Value, String> {
    if let Some(message) = self.pending_responses.remove(&expected_id) {
      return self.parse_response_message(message);
    }

    loop {
      let message = read_jsonrpc_message(&mut self.stdout)?;
      let Some(id) = message.get("id").and_then(|id| id.as_u64()) else {
        continue;
      };
      if self.ignored_response_ids.remove(&id) {
        continue;
      }
      if id == expected_id {
        return self.parse_response_message(message);
      }
      self.pending_responses.insert(id, message);
    }
  }

  fn read_any_response(&mut self, expected_ids: &[u64]) -> Result<(u64, Value), String> {
    for expected_id in expected_ids {
      if let Some(message) = self.pending_responses.remove(expected_id) {
        return Ok((*expected_id, message));
      }
    }

    loop {
      let message = read_jsonrpc_message(&mut self.stdout)?;
      let Some(id) = message.get("id").and_then(|id| id.as_u64()) else {
        continue;
      };
      if self.ignored_response_ids.remove(&id) {
        continue;
      }
      if expected_ids.contains(&id) {
        return Ok((id, message));
      }
      self.pending_responses.insert(id, message);
    }
  }

  fn parse_response_message(&self, message: Value) -> Result<Value, String> {
    if let Some(error) = message.get("error") {
      let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Copilot JSON-RPC request failed");
      return Err(message.to_string());
    }

    Ok(message.get("result").cloned().unwrap_or(Value::Null))
  }
}

impl Drop for CopilotServer {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

fn read_jsonrpc_message(reader: &mut BufReader<ChildStdout>) -> Result<Value, String> {
  let mut content_length = None::<usize>;
  loop {
    let mut line = String::new();
    let read = reader
      .read_line(&mut line)
      .map_err(|error| format!("failed to read Copilot JSON-RPC header: {error}"))?;
    if read == 0 {
      return Err("Copilot language server closed the connection".to_string());
    }

    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
      break;
    }
    if let Some((name, value)) = trimmed.split_once(':')
      && name.eq_ignore_ascii_case("content-length")
    {
      content_length = Some(
        value
          .trim()
          .parse::<usize>()
          .map_err(|error| format!("invalid Copilot JSON-RPC content length: {error}"))?,
      );
    }
  }

  let content_length = content_length
    .ok_or_else(|| "Copilot JSON-RPC message is missing Content-Length".to_string())?;
  let mut body = vec![0u8; content_length];
  reader
    .read_exact(&mut body)
    .map_err(|error| format!("failed to read Copilot JSON-RPC body: {error}"))?;
  serde_json::from_slice(&body)
    .map_err(|error| format!("failed to parse Copilot JSON-RPC body: {error}"))
}

fn resolve_server_command(workspace_root: &Path) -> Result<(OsString, Vec<OsString>), String> {
  let node = env::var_os("THE_EDITOR_COPILOT_NODE").unwrap_or_else(|| OsString::from("node"));
  let server = if let Some(server) = env::var_os("THE_EDITOR_COPILOT_LANGUAGE_SERVER") {
    PathBuf::from(server)
  } else {
    resolve_server_path(workspace_root)?
  };

  Ok((node, vec![
    OsString::from("--experimental-sqlite"),
    server.into_os_string(),
    OsString::from("--stdio"),
  ]))
}

fn resolve_server_path(workspace_root: &Path) -> Result<PathBuf, String> {
  const RELATIVE_SERVER: &str =
    "node_modules/@github/copilot-language-server/dist/language-server.js";

  let mut candidates = vec![
    workspace_root.join(RELATIVE_SERVER),
    PathBuf::from(env::var("HOME").unwrap_or_default())
      .join(".config/the-editor")
      .join(RELATIVE_SERVER),
  ];

  if let Ok(output) = ProcessCommand::new("npm").args(["root", "-g"]).output()
    && output.status.success()
  {
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !root.is_empty() {
      candidates
        .push(PathBuf::from(root).join("@github/copilot-language-server/dist/language-server.js"));
    }
  }

  candidates
    .into_iter()
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| {
      "Could not find @github/copilot-language-server. Set THE_EDITOR_COPILOT_LANGUAGE_SERVER or \
       install the package."
        .to_string()
    })
}

fn resolve_supermaven_binary_path(workspace_root: &Path) -> Result<PathBuf, String> {
  if let Some(path) = env::var_os("THE_EDITOR_SUPERMAVEN_BINARY") {
    let path = PathBuf::from(path);
    if path.is_file() {
      return Ok(path);
    }
  }

  let home = PathBuf::from(env::var("HOME").unwrap_or_default());
  let data_home = env::var_os("XDG_DATA_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| home.join(".supermaven"));
  let platform = match env::consts::OS {
    "macos" => "macosx",
    "linux" => "linux",
    "windows" => "windows",
    other => other,
  };
  let arch = match env::consts::ARCH {
    "aarch64" => "aarch64",
    "x86_64" => "x86_64",
    other => other,
  };
  let binary_name = if cfg!(windows) {
    "sm-agent.exe"
  } else {
    "sm-agent"
  };

  let candidates = [
    workspace_root
      .join(".supermaven")
      .join("binary")
      .join("v20")
      .join(format!("{platform}-{arch}"))
      .join(binary_name),
    data_home
      .join("binary")
      .join("v20")
      .join(format!("{platform}-{arch}"))
      .join(binary_name),
    home
      .join(".supermaven")
      .join("binary")
      .join("v20")
      .join(format!("{platform}-{arch}"))
      .join(binary_name),
  ];

  candidates
    .into_iter()
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| {
      "Could not find sm-agent. Set THE_EDITOR_SUPERMAVEN_BINARY or install Supermaven.".to_string()
    })
}

fn normalize_copilot_suggestion(
  request: &CopilotQuery,
  raw: WorkerSuggestionJson,
) -> Option<WorkerSuggestion> {
  let text = Rope::from(request.text.as_str());
  let mut from = utf16_position_to_char_idx(&text, raw.range.start.line, raw.range.start.character);
  let mut to = utf16_position_to_char_idx(&text, raw.range.end.line, raw.range.end.character);
  if from > to {
    std::mem::swap(&mut from, &mut to);
  }

  let existing = text.slice(from..to).to_string();
  let (prefix_chars, prefix_bytes) = shared_prefix(&existing, &raw.insert_text);
  let (suffix_chars, suffix_bytes) =
    shared_suffix(&existing[prefix_bytes..], &raw.insert_text[prefix_bytes..]);

  from = from.saturating_add(prefix_chars);
  to = to.saturating_sub(suffix_chars);

  let trimmed_bytes_end = raw.insert_text.len().saturating_sub(suffix_bytes);
  let trimmed = raw.insert_text[prefix_bytes..trimmed_bytes_end].to_string();
  if trimmed.trim().is_empty() {
    inline_completion_trace(
      "copilot_inline_completion_skip_empty_after_trim",
      json!({
        "uri": request.uri.as_str(),
        "version": request.version,
        "range": inline_completion_trace_range(Some(&raw.range)),
      }),
    );
    return None;
  }

  let display_range = json_range_for_char_range(&text, from, to);

  Some(WorkerSuggestion {
    from,
    to,
    text: trimmed,
    source: InlineSuggestionSource::CopilotInlineCompletion,
    command: raw.command,
    display_range,
  })
}

fn json_range_for_char_range(text: &Rope, from: usize, to: usize) -> Option<JsonRange> {
  let max_char = text.len_chars();
  if from > max_char || to > max_char {
    return None;
  }
  let (start_line, start_character) = char_idx_to_utf16_position(text, from);
  let (end_line, end_character) = char_idx_to_utf16_position(text, to);
  Some(JsonRange {
    start: JsonPosition {
      line:      start_line,
      character: start_character,
    },
    end:   JsonPosition {
      line:      end_line,
      character: end_character,
    },
  })
}

fn line_char_len_without_newline(line: ropey::RopeSlice<'_>) -> usize {
  let text = line.to_string();
  text.trim_end_matches(['\r', '\n']).chars().count()
}

fn shared_prefix(left: &str, right: &str) -> (usize, usize) {
  let mut chars = 0usize;
  let mut bytes = 0usize;
  for (a, b) in left.chars().zip(right.chars()) {
    if a != b {
      break;
    }
    chars += 1;
    bytes += a.len_utf8();
  }
  (chars, bytes)
}

fn shared_suffix(left: &str, right: &str) -> (usize, usize) {
  let mut chars = 0usize;
  let mut bytes = 0usize;
  for (a, b) in left.chars().rev().zip(right.chars().rev()) {
    if a != b {
      break;
    }
    chars += 1;
    bytes += a.len_utf8();
  }
  (chars, bytes)
}

fn next_word_fragment(text: &str) -> String {
  if text.is_empty() {
    return String::new();
  }

  let mut end = 0usize;
  let mut chars = text.char_indices().peekable();

  while let Some((idx, ch)) = chars.peek().copied() {
    if !ch.is_whitespace() {
      break;
    }
    end = idx + ch.len_utf8();
    chars.next();
  }

  while let Some((idx, ch)) = chars.peek().copied() {
    if ch.is_whitespace() {
      break;
    }
    end = idx + ch.len_utf8();
    chars.next();
  }

  while let Some((idx, ch)) = chars.peek().copied() {
    if !ch.is_whitespace() {
      break;
    }
    end = idx + ch.len_utf8();
    chars.next();
  }

  text[..end.max(1).min(text.len())].to_string()
}

fn next_line_fragment(text: &str) -> String {
  if text.is_empty() {
    return String::new();
  }

  match text.char_indices().find(|(_, ch)| *ch == '\n') {
    Some((idx, ch)) => text[..idx + ch.len_utf8()].to_string(),
    None => text.to_string(),
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InlineCompletionResult {
  items: Vec<WorkerSuggestionJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptUserDeviceFlow {
  user_code: String,
  command:   JsonCommand,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonCommand {
  title:     String,
  command:   String,
  arguments: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerSuggestionJson {
  insert_text: String,
  range:       JsonRange,
  command:     Option<JsonCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRange {
  start: JsonPosition,
  end:   JsonPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonPosition {
  line:      u32,
  character: u32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(tag = "status")]
enum SignInStatus {
  #[serde(rename = "OK")]
  Ok {
    user: Option<String>,
  },
  MaybeOk {
    user: String,
  },
  AlreadySignedIn {
    user: String,
  },
  NotAuthorized {
    user: String,
  },
  NotSignedIn,
}

fn is_authorized(status: &SignInStatus) -> bool {
  matches!(
    status,
    SignInStatus::Ok { .. } | SignInStatus::MaybeOk { .. } | SignInStatus::AlreadySignedIn { .. }
  )
}

fn authorized_user(status: &SignInStatus) -> Option<String> {
  match status {
    SignInStatus::Ok { user } => user.clone(),
    SignInStatus::MaybeOk { user } | SignInStatus::AlreadySignedIn { user } => Some(user.clone()),
    SignInStatus::NotAuthorized { .. } | SignInStatus::NotSignedIn => None,
  }
}

fn sign_in_status_message(status: &SignInStatus) -> String {
  match status {
    SignInStatus::Ok { user } => {
      match user {
        Some(user) => format!("Copilot connected as {user}"),
        None => "Copilot connected".to_string(),
      }
    },
    SignInStatus::MaybeOk { user } | SignInStatus::AlreadySignedIn { user } => {
      format!("Copilot connected as {user}")
    },
    SignInStatus::NotAuthorized { user } => {
      format!(
        "Copilot is not authorized for {user}. Run :copilot-sign-in to start the GitHub device \
         flow."
      )
    },
    SignInStatus::NotSignedIn => {
      "Copilot is not signed in. Run :copilot-sign-in to connect GitHub Copilot.".to_string()
    },
  }
}

#[cfg(test)]
mod tests {
  use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    path::{
      Path,
      PathBuf,
    },
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
    },
    messages::MessageCenter,
    registers::Registers,
    render::{
      FrameRenderPlan,
      GutterConfig,
      RenderPlan,
      graphics::Rect,
      text_annotations::TextAnnotations,
      text_format::TextFormat,
      theme::{
        Theme,
        default_theme,
      },
    },
    selection::Selection,
    view::ViewState,
  };

  use super::{
    InFlightQuery,
    InlineCompletionProvider,
    InlineCompletionState,
    InlineSuggestion,
    InlineSuggestionSource,
    QueryKey,
    WorkerEvent,
    WorkerSuggestion,
    accept_inline_completion_line,
    apply_worker_event,
    completion_menu_inline_item,
    copilot_incremental_change,
    handle_pre_on_keypress,
    next_line_fragment,
    next_word_fragment,
    preview_text_for_cursor,
    replacement_first_line_len,
    shared_prefix,
    shared_suffix,
    split_preview_lines,
    sync_inline_completion_annotations,
  };
  use crate::{
    CommandPaletteState,
    CommandPaletteStyle,
    CommandPromptState,
    CommandRegistry,
    CompletionMenuState,
    DefaultContext,
    DispatchRef,
    FilePickerState,
    FileTreeState,
    Key,
    KeyBinding,
    KeyEvent,
    Keymaps,
    Mode,
    Motion,
    OwnedTextAnnotations,
    PendingInput,
    RenderWaker,
    SearchPromptState,
    WorkingDirectoryState,
  };

  struct TestCtx {
    editor:                        Editor,
    messages:                      MessageCenter,
    workspace_root:                PathBuf,
    file_path:                     Option<PathBuf>,
    working_directory:             WorkingDirectoryState,
    mode:                          Mode,
    render_requested:              bool,
    completion_menu:               CompletionMenuState,
    inline_completion:             InlineCompletionState,
    inline_completion_annotations: OwnedTextAnnotations,
    file_picker:                   FilePickerState,
    file_tree:                     FileTreeState,
    picker_runtime:                crate::PickerRuntimeStore<TestCtx>,
  }

  impl TestCtx {
    fn new() -> Self {
      Self::with_text("")
    }

    fn with_text(text: &str) -> Self {
      let view = ViewState::new(
        Rect::new(0, 0, 80, 24),
        the_lib::position::Position::new(0, 0),
      );
      let editor = Editor::new(
        EditorId::new(NonZeroUsize::new(1).unwrap()),
        Document::new(
          DocumentId::new(NonZeroUsize::new(1).unwrap()),
          Rope::from_str(text),
        ),
        view,
      );
      let workspace_root = PathBuf::from("/tmp");
      Self {
        editor,
        messages: MessageCenter::default(),
        workspace_root: workspace_root.clone(),
        file_path: Some(PathBuf::from("/tmp/demo.rs")),
        working_directory: WorkingDirectoryState {
          current:  Some(workspace_root),
          previous: None,
        },
        mode: Mode::Insert,
        render_requested: false,
        completion_menu: CompletionMenuState::default(),
        inline_completion: InlineCompletionState::default(),
        inline_completion_annotations: OwnedTextAnnotations::default(),
        file_picker: FilePickerState::default(),
        file_tree: FileTreeState::default(),
        picker_runtime: crate::PickerRuntimeStore::default(),
      }
    }
  }

  impl DefaultContext for TestCtx {
    fn editor(&mut self) -> &mut Editor {
      &mut self.editor
    }

    fn editor_ref(&self) -> &Editor {
      &self.editor
    }

    fn file_path(&self) -> Option<&Path> {
      self.file_path.as_deref()
    }

    fn workspace_root(&self) -> PathBuf {
      self.workspace_root.clone()
    }

    fn working_directory_state(&self) -> &WorkingDirectoryState {
      &self.working_directory
    }

    fn working_directory_state_mut(&mut self) -> &mut WorkingDirectoryState {
      &mut self.working_directory
    }

    fn request_render(&mut self) {
      self.render_requested = true;
    }

    fn render_waker(&self) -> RenderWaker {
      let (tx, _rx) = std::sync::mpsc::channel();
      RenderWaker::new(tx)
    }

    fn messages(&self) -> &MessageCenter {
      &self.messages
    }

    fn messages_mut(&mut self) -> &mut MessageCenter {
      &mut self.messages
    }

    fn build_render_plan(&mut self) -> RenderPlan {
      todo!()
    }

    fn build_frame_render_plan(&mut self) -> FrameRenderPlan {
      todo!()
    }

    fn request_quit(&mut self) {}

    fn mode(&self) -> Mode {
      self.mode
    }

    fn set_mode(&mut self, mode: Mode) {
      self.mode = mode;
    }

    fn keymaps(&mut self) -> &mut Keymaps {
      todo!()
    }

    fn command_prompt_mut(&mut self) -> &mut CommandPromptState {
      todo!()
    }

    fn command_prompt_ref(&self) -> &CommandPromptState {
      todo!()
    }

    fn command_registry_mut(&mut self) -> &mut CommandRegistry<Self> {
      todo!()
    }

    fn command_registry_ref(&self) -> &CommandRegistry<Self> {
      todo!()
    }

    fn command_palette(&self) -> &CommandPaletteState {
      todo!()
    }

    fn command_palette_mut(&mut self) -> &mut CommandPaletteState {
      todo!()
    }

    fn command_palette_style(&self) -> &CommandPaletteStyle {
      todo!()
    }

    fn command_palette_style_mut(&mut self) -> &mut CommandPaletteStyle {
      todo!()
    }

    fn completion_menu(&self) -> &CompletionMenuState {
      &self.completion_menu
    }

    fn completion_menu_mut(&mut self) -> &mut CompletionMenuState {
      &mut self.completion_menu
    }

    fn completion_menu_keymaps(&self) -> &Keymaps {
      todo!()
    }

    fn completion_menu_keymaps_mut(&mut self) -> &mut Keymaps {
      todo!()
    }

    fn inline_completion(&self) -> &InlineCompletionState {
      &self.inline_completion
    }

    fn inline_completion_mut(&mut self) -> &mut InlineCompletionState {
      &mut self.inline_completion
    }

    fn file_picker(&self) -> &FilePickerState {
      &self.file_picker
    }

    fn file_picker_mut(&mut self) -> &mut FilePickerState {
      &mut self.file_picker
    }

    fn file_tree(&self) -> &FileTreeState {
      &self.file_tree
    }

    fn file_tree_mut(&mut self) -> &mut FileTreeState {
      &mut self.file_tree
    }

    fn picker_runtime_store(&self) -> &crate::PickerRuntimeStore<Self> {
      &self.picker_runtime
    }

    fn picker_runtime_store_mut(&mut self) -> &mut crate::PickerRuntimeStore<Self> {
      &mut self.picker_runtime
    }

    fn search_prompt_ref(&self) -> &SearchPromptState {
      todo!()
    }

    fn search_prompt_mut(&mut self) -> &mut SearchPromptState {
      todo!()
    }

    fn dispatch(&self) -> DispatchRef<Self> {
      todo!()
    }

    fn pending_input(&self) -> Option<&PendingInput> {
      None
    }

    fn set_pending_input(&mut self, _pending: Option<PendingInput>) {}

    fn set_inline_completion_annotations(&mut self, annotations: OwnedTextAnnotations) {
      self.inline_completion_annotations = annotations;
    }

    fn clear_inline_completion_annotations(&mut self) {
      self.inline_completion_annotations = OwnedTextAnnotations::default();
    }

    fn registers(&self) -> &Registers {
      todo!()
    }

    fn registers_mut(&mut self) -> &mut Registers {
      todo!()
    }

    fn register(&self) -> Option<char> {
      None
    }

    fn set_register(&mut self, _register: Option<char>) {}

    fn macro_recording(&self) -> &Option<(char, Vec<KeyBinding>)> {
      todo!()
    }

    fn set_macro_recording(&mut self, _recording: Option<(char, Vec<KeyBinding>)>) {}

    fn macro_replaying(&self) -> &Vec<char> {
      todo!()
    }

    fn macro_replaying_mut(&mut self) -> &mut Vec<char> {
      todo!()
    }

    fn macro_queue(&self) -> &VecDeque<KeyEvent> {
      todo!()
    }

    fn macro_queue_mut(&mut self) -> &mut VecDeque<KeyEvent> {
      todo!()
    }

    fn last_motion(&self) -> Option<Motion> {
      None
    }

    fn set_last_motion(&mut self, _motion: Option<Motion>) {}

    fn text_format(&self) -> TextFormat {
      TextFormat::default()
    }

    fn soft_wrap_enabled(&self) -> bool {
      false
    }

    fn set_soft_wrap_enabled(&mut self, _enabled: bool) {}

    fn gutter_config(&self) -> &GutterConfig {
      todo!()
    }

    fn gutter_config_mut(&mut self) -> &mut GutterConfig {
      todo!()
    }

    fn text_annotations(&self) -> TextAnnotations<'_> {
      todo!()
    }

    fn syntax_loader(&self) -> Option<&the_lib::syntax::Loader> {
      None
    }

    fn ui_theme(&self) -> &Theme {
      default_theme()
    }

    fn ui_theme_name(&self) -> &str {
      "test"
    }

    fn available_theme_names(&self) -> Vec<String> {
      vec!["test".to_string()]
    }

    fn set_ui_theme(&mut self, _theme_name: &str) -> Result<(), String> {
      Ok(())
    }

    fn set_ui_theme_preview(&mut self, _theme_name: &str) -> Result<(), String> {
      Ok(())
    }

    fn clear_ui_theme_preview(&mut self) {}

    fn set_file_path(&mut self, path: Option<PathBuf>) {
      self.file_path = path;
    }

    fn open_file(&mut self, _path: &Path) -> std::io::Result<()> {
      Ok(())
    }
  }

  fn test_inline_suggestion(key: QueryKey, from: usize, to: usize, text: &str) -> InlineSuggestion {
    InlineSuggestion {
      key,
      from,
      to,
      text: text.to_string(),
      source: InlineSuggestionSource::CopilotInlineCompletion,
      command: None,
      display_range: None,
      reported_to_provider: false,
    }
  }

  fn test_query_key(ctx: &TestCtx, cursor_char: usize) -> QueryKey {
    QueryKey {
      buffer_id: ctx.editor.active_buffer_id(),
      doc_version: ctx.editor.document().version(),
      cursor_char,
      file_path: "/tmp/demo.rs".into(),
    }
  }

  #[test]
  fn provider_parse_accepts_expected_labels() {
    assert_eq!(
      InlineCompletionProvider::parse("copilot"),
      Some(InlineCompletionProvider::Copilot)
    );
    assert_eq!(
      InlineCompletionProvider::parse("supermaven"),
      Some(InlineCompletionProvider::Supermaven)
    );
    assert_eq!(
      InlineCompletionProvider::parse("off"),
      Some(InlineCompletionProvider::None)
    );
    assert_eq!(InlineCompletionProvider::parse("wat"), None);
  }

  #[test]
  fn next_word_fragment_includes_trailing_space() {
    assert_eq!(next_word_fragment("hello world"), "hello ");
    assert_eq!(next_word_fragment("  hello world"), "  hello ");
    assert_eq!(next_word_fragment("x"), "x");
  }

  #[test]
  fn next_line_fragment_stops_at_newline() {
    assert_eq!(next_line_fragment("hello\nworld"), "hello\n");
    assert_eq!(next_line_fragment("hello"), "hello");
  }

  #[test]
  fn preview_text_skips_existing_prefix() {
    let suggestion = test_inline_suggestion(
      QueryKey {
        buffer_id:   super::BufferId::new(std::num::NonZeroUsize::new(1).unwrap()),
        doc_version: 1,
        cursor_char: 4,
        file_path:   "/tmp/demo.rs".into(),
      },
      0,
      0,
      "hello world",
    );

    assert_eq!(
      preview_text_for_cursor("hell".to_string(), 4, &suggestion),
      "o world"
    );
  }

  #[test]
  fn escape_dismisses_inline_completion_without_consuming_escape() {
    let mut ctx = TestCtx::new();
    let _ = ctx
      .inline_completion_annotations
      .add_inline_text(0, "ghost", None);
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      QueryKey {
        buffer_id:   ctx.editor.active_buffer_id(),
        doc_version: ctx.editor.document().version(),
        cursor_char: 0,
        file_path:   "/tmp/demo.rs".into(),
      },
      0,
      0,
      "ghost",
    ));

    let handled = handle_pre_on_keypress(&mut ctx, KeyEvent {
      key:       Key::Escape,
      modifiers: crate::Modifiers::empty(),
    });

    assert!(!handled);
    assert!(ctx.inline_completion.suggestion.is_none());
    assert!(ctx.inline_completion_annotations.is_empty());
    assert!(ctx.render_requested);
  }

  #[test]
  fn escape_preserves_inline_completion_when_completion_menu_is_active() {
    let mut ctx = TestCtx::new();
    ctx.completion_menu.active = true;
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      QueryKey {
        buffer_id:   ctx.editor.active_buffer_id(),
        doc_version: ctx.editor.document().version(),
        cursor_char: 0,
        file_path:   "/tmp/demo.rs".into(),
      },
      0,
      0,
      "ghost",
    ));

    let handled = handle_pre_on_keypress(&mut ctx, KeyEvent {
      key:       Key::Escape,
      modifiers: crate::Modifiers::empty(),
    });

    assert!(!handled);
    assert!(ctx.inline_completion.suggestion.is_some());
  }

  #[test]
  fn line_accept_inserts_only_the_next_line() {
    let mut ctx = TestCtx::new();
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      QueryKey {
        buffer_id:   ctx.editor.active_buffer_id(),
        doc_version: ctx.editor.document().version(),
        cursor_char: 0,
        file_path:   "/tmp/demo.rs".into(),
      },
      0,
      0,
      "hello\nworld",
    ));

    assert!(accept_inline_completion_line(&mut ctx));
    assert_eq!(ctx.editor.document().text().to_string(), "hello\n");
  }

  #[test]
  fn copilot_ghost_text_keeps_annotations_without_popover() {
    let mut ctx = TestCtx::new();
    ctx.inline_completion.provider = InlineCompletionProvider::Copilot;
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      test_query_key(&ctx, 0),
      0,
      0,
      "printf(\"hello world\");",
    ));

    sync_inline_completion_annotations(&mut ctx);

    assert!(ctx.inline_completion.presentation.is_none());
    assert!(!ctx.inline_completion_annotations.is_empty());
  }

  #[test]
  fn non_local_inline_completion_does_not_render_prediction() {
    let mut ctx = TestCtx::with_text("line 1\nline 2\nline 3\n");
    ctx.inline_completion.provider = InlineCompletionProvider::Copilot;
    let cursor_char = ctx.editor.document().text().line_to_char(0);
    let target_char = ctx.editor.document().text().line_to_char(1);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(cursor_char));
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      test_query_key(&ctx, cursor_char),
      target_char,
      target_char,
      "printf(\"hello world\");",
    ));

    sync_inline_completion_annotations(&mut ctx);

    assert!(ctx.inline_completion.presentation.is_none());
    assert!(ctx.inline_completion_annotations.is_empty());
  }

  #[test]
  fn completion_menu_keeps_ghost_text_without_prediction_popover() {
    let mut ctx = TestCtx::new();
    ctx.inline_completion.provider = InlineCompletionProvider::Copilot;
    ctx.completion_menu.active = true;
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      test_query_key(&ctx, 0),
      0,
      0,
      "printf(\"hello world\");",
    ));

    sync_inline_completion_annotations(&mut ctx);

    assert!(ctx.inline_completion.presentation.is_none());
    assert!(!ctx.inline_completion_annotations.is_empty());
  }

  #[test]
  fn large_multiline_insert_switches_to_compact_popover() {
    let mut ctx = TestCtx::new();
    ctx.inline_completion.provider = InlineCompletionProvider::Copilot;
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      test_query_key(&ctx, 0),
      0,
      0,
      "printf(\"hello\");\nlet first = 1;\nlet second = 2;\nlet third = 3;",
    ));

    sync_inline_completion_annotations(&mut ctx);

    assert!(!ctx.inline_completion_annotations.is_empty());
    assert!(ctx.inline_completion_annotations.virtual_lines().is_empty());
    let presentation = ctx
      .inline_completion
      .presentation
      .as_ref()
      .expect("presentation");
    assert_eq!(presentation.kind, InlineCompletionPresentationKind::Popover);
    assert_eq!(presentation.lines.len(), 3);
    assert_eq!(
      presentation.lines[0].kind,
      InlineCompletionPresentationLineKind::Addition
    );
    assert_eq!(presentation.lines[0].text, "let first = 1;");
  }

  #[test]
  fn replacement_suggestion_uses_diff_presentation_without_inline_annotations() {
    let mut ctx = TestCtx::with_text("foo()\n");
    ctx.inline_completion.provider = InlineCompletionProvider::Copilot;
    let _ = ctx.editor.document_mut().set_selection(Selection::point(0));
    ctx.inline_completion.suggestion = Some(test_inline_suggestion(
      test_query_key(&ctx, 0),
      0,
      5,
      "bar()",
    ));

    sync_inline_completion_annotations(&mut ctx);

    assert!(ctx.inline_completion_annotations.is_empty());
    let presentation = ctx
      .inline_completion
      .presentation
      .as_ref()
      .expect("presentation");
    assert_eq!(presentation.kind, InlineCompletionPresentationKind::Diff);
    assert_eq!(presentation.lines.len(), 2);
    assert_eq!(
      presentation.lines[0].kind,
      InlineCompletionPresentationLineKind::Removal
    );
    assert_eq!(presentation.lines[0].text, "foo()");
    assert_eq!(
      presentation.lines[1].kind,
      InlineCompletionPresentationLineKind::Addition
    );
    assert_eq!(presentation.lines[1].text, "bar()");
  }

  #[test]
  fn completion_menu_inline_item_uses_provider_metadata() {
    let mut ctx = TestCtx::new();
    ctx.inline_completion.provider = InlineCompletionProvider::Supermaven;
    let mut suggestion = test_inline_suggestion(
      test_query_key(&ctx, 0),
      0,
      0,
      "printf(\"hello world\");\nreturn 0;",
    );
    suggestion.source = InlineSuggestionSource::Supermaven;
    ctx.inline_completion.suggestion = Some(suggestion);

    let item = completion_menu_inline_item(&ctx).expect("completion item");

    assert_eq!(item.label, "printf(\"hello world\");");
    assert_eq!(item.detail.as_deref(), Some("Supermaven"));
    assert_eq!(
      item.documentation.as_deref(),
      Some("printf(\"hello world\");\nreturn 0;")
    );
    assert_eq!(item.kind_icon.as_deref(), Some("supermaven"));
    assert_eq!(
      item.kind_color,
      Some(the_lib::render::graphics::Color::Rgb(0xF2, 0xC1, 0x63))
    );
  }

  #[test]
  fn query_result_drops_non_local_inline_completion() {
    let mut ctx = TestCtx::with_text("line 1\nline 2\nline 3\n");
    ctx.inline_completion.provider = InlineCompletionProvider::Copilot;
    let cursor_char = ctx.editor.document().text().line_to_char(0);
    let target_char = ctx.editor.document().text().line_to_char(1);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(cursor_char));
    ctx.inline_completion.in_flight = Some(InFlightQuery {
      request_id: 7,
      key:        test_query_key(&ctx, cursor_char),
    });

    apply_worker_event(&mut ctx, WorkerEvent::QueryResult {
      request_id: 7,
      result:     Ok(Some(WorkerSuggestion {
        from:          target_char,
        to:            target_char,
        text:          "printf(\"hello world\");".to_string(),
        source:        InlineSuggestionSource::CopilotInlineCompletion,
        command:       None,
        display_range: None,
      })),
    });

    assert!(ctx.inline_completion.suggestion.is_none());
  }

  #[test]
  fn copilot_incremental_change_tracks_single_insert() {
    let change = copilot_incremental_change("abc\n", "abxc\n").expect("change");

    assert_eq!(change.start_char, 2);
    assert_eq!(change.end_char, 2);
    assert_eq!(change.text, "x");
    assert_eq!(change.range.start.line, 0);
    assert_eq!(change.range.start.character, 2);
    assert_eq!(change.range.end.line, 0);
    assert_eq!(change.range.end.character, 2);
  }

  #[test]
  fn copilot_incremental_change_tracks_multiline_replacement() {
    let change = copilot_incremental_change("a\nold\nc\n", "a\nnew value\nc\n").expect("change");

    assert_eq!(change.text, "new value");
    assert_eq!(change.range.start.line, 1);
    assert_eq!(change.range.start.character, 0);
    assert_eq!(change.range.end.line, 1);
    assert_eq!(change.range.end.character, 3);
  }

  #[test]
  fn split_preview_lines_returns_first_and_remaining() {
    assert_eq!(
      split_preview_lines("hello\nworld"),
      ("hello".to_string(), "world".to_string())
    );
    assert_eq!(
      split_preview_lines("hello"),
      ("hello".to_string(), String::new())
    );
  }

  #[test]
  fn replacement_first_line_len_stops_at_newline() {
    assert_eq!(replacement_first_line_len("hello\nworld"), 5);
    assert_eq!(replacement_first_line_len("hello"), 5);
  }

  #[test]
  fn shared_prefix_and_suffix_count_chars_and_bytes() {
    assert_eq!(shared_prefix("abc", "abd"), (2, 2));
    assert_eq!(shared_suffix("xyz", "qyz"), (2, 2));
  }
}
