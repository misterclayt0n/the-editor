/// Handler for LSP hover information
///
/// Shows a popup with type information and documentation when space+k is
/// pressed
use the_editor_lsp_types::types as lsp;

/// Request hover information from language server
pub async fn request_hover() -> anyhow::Result<()> {
  // Get hover future from main thread
  let (tx_future, rx_future) = tokio::sync::oneshot::channel();
  let (tx_name, rx_name) = tokio::sync::oneshot::channel();

  crate::ui::job::dispatch_blocking(move |editor, _compositor| {
    let (view, doc) = crate::current_ref!(editor);

    // Find first language server that supports hover
    let Some(ls) = doc.language_servers().find(|ls| {
      matches!(
        ls.capabilities().hover_provider,
        Some(lsp::HoverProviderCapability::Simple(true) | lsp::HoverProviderCapability::Options(_))
      )
    }) else {
      let _ = tx_future.send(None);
      return;
    };

    let server_name = ls.name().to_string();
    let pos = doc.position(view.id, ls.offset_encoding());
    let doc_id = doc.identifier();
    let future = ls.text_document_hover(doc_id, pos, None);
    let _ = tx_name.send(server_name);
    let _ = tx_future.send(future);
  });

  // Wait for the future from main thread
  let Some(future) = rx_future.await.ok().flatten() else {
    crate::ui::job::dispatch_blocking(move |editor, _compositor| {
      editor.set_error("No language server supports hover");
    });
    return Ok(());
  };

  let server_name = rx_name.await.ok().unwrap_or_else(|| "unknown".to_string());

  // Await the hover response
  let hover = match future.await {
    Ok(Some(hover)) => hover,
    Ok(None) => {
      crate::ui::job::dispatch_blocking(move |editor, _compositor| {
        editor.set_status("No hover information available");
      });
      return Ok(());
    },
    Err(err) => {
      log::error!("Hover request failed: {}", err);
      crate::ui::job::dispatch_blocking(move |editor, _compositor| {
        editor.set_error("Hover request failed");
      });
      return Ok(());
    },
  };

  // Display popup with result
  crate::ui::job::dispatch_blocking(move |_editor, compositor| {
    use crate::ui::components::hover::Hover;

    // Wrap in vec for compatibility with multi-server UI (future enhancement)
    let hovers = vec![(server_name, hover)];
    let contents = Hover::new(hovers);
    compositor.replace_or_push(Hover::ID, contents);
  });

  Ok(())
}
