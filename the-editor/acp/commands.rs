use anyhow::Result;

use crate::{
  core::{
    command_line::Args,
    commands::Context,
    special_buffer::SpecialBufferKind,
  },
  editor::Action,
  ui::{
    components::prompt::PromptEvent,
    job::LocalCallback,
  },
};

/// Create a new ACP session with Claude Code
pub fn acp_new_session(cx: &mut Context) {
  log::info!("ACP: acp_new_session called");

  // Create a new scratch document for the session
  let doc_id = cx.editor.new_file(Action::VerticalSplit);
  log::info!("ACP: Created new document with id {:?}", doc_id);

  // Mark the document as an ACP buffer
  cx.editor
    .mark_special_buffer(doc_id, SpecialBufferKind::Acp);
  log::info!("ACP: Marked document {:?} as ACP buffer", doc_id);

  // Get registry handle for async operations
  let registry = cx.editor.acp_sessions.handle();
  log::info!("ACP: Got registry handle, spawning local job");

  // Spawn a local job to create the session
  cx.jobs.callback_local(async move {
    log::info!("ACP: Inside callback_local, calling registry.new_session");
    match registry.new_session("claude-code", doc_id).await {
      Ok(session_id) => {
        log::info!("ACP: Session created successfully: {}", session_id.0);
        let message = format!("Created ACP session: {}", session_id.0);
        let session_id_str = session_id.0.to_string();
        Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_status(message);
          // Set the session ID in the document
          if let Some(doc) = editor.documents.get_mut(&doc_id) {
            doc.acp_session_id = Some(session_id_str.clone());
            log::info!("ACP: Set session ID in document");
          }
        }))))
      },
      Err(err) => {
        log::error!("ACP: Failed to create session: {}", err);
        let message = format!("Failed to create ACP session: {}", err);
        Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_error(message);
        }))))
      },
    }
  });
}

/// Send a prompt to the current ACP session
pub fn acp_send_prompt(cx: &mut Context) {
  log::info!("ACP: acp_send_prompt called");

  // Get the current document
  let (view, doc) = crate::current!(cx.editor);
  let doc_id = view.doc;
  log::info!("ACP: Current document id: {:?}", doc_id);

  // Get the text from the current selection or line
  let selection = doc.selection(view.id);
  let text = doc.text();

  // Get text from primary selection
  let prompt_text = selection.primary().fragment(text.slice(..)).to_string();

  log::info!("ACP: Prompt text length: {} chars", prompt_text.len());

  if prompt_text.is_empty() {
    log::warn!("ACP: No text selected");
    cx.editor.set_error("No text selected to send as prompt");
    return;
  }

  // Get registry handle
  let registry = cx.editor.acp_sessions.handle();

  // Spawn a local job to send the prompt
  cx.jobs.callback_local(async move {
    log::info!("ACP: Inside send_prompt callback_local");
    // First get the session ID for this document
    let session_id = match registry.get_session_id_by_doc(doc_id).await {
      Some(id) => {
        log::info!("ACP: Found session {:?} for document {:?}", id, doc_id);
        id
      },
      None => {
        log::warn!("ACP: No session found for document {:?}", doc_id);
        let message = "No ACP session associated with this document".to_string();
        return Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_error(message);
        }))));
      },
    };

    // Create the prompt content
    let content = vec![crate::acp::acp::ContentBlock::Text(
      crate::acp::acp::TextContent {
        text:        prompt_text.clone(),
        annotations: None,
        meta:        None,
      },
    )];

    log::info!(
      "ACP: Sending prompt with {} chars to session",
      prompt_text.len()
    );

    // Send the prompt
    match registry.send_prompt(&session_id, content).await {
      Ok(_response) => {
        log::info!("ACP: Prompt sent successfully");
        Ok(Some(LocalCallback::Editor(Box::new(|editor| {
          editor.set_status("Prompt sent to agent");
        }))))
      },
      Err(err) => {
        log::error!("ACP: Failed to send prompt: {}", err);
        let message = format!("Failed to send prompt: {}", err);
        Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_error(message);
        }))))
      },
    }
  });
}

/// Close the current ACP session
pub fn acp_close_session(cx: &mut Context) {
  log::info!("ACP: acp_close_session called");

  // Get the current document
  let (view, _doc) = crate::current!(cx.editor);
  let doc_id = view.doc;
  log::info!("ACP: Closing session for document {:?}", doc_id);

  // Get registry handle
  let registry = cx.editor.acp_sessions.handle();

  // Spawn a local job to close the session
  cx.jobs.callback_local(async move {
    log::info!("ACP: Inside close_session callback_local");
    // First get the session ID for this document
    let session_id = match registry.get_session_id_by_doc(doc_id).await {
      Some(id) => {
        log::info!("ACP: Found session {:?} to close", id);
        id
      },
      None => {
        log::warn!("ACP: No session found for document {:?}", doc_id);
        let message = "No ACP session associated with this document".to_string();
        return Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_error(message);
        }))));
      },
    };

    // Close the session
    match registry.close_session(&session_id).await {
      Ok(()) => {
        log::info!("ACP: Session closed successfully");
        Ok(Some(LocalCallback::EditorCompositor(Box::new(
          move |editor, _compositor| {
            editor.set_status("ACP session closed");
            // TODO: Optionally close the document as well
          },
        ))))
      },
      Err(err) => {
        log::error!("ACP: Failed to close session: {}", err);
        let message = format!("Failed to close session: {}", err);
        Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_error(message);
        }))))
      },
    }
  });
}

// Command registry wrappers (conform to CommandFn signature)

/// Command wrapper for acp-new-session
pub fn cmd_acp_new_session(cx: &mut Context, _args: Args, _event: PromptEvent) -> Result<()> {
  acp_new_session(cx);
  Ok(())
}

/// Command wrapper for acp-send-prompt
pub fn cmd_acp_send_prompt(cx: &mut Context, _args: Args, _event: PromptEvent) -> Result<()> {
  acp_send_prompt(cx);
  Ok(())
}

/// Command wrapper for acp-close-session
pub fn cmd_acp_close_session(cx: &mut Context, _args: Args, _event: PromptEvent) -> Result<()> {
  acp_close_session(cx);
  Ok(())
}
