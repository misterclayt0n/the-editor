use std::{
  env,
  ffi::OsString,
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
  sync::mpsc::{
    self,
    Receiver,
    Sender,
    TryRecvError,
  },
  thread,
  time::{
    Duration,
    Instant,
  },
};

use serde::Deserialize;
use serde_json::{
  Value,
  json,
};
use the_default::{
  CommandBuilder,
  DefaultApi,
  DefaultContext,
  EditorPreset,
  Key,
  KeyEvent,
  Mode,
  build_dispatch as default_dispatch,
  default_editor_preset,
  default_pre_on_keypress,
};
use the_lib::{
  editor::BufferId,
  indent::IndentStyle,
  render::RenderPlan,
  selection::{
    CursorId,
    Selection,
  },
  syntax::Highlight,
  transaction::Transaction,
};
use the_lsp::text_sync::{
  char_idx_to_utf16_position,
  file_uri_for_path,
  utf16_position_to_char_idx,
};

const SOURCE: &str = "copilot";
const DEBOUNCE: Duration = Duration::from_millis(75);
const RETRY_AFTER_ERROR: Duration = Duration::from_secs(10);
const EDITOR_NAME: &str = "the-editor";
const EDITOR_PLUGIN_NAME: &str = "the-editor-copilot-config";

pub fn build_dispatch<Ctx>() -> impl DefaultApi<Ctx>
where
  Ctx: DefaultContext,
{
  default_dispatch::<Ctx>()
    .with_pre_on_keypress(pre_on_keypress::<Ctx>)
    .with_pre_render(pre_render::<Ctx>)
}

pub fn build_editor_preset<Ctx>() -> EditorPreset<Ctx, impl DefaultApi<Ctx>>
where
  Ctx: DefaultContext,
{
  default_editor_preset::<Ctx>()
    .with_dispatch(build_dispatch::<Ctx>())
    .install_extension_state(CopilotState::default())
    .install_render_plan_post_processor(render_copilot_preview::<Ctx>)
    .install_command(
      CommandBuilder::new(
        "copilot-toggle",
        "Toggle Copilot inline completions",
        |ctx, _args, _event| {
          toggle_copilot(ctx);
          Ok(())
        },
      )
      .build(),
    )
    .install_command(
      CommandBuilder::new(
        "copilot-sign-in",
        "Start GitHub Copilot device sign-in",
        |ctx, _args, _event| {
          start_copilot_sign_in(ctx);
          Ok(())
        },
      )
      .build(),
    )
    .install_command(
      CommandBuilder::new(
        "copilot-status",
        "Show Copilot backend status",
        |ctx, _args, _event| {
          show_copilot_status(ctx);
          Ok(())
        },
      )
      .build(),
    )
    .install_command(
      CommandBuilder::new(
        "copilot-accept",
        "Accept the active Copilot suggestion",
        |ctx, _args, _event| {
          accept_suggestion(ctx, AcceptKind::Full);
          Ok(())
        },
      )
      .build(),
    )
    .install_command(
      CommandBuilder::new(
        "copilot-accept-word",
        "Accept the next word from the active Copilot suggestion",
        |ctx, _args, _event| {
          accept_suggestion(ctx, AcceptKind::Word);
          Ok(())
        },
      )
      .build(),
    )
    .install_command(
      CommandBuilder::new(
        "copilot-dismiss",
        "Dismiss the active Copilot suggestion",
        |ctx, _args, _event| {
          dismiss_suggestion(ctx);
          Ok(())
        },
      )
      .build(),
    )
    .install_command(
      CommandBuilder::new(
        "copilot-retry",
        "Retry Copilot backend startup immediately",
        |ctx, _args, _event| {
          retry_copilot(ctx);
          Ok(())
        },
      )
      .build(),
    )
}

fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if ctx.mode() == Mode::Insert && !ctx.completion_menu().active {
    match key.key {
      Key::Tab if key.modifiers.is_empty() => {
        if accept_suggestion(ctx, AcceptKind::Full) {
          return;
        }
      },
      Key::Char(']') if key.modifiers.alt() => {
        if accept_suggestion(ctx, AcceptKind::Word) {
          return;
        }
      },
      Key::Escape => {
        dismiss_suggestion(ctx);
      },
      _ => {},
    }
  }

  default_pre_on_keypress(ctx, key);
}

fn pre_render<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  drain_worker_events(ctx);
  schedule_or_send_query(ctx);
}

fn render_copilot_preview<Ctx: DefaultContext>(ctx: &mut Ctx, plan: &mut RenderPlan) {
  if ctx.mode() != Mode::Insert || ctx.completion_menu().active {
    return;
  }

  let Some(state) = ctx.extension_state::<CopilotState>() else {
    return;
  };
  let Some(suggestion) = state.suggestion.as_ref() else {
    return;
  };
  let Some(cursor) = plan.cursors.first().cloned() else {
    return;
  };

  let editor = ctx.editor_ref();
  let document = editor.document();
  let Some(range) = single_active_range(document.selection(), editor.view().active_cursor) else {
    return;
  };
  if !range.is_empty() {
    return;
  }
  let cursor_char = range.head;
  let text = document.text().slice(..);
  let existing_prefix = if cursor_char > suggestion.from {
    text
      .slice(suggestion.from..cursor_char.min(text.len_chars()))
      .to_string()
  } else {
    String::new()
  };
  let preview_text = preview_text_for_cursor(existing_prefix, cursor_char, suggestion);
  if preview_text.is_empty() {
    return;
  }

  let mut lines = preview_text.lines();
  let first_line = lines.next().unwrap_or_default();
  if !first_line.is_empty() {
    let style = highlight_style(ctx.ui_theme().find_highlight("ui.virtual.inline"), ctx);
    let _ = plan.add_overlay_text(cursor.pos, first_line.to_string(), style);
  }
}

fn highlight_style<Ctx: DefaultContext>(
  highlight: Option<Highlight>,
  ctx: &Ctx,
) -> the_lib::render::graphics::Style {
  highlight
    .map(|highlight| ctx.ui_theme().highlight(highlight))
    .unwrap_or_default()
}

fn preview_text_for_cursor(
  existing_prefix: String,
  cursor_char: usize,
  suggestion: &CopilotSuggestion,
) -> String {
  if cursor_char <= suggestion.from {
    return suggestion.text.clone();
  }

  let (_, prefix_bytes) = shared_prefix(&existing_prefix, &suggestion.text);
  suggestion.text[prefix_bytes..].to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptKind {
  Full,
  Word,
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
  request: CopilotQuery,
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

#[derive(Debug, Clone)]
struct CopilotSuggestion {
  key:  QueryKey,
  from: usize,
  to:   usize,
  text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendStatus {
  Idle,
  Starting,
  Ready,
  Error,
}

#[derive(Debug)]
struct CopilotTransport {
  tx: Sender<WorkerCommand>,
  rx: Receiver<WorkerEvent>,
}

#[derive(Debug)]
struct CopilotState {
  enabled:             bool,
  status:              BackendStatus,
  transport:           Option<CopilotTransport>,
  sign_in_code:        Option<String>,
  scheduled:           Option<ScheduledQuery>,
  in_flight:           Option<InFlightQuery>,
  suggestion:          Option<CopilotSuggestion>,
  last_completed_key:  Option<QueryKey>,
  last_error:          Option<String>,
  last_reported_error: Option<String>,
  retry_at:            Option<Instant>,
  next_request_id:     u64,
}

impl Default for CopilotState {
  fn default() -> Self {
    Self {
      enabled:             true,
      status:              BackendStatus::Idle,
      transport:           None,
      sign_in_code:        None,
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
}

fn toggle_copilot<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let enabled = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    state.enabled = !state.enabled;
    if !state.enabled {
      state.sign_in_code = None;
      state.scheduled = None;
      state.in_flight = None;
      state.suggestion = None;
    } else {
      state.retry_at = None;
      state.last_error = None;
    }
    state.enabled
  };

  if enabled {
    ctx.push_info(SOURCE, "Copilot inline completions enabled");
  } else {
    ctx.push_info(SOURCE, "Copilot inline completions disabled");
  }
}

fn start_copilot_sign_in<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if ensure_transport(ctx).is_err() {
    return;
  }

  let workspace_root = ctx.workspace_root();
  let send_result = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    state.status = BackendStatus::Starting;
    state.last_error = None;
    state.sign_in_code = None;
    match state.transport.as_ref() {
      Some(transport) => transport
        .tx
        .send(WorkerCommand::SignIn {
          workspace_root,
        })
        .map_err(|error| format!("failed to send Copilot sign-in request to worker: {error}")),
      None => Err("Copilot transport is unavailable".to_string()),
    }
  };

  match send_result {
    Ok(()) => {
      ctx.push_info(
        SOURCE,
        "Starting Copilot sign-in. If your browser does not open, wait for the device code and \
         visit https://github.com/login/device.",
      );
      ctx.request_render();
    },
    Err(error) => apply_worker_event(ctx, WorkerEvent::Error(error)),
  }
}

fn retry_copilot<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let state = ctx.extension_state_or_default::<CopilotState>();
  state.retry_at = None;
  state.last_error = None;
  state.sign_in_code = None;
  state.last_completed_key = None;
  state.status = if state.transport.is_some() {
    BackendStatus::Ready
  } else {
    BackendStatus::Idle
  };
  ctx.push_info(SOURCE, "Copilot backend retry cleared");
}

fn dismiss_suggestion<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let dismissed = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    state.suggestion.take().is_some()
  };
  if dismissed {
    ctx.request_render();
  }
  dismissed
}

fn accept_suggestion<Ctx: DefaultContext>(ctx: &mut Ctx, kind: AcceptKind) -> bool {
  let suggestion = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    state.suggestion.clone()
  };
  let Some(suggestion) = suggestion else {
    return false;
  };

  let accepted = match kind {
    AcceptKind::Full => suggestion.text.clone(),
    AcceptKind::Word => next_word_fragment(&suggestion.text),
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
    ctx.push_error(SOURCE, "failed to apply Copilot suggestion");
    return false;
  }

  let _ = ctx.editor().document_mut().commit();
  {
    let state = ctx.extension_state_or_default::<CopilotState>();
    state.suggestion = None;
    state.last_completed_key = None;
  }
  ctx.request_render();
  true
}

fn show_copilot_status<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let message = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    match (
      state.enabled,
      state.status,
      state.sign_in_code.as_ref(),
      state.suggestion.as_ref(),
      state.last_error.as_ref(),
    ) {
      (false, ..) => "Copilot is disabled".to_string(),
      (_, _, _, _, Some(error)) => format!("Copilot backend error: {error}"),
      (_, BackendStatus::Starting, Some(code), _, _) => {
        format!(
          "Copilot sign-in pending. Enter code {code} at https://github.com/login/device"
        )
      },
      (_, BackendStatus::Starting, None, _, _) => "Copilot backend is starting".to_string(),
      (_, BackendStatus::Ready, _, Some(suggestion), _) => {
        format!(
          "Copilot ready: active suggestion {} chars",
          suggestion.text.chars().count()
        )
      },
      (_, BackendStatus::Ready, _, None, _) => "Copilot ready: no active suggestion".to_string(),
      (_, BackendStatus::Idle, _, _, _) => {
        "Copilot idle. Run :copilot-sign-in if you have not connected GitHub Copilot yet."
          .to_string()
      },
      (_, BackendStatus::Error, _, _, _) => "Copilot backend is in an error state".to_string(),
    }
  };
  ctx.push_info(SOURCE, message);
}

fn drain_worker_events<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let mut events = Vec::new();
  {
    let Some(state) = ctx.extension_state::<CopilotState>() else {
      return;
    };
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
      let state = ctx.extension_state_or_default::<CopilotState>();
      state.status = BackendStatus::Ready;
      state.last_error = None;
      state.retry_at = None;
    },
    WorkerEvent::AuthPrompt {
      user_code,
    } => {
      {
        let state = ctx.extension_state_or_default::<CopilotState>();
        state.status = BackendStatus::Starting;
        state.sign_in_code = Some(user_code.clone());
        state.last_error = None;
        state.retry_at = None;
      }
      ctx.push_info(
        SOURCE,
        format!(
          "Copilot sign-in code: {user_code}. Open https://github.com/login/device if the \
           browser does not open automatically."
        ),
      );
    },
    WorkerEvent::Authenticated {
      user,
    } => {
      {
        let state = ctx.extension_state_or_default::<CopilotState>();
        state.status = BackendStatus::Ready;
        state.sign_in_code = None;
        state.last_error = None;
        state.retry_at = None;
      }
      match user {
        Some(user) => ctx.push_info(SOURCE, format!("Copilot connected as {user}")),
        None => ctx.push_info(SOURCE, "Copilot connected"),
      };
    },
    WorkerEvent::Error(error) => {
      let mut report = None;
      {
        let state = ctx.extension_state_or_default::<CopilotState>();
        state.status = BackendStatus::Error;
        state.last_error = Some(error.clone());
        state.in_flight = None;
        state.suggestion = None;
        state.retry_at = Some(Instant::now() + RETRY_AFTER_ERROR);
        if state.last_reported_error.as_deref() != Some(error.as_str()) {
          state.last_reported_error = Some(error.clone());
          report = Some(error);
        }
      }
      if let Some(error) = report {
        ctx.push_error(SOURCE, error);
      }
    },
    WorkerEvent::QueryResult { request_id, result } => {
      let ticket = {
        let state = ctx.extension_state_or_default::<CopilotState>();
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
        return;
      };

      match result {
        Ok(worker_suggestion) => {
          let next = worker_suggestion.and_then(|raw| {
            let prepared = current_prepared_query(ctx)?;
            if prepared.key != ticket.key {
              return None;
            }
            trim_worker_suggestion(ctx, prepared.key, raw)
          });
          let state = ctx.extension_state_or_default::<CopilotState>();
          state.status = BackendStatus::Ready;
          state.suggestion = next;
          ctx.request_render();
        },
        Err(error) => {
          apply_worker_event(ctx, WorkerEvent::Error(error));
        },
      }
    },
  }
}

fn schedule_or_send_query<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let now = Instant::now();
  let prepared = current_prepared_query(ctx);
  let mut should_spin;
  let mut to_send = None;

  {
    let state = ctx.extension_state_or_default::<CopilotState>();
    if !state.enabled {
      state.scheduled = None;
      state.in_flight = None;
      state.suggestion = None;
      return;
    }

    if let Some(retry_at) = state.retry_at
      && now < retry_at
    {
      return;
    }

    let Some(prepared) = prepared else {
      state.scheduled = None;
      state.suggestion = None;
      state.last_completed_key = None;
      return;
    };

    if state
      .suggestion
      .as_ref()
      .is_some_and(|suggestion| suggestion.key != prepared.key)
    {
      state.suggestion = None;
    }

    if state
      .in_flight
      .as_ref()
      .is_some_and(|ticket| ticket.key == prepared.key)
    {
      should_spin = true;
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
      should_spin = true;
      if state
        .scheduled
        .as_ref()
        .is_some_and(|scheduled| scheduled.ready_at <= now)
        && state.in_flight.is_none()
      {
        to_send = state.scheduled.take().map(|scheduled| scheduled.prepared);
      }
    } else {
      state.scheduled = Some(ScheduledQuery {
        prepared,
        ready_at: now + DEBOUNCE,
      });
      should_spin = true;
    }
  }

  if let Some(prepared) = to_send {
    send_query(ctx, prepared);
    should_spin = true;
  }

  if should_spin {
    ctx.request_render();
  }
}

fn send_query<Ctx: DefaultContext>(ctx: &mut Ctx, prepared: PreparedQuery) {
  if ensure_transport(ctx).is_err() {
    return;
  }

  let request_id = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    let request_id = state.next_request_id;
    state.next_request_id = state.next_request_id.saturating_add(1);
    state.in_flight = Some(InFlightQuery {
      request_id,
      key: prepared.key.clone(),
    });
    request_id
  };

  let send_result = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    match state.transport.as_ref() {
      Some(transport) => {
        transport
          .tx
          .send(WorkerCommand::Query {
            request_id,
            request: prepared.request,
          })
          .map_err(|error| format!("failed to send Copilot query to worker: {error}"))
      },
      None => Err("Copilot transport is unavailable".to_string()),
    }
  };

  if let Err(error) = send_result {
    apply_worker_event(ctx, WorkerEvent::Error(error));
  }
}

fn ensure_transport<Ctx: DefaultContext>(ctx: &mut Ctx) -> Result<(), ()> {
  let already_started = {
    let state = ctx.extension_state_or_default::<CopilotState>();
    state.transport.is_some()
  };
  if already_started {
    return Ok(());
  }

  let (tx, rx) = mpsc::channel();
  let (event_tx, event_rx) = mpsc::channel();
  let spawn_result = thread::Builder::new()
    .name("the-editor-copilot".to_string())
    .spawn(move || worker_main(rx, event_tx));

  let handle = match spawn_result {
    Ok(handle) => handle,
    Err(error) => {
      apply_worker_event(
        ctx,
        WorkerEvent::Error(format!("failed to spawn Copilot worker thread: {error}")),
      );
      return Err(());
    },
  };
  std::mem::forget(handle);

  let state = ctx.extension_state_or_default::<CopilotState>();
  state.transport = Some(CopilotTransport { tx, rx: event_rx });
  state.status = BackendStatus::Starting;
  Ok(())
}

fn current_prepared_query<Ctx: DefaultContext>(ctx: &Ctx) -> Option<PreparedQuery> {
  if ctx.mode() != Mode::Insert || ctx.completion_menu().active {
    return None;
  }

  let path = ctx.file_path()?.to_path_buf();
  let uri = file_uri_for_path(&path)?;
  let editor = ctx.editor_ref();
  let document = editor.document();
  let selection = document.selection();
  let range = single_active_range(selection, editor.view().active_cursor)?;
  if !range.is_empty() {
    return None;
  }

  let cursor_char = range.head;
  let (line, character) = char_idx_to_utf16_position(document.text(), cursor_char);
  let (tab_size, insert_spaces) = formatting_options(document.indent_style());
  let language_id = language_id_for_path(ctx, &path);

  Some(PreparedQuery {
    key:     QueryKey {
      buffer_id: editor.active_buffer_id(),
      doc_version: document.version(),
      cursor_char,
      file_path: path.clone(),
    },
    request: CopilotQuery {
      workspace_root: ctx.workspace_root(),
      uri,
      language_id,
      version: document.version().min(i32::MAX as u64) as i32,
      text: document.text().to_string(),
      line,
      character,
      tab_size,
      insert_spaces,
    },
  })
}

fn single_active_range(
  selection: &the_lib::selection::Selection,
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

fn trim_worker_suggestion<Ctx: DefaultContext>(
  ctx: &Ctx,
  key: QueryKey,
  raw: WorkerSuggestion,
) -> Option<CopilotSuggestion> {
  let document = ctx.editor_ref().document();
  if document.version() != key.doc_version {
    return None;
  }

  let text = document.text();
  let mut from = utf16_position_to_char_idx(text, raw.range.start.line, raw.range.start.character);
  let mut to = utf16_position_to_char_idx(text, raw.range.end.line, raw.range.end.character);
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
    return None;
  }

  Some(CopilotSuggestion {
    key,
    from,
    to,
    text: trimmed,
  })
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

#[derive(Debug)]
enum WorkerCommand {
  Query {
    request_id: u64,
    request:    CopilotQuery,
  },
  SignIn {
    workspace_root: PathBuf,
  },
}

#[derive(Debug)]
enum WorkerEvent {
  Ready,
  AuthPrompt {
    user_code: String,
  },
  Authenticated {
    user: Option<String>,
  },
  Error(String),
  QueryResult {
    request_id: u64,
    result:     Result<Option<WorkerSuggestion>, String>,
  },
}

#[derive(Debug, Clone)]
struct WorkerSuggestion {
  insert_text: String,
  range:       JsonRange,
}

fn worker_main(rx: Receiver<WorkerCommand>, event_tx: Sender<WorkerEvent>) {
  let mut server = None;
  while let Ok(command) = rx.recv() {
    match command {
      WorkerCommand::Query {
        request_id,
        request,
      } => {
        if server.is_none() {
          match CopilotServer::start(&request.workspace_root) {
            Ok(next) => {
              let _ = event_tx.send(WorkerEvent::Ready);
              server = Some(next);
            },
            Err(error) => {
              let _ = event_tx.send(WorkerEvent::Error(error));
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
        let _ = event_tx.send(WorkerEvent::QueryResult { request_id, result });
      },
      WorkerCommand::SignIn {
        workspace_root,
      } => {
        if server.is_none() {
          match CopilotServer::start(&workspace_root) {
            Ok(next) => {
              server = Some(next);
            },
            Err(error) => {
              let _ = event_tx.send(WorkerEvent::Error(error));
              continue;
            },
          }
        }

        let result = match server.as_mut() {
          Some(server) => server.sign_in(&event_tx),
          None => Err("Copilot server failed to start".to_string()),
        };

        match result {
          Ok(user) => {
            let _ = event_tx.send(WorkerEvent::Authenticated { user });
            let _ = event_tx.send(WorkerEvent::Ready);
          },
          Err(error) => {
            server = None;
            let _ = event_tx.send(WorkerEvent::Error(error));
          },
        }
      },
    }
  }
}

struct CopilotServer {
  child:      Child,
  stdin:      BufWriter<ChildStdin>,
  stdout:     BufReader<ChildStdout>,
  next_id:    u64,
  opened_uri: Option<String>,
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
      opened_uri: None,
    };
    server.initialize(workspace_root)?;
    Ok(server)
  }

  fn initialize(&mut self, workspace_root: &Path) -> Result<(), String> {
    let root_uri = file_uri_for_path(workspace_root);
    let initialize_id = self.next_request_id();
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "id": initialize_id,
      "method": "initialize",
      "params": {
        "processId": serde_json::Value::Null,
        "rootUri": root_uri,
        "capabilities": {},
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
        "workspaceFolders": root_uri.as_ref().map(|uri| vec![{
          json!({
            "uri": uri,
            "name": workspace_root.file_name().and_then(|name| name.to_str()).unwrap_or("workspace"),
          })
        }]).unwrap_or_default(),
      }
    }))?;
    self.read_response(initialize_id)?;
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "method": "initialized",
      "params": {}
    }))?;
    Ok(())
  }

  fn inline_completion(
    &mut self,
    request: &CopilotQuery,
  ) -> Result<Option<WorkerSuggestion>, String> {
    self.ensure_signed_in_for_queries()?;
    self.reopen_document(request)?;
    let request_id = self.next_request_id();
    self.write_message(&json!({
      "jsonrpc": "2.0",
      "id": request_id,
      "method": "textDocument/inlineCompletion",
      "params": {
        "textDocument": {
          "uri": request.uri,
          "version": request.version,
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

    let value = self.read_response(request_id)?;
    let result: InlineCompletionResult = serde_json::from_value(value)
      .map_err(|error| format!("failed to parse Copilot inline completion response: {error}"))?;
    Ok(result.items.into_iter().next().map(|item| {
      WorkerSuggestion {
        insert_text: item.insert_text,
        range:       item.range,
      }
    }))
  }

  fn sign_in(&mut self, event_tx: &Sender<WorkerEvent>) -> Result<Option<String>, String> {
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
    let _ = event_tx.send(WorkerEvent::AuthPrompt {
      user_code: prompt.user_code.clone(),
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
    let result = self.read_response(request_id)?;
    if let Ok(status) = serde_json::from_value::<SignInStatus>(result.clone()) {
      return Ok(status);
    }

    for _ in 0..300 {
      thread::sleep(Duration::from_secs(1));
      let status = self.check_status()?;
      match status {
        SignInStatus::Ok {
          ..
        }
        | SignInStatus::MaybeOk {
          ..
        }
        | SignInStatus::AlreadySignedIn {
          ..
        } => return Ok(status),
        SignInStatus::NotAuthorized {
          ..
        }
        | SignInStatus::NotSignedIn => continue,
      }
    }

    Err(
      "Copilot sign-in timed out. Visit https://github.com/login/device, complete the device \
       flow, then run :copilot-sign-in again."
        .to_string(),
    )
  }

  fn reopen_document(&mut self, request: &CopilotQuery) -> Result<(), String> {
    if let Some(opened_uri) = self.opened_uri.take() {
      self.write_message(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
          "textDocument": {
            "uri": opened_uri,
          }
        }
      }))?;
    }

    self.write_message(&json!({
      "jsonrpc": "2.0",
      "method": "textDocument/didOpen",
      "params": {
        "textDocument": {
          "uri": request.uri,
          "languageId": request.language_id,
          "version": request.version,
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
    self.opened_uri = Some(request.uri.clone());
    Ok(())
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
    loop {
      let message = read_jsonrpc_message(&mut self.stdout)?;
      let Some(id) = message.get("id").and_then(|id| id.as_u64()) else {
        continue;
      };
      if id != expected_id {
        continue;
      }

      if let Some(error) = message.get("error") {
        let message = error
          .get("message")
          .and_then(Value::as_str)
          .unwrap_or("Copilot JSON-RPC request failed");
        return Err(message.to_string());
      }

      return Ok(message.get("result").cloned().unwrap_or(Value::Null));
    }
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InlineCompletionResult {
  items: Vec<InlineCompletionItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptUserDeviceFlow {
  user_code: String,
  command:   JsonCommand,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct JsonCommand {
  title:     String,
  command:   String,
  arguments: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InlineCompletionItem {
  insert_text: String,
  range:       JsonRange,
}

#[derive(Debug, Clone, Deserialize)]
struct JsonRange {
  start: JsonPosition,
  end:   JsonPosition,
}

#[derive(Debug, Clone, Deserialize)]
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
    SignInStatus::Ok {
      ..
    } | SignInStatus::MaybeOk {
      ..
    } | SignInStatus::AlreadySignedIn {
      ..
    }
  )
}

fn authorized_user(status: &SignInStatus) -> Option<String> {
  match status {
    SignInStatus::Ok {
      user,
    } => user.clone(),
    SignInStatus::MaybeOk {
      user,
    }
    | SignInStatus::AlreadySignedIn {
      user,
    } => Some(user.clone()),
    SignInStatus::NotAuthorized {
      ..
    }
    | SignInStatus::NotSignedIn => None,
  }
}

fn sign_in_status_message(status: &SignInStatus) -> String {
  match status {
    SignInStatus::Ok {
      user,
    } => match user {
      Some(user) => format!("Copilot connected as {user}"),
      None => "Copilot connected".to_string(),
    },
    SignInStatus::MaybeOk {
      user,
    }
    | SignInStatus::AlreadySignedIn {
      user,
    } => format!("Copilot connected as {user}"),
    SignInStatus::NotAuthorized {
      user,
    } => format!(
      "Copilot is not authorized for {user}. Run :copilot-sign-in to start the GitHub device \
       flow."
    ),
    SignInStatus::NotSignedIn => {
      "Copilot is not signed in. Run :copilot-sign-in to connect GitHub Copilot.".to_string()
    },
  }
}
