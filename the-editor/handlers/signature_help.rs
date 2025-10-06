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
#[derive(Default)]
pub struct SignatureHelpHandler {
  trigger: Option<SignatureHelpInvoked>,
  state:   State,
}

impl SignatureHelpHandler {
  pub fn new() -> Self {
    Self::default()
  }
}

impl AsyncHook for SignatureHelpHandler {
  type Event = SignatureHelpEvent;

  fn handle_event(&mut self, event: Self::Event, timeout: Option<Instant>) -> Option<Instant> {
    log::info!("SignatureHelp: handle_event {:?}", event);
    match event {
      SignatureHelpEvent::Invoked => {
        log::info!("SignatureHelp: Manual invocation");
        self.trigger = Some(SignatureHelpInvoked::Manual);
        self.state = State::Closed;
        self.finish_debounce();
        return None;
      },
      SignatureHelpEvent::Trigger => {
        log::info!("SignatureHelp: Trigger event");
      },
      SignatureHelpEvent::ReTrigger => {
        log::info!("SignatureHelp: ReTrigger event, state: {:?}", self.state);
        // Don't retrigger if we aren't open/pending yet
        if matches!(self.state, State::Closed) {
          return timeout;
        }
      },
      SignatureHelpEvent::Cancel => {
        log::info!("SignatureHelp: Cancel event");
        self.state = State::Closed;
        return None;
      },
      SignatureHelpEvent::RequestComplete { open } => {
        log::info!("SignatureHelp: RequestComplete open={}", open);
        self.state = if open { State::Open } else { State::Closed };
        return timeout;
      },
    }

    if self.trigger.is_none() {
      self.trigger = Some(SignatureHelpInvoked::Automatic);
    }

    log::info!("SignatureHelp: Setting timeout for {}ms", TIMEOUT_MS);
    Some(Instant::now() + Duration::from_millis(TIMEOUT_MS))
  }

  fn finish_debounce(&mut self) {
    let invoked = self.trigger.take().unwrap();
    self.state = State::Pending;

    log::info!("SignatureHelp: finish_debounce called, spawning request");

    // Spawn task to request signature help
    tokio::spawn(async move {
      request_signature_help(invoked).await;
    });
  }
}

async fn request_signature_help(invoked: SignatureHelpInvoked) {
  log::info!("SignatureHelp: request_signature_help started");

  // Create a oneshot channel to get the signature help future from the main thread
  let (tx, rx) = tokio::sync::oneshot::channel();

  crate::ui::job::dispatch_blocking(move |editor, _compositor| {
    log::info!("SignatureHelp: Inside dispatch_blocking");
    let (view, doc) = crate::current_ref!(editor);

    // Find first language server that supports signature help
    let Some(ls) = doc.language_servers().find(|ls| {
      matches!(
        ls.capabilities().signature_help_provider,
        Some(lsp::SignatureHelpOptions { .. })
      )
    }) else {
      log::warn!("SignatureHelp: No language server with signature help support found");
      let _ = tx.send(None);
      return;
    };

    log::info!("SignatureHelp: Found language server, requesting signature help");
    let pos = doc.position(view.id, ls.offset_encoding());
    let doc_id = doc.identifier();
    let future = ls.text_document_signature_help(doc_id, pos, None);
    log::info!("SignatureHelp: Got future, sending through channel");
    let _ = tx.send(future);
  });

  // Wait for the future from main thread
  let Some(future) = rx.await.ok().flatten() else {
    log::warn!("SignatureHelp: Failed to receive future from channel");
    return;
  };

  log::info!("SignatureHelp: Awaiting signature help response");

  // Await the signature help response
  let response = match future.await {
    Ok(res) => res,
    Err(err) => {
      log::error!("Signature help request failed: {}", err);
      return;
    },
  };

  log::info!("SignatureHelp: Got response: {:?}", response);

  // Update UI with response
  crate::ui::job::dispatch_blocking(move |editor, compositor| {
    log::info!("SignatureHelp: Calling show_signature_help");
    crate::ui::show_signature_help(editor, compositor, invoked, response);
  });
}

/// A single signature with optional active parameter range
#[derive(Debug, Clone)]
pub struct Signature {
  pub signature:           String,
  pub signature_doc:       Option<String>,
  pub active_param_range:  Option<(usize, usize)>,
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

  log::info!("SignatureHelp: Registering hooks");
  let tx = handlers.signature_hints.clone();

  // Trigger on mode switch
  let tx_mode = tx.clone();
  register_hook!(move |event: &mut OnModeSwitch<'_, '_>| {
    log::info!(
      "SignatureHelp: OnModeSwitch hook - old: {:?}, new: {:?}",
      event.old_mode,
      event.new_mode
    );
    match (event.old_mode, event.new_mode) {
      (Mode::Insert, _) => {
        log::info!("SignatureHelp: Leaving insert mode, sending Cancel");
        send_blocking(&tx_mode, SignatureHelpEvent::Cancel);
        // Also clear the UI signature help popup immediately
        event.cx.callback.push(Box::new(|compositor: &mut crate::ui::compositor::Compositor, _| {
          if let Some(editor_view) = compositor.find::<crate::ui::EditorView>() {
            editor_view.clear_signature_help();
          }
        }));
      },
      (_, Mode::Insert) => {
        log::info!("SignatureHelp: Entering insert mode, sending Trigger");
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
    log::info!("SignatureHelp: PostInsertChar hook - char: '{}'", event.c);

    if !event.cx.editor.config().lsp.auto_signature_help {
      log::info!("SignatureHelp: auto_signature_help is disabled");
      return Ok(());
    }

    let (view, doc) = crate::current_ref!(event.cx.editor);

    // Find first language server that supports signature help
    let Some(ls) = doc.language_servers().find(|ls| {
      matches!(
        ls.capabilities().signature_help_provider,
        Some(lsp::SignatureHelpOptions { .. })
      )
    }) else {
      log::info!("SignatureHelp: No language server with signature help support");
      return Ok(());
    };

    // Check if the text ends with a trigger character for this server
    let capabilities = ls.capabilities();
    if let Some(lsp::SignatureHelpOptions {
      trigger_characters: Some(ref triggers),
      ..
    }) = capabilities.signature_help_provider
    {
      log::info!("SignatureHelp: Trigger characters: {:?}", triggers);
      let mut text = doc.text().slice(..);
      let cursor = doc.selection(view.id).primary().cursor(text);
      text = text.slice(..cursor);

      // Get last few characters for debugging
      let text_str = text.to_string();
      let last_chars: String = text_str.chars().rev().take(10).collect::<Vec<_>>().into_iter().rev().collect();
      log::info!("SignatureHelp: Last 10 chars before cursor: {:?}", last_chars);

      if triggers.iter().any(|trigger| text.ends_with(trigger.as_str())) {
        log::info!("SignatureHelp: Text ends with trigger character, sending Trigger");
        send_blocking(&tx, SignatureHelpEvent::Trigger);
      } else {
        log::info!("SignatureHelp: Text does not end with any trigger character");
      }
    }

    Ok(())
  });

  // ReTrigger signature help on document changes
  let tx_doc_change = handlers.signature_hints.clone();
  register_hook!(move |event: &mut DocumentDidChange<'_>| {
    if event.doc.config.load().lsp.auto_signature_help && !event.ghost_transaction {
      log::info!("SignatureHelp: DocumentDidChange - sending ReTrigger");
      send_blocking(&tx_doc_change, SignatureHelpEvent::ReTrigger);
    }
    Ok(())
  });

  // Trigger on selection change (when cursor moves)
  let tx_selection = handlers.signature_hints.clone();
  register_hook!(move |event: &mut SelectionDidChange<'_>| {
    if event.doc.config.load().lsp.auto_signature_help {
      log::info!("SignatureHelp: SelectionDidChange - sending ReTrigger");
      send_blocking(&tx_selection, SignatureHelpEvent::ReTrigger);
    }
    Ok(())
  });
}


