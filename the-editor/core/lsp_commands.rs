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
) -> Location {
  Location {
    uri: location.uri,
    range: location.range,
    offset_encoding,
  }
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
  compositor: &mut Compositor,
  locations: Vec<Location>,
) {
  use crate::ui::components::{
    Column,
    Picker,
    PickerAction,
  };

  match locations.as_slice() {
    [location] => {
      jump_to_location(editor, location, Action::Replace);
    },
    [] => {
      editor.set_error("No locations found");
    },
    _locations => {
      // Show picker for multiple locations
      let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

      // Define column: format location as "path:line"
      let columns = vec![Column::new("Location", |location: &Location, _cwd: &std::path::PathBuf| {
        let path = location.uri.to_file_path().ok();
        let path_str = if let Some(p) = &path {
          p.strip_prefix(&_cwd).unwrap_or(p).display().to_string()
        } else {
          location.uri.to_string()
        };
        format!("{}:{}", path_str, location.range.start.line + 1)
      })];

      let editor_data = cwd.clone();

      // Create action handler
      let action_handler = std::sync::Arc::new(
        move |location: &Location, _data: &std::path::PathBuf, action: PickerAction| {
          let action_type = match action {
            PickerAction::Primary => Action::Replace,
            PickerAction::Secondary => Action::HorizontalSplit,
            PickerAction::Tertiary => Action::VerticalSplit,
          };

          // Clone location to move into the closure
          let location = location.clone();

          // Jump to the location
          crate::ui::job::dispatch_blocking(move |editor, _compositor| {
            jump_to_location(editor, &location, action_type);
          });

          true // Close picker
        },
      );

      let picker = Picker::new(
        columns,
        0, // primary column index
        locations,
        editor_data,
        |_| {}, // Dummy on_select since we're using action_handler
      )
      .with_action_handler(action_handler)
      .with_preview(|location: &Location| {
        // Return path and line range (LSP lines are already 0-indexed)
        location.uri.to_file_path().ok().map(|path| {
          (path, Some((location.range.start.line as usize, location.range.end.line as usize)))
        })
      });

      compositor.push(Box::new(picker));
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
              locations.push(lsp_location_to_location(lsp_location, offset_encoding));
            },
            Some(lsp_types::GotoDefinitionResponse::Array(lsp_locations)) => {
              locations.extend(lsp_locations.into_iter().map(|location| {
                lsp_location_to_location(location, offset_encoding)
              }));
            },
            Some(lsp_types::GotoDefinitionResponse::Link(lsp_locations)) => {
              locations.extend(lsp_locations.into_iter().map(|location_link| {
                let location = lsp_types::Location::new(
                  location_link.target_uri,
                  location_link.target_range,
                );
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

pub fn goto_implementation(cx: &mut Context) {
  goto_single_impl(
    cx,
    LanguageServerFeature::GotoImplementation,
    |ls, pos, doc_id| ls.goto_implementation(doc_id, pos, None),
    "No implementation found",
  );
}

pub fn goto_reference(cx: &mut Context) {
  let (view, doc) = current_ref!(cx.editor);

  // Collect all the futures with their offset encodings
  let requests: Vec<_> = doc
    .language_servers_with_feature(LanguageServerFeature::GotoReference)
    .filter_map(|language_server| {
      let offset_encoding = language_server.offset_encoding();
      let pos = doc.position(view.id, offset_encoding);
      language_server
        .goto_reference(doc.identifier(), pos, true, None)
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
        Ok((lsp_locations, offset_encoding)) => {
          locations.extend(
            lsp_locations
              .into_iter()
              .flatten()
              .map(|location| lsp_location_to_location(location, offset_encoding)),
          );
        },
        Err(err) => {
          log::error!("Error requesting references: {err}");
        },
      }
    }

    let call = move |editor: &mut crate::editor::Editor, compositor: &mut Compositor| {
      if locations.is_empty() {
        editor.set_error("No references found");
      } else {
        goto_impl(editor, compositor, locations);
      }
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
  });
}

pub fn code_action(cx: &mut Context) {
  let (view, doc) = current_ref!(cx.editor);

  // Get selection range
  let selection = doc.selection(view.id).primary();

  // Collect all the futures
  let requests: Vec<_> = doc
    .language_servers_with_feature(LanguageServerFeature::CodeAction)
    .filter_map(|language_server| {
      let offset_encoding = language_server.offset_encoding();

      // Convert selection to LSP range
      let range = crate::lsp::util::range_to_lsp_range(
        doc.text(),
        selection,
        offset_encoding,
      );

      // Get diagnostics overlapping the selection
      let diagnostics: Vec<lsp_types::Diagnostic> = doc
        .diagnostics()
        .iter()
        .filter(|diag| {
          let diag_range =
            crate::core::selection::Range::new(diag.range.start, diag.range.end);
          selection.overlaps(&diag_range)
        })
        .map(|diag| {
          crate::lsp::util::diagnostic_to_lsp_diagnostic(
            doc.text(),
            diag,
            offset_encoding,
          )
        })
        .collect();

      let context = lsp_types::CodeActionContext {
        diagnostics,
        only: None,
        trigger_kind: Some(lsp_types::CodeActionTriggerKind::INVOKED),
      };

      language_server.code_actions(doc.identifier(), range, context)
    })
    .collect();

  if requests.is_empty() {
    cx.editor.set_error("No language server with code action support");
    return;
  }

  cx.jobs.callback(async move {
    let mut all_actions = Vec::new();

    for future in requests {
      match future.await {
        Ok(Some(actions)) => {
          // Filter out disabled actions
          let enabled_actions: Vec<_> = actions
            .into_iter()
            .filter(|action| match action {
              lsp_types::CodeActionOrCommand::CodeAction(action) => action.disabled.is_none(),
              _ => true,
            })
            .collect();
          all_actions.extend(enabled_actions);
        }
        Ok(None) => {}
        Err(err) => {
          log::error!("Error requesting code actions: {err}");
        }
      }
    }

    let call = move |editor: &mut crate::editor::Editor, compositor: &mut Compositor| {
      if all_actions.is_empty() {
        editor.set_status("No code actions available");
      } else {
        use crate::ui::components::CodeActionMenu;
        compositor.push(Box::new(CodeActionMenu::new(all_actions)));
      }
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
  });
}
