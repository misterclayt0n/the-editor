use std::sync::Arc;

use arc_swap::{
  ArcSwap,
  access::Map,
};

use crate::{
  core::{
    config::Config,
    graphics::Rect,
    theme,
  },
  editor::{
    Editor,
    EditorConfig,
  },
  handlers::Handlers,
};

mod core;
mod editor;
mod event;
mod expansion;
pub mod handlers;
mod input;
pub mod keymap;
mod lsp;
mod snippets;
mod ui;

fn main() -> anyhow::Result<()> {
  // Register all event types and hooks up front.
  crate::event::register_all_events();

  // Spawn a Tokio runtime for async hooks/handlers (word index, LSP, etc.).
  let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;
  // Enter the runtime before constructing handlers that spawn tasks.
  let _guard = rt.enter();

  // Prepare theme loader.
  let mut theme_parent_dirs = vec![the_editor_loader::config_dir()];
  theme_parent_dirs.extend(the_editor_loader::runtime_dirs().iter().cloned());
  let theme_loader = Arc::new(theme::Loader::new(&theme_parent_dirs));

  // Initial logical editor area (updated on window resize by renderer).
  let area = Rect::new(0, 0, 120, 40);

  // Load language configuration/loader.
  let lang_loader = crate::core::config::user_lang_loader()
    .unwrap_or_else(|_err| crate::core::config::default_lang_loader());

  // Load config (use defaults for now).
  let config = Config::default();
  let config_ptr = Arc::new(ArcSwap::from_pointee(config.clone()));

  // Build handlers and register hooks.
  let (completion_tx, _completion_rx) = tokio::sync::mpsc::channel(100);
  let (signature_tx, _signature_rx) = tokio::sync::mpsc::channel(100);
  let (auto_save_tx, _auto_save_rx) = tokio::sync::mpsc::channel(100);
  let (colors_tx, _colors_rx) = tokio::sync::mpsc::channel(100);

  let handlers = Handlers {
    completions:     crate::handlers::completion::CompletionHandler::new(completion_tx),
    signature_hints: signature_tx,
    auto_save:       auto_save_tx,
    document_colors: colors_tx,
    word_index:      crate::handlers::word_index::Handler::spawn(),
  };
  crate::handlers::register_hooks(&handlers);

  let mut editor = Editor::new(
    area,
    theme_loader.clone(),
    Arc::new(ArcSwap::from_pointee(lang_loader)),
    Arc::new(Map::new(
      Arc::clone(&config_ptr),
      |c: &Config| -> &EditorConfig { &c.editor },
    )),
    handlers,
  );

  // Create the initial view by opening a new empty file (like helix does).
  {
    use crate::editor::Action;
    editor.new_file(Action::VerticalSplit);
  }

  // Apply configured theme if present.
  if let Some(theme_name) = config.theme.as_deref() {
    if let Ok(theme) = theme_loader.load(theme_name) {
      editor.set_theme(theme);
    }
  }

  the_editor_renderer::run("The Editor - Modern Text Editor", 1024, 768, editor)
    .map_err(|e| anyhow::anyhow!("Failed to run renderer: {}", e))
}
