/// Handler for LSP signature help (function argument hints)
///
/// Shows a popup with function signature and parameter information when typing
/// inside function calls in insert mode.
use the_editor_event::AsyncHook;
use the_editor_lsp_types::types as lsp;
use the_editor_stdx::rope::RopeSliceExt;
use tokio::time::{
  Duration,
  Instant,
};

use crate::core::syntax::config::LanguageServerFeature;
pub use crate::handlers::lsp::{
  SignatureHelpEvent,
  SignatureHelpInvoked,
};

/// Debounce timeout in ms (from VSCode)
const TIMEOUT_MS: u64 = 120;

#[derive(Debug, PartialEq, Eq, Default)]
enum State {
  Open,
  #[default]
  Closed,
  Pending,
}

/// Handler for signature help requests
pub struct SignatureHelpHandler {
  trigger:             Option<SignatureHelpInvoked>,
  state:               State,
  task:                Option<tokio::task::JoinHandle<()>>,
  request_generation:  u64,
  inflight_generation: Option<u64>,
}

impl Default for SignatureHelpHandler {
  fn default() -> Self {
    Self {
      trigger:             None,
      state:               State::Closed,
      task:                None,
      request_generation:  0,
      inflight_generation: None,
    }
  }
}

impl SignatureHelpHandler {
  pub fn new() -> Self {
    Self::default()
  }
}

impl AsyncHook for SignatureHelpHandler {
  type Event = SignatureHelpEvent;

  fn handle_event(&mut self, event: Self::Event, timeout: Option<Instant>) -> Option<Instant> {
    match event {
      SignatureHelpEvent::Invoked => {
        self.launch_request(SignatureHelpInvoked::Manual);
        None
      },
      SignatureHelpEvent::Trigger => {
        self.trigger = Some(SignatureHelpInvoked::Automatic);
        Some(Instant::now() + Duration::from_millis(TIMEOUT_MS))
      },
      SignatureHelpEvent::ReTrigger => {
        self.trigger = Some(SignatureHelpInvoked::Automatic);
        if matches!(self.state, State::Closed) {
          self.launch_request(SignatureHelpInvoked::Automatic);
          timeout
        } else {
          Some(Instant::now() + Duration::from_millis(TIMEOUT_MS))
        }
      },
      SignatureHelpEvent::Cancel => {
        self.state = State::Closed;
        self.trigger = None;
        self.inflight_generation = None;
        if let Some(task) = self.task.take() {
          task.abort();
        }
        None
      },
      SignatureHelpEvent::RequestComplete { open, generation } => {
        if self
          .inflight_generation
          .map_or(false, |current| generation < current)
        {
          timeout
        } else {
          self.inflight_generation = None;
          self.state = if open { State::Open } else { State::Closed };
          self.task = None;
          timeout
        }
      },
    }
  }

  fn finish_debounce(&mut self) {
    let invoked = self
      .trigger
      .take()
      .unwrap_or(SignatureHelpInvoked::Automatic);
    self.launch_request(invoked);
  }
}

impl SignatureHelpHandler {
  fn launch_request(&mut self, invoked: SignatureHelpInvoked) {
    self.request_generation = self.request_generation.wrapping_add(1);
    let generation = self.request_generation;
    self.state = State::Pending;
    self.inflight_generation = Some(generation);
    self.trigger = None;

    if let Some(task) = self.task.take() {
      task.abort();
    }

    self.task = Some(tokio::spawn(async move {
      request_signature_help(invoked, generation).await;
    }));
  }
}

async fn request_signature_help(invoked: SignatureHelpInvoked, generation: u64) {
  // Create a oneshot channel to get the signature help future from the main
  // thread
  let (tx, rx) = tokio::sync::oneshot::channel();

  crate::ui::job::dispatch_blocking(move |editor, _compositor| {
    // Skip if focused view is not a document (e.g., terminal)
    let Some((view, doc)) = crate::try_current_ref!(editor) else {
      let _ = tx.send(None);
      return;
    };

    // Find first language server that supports signature help
    let mut servers = doc.language_servers_with_feature(LanguageServerFeature::SignatureHelp);
    let Some(ls) = servers.next() else {
      let _ = tx.send(None);
      return;
    };

    let pos = doc.position(view.id, ls.offset_encoding());
    let doc_id = doc.identifier();
    let future = ls.text_document_signature_help(doc_id, pos, None);
    let _ = tx.send(future);
  });

  // Wait for the future from main thread
  let invoked_for_none = invoked;
  let Some(future) = rx.await.ok().flatten() else {
    crate::ui::job::dispatch_blocking(move |editor, compositor| {
      crate::ui::show_signature_help(editor, compositor, invoked_for_none, None, generation);
    });
    return;
  };

  // Await the signature help response
  let response = match future.await {
    Ok(res) => res,
    Err(err) => {
      log::error!("Signature help request failed: {}", err);
      let invoked_for_err = invoked;
      crate::ui::job::dispatch_blocking(move |editor, compositor| {
        crate::ui::show_signature_help(editor, compositor, invoked_for_err, None, generation);
      });
      return;
    },
  };

  // Update UI with response
  crate::ui::job::dispatch_blocking(move |editor, compositor| {
    crate::ui::show_signature_help(editor, compositor, invoked, response, generation);
  });
}

/// A single signature with optional active parameter range
#[derive(Debug, Clone)]
pub struct Signature {
  pub signature:          String,
  pub signature_doc:      Option<String>,
  pub active_param_range: Option<(usize, usize)>,
}

/// Calculate the active parameter range for highlighting
pub fn active_param_range(
  signature: &lsp::SignatureInformation,
  response_active_parameter: Option<u32>,
) -> Option<(usize, usize)> {
  let param_idx = signature
    .active_parameter
    .or(response_active_parameter)
    .unwrap_or(0) as usize;

  let param = signature.parameters.as_ref()?.get(param_idx)?;

  match &param.label {
    lsp::ParameterLabel::Simple(string) => {
      let start = signature.label.find(string.as_str())?;
      Some((start, start + string.len()))
    },
    lsp::ParameterLabel::LabelOffsets([start, end]) => {
      // LSP sends UTF-16 offsets, convert to byte offsets
      let from = signature
        .label
        .char_indices()
        .nth(*start as usize)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
      let to = signature
        .label
        .char_indices()
        .nth(*end as usize)
        .map(|(idx, _)| idx)
        .unwrap_or(signature.label.len());
      Some((from, to))
    },
  }
}

/// Register hooks for signature help events
pub fn register_hooks(handlers: &crate::handlers::Handlers) {
  use the_editor_event::{
    register_hook,
    send_blocking,
  };

  use crate::{
    event::{
      DocumentDidChange,
      OnModeSwitch,
      PostInsertChar,
      SelectionDidChange,
    },
    keymap::Mode,
  };

  let tx = handlers.signature_hints.clone();

  // Trigger on mode switch
  let tx_mode = tx.clone();
  register_hook!(move |event: &mut OnModeSwitch<'_, '_>| {
    match (event.old_mode, event.new_mode) {
      (Mode::Insert, _) => {
        send_blocking(&tx_mode, SignatureHelpEvent::Cancel);
        // Also clear the UI signature help popup immediately
        event.cx.callback.push(Box::new(
          |compositor: &mut crate::ui::compositor::Compositor, _| {
            if let Some(editor_view) = compositor.find::<crate::ui::EditorView>() {
              editor_view.clear_signature_help();
            }
          },
        ));
      },
      (_, Mode::Insert) => {
        if event.cx.editor.config().lsp.auto_signature_help {
          send_blocking(&tx_mode, SignatureHelpEvent::Trigger);
        }
      },
      _ => {},
    }
    Ok(())
  });

  // Trigger on signature help trigger characters (like '(' and ',')
  register_hook!(move |event: &mut PostInsertChar<'_, '_>| {
    if !event.cx.editor.config().lsp.auto_signature_help {
      return Ok(());
    }

    // Skip if focused view is not a document (e.g., terminal)
    let Some((view, doc)) = crate::try_current_ref!(event.cx.editor) else {
      return Ok(());
    };

    // Find first language server that supports signature help
    let Some(ls) = doc.language_servers().find(|ls| {
      matches!(
        ls.capabilities().signature_help_provider,
        Some(lsp::SignatureHelpOptions { .. })
      )
    }) else {
      return Ok(());
    };

    // Check if the text ends with a trigger character for this server
    let capabilities = ls.capabilities();
    if let Some(lsp::SignatureHelpOptions {
      trigger_characters: Some(ref triggers),
      ..
    }) = capabilities.signature_help_provider
    {
      let mut text = doc.text().slice(..);
      let cursor = doc.selection(view.id).primary().cursor(text);
      text = text.slice(..cursor);

      if triggers
        .iter()
        .any(|trigger| text.ends_with(trigger.as_str()))
      {
        send_blocking(&tx, SignatureHelpEvent::Trigger);
      }
    }

    Ok(())
  });

  // ReTrigger signature help on document changes
  let tx_doc_change = handlers.signature_hints.clone();
  register_hook!(move |event: &mut DocumentDidChange<'_>| {
    if event.doc.config.load().lsp.auto_signature_help && !event.ghost_transaction {
      send_blocking(&tx_doc_change, SignatureHelpEvent::ReTrigger);
    }
    Ok(())
  });

  // Trigger on selection change (when cursor moves)
  let tx_selection = handlers.signature_hints.clone();
  register_hook!(move |event: &mut SelectionDidChange<'_>| {
    if event.doc.config.load().lsp.auto_signature_help {
      send_blocking(&tx_selection, SignatureHelpEvent::ReTrigger);
    }
    Ok(())
  });
}
