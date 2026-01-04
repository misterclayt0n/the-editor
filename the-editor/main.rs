#![recursion_limit = "512"]

use std::{
  path::Path,
  sync::Arc,
  time::Instant,
};

use arc_swap::{
  ArcSwap,
  access::Map,
};
use smallvec::SmallVec;
use the_editor_event::AsyncHook;

use crate::{
  cli::CliOptions,
  core::{
    config::Config,
    document::{
      Document,
      DocumentOpenError,
    },
    graphics::Rect,
    position::Position,
    selection::{
      Range,
      Selection,
    },
    theme,
  },
  editor::{
    Action,
    Editor,
    EditorConfig,
  },
  handlers::Handlers,
};

pub mod acp;
mod application;
mod cli;
mod core;
mod editor;
mod event;
pub mod handlers;
mod health;
mod increment;
mod input;
pub mod keymap;
mod lsp;
mod profiling;
mod snippets;
mod ui;

fn main() -> anyhow::Result<()> {
  let CliOptions {
    display_version,
    load_tutor,
    fetch_grammars,
    build_grammars,
    health: run_health,
    health_category,
    split,
    verbosity,
    log_file,
    config_file,
    working_dir,
    mut files,
  } = CliOptions::parse()?;

  if display_version {
    println!("the-editor {}", the_editor_loader::VERSION_AND_GIT_HASH);
    return Ok(());
  }

  the_editor_loader::initialize_config_file(config_file);
  the_editor_loader::initialize_log_file(log_file);

  if run_health {
    health::run(health_category.as_deref())?;
    return Ok(());
  }

  if fetch_grammars {
    the_editor_loader::grammar::fetch_grammars()?;
    return Ok(());
  }

  if build_grammars {
    the_editor_loader::grammar::build_grammars(None)?;
    return Ok(());
  }

  setup_logging(verbosity)?;

  let startup_total = Instant::now();

  // Register all event types and hooks up front.
  let t = Instant::now();
  crate::event::register_all_events();
  log::info!("[STARTUP] Event registration: {:?}", t.elapsed());

  // Spawn a Tokio runtime for async hooks/handlers (word index, LSP, etc.).
  let t = Instant::now();
  let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;
  // Enter the runtime before constructing handlers that spawn tasks.
  let guard = rt.enter();
  log::info!("[STARTUP] Tokio runtime: {:?}", t.elapsed());

  // Prepare theme loader.
  let t = Instant::now();
  let mut theme_parent_dirs = vec![the_editor_loader::config_dir()];
  theme_parent_dirs.extend(the_editor_loader::runtime_dirs().iter().cloned());
  let theme_loader = Arc::new(theme::Loader::new(&theme_parent_dirs));
  log::info!("[STARTUP] Theme loader: {:?}", t.elapsed());

  // Initial logical editor area (updated on window resize by renderer).
  let area = Rect::new(0, 0, 120, 40);

  // Load language configuration/loader.
  let t = Instant::now();
  let lang_loader = crate::core::config::user_lang_loader()
    .unwrap_or_else(|_err| crate::core::config::default_lang_loader());
  log::info!("[STARTUP] Language loader: {:?}", t.elapsed());

  // Load config from ~/.config/the-editor/config.toml (falls back to defaults).
  let t = Instant::now();
  let config = match crate::core::config::Config::load_user() {
    Ok(cfg) => cfg,
    Err(err) => {
      println!("Failed to load user config, falling back to defaults: {err:?}");
      Config::default()
    },
  };
  let config_ptr = Arc::new(ArcSwap::from_pointee(config.clone()));
  log::info!("[STARTUP] Config loading: {:?}", t.elapsed());

  // Build handlers and register hooks.
  let t = Instant::now();
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
  crate::handlers::register_hooks(&handlers, &config.editor);
  log::info!("[STARTUP] Handlers setup: {:?}", t.elapsed());

  let t = Instant::now();
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
  editor.set_keymaps(&config.keys);
  log::info!("[STARTUP] Editor construction: {:?}", t.elapsed());

  let t = Instant::now();
  let theme = config
    .theme
    .as_deref()
    .and_then(|name| theme_loader.load(name).ok())
    .unwrap_or_else(|| theme_loader.default_theme(config.editor.true_color));
  // Use instant theme setting at startup to avoid triggering animation
  editor.set_theme_instant(theme);
  log::info!("[STARTUP] Theme loading: {:?}", t.elapsed());

  if let Some(dir) = working_dir {
    the_editor_stdx::env::set_current_working_dir(&dir)?;
  } else if let Some(first_path) = files
    .first()
    .map(|(path, _)| path.clone())
    .filter(|path| path.is_dir())
  {
    the_editor_stdx::env::set_current_working_dir(&first_path)?;
    files.shift_remove(&first_path);
  }

  let t = Instant::now();
  let opened = if load_tutor {
    if !files.is_empty() {
      log::warn!("Ignoring additional file arguments because --tutor was set");
    }
    open_tutor(&mut editor)?
  } else {
    open_cli_files(&mut editor, files, split)?
  };

  if opened == 0 {
    editor.new_file(Action::VerticalSplit);
  } else {
    editor.set_status(format!(
      "Loaded {} file{}.",
      opened,
      if opened == 1 { "" } else { "s" }
    ));
  }
  log::info!(
    "[STARTUP] File opening ({} files): {:?}",
    opened,
    t.elapsed()
  );

  // Create the application wrapper with runtime handle
  let t = Instant::now();
  let app = crate::application::App::new(editor, rt.handle().clone(), config_ptr.clone());
  log::info!("[STARTUP] App creation: {:?}", t.elapsed());

  // Build window configuration from editor config
  let window_config = the_editor_renderer::WindowConfig::new("The Editor", 1024, 768)
    .with_decorations(config.editor.window_decorations);

  log::info!(
    "[STARTUP] Total pre-renderer: {:?}",
    startup_total.elapsed()
  );

  let result = the_editor_renderer::run(window_config, app)
    .map_err(|e| anyhow::anyhow!("Failed to run renderer: {}", e));

  // Explicitly shutdown the runtime with a timeout to avoid blocking on exit
  // Drop the guard first to exit the runtime context
  let t = Instant::now();
  drop(guard);
  log::info!("[SHUTDOWN] Runtime guard drop: {:?}", t.elapsed());

  // Shutdown the runtime with a 5 second timeout for graceful cleanup.
  // Previous 100ms was too aggressive and caused:
  // - Incomplete GPU resource cleanup (freezing Wayland compositor)
  // - Abrupt termination of LSP clients and background tasks
  // - Audio subsystem glitches (PulseAudio/PipeWire not properly disconnected)
  // 5 seconds gives background threads time to:
  // - Complete pending operations
  // - Close network connections gracefully
  // - Release GPU resources via wgpu
  // - Flush file buffers
  let t = Instant::now();
  rt.shutdown_timeout(std::time::Duration::from_secs(5));
  log::info!(
    "[SHUTDOWN] Runtime shutdown (5s timeout): {:?}",
    t.elapsed()
  );

  // Flush logs before exiting to ensure shutdown timing is written
  log::logger().flush();

  result
}

fn setup_logging(verbosity: u8) -> anyhow::Result<()> {
  use chrono::Local;
  use fern::Dispatch;
  use log::LevelFilter;

  let level = match verbosity {
    0 => LevelFilter::Warn,
    1 => LevelFilter::Info,
    2 => LevelFilter::Debug,
    _ => LevelFilter::Trace,
  };

  let file_config = Dispatch::new()
    .format(|out, message, record| {
      out.finish(format_args!(
        "{} {} [{}] {}",
        Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
        record.target(),
        record.level(),
        message
      ))
    })
    .level(level)
    .chain(fern::log_file(the_editor_loader::log_file())?);

  Dispatch::new().level(level).chain(file_config).apply()?;

  Ok(())
}

fn open_tutor(editor: &mut Editor) -> anyhow::Result<usize> {
  let path = the_editor_loader::runtime_file(Path::new("tutor"));
  let doc_id = editor.open(&path, Action::VerticalSplit)?;
  {
    let doc = crate::doc_mut!(editor, &doc_id);
    doc.set_path(None);
  }
  Ok(1)
}

fn open_cli_files(
  editor: &mut Editor,
  files: indexmap::IndexMap<std::path::PathBuf, Vec<Position>>,
  split: Option<cli::SplitMode>,
) -> anyhow::Result<usize> {
  let mut opened = 0usize;
  let has_view = editor.focused_view_id().is_some();

  for (path, positions) in files {
    if path.is_dir() {
      anyhow::bail!(
        "expected a path to file, but found a directory: {}. (to open a directory pass it as \
         first argument)",
        path.display()
      );
    }

    // Determine the action to use:
    // - If split mode is specified, use that
    // - If no view exists yet, use VerticalSplit to create one
    // - Otherwise, use Load to open in the current view
    let action = match split {
      Some(cli::SplitMode::Vertical) => Action::VerticalSplit,
      Some(cli::SplitMode::Horizontal) => Action::HorizontalSplit,
      None => {
        if has_view || opened > 0 {
          Action::Load
        } else {
          Action::VerticalSplit
        }
      },
    };

    match editor.open(&path, action) {
      Ok(doc_id) => {
        opened += 1;
        let view_id = editor
          .focused_view_id()
          .expect("view must exist after opening document");
        let doc = crate::doc_mut!(editor, &doc_id);
        let selection = selection_from_positions(doc, &positions);
        doc.set_selection(view_id, selection);
      },
      Err(DocumentOpenError::IrregularFile) => {
        log::warn!("Skipping irregular file {}", path.display());
      },
      Err(err) => return Err(err.into()),
    }
  }

  Ok(opened)
}

fn selection_from_positions(doc: &Document, positions: &[Position]) -> Selection {
  if positions.is_empty() {
    return Selection::point(0);
  }

  let mut ranges: SmallVec<[Range; 1]> = SmallVec::with_capacity(positions.len());
  let text = doc.text();

  for pos in positions {
    let offset = position_to_char_index(text, pos);
    ranges.push(Range::point(offset));
  }

  let primary_index = ranges.len().saturating_sub(1);
  Selection::new(ranges, primary_index)
}

fn position_to_char_index(text: &ropey::Rope, position: &Position) -> usize {
  if text.len_lines() == 0 {
    return 0;
  }

  let max_row = text.len_lines().saturating_sub(1);
  let row = position.row.min(max_row);
  let line_start = text.line_to_char(row);
  let line = text.line(row);
  let col = position.col.min(line.len_chars());
  line_start + col
}
