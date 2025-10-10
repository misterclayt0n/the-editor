use std::sync::Arc;

use arc_swap::{
  ArcSwap,
  access::Map,
};
use the_editor_event::AsyncHook;

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

mod application;
mod core;
mod editor;
mod event;
mod expansion;
pub mod handlers;
mod increment;
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
  let guard = rt.enter();

  // Prepare theme loader.
  let mut theme_parent_dirs = vec![the_editor_loader::config_dir()];
  theme_parent_dirs.extend(the_editor_loader::runtime_dirs().iter().cloned());
  let theme_loader = Arc::new(theme::Loader::new(&theme_parent_dirs));

  // Initial logical editor area (updated on window resize by renderer).
  let area = Rect::new(0, 0, 120, 40);

  // Load language configuration/loader.
  let lang_loader = crate::core::config::user_lang_loader()
    .unwrap_or_else(|_err| crate::core::config::default_lang_loader());

  // Load config from ~/.config/the-editor/config.toml (falls back to defaults).
  let config = crate::core::config::Config::load_user().unwrap_or_default();
  let config_ptr = Arc::new(ArcSwap::from_pointee(config.clone()));

  // Build handlers and register hooks.
  // Spawn the completion request hook (async debouncer)
  let completion_hook = crate::handlers::completion_request::CompletionRequestHook::new();
  let completion_tx = completion_hook.spawn();

  let signature_help_handler = crate::handlers::signature_help::SignatureHelpHandler::new();
  let signature_tx = signature_help_handler.spawn();
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

  let theme = config
    .theme
    .as_deref()
    .and_then(|name| theme_loader.load(name).ok())
    .unwrap_or_else(|| theme_loader.default_theme(config.editor.true_color));
  editor.set_theme(theme);

  // Create the initial view by opening a new empty file (like helix does).
  {
    use crate::editor::Action;
    editor.new_file(Action::VerticalSplit);
  }

  // Create the application wrapper
  let app = crate::application::App::new(editor);

  // Build window configuration from editor config
  let window_config =
    the_editor_renderer::WindowConfig::new("The Editor - Modern Text Editor", 1024, 768)
      .with_decorations(config.editor.window_decorations);

  let result = the_editor_renderer::run(window_config, app)
    .map_err(|e| anyhow::anyhow!("Failed to run renderer: {}", e));

  // Explicitly shutdown the runtime with a timeout to avoid blocking on exit
  // Drop the guard first to exit the runtime context
  drop(guard);

  // Shutdown the runtime with a 100ms timeout
  // This prevents the editor from hanging on exit waiting for background tasks
  rt.shutdown_timeout(std::time::Duration::from_millis(100));

  result
}
