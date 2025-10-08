use futures_util::{
  future::BoxFuture,
  stream::FuturesOrdered,
  StreamExt,
};

use crate::{
  core::{
    commands::Context,
    selection::Selection,
    syntax::config::LanguageServerFeature,
    view::{
      Align,
      align_view,
    },
  },
  current_ref,
  editor::Action,
  lsp::{
    self,
    Client,
    OffsetEncoding,
    lsp::types as lsp_types,
    util::lsp_range_to_range,
  },
  ui::{
    compositor::Compositor,
    job::Callback,
  },
};

/// Wrapper around `lsp::Location` that includes the offset encoding
#[derive(Debug, Clone, PartialEq, Eq)]
struct Location {
  uri:             lsp_types::Url,
  range:           lsp_types::Range,
  offset_encoding: OffsetEncoding,
}

fn lsp_location_to_location(
  location: lsp_types::Location,
  offset_encoding: OffsetEncoding,
) -> Option<Location> {
  Some(Location {
    uri: location.uri,
    range: location.range,
    offset_encoding,
  })
}

fn jump_to_location(
  editor: &mut crate::editor::Editor,
  location: &Location,
  action: Action,
) {
  let Some(path) = location.uri.to_file_path().ok() else {
    editor.set_error(format!("Unable to convert URI to filepath: {:?}", location.uri));
    return;
  };

  let doc_id = match editor.open(&path, action) {
    Ok(id) => id,
    Err(err) => {
      editor.set_error(format!("Failed to open path: {:?}: {:?}", path, err));
      return;
    },
  };

  let view = editor.tree.get(editor.tree.focus);
  let doc = editor.documents.get_mut(&doc_id).unwrap();

  // Convert LSP range to editor range
  let Some(new_range) = lsp_range_to_range(doc.text(), location.range, location.offset_encoding)
  else {
    log::warn!("LSP position out of bounds - {:?}", location.range);
    return;
  };

  // Set cursor to the start of the definition
  doc.set_selection(view.id, Selection::single(new_range.head, new_range.anchor));

  // Align the view to center the cursor
  align_view(doc, view, Align::Center);
}

fn goto_impl(
  editor: &mut crate::editor::Editor,
  _compositor: &mut Compositor,
  locations: Vec<Location>,
) {
  match locations.as_slice() {
    [location] => {
      jump_to_location(editor, location, Action::Replace);
    },
    _locations => {
      // TODO: For multiple locations, show a picker
      // For now, just jump to the first one
      jump_to_location(editor, &locations[0], Action::Replace);
      editor.set_status(format!("Found {} definitions, jumped to first", locations.len()));
    },
  }
}

/// Generic helper function for goto requests (definition, declaration, type definition, etc.)
fn goto_single_impl<P>(
  cx: &mut Context,
  feature: LanguageServerFeature,
  request_provider: P,
  error_msg: &'static str,
) where
  P: Fn(
    &Client,
    lsp_types::Position,
    lsp_types::TextDocumentIdentifier,
  ) -> Option<BoxFuture<'static, lsp::Result<Option<lsp_types::GotoDefinitionResponse>>>>,
{
  let (view, doc) = current_ref!(cx.editor);

  // Collect all the futures with their offset encodings
  let requests: Vec<_> = doc
    .language_servers_with_feature(feature)
    .filter_map(|language_server| {
      let offset_encoding = language_server.offset_encoding();
      let pos = doc.position(view.id, offset_encoding);
      request_provider(language_server, pos, doc.identifier())
        .map(|future| (future, offset_encoding))
    })
    .collect();

  cx.jobs.callback(async move {
    let mut futures: FuturesOrdered<_> = requests
      .into_iter()
      .map(|(future, offset_encoding)| async move {
        anyhow::Ok((future.await?, offset_encoding))
      })
      .collect();

    let mut locations = Vec::new();

    while let Some(response) = futures.next().await {
      match response {
        Ok((response, offset_encoding)) => {
          match response {
            Some(lsp_types::GotoDefinitionResponse::Scalar(lsp_location)) => {
              locations.extend(lsp_location_to_location(lsp_location, offset_encoding));
            },
            Some(lsp_types::GotoDefinitionResponse::Array(lsp_locations)) => {
              locations.extend(lsp_locations.into_iter().filter_map(|location| {
                lsp_location_to_location(location, offset_encoding)
              }));
            },
            Some(lsp_types::GotoDefinitionResponse::Link(lsp_locations)) => {
              locations.extend(lsp_locations.into_iter().map(|location_link| {
                lsp_types::Location::new(
                  location_link.target_uri,
                  location_link.target_range,
                )
              }).filter_map(|location| {
                lsp_location_to_location(location, offset_encoding)
              }));
            },
            None => (),
          }
        },
        Err(err) => {
          log::error!("Error requesting {}: {err}", error_msg);
        },
      }
    }

    let call = move |editor: &mut crate::editor::Editor, compositor: &mut Compositor| {
      if locations.is_empty() {
        editor.set_error(error_msg);
      } else {
        goto_impl(editor, compositor, locations);
      }
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
  });
}

pub fn goto_definition(cx: &mut Context) {
  goto_single_impl(
    cx,
    LanguageServerFeature::GotoDefinition,
    |ls, pos, doc_id| ls.goto_definition(doc_id, pos, None),
    "No definition found",
  );
}

pub fn goto_declaration(cx: &mut Context) {
  goto_single_impl(
    cx,
    LanguageServerFeature::GotoDeclaration,
    |ls, pos, doc_id| ls.goto_declaration(doc_id, pos, None),
    "No declaration found",
  );
}

pub fn goto_type_definition(cx: &mut Context) {
  goto_single_impl(
    cx,
    LanguageServerFeature::GotoTypeDefinition,
    |ls, pos, doc_id| ls.goto_type_definition(doc_id, pos, None),
    "No type definition found",
  );
}
