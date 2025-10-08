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
  current,
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

pub fn rename_symbol(cx: &mut Context) {
  let (view, doc) = current_ref!(cx.editor);

  // Check if any language server supports rename
  if doc
    .language_servers_with_feature(LanguageServerFeature::RenameSymbol)
    .next()
    .is_none()
  {
    cx.editor.set_error("No language server with rename symbol support");
    return;
  }

  // Get the word under cursor as prefill
  let text = doc.text().slice(..);
  let selection = doc.selection(view.id).primary();

  // Expand selection to full word if it's just a cursor or single char
  let word_range = if selection.len() <= 1 {
    crate::core::textobject::textobject_word(
      text,
      selection,
      crate::core::textobject::TextObject::Inside,
      1,
      false,
    )
  } else {
    selection
  };

  let prefill = word_range.fragment(text).to_string();

  // Set custom mode string
  cx.editor.set_custom_mode_str("RENAME".to_string());

  // Set mode to Command so prompt is shown
  cx.editor.set_mode(crate::keymap::Mode::Command);

  // Create prompt with callback
  let prompt = crate::ui::components::Prompt::new(String::new())
    .with_prefill(prefill)
    .with_callback(|cx, input, event| {
      // Handle events
      match event {
        crate::ui::components::prompt::PromptEvent::Validate => {
          // Clear custom mode string
          cx.editor.clear_custom_mode_str();

          let (view, doc) = current!(cx.editor);

          // Get language server
          let Some(language_server) = doc
            .language_servers_with_feature(LanguageServerFeature::RenameSymbol)
            .next()
          else {
            cx.editor.set_error("No language server with rename symbol support");
            return;
          };

          let offset_encoding = language_server.offset_encoding();
          let pos = doc.position(view.id, offset_encoding);

          // Call rename_symbol
          let Some(future) = language_server.rename_symbol(doc.identifier(), pos, input.to_string())
          else {
            cx.editor.set_error("Language server does not support rename");
            return;
          };

          // Block on the future to get the result synchronously (like Helix does)
          use futures_executor::block_on;
          log::info!("rename_symbol: calling LSP with new name: {}", input);
          match block_on(future) {
            Ok(Some(workspace_edit)) => {
              log::info!("rename_symbol: received workspace edit: {:?}", workspace_edit);
              if let Err(err) = apply_workspace_edit(cx.editor, &workspace_edit) {
                log::error!("rename_symbol: failed to apply workspace edit: {}", err);
                cx.editor.set_error(format!("Failed to apply rename: {}", err));
              } else {
                log::info!("rename_symbol: workspace edit applied successfully");
                cx.editor.set_status("Symbol renamed");
              }
            }
            Ok(None) => {
              log::warn!("rename_symbol: LSP returned no workspace edit");
              cx.editor.set_status("No changes from rename");
            }
            Err(err) => {
              log::error!("rename_symbol: LSP error: {}", err);
              cx.editor.set_error(format!("Rename failed: {}", err));
            }
          }
        }
        crate::ui::components::prompt::PromptEvent::Abort => {
          // Clear custom mode string on abort
          cx.editor.clear_custom_mode_str();
        }
        crate::ui::components::prompt::PromptEvent::Update => {
          // Nothing to do on update
        }
      }
    });

  // Push prompt to compositor with statusline slide animation
  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    compositor.push(Box::new(prompt));
  }));
}

/// Apply a workspace edit to the editor
fn apply_workspace_edit(
  editor: &mut crate::editor::Editor,
  edit: &lsp_types::WorkspaceEdit,
) -> anyhow::Result<()> {
  use crate::lsp::util::lsp_range_to_range;

  // Apply document changes
  if let Some(ref changes) = edit.changes {
    for (uri, text_edits) in changes {
      let path = uri
        .to_file_path()
        .map_err(|_| anyhow::anyhow!("Invalid file path"))?;

      // Open or get the document
      let doc_id = editor.open(&path, Action::Replace)?;
      let doc = editor
        .documents
        .get_mut(&doc_id)
        .ok_or_else(|| anyhow::anyhow!("Failed to get document"))?;

      // Get the language server to determine offset encoding
      let language_server = doc
        .language_servers_with_feature(LanguageServerFeature::RenameSymbol)
        .next()
        .ok_or_else(|| anyhow::anyhow!("No language server"))?;

      let offset_encoding = language_server.offset_encoding();

      // Apply edits in reverse order to maintain offsets
      let mut edits: Vec<_> = text_edits.iter().collect();
      edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start));

      for edit in edits {
        let text = doc.text();
        if let Some(range) = lsp_range_to_range(text, edit.range, offset_encoding) {
          let transaction = crate::core::transaction::Transaction::change(
            text,
            [(range.anchor, range.head, Some(edit.new_text.as_str().into()))].into_iter(),
          );

          doc.apply(&transaction, editor.tree.focus);
        }
      }
    }
  }

  // Handle document_changes (TextDocumentEdit)
  if let Some(ref document_changes) = edit.document_changes {
    use lsp_types::DocumentChanges;

    let text_edits = match document_changes {
      DocumentChanges::Edits(edits) => edits.as_slice(),
      DocumentChanges::Operations(ops) => {
        // For now, just extract TextDocumentEdit operations
        // Ignore other operations like create/rename/delete files
        let mut edits = Vec::new();
        for op in ops {
          if let lsp_types::DocumentChangeOperation::Edit(edit) = op {
            edits.push(edit.clone());
          }
        }
        // This is a workaround - we'd need to handle this differently
        // but for rename operations, we typically only get Edits variant
        return Err(anyhow::anyhow!("Unsupported document changes format"));
      }
    };

    for text_doc_edit in text_edits {
      let path = text_doc_edit.text_document.uri
        .to_file_path()
        .map_err(|_| anyhow::anyhow!("Invalid file path"))?;

      // Open or get the document
      let doc_id = editor.open(&path, Action::Replace)?;
      let doc = editor
        .documents
        .get_mut(&doc_id)
        .ok_or_else(|| anyhow::anyhow!("Failed to get document"))?;

      // Get the language server to determine offset encoding
      let language_server = doc
        .language_servers_with_feature(LanguageServerFeature::RenameSymbol)
        .next()
        .ok_or_else(|| anyhow::anyhow!("No language server"))?;

      let offset_encoding = language_server.offset_encoding();

      // Extract text edits from the annotated edit union
      let mut text_edits_vec = Vec::new();
      for edit in &text_doc_edit.edits {
        match edit {
          lsp_types::OneOf::Left(text_edit) => {
            text_edits_vec.push(text_edit);
          }
          lsp_types::OneOf::Right(annotated_edit) => {
            text_edits_vec.push(&annotated_edit.text_edit);
          }
        }
      }

      // Apply edits in reverse order to maintain offsets
      text_edits_vec.sort_by_key(|edit| std::cmp::Reverse(edit.range.start));

      for edit in text_edits_vec {
        let text = doc.text();
        if let Some(range) = lsp_range_to_range(text, edit.range, offset_encoding) {
          let transaction = crate::core::transaction::Transaction::change(
            text,
            [(range.anchor, range.head, Some(edit.new_text.as_str().into()))].into_iter(),
          );

          doc.apply(&transaction, editor.tree.focus);
        }
      }
    }
  }

  Ok(())
}
/// Flat symbol for picker display
#[derive(Debug, Clone)]
struct FlatSymbol {
  name:            String,
  kind:            lsp_types::SymbolKind,
  range:           lsp_types::Range,
  uri:             lsp_types::Url,
  offset_encoding: OffsetEncoding,
}

/// Convert SymbolKind to display string
fn symbol_kind_to_string(kind: lsp_types::SymbolKind) -> String {
  match kind {
    lsp_types::SymbolKind::FILE => "File",
    lsp_types::SymbolKind::MODULE => "Module",
    lsp_types::SymbolKind::NAMESPACE => "Namespace",
    lsp_types::SymbolKind::PACKAGE => "Package",
    lsp_types::SymbolKind::CLASS => "Class",
    lsp_types::SymbolKind::METHOD => "Method",
    lsp_types::SymbolKind::PROPERTY => "Property",
    lsp_types::SymbolKind::FIELD => "Field",
    lsp_types::SymbolKind::CONSTRUCTOR => "Constructor",
    lsp_types::SymbolKind::ENUM => "Enum",
    lsp_types::SymbolKind::INTERFACE => "Interface",
    lsp_types::SymbolKind::FUNCTION => "Function",
    lsp_types::SymbolKind::VARIABLE => "Variable",
    lsp_types::SymbolKind::CONSTANT => "Constant",
    lsp_types::SymbolKind::STRING => "String",
    lsp_types::SymbolKind::NUMBER => "Number",
    lsp_types::SymbolKind::BOOLEAN => "Boolean",
    lsp_types::SymbolKind::ARRAY => "Array",
    lsp_types::SymbolKind::OBJECT => "Object",
    lsp_types::SymbolKind::KEY => "Key",
    lsp_types::SymbolKind::NULL => "Null",
    lsp_types::SymbolKind::ENUM_MEMBER => "EnumMember",
    lsp_types::SymbolKind::STRUCT => "Struct",
    lsp_types::SymbolKind::EVENT => "Event",
    lsp_types::SymbolKind::OPERATOR => "Operator",
    lsp_types::SymbolKind::TYPE_PARAMETER => "TypeParam",
    _ => "Unknown",
  }
  .to_string()
}

/// Document symbols picker - navigate within current file by symbols
pub fn document_symbols(cx: &mut Context) {
  // Helper function to flatten hierarchical DocumentSymbol
  fn flatten_document_symbol(
    symbols: &mut Vec<FlatSymbol>,
    uri: lsp_types::Url,
    symbol: lsp_types::DocumentSymbol,
    offset_encoding: OffsetEncoding,
  ) {
    symbols.push(FlatSymbol {
      name:            symbol.name.clone(),
      kind:            symbol.kind,
      range:           symbol.selection_range,
      uri:             uri.clone(),
      offset_encoding,
    });

    if let Some(children) = symbol.children {
      for child in children {
        flatten_document_symbol(symbols, uri.clone(), child, offset_encoding);
      }
    }
  }

  let (_view, doc) = current_ref!(cx.editor);

  //Get current document URL
  let current_url = doc.url();

  // Collect all document symbols from language servers
  let requests: Vec<_> = doc
    .language_servers_with_feature(LanguageServerFeature::DocumentSymbols)
    .filter_map(|language_server| {
      let offset_encoding = language_server.offset_encoding();
      language_server
        .document_symbols(doc.identifier())
        .map(|future| (future, offset_encoding))
    })
    .collect();

  // Check for URL after collecting requests (can't call set_error before)
  let Some(current_url) = current_url else {
    return; // Silently fail if no URL - shouldn't happen for valid documents
  };

  let current_url_clone = current_url.clone();

  cx.jobs.callback(async move {
    let mut all_symbols = Vec::new();

    for (future, offset_encoding) in requests {
      match future.await {
        Ok(Some(response)) => {
          match response {
            lsp_types::DocumentSymbolResponse::Flat(symbols) => {
              for symbol in symbols {
                all_symbols.push(FlatSymbol {
                  name:            symbol.name,
                  kind:            symbol.kind,
                  range:           symbol.location.range,
                  uri:             symbol.location.uri.clone(),
                  offset_encoding,
                });
              }
            }
            lsp_types::DocumentSymbolResponse::Nested(symbols) => {
              for symbol in symbols {
                flatten_document_symbol(
                  &mut all_symbols,
                  current_url_clone.clone(),
                  symbol,
                  offset_encoding,
                );
              }
            }
          }
        }
        Ok(None) => {}
        Err(err) => {
          log::error!("Error requesting document symbols: {err}");
        }
      }
    }

    let call = move |editor: &mut crate::editor::Editor, compositor: &mut Compositor| {
      if all_symbols.is_empty() {
        editor.set_status("No symbols found");
        return;
      }

      // Create picker columns
      use crate::ui::components::{
        Column,
        Picker,
        PickerAction,
      };

      let columns = vec![
        Column::new("Kind", |symbol: &FlatSymbol, _: &()| {
          symbol_kind_to_string(symbol.kind)
        }),
        Column::new("Symbol", |symbol: &FlatSymbol, _: &()| symbol.name.clone()),
        Column::new("Line", |symbol: &FlatSymbol, _: &()| {
          format!("{}", symbol.range.start.line + 1)
        }),
      ];

      // Create action handler to jump to symbol
      let action_handler = std::sync::Arc::new(
        move |symbol: &FlatSymbol, _: &(), _action: PickerAction| {
          // Clone symbol to move into the closure
          let symbol = symbol.clone();

          // Jump to the symbol location
          crate::ui::job::dispatch_blocking(move |editor, _compositor| {
            // Get the current view
            let view = editor.tree.get_mut(editor.tree.focus);
            let doc = editor.documents.get_mut(&view.doc).unwrap();

            // Convert LSP range to editor range
            if let Some(range) =
              lsp_range_to_range(doc.text(), symbol.range, symbol.offset_encoding)
            {
              // Set selection to the symbol location
              doc.set_selection(view.id, Selection::single(range.anchor, range.head));

              // Align view to center the symbol
              align_view(doc, view, Align::Center);
            }
          });

          true // Close picker
        },
      );

      let picker = Picker::new(
        columns,
        1, // Primary column is "Symbol"
        all_symbols,
        (), // No editor data needed
        |_| {}, // Dummy on_select
      )
      .with_action_handler(action_handler)
      .with_preview(|symbol: &FlatSymbol| {
        // Return path and line range for preview with selection
        symbol.uri.to_file_path().ok().map(|path| {
          (
            path,
            Some((
              symbol.range.start.line as usize,
              symbol.range.end.line as usize,
            )),
          )
        })
      });

      compositor.push(Box::new(picker));
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
  });
}
/// Workspace symbols picker - navigate across entire workspace by symbols
pub fn workspace_symbols(cx: &mut Context) {
  let (_view, doc) = current_ref!(cx.editor);

  // Collect all workspace symbols from language servers
  let requests: Vec<_> = doc
    .language_servers_with_feature(LanguageServerFeature::WorkspaceSymbols)
    .filter_map(|language_server| {
      let offset_encoding = language_server.offset_encoding();
      // Use empty query to get all symbols, let picker handle filtering
      language_server
        .workspace_symbols(String::new())
        .map(|future| (future, offset_encoding))
    })
    .collect();

  if requests.is_empty() {
    cx.editor.set_error("No language server with workspace symbols support");
    return;
  }

  cx.jobs.callback(async move {
    let mut all_symbols = Vec::new();

    for (future, offset_encoding) in requests {
      match future.await {
        Ok(Some(response)) => {
          match response {
            lsp_types::WorkspaceSymbolResponse::Flat(symbols) => {
              for symbol in symbols {
                all_symbols.push(FlatSymbol {
                  name:            symbol.name,
                  kind:            symbol.kind,
                  range:           symbol.location.range,
                  uri:             symbol.location.uri.clone(),
                  offset_encoding,
                });
              }
            }
            lsp_types::WorkspaceSymbolResponse::Nested(_) => {
              // Nested workspace symbols are rare, skip for now
              log::warn!("Nested workspace symbols not supported");
            }
          }
        }
        Ok(None) => {}
        Err(err) => {
          log::error!("Error requesting workspace symbols: {err}");
        }
      }
    }

    let call = move |editor: &mut crate::editor::Editor, compositor: &mut Compositor| {
      if all_symbols.is_empty() {
        editor.set_status("No workspace symbols found");
        return;
      }

      // Create picker columns
      use crate::ui::components::{
        Column,
        Picker,
        PickerAction,
      };

      let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

      let columns = vec![
        Column::new("Kind", |symbol: &FlatSymbol, _: &std::path::PathBuf| {
          symbol_kind_to_string(symbol.kind)
        }),
        Column::new("Symbol", |symbol: &FlatSymbol, _: &std::path::PathBuf| {
          symbol.name.clone()
        }),
        Column::new("File", |symbol: &FlatSymbol, cwd: &std::path::PathBuf| {
          if let Ok(path) = symbol.uri.to_file_path() {
            path
              .strip_prefix(cwd)
              .unwrap_or(&path)
              .display()
              .to_string()
          } else {
            symbol.uri.to_string()
          }
        }),
        Column::new("Line", |symbol: &FlatSymbol, _: &std::path::PathBuf| {
          format!("{}", symbol.range.start.line + 1)
        }),
      ];

      // Create action handler to jump to symbol
      let action_handler = std::sync::Arc::new(
        move |symbol: &FlatSymbol, _: &std::path::PathBuf, action: PickerAction| {
          // Clone symbol to move into the closure
          let symbol = symbol.clone();

          let action_type = match action {
            PickerAction::Primary => Action::Replace,
            PickerAction::Secondary => Action::HorizontalSplit,
            PickerAction::Tertiary => Action::VerticalSplit,
          };

          // Jump to the symbol location
          crate::ui::job::dispatch_blocking(move |editor, _compositor| {
            let path = match symbol.uri.to_file_path() {
              Ok(p) => p,
              Err(_) => {
                editor.set_error(format!("Invalid URI: {}", symbol.uri));
                return;
              }
            };

            // Open the file
            let doc_id = match editor.open(&path, action_type) {
              Ok(id) => id,
              Err(err) => {
                editor.set_error(format!("Failed to open file: {}", err));
                return;
              }
            };

            // Get the view and document
            let view = editor.tree.get_mut(editor.tree.focus);
            let doc = editor.documents.get_mut(&doc_id).unwrap();

            // Convert LSP range to editor range
            if let Some(range) =
              lsp_range_to_range(doc.text(), symbol.range, symbol.offset_encoding)
            {
              // Set selection to the symbol location
              doc.set_selection(view.id, Selection::single(range.anchor, range.head));

              // Align view to center the symbol
              align_view(doc, view, Align::Center);
            }
          });

          true // Close picker
        },
      );

      let picker = Picker::new(
        columns,
        1, // Primary column is "Symbol"
        all_symbols,
        cwd,
        |_| {}, // Dummy on_select
      )
      .with_action_handler(action_handler)
      .with_preview(|symbol: &FlatSymbol| {
        // Return path and line range for preview with selection
        symbol.uri.to_file_path().ok().map(|path| {
          (
            path,
            Some((
              symbol.range.start.line as usize,
              symbol.range.end.line as usize,
            )),
          )
        })
      });

      compositor.push(Box::new(picker));
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
  });
}
/// Document diagnostics picker - view all diagnostics in current file
pub fn document_diagnostics(cx: &mut Context) {
  let (_view, doc) = current_ref!(cx.editor);

  // Get document diagnostics
  let diagnostics = doc.diagnostics();

  if diagnostics.is_empty() {
    cx.editor.set_status("No diagnostics in current document");
    return;
  }

  // Get document URL for preview
  let doc_url = doc.url();

  // Clone diagnostics for async callback
  let diagnostics: Vec<_> = diagnostics.iter().cloned().collect();

  cx.callback.push(Box::new(move |compositor, _cx| {
    use crate::{
      core::diagnostics::Severity,
      ui::components::{
        Column,
        Picker,
        PickerAction,
      },
    };

    let columns = vec![
      Column::new("Severity", |diag: &crate::core::diagnostics::Diagnostic, _: &()| {
        match diag.severity {
          Some(Severity::Error) => "ERROR",
          Some(Severity::Warning) => "WARN",
          Some(Severity::Info) => "INFO",
          Some(Severity::Hint) => "HINT",
          None => "",
        }
        .to_string()
      }),
      Column::new("Source", |diag: &crate::core::diagnostics::Diagnostic, _: &()| {
        diag.source.clone().unwrap_or_default()
      }),
      Column::new("Code", |diag: &crate::core::diagnostics::Diagnostic, _: &()| {
        match &diag.code {
          Some(crate::core::diagnostics::NumberOrString::Number(n)) => n.to_string(),
          Some(crate::core::diagnostics::NumberOrString::String(s)) => s.clone(),
          None => String::new(),
        }
      }),
      Column::new("Line", |diag: &crate::core::diagnostics::Diagnostic, _: &()| {
        format!("{}", diag.line + 1)
      }),
      Column::new("Message", |diag: &crate::core::diagnostics::Diagnostic, _: &()| {
        diag.message.clone()
      }),
    ];

    // Create action handler to jump to diagnostic
    let action_handler = std::sync::Arc::new(
      move |diag: &crate::core::diagnostics::Diagnostic, _: &(), _action: PickerAction| {
        // Clone diagnostic to move into the closure
        let diag = diag.clone();

        // Jump to the diagnostic location
        crate::ui::job::dispatch_blocking(move |editor, _compositor| {
          // Get the current view and document
          let view = editor.tree.get_mut(editor.tree.focus);
          let doc = editor.documents.get_mut(&view.doc).unwrap();

          // Set selection to the diagnostic location
          doc.set_selection(view.id, Selection::single(diag.range.start, diag.range.end));

          // Align view to center the diagnostic
          align_view(doc, view, Align::Center);
        });

        true // Close picker
      },
    );

    let picker = Picker::new(
      columns,
      4, // Primary column is "Message"
      diagnostics,
      (),
      |_| {}, // Dummy on_select
    )
    .with_action_handler(action_handler)
    .with_preview(move |diag: &crate::core::diagnostics::Diagnostic| {
      // Return path and line range for preview with selection
      doc_url.as_ref().and_then(|url| {
        url.to_file_path().ok().map(|path| {
          (
            path,
            Some((diag.line, diag.line)),
          )
        })
      })
    });

    compositor.push(Box::new(picker));
  }));
}

/// Workspace diagnostics picker - view all diagnostics across the workspace
pub fn workspace_diagnostics(cx: &mut Context) {
  // Collect diagnostics from all documents
  #[derive(Debug, Clone)]
  struct DiagnosticItem {
    diagnostic: crate::core::diagnostics::Diagnostic,
    doc_id:     crate::core::DocumentId,
    path:       Option<std::path::PathBuf>,
    file_name:  String,
  }

  let mut all_diagnostics = Vec::new();

  // Iterate through all documents
  for (doc_id, doc) in &cx.editor.documents {
    let diagnostics = doc.diagnostics();
    if diagnostics.is_empty() {
      continue;
    }

    let path = doc.path().map(|p| p.to_path_buf());
    let file_name = path
      .as_ref()
      .and_then(|p| p.file_name())
      .and_then(|n| n.to_str())
      .unwrap_or("[No Name]")
      .to_string();

    for diag in diagnostics.iter() {
      all_diagnostics.push(DiagnosticItem {
        diagnostic: diag.clone(),
        doc_id:     *doc_id,
        path:       path.clone(),
        file_name:  file_name.clone(),
      });
    }
  }

  if all_diagnostics.is_empty() {
    cx.editor.set_status("No diagnostics in workspace");
    return;
  }

  cx.callback.push(Box::new(move |compositor, _cx| {
    use crate::{
      core::diagnostics::Severity,
      ui::components::{
        Column,
        Picker,
        PickerAction,
      },
    };

    let columns = vec![
      Column::new("Severity", |item: &DiagnosticItem, _: &()| {
        match item.diagnostic.severity {
          Some(Severity::Error) => "ERROR",
          Some(Severity::Warning) => "WARN",
          Some(Severity::Info) => "INFO",
          Some(Severity::Hint) => "HINT",
          None => "",
        }
        .to_string()
      }),
      Column::new("File", |item: &DiagnosticItem, _: &()| item.file_name.clone()),
      Column::new("Line", |item: &DiagnosticItem, _: &()| {
        format!("{}", item.diagnostic.line + 1)
      }),
      Column::new("Source", |item: &DiagnosticItem, _: &()| {
        item.diagnostic.source.clone().unwrap_or_default()
      }),
      Column::new("Code", |item: &DiagnosticItem, _: &()| {
        match &item.diagnostic.code {
          Some(crate::core::diagnostics::NumberOrString::Number(n)) => n.to_string(),
          Some(crate::core::diagnostics::NumberOrString::String(s)) => s.clone(),
          None => String::new(),
        }
      }),
      Column::new("Message", |item: &DiagnosticItem, _: &()| item.diagnostic.message.clone()),
    ];

    // Create action handler to jump to diagnostic
    let action_handler = std::sync::Arc::new(move |item: &DiagnosticItem, _: &(), _action: PickerAction| {
      // Clone item to move into the closure
      let item = item.clone();

      // Jump to the diagnostic location
      crate::ui::job::dispatch_blocking(move |editor, _compositor| {
        // First, ensure the document is open
        let doc_id = editor.documents.get(&item.doc_id).map(|_| item.doc_id).or_else(|| {
          // Document might have been closed, try to open it
          item.path.as_ref().and_then(|path| {
            editor.open(path, crate::editor::Action::Replace).ok()
          })
        });

        if let Some(doc_id) = doc_id {
          // Focus the document
          let view_id = editor.tree.focus;
          let view = editor.tree.get_mut(view_id);
          view.doc = doc_id;

          // Set selection to the diagnostic location
          let doc = editor.documents.get_mut(&doc_id).unwrap();
          doc.set_selection(view_id, Selection::single(item.diagnostic.range.start, item.diagnostic.range.end));

          // Align view to center the diagnostic
          align_view(doc, editor.tree.get_mut(view_id), Align::Center);
        }
      });

      true // Close picker
    });

    let picker = Picker::new(
      columns,
      5, // Primary column is "Message"
      all_diagnostics,
      (),
      |_| {}, // Dummy on_select
    )
    .with_action_handler(action_handler)
    .with_preview(move |item: &DiagnosticItem| {
      // Return path and line range for preview with selection
      item.path.as_ref().map(|path| {
        (
          path.clone(),
          Some((item.diagnostic.line, item.diagnostic.line)),
        )
      })
    });

    compositor.push(Box::new(picker));
  }));
}
