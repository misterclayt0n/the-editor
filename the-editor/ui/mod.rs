use the_editor_renderer::Color;

pub mod components;
pub mod compositor;
pub mod editor_view;
pub mod explorer;
pub mod file_icons;
pub mod gutter;
pub mod inline_diagnostic_animation;
pub mod job;
pub mod popup_positioning;
pub mod render_cache;
pub mod render_commands;
pub mod text_decorations;
pub mod tree;

pub use editor_view::EditorView;
// Explorer-related exports (currently work in progress)
#[allow(unused_imports)]
pub use explorer::{
  Explorer,
  ExplorerPosition,
};
#[allow(unused_imports)]
pub use tree::{
  GitFileStatus,
  TreeOp,
  TreeView,
  TreeViewItem,
  tree_view_help,
};

// UI Font constants - used across all UI components for consistency
pub const UI_FONT_SIZE: f32 = 14.0;
// Font width calculated based on the monospace font at UI_FONT_SIZE
// Monospace fonts typically have a width-to-height ratio of ~0.6
pub const UI_FONT_WIDTH: f32 = UI_FONT_SIZE * 0.6; // ~8.4 pixels for 14pt font

/// Convert theme color to renderer color
pub fn theme_color_to_renderer_color(theme_color: crate::core::graphics::Color) -> Color {
  use crate::core::graphics::Color as ThemeColor;
  match theme_color {
    ThemeColor::Reset => Color::BLACK,
    ThemeColor::Black => Color::BLACK,
    ThemeColor::Red => Color::RED,
    ThemeColor::Green => Color::GREEN,
    ThemeColor::Yellow => Color::rgb(1.0, 1.0, 0.0),
    ThemeColor::Blue => Color::BLUE,
    ThemeColor::Magenta => Color::rgb(1.0, 0.0, 1.0),
    ThemeColor::Cyan => Color::rgb(0.0, 1.0, 1.0),
    ThemeColor::Gray => Color::GRAY,
    ThemeColor::LightRed => Color::rgb(1.0, 0.5, 0.5),
    ThemeColor::LightGreen => Color::rgb(0.5, 1.0, 0.5),
    ThemeColor::LightYellow => Color::rgb(1.0, 1.0, 0.5),
    ThemeColor::LightBlue => Color::rgb(0.5, 0.5, 1.0),
    ThemeColor::LightMagenta => Color::rgb(1.0, 0.5, 1.0),
    ThemeColor::LightCyan => Color::rgb(0.5, 1.0, 1.0),
    ThemeColor::LightGray => Color::rgb(0.75, 0.75, 0.75),
    ThemeColor::White => Color::WHITE,
    ThemeColor::Rgb(r, g, b) => Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
    ThemeColor::Indexed(i) => {
      // Convert 8-bit indexed colors to approximate RGB values
      match i {
        0 => Color::BLACK,
        1 => Color::RED,
        2 => Color::GREEN,
        3 => Color::rgb(1.0, 1.0, 0.0), // yellow
        4 => Color::BLUE,
        5 => Color::rgb(1.0, 0.0, 1.0), // magenta
        6 => Color::rgb(0.0, 1.0, 1.0), // cyan
        7 => Color::WHITE,
        8 => Color::GRAY,
        9 => Color::rgb(1.0, 0.5, 0.5),  // light red
        10 => Color::rgb(0.5, 1.0, 0.5), // light green
        11 => Color::rgb(1.0, 1.0, 0.5), // light yellow
        12 => Color::rgb(0.5, 0.5, 1.0), // light blue
        13 => Color::rgb(1.0, 0.5, 1.0), // light magenta
        14 => Color::rgb(0.5, 1.0, 1.0), // light cyan
        15 => Color::WHITE,
        // For extended colors (16-255), use a simple grayscale approximation
        _ => {
          let gray = (i as f32 - 16.0) / 239.0;
          Color::rgb(gray, gray, gray)
        },
      }
    },
  }
}

/// Show signature help popup
pub fn show_signature_help(
  editor: &mut crate::editor::Editor,
  compositor: &mut compositor::Compositor,
  invoked: crate::handlers::lsp::SignatureHelpInvoked,
  response: Option<the_editor_lsp_types::types::SignatureHelp>,
  generation: u64,
) {
  use the_editor_event::send_blocking;
  use the_editor_lsp_types::types as lsp;

  use crate::handlers::{
    lsp::{
      SignatureHelpEvent,
      SignatureHelpInvoked,
    },
    signature_help::{
      Signature,
      active_param_range,
    },
  };

  let config = &editor.config();

  // Check if we should show signature help
  if !(config.lsp.auto_signature_help || invoked == SignatureHelpInvoked::Manual) {
    return;
  }

  // Don't show if not in insert mode (for automatic invocations)
  if invoked == SignatureHelpInvoked::Automatic && editor.mode != crate::keymap::Mode::Insert {
    return;
  }

  let response = match response {
    Some(s) if !s.signatures.is_empty() => s,
    _ => {
      send_blocking(
        &editor.handlers.signature_hints,
        SignatureHelpEvent::RequestComplete {
          open: false,
          generation,
        },
      );
      // Clear signature help from EditorView
      if let Some(editor_view) = compositor.find::<EditorView>() {
        editor_view.signature_help = None;
      }
      return;
    },
  };

  send_blocking(
    &editor.handlers.signature_hints,
    SignatureHelpEvent::RequestComplete {
      open: true,
      generation,
    },
  );

  let doc = crate::doc!(editor);
  let language = doc.language_name().unwrap_or("");

  if response.signatures.is_empty() {
    return;
  }

  let signatures: Vec<Signature> = response
    .signatures
    .into_iter()
    .map(|s| {
      let active_param_range_val = active_param_range(&s, response.active_parameter);

      let signature_doc = if config.lsp.display_signature_help_docs {
        s.documentation.map(|doc| {
          match doc {
            lsp::Documentation::String(s) => s,
            lsp::Documentation::MarkupContent(markup) => markup.value,
          }
        })
      } else {
        None
      };

      Signature {
        signature: s.label,
        signature_doc,
        active_param_range: active_param_range_val,
      }
    })
    .collect();

  // Update EditorView with signature help
  if let Some(editor_view) = compositor.find::<EditorView>() {
    if editor_view.completion.is_some() {
      // Mirror Helix behavior: completion popups take precedence, so skip showing
      // signature help when completion is visible to avoid overlapping UI.
      return;
    }
    let mut active_signature = response.active_signature.map(|s| s as usize);
    if active_signature.is_none() {
      active_signature = editor_view
        .signature_help
        .as_ref()
        .map(|helper| helper.active_signature_index());
    }
    let mut active_signature = active_signature.unwrap_or(0);
    let max_index = signatures.len().saturating_sub(1);
    active_signature = active_signature.min(max_index);
    editor_view.set_signature_help(language.to_string(), active_signature, signatures);
  }
}

/// Editor data for file picker (contains root directory)
pub struct FilePickerData {
  root: std::path::PathBuf,
}

/// Create a file picker for the given directory
///
/// The callback receives the selected path and the action (Primary for Enter,
/// Secondary for Ctrl+S horizontal split, Tertiary for Ctrl+V vertical split).
pub fn file_picker<F>(
  root: std::path::PathBuf,
  on_select: F,
) -> components::Picker<std::path::PathBuf, FilePickerData>
where
  F: Fn(&std::path::PathBuf, components::PickerAction) + Send + Sync + 'static,
{
  use ignore::WalkBuilder;

  // Define column: format path relative to root
  let columns = vec![components::Column::new(
    "File",
    |path: &std::path::PathBuf, data: &FilePickerData| {
      // Format the path relative to root, stripping the root prefix
      path
        .strip_prefix(&data.root)
        .ok()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.display().to_string())
    },
  )];

  let editor_data = FilePickerData { root: root.clone() };

  // Create action handler that supports open, hsplit, and vsplit
  let action_handler = std::sync::Arc::new(
    move |path: &std::path::PathBuf, _data: &FilePickerData, action: components::PickerAction| {
      on_select(path, action);
      true // Close picker
    },
  );

  let picker = components::Picker::new(
    columns,
    0,          // primary column index
    Vec::new(), // no initial items (will be injected asynchronously)
    editor_data,
    |_| {}, // Dummy on_select since we're using action_handler
  )
  .with_action_handler(action_handler)
  .with_preview(|path: &std::path::PathBuf| Some((path.clone(), None)));

  let injector = picker.injector();
  let root_clone = root.clone();

  // Spawn background thread to walk directory and inject files
  std::thread::spawn(move || {
    let mut walker = WalkBuilder::new(&root_clone);
    walker
      .hidden(false)
      .parents(true) // Read .gitignore files from parent directories
      .git_ignore(true)
      .git_global(false)
      .git_exclude(false)
      .filter_entry(|entry| {
        // Skip .git directories
        !entry.file_name().to_str().map(|s| s == ".git").unwrap_or(false)
      })
      .sort_by_file_name(|a, b| a.cmp(b));

    for entry in walker.build() {
      let Ok(entry) = entry else { continue };
      if entry.file_type().is_some_and(|ft| ft.is_file()) {
        let path = entry.into_path();
        if injector.push(path).is_err() {
          break; // Picker was closed
        }
      }
    }
  });

  picker
}

/// File explorer metadata shared with picker columns
pub struct FileExplorerData {
  pub root: std::path::PathBuf,
}

/// Selection variants produced by the file explorer picker
#[derive(Debug, Clone)]
pub enum FileExplorerSelection {
  /// Open the target path with the given picker action
  Open {
    path:   std::path::PathBuf,
    action: components::PickerAction,
  },
  /// Enter the selected directory (the path is already normalized)
  Enter(std::path::PathBuf),
}

fn directory_content(path: &std::path::Path) -> std::io::Result<Vec<(std::path::PathBuf, bool)>> {
  let mut entries: Vec<_> = std::fs::read_dir(path)?
    .flatten()
    .map(|entry| {
      let entry_path = entry.path();
      let is_dir = entry
        .file_type()
        .map(|file_type| file_type.is_dir())
        .unwrap_or_else(|_| entry_path.is_dir());
      (entry_path, is_dir)
    })
    .collect();

  entries.sort_by(|(path_a, is_dir_a), (path_b, is_dir_b)| {
    (!*is_dir_a, path_a).cmp(&(!*is_dir_b, path_b))
  });

  if path.parent().is_some() {
    entries.insert(0, (path.join(".."), true));
  }

  Ok(entries)
}

/// Create a file explorer picker rooted at the provided directory
pub fn file_explorer<F>(
  root: std::path::PathBuf,
  on_select: F,
) -> std::io::Result<components::Picker<(std::path::PathBuf, bool), FileExplorerData>>
where
  F: Fn(FileExplorerSelection) + Send + Sync + 'static,
{
  use std::sync::Arc;

  let root = the_editor_stdx::path::normalize(root);
  let data = FileExplorerData { root: root.clone() };

  let columns = vec![components::Column::new(
    "Path",
    |(path, is_dir): &(std::path::PathBuf, bool), data: &FileExplorerData| {
      let display = path
        .strip_prefix(&data.root)
        .unwrap_or(path)
        .to_string_lossy();

      if *is_dir {
        format!("{}/", display)
      } else {
        display.to_string()
      }
    },
  )];

  let items = directory_content(&root)?;

  let on_select = Arc::new(on_select);
  let selection_handler = {
    let on_select = Arc::clone(&on_select);
    Arc::new(
      move |(path, is_dir): &(std::path::PathBuf, bool),
            _data: &FileExplorerData,
            action: components::PickerAction| {
        if *is_dir {
          let normalized = the_editor_stdx::path::normalize(path);
          on_select(FileExplorerSelection::Enter(normalized));
        } else {
          on_select(FileExplorerSelection::Open {
            path: path.clone(),
            action,
          });
        }
        true
      },
    )
  };

  let picker = components::Picker::new(columns, 0, items, data, |_| {})
    .with_action_handler(selection_handler)
    .with_preview(|(path, _is_dir): &(std::path::PathBuf, bool)| Some((path.clone(), None)));

  Ok(picker)
}
