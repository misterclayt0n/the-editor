use ropey::Rope;
use the_editor_event::{events, register_event};

use crate::{
  core::{
    commands,
    document::Document,
    transaction::ChangeSet,
    DocumentId,
    ViewId,
  },
  editor::{Editor, EditorConfig},
  keymap::Mode,
  lsp::LanguageServerId,
};

events! {
    DocumentDidOpen<'a> {
        editor: &'a mut Editor,
        doc: DocumentId
    }
    DocumentDidChange<'a> {
        doc: &'a mut Document,
        view: ViewId,
        old_text: &'a Rope,
        changes: &'a ChangeSet,
        ghost_transaction: bool
    }
    DocumentDidClose<'a> {
        editor: &'a mut Editor,
        doc: Document
    }
    SelectionDidChange<'a> { doc: &'a mut Document, view: ViewId }
    DiagnosticsDidChange<'a> { editor: &'a mut Editor, doc: DocumentId }
    // called **after** a document loses focus (but not when its closed)
    DocumentFocusLost<'a> { editor: &'a mut Editor, doc: DocumentId }

    LanguageServerInitialized<'a> {
        editor: &'a mut Editor,
        server_id: LanguageServerId
    }
    LanguageServerExited<'a> {
        editor: &'a mut Editor,
        server_id: LanguageServerId
    }

    // NOTE: this event is simple for now and is expected to change as the config system evolves.
    // Ideally it would say what changed.
  ConfigDidChange<'a> { editor: &'a mut Editor, old: &'a EditorConfig, new: &'a EditorConfig }

  OnModeSwitch<'a, 'cx> { old_mode: Mode, new_mode: Mode, cx: &'a mut commands::Context<'cx> }
  PostInsertChar<'a, 'cx> { c: char, cx: &'a mut commands::Context<'cx> }
  // PostCommand<'a, 'cx> { command: & 'a MappableCommand, cx: &'a mut commands::Context<'cx> }
}

// Register all events defined above with the global registry.
// Must be called before registering hooks or dispatching events.
pub fn register_all_events() {
  register_event::<DocumentDidOpen>();
  register_event::<DocumentDidChange>();
  register_event::<DocumentDidClose>();
  register_event::<SelectionDidChange>();
  register_event::<DiagnosticsDidChange>();
  register_event::<DocumentFocusLost>();
  register_event::<LanguageServerInitialized>();
  register_event::<LanguageServerExited>();
  register_event::<ConfigDidChange>();
  register_event::<OnModeSwitch<'_, '_>>();
  register_event::<PostInsertChar<'_, '_>>();
}
