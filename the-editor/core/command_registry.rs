use std::{
  collections::HashMap,
  fmt,
  sync::Arc,
};

use anyhow::{
  Result,
  anyhow,
  bail,
};

use super::commands::Context;
use crate::{
  doc,
  editor::{
    Action,
    Editor,
  },
  ui::components::prompt::Completion,
};

/// Type alias for a command function that takes a context and arguments
pub type CommandFn = fn(&mut Context, &[&str]) -> Result<()>;

/// Type alias for a completer function
/// Takes the editor and current input, returns completions
pub type Completer = fn(&Editor, &str) -> Vec<Completion>;

/// Command completer configuration
#[derive(Clone, Copy)]
pub struct CommandCompleter {
  /// Completers for positional arguments (index-based)
  pub positional: &'static [Completer],
  /// Completer for variadic arguments (all remaining args use this)
  pub variadic:   Completer,
}

impl CommandCompleter {
  /// No completion
  pub const fn none() -> Self {
    Self {
      positional: &[],
      variadic:   completers::none,
    }
  }

  /// Use the same completer for all arguments
  pub const fn all(completer: Completer) -> Self {
    Self {
      positional: &[],
      variadic:   completer,
    }
  }

  /// Use specific completers for specific positions, with fallback for extra
  /// args
  pub const fn positional(positional: &'static [Completer], variadic: Completer) -> Self {
    Self {
      positional,
      variadic,
    }
  }

  /// Get the completer for a specific argument position
  pub fn get(&self, index: usize) -> Completer {
    self.positional.get(index).copied().unwrap_or(self.variadic)
  }
}

/// A typable command that can be executed in command mode
#[derive(Clone)]
pub struct TypableCommand {
  /// Command name (primary identifier)
  pub name:      &'static str,
  /// Command aliases (alternative names)
  pub aliases:   &'static [&'static str],
  /// Short documentation string
  pub doc:       &'static str,
  /// The function to execute
  pub fun:       CommandFn,
  /// Completion configuration for arguments
  pub completer: CommandCompleter,
}

impl TypableCommand {
  /// Create a new typable command
  pub const fn new(
    name: &'static str,
    aliases: &'static [&'static str],
    doc: &'static str,
    fun: CommandFn,
    completer: CommandCompleter,
  ) -> Self {
    Self {
      name,
      aliases,
      doc,
      fun,
      completer,
    }
  }

  /// Execute the command with given context and arguments
  pub fn execute(&self, cx: &mut Context, args: &[&str]) -> Result<()> {
    (self.fun)(cx, args)
  }
}

impl fmt::Debug for TypableCommand {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TypableCommand")
      .field("name", &self.name)
      .field("aliases", &self.aliases)
      .field("doc", &self.doc)
      .finish()
  }
}

/// Registry that holds all available commands
#[derive(Debug, Clone)]
pub struct CommandRegistry {
  commands: HashMap<String, Arc<TypableCommand>>,
}

impl CommandRegistry {
  /// Create a new command registry with default commands
  pub fn new() -> Self {
    let mut registry = Self {
      commands: HashMap::new(),
    };

    // Register built-in commands
    registry.register_builtin_commands();
    registry
  }

  /// Register a command in the registry
  pub fn register(&mut self, command: TypableCommand) {
    let cmd = Arc::new(command);

    // Register primary name
    self.commands.insert(cmd.name.to_string(), cmd.clone());

    // Register aliases
    for alias in cmd.aliases {
      self.commands.insert(alias.to_string(), cmd.clone());
    }
  }

  /// Get a command by name or alias
  pub fn get(&self, name: &str) -> Option<&TypableCommand> {
    self.commands.get(name).map(|cmd| cmd.as_ref())
  }

  /// Execute a command with the given name and arguments
  pub fn execute(&self, cx: &mut Context, name: &str, args: &[&str]) -> Result<()> {
    match self.get(name) {
      Some(command) => command.execute(cx, args),
      None => Err(anyhow!("command not found: {}", name)),
    }
  }

  /// Get all registered command names (for completion)
  pub fn command_names(&self) -> Vec<&str> {
    let mut names: Vec<_> = self.commands.values().map(|cmd| cmd.name).collect();
    names.sort();
    names.dedup();
    names
  }

  /// Get command completions that start with the given prefix
  pub fn completions(&self, prefix: &str) -> Vec<&str> {
    self
      .command_names()
      .into_iter()
      .filter(|name| name.starts_with(prefix))
      .collect()
  }

  /// Complete a command line input (command name or arguments)
  /// This is the main completion function used by the prompt
  pub fn complete_command_line(&self, editor: &Editor, input: &str) -> Vec<Completion> {
    // Split input into command and arguments
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
      // Empty input - show all commands
      return self
        .command_names()
        .into_iter()
        .map(|name| {
          Completion {
            range: 0..,
            text:  name.to_string(),
            doc:   self.get(name).map(|cmd| cmd.doc.to_string()),
          }
        })
        .collect();
    }

    let first_word = parts[0];

    // Check if we're still typing the command name or have moved to arguments
    let complete_command_name = if input.ends_with(char::is_whitespace) {
      // Input ends with whitespace - complete arguments
      false
    } else if parts.len() == 1 {
      // Only one word and no trailing space - still typing command
      true
    } else {
      // Multiple words - complete arguments
      false
    };

    if complete_command_name {
      // Complete command names
      let input_lower = first_word.to_lowercase();
      self
        .command_names()
        .into_iter()
        .filter(|name| name.to_lowercase().contains(&input_lower))
        .map(|name| {
          Completion {
            range: 0..,
            text:  name.to_string(),
            doc:   self.get(name).map(|cmd| cmd.doc.to_string()),
          }
        })
        .collect()
    } else {
      // Complete arguments for the command
      if let Some(cmd) = self.get(first_word) {
        // Calculate which argument we're completing
        let arg_index = if input.ends_with(char::is_whitespace) {
          parts.len() - 1 // Starting a new argument
        } else {
          parts.len() - 2 // Completing current argument
        };

        // Get the completer for this argument position
        let completer = cmd.completer.get(arg_index);

        // Get the current argument text (what we're completing)
        let arg_input = if input.ends_with(char::is_whitespace) {
          ""
        } else {
          parts.last().copied().unwrap_or("")
        };

        // Calculate the offset where the argument starts
        let arg_start_offset = if arg_input.is_empty() {
          input.len()
        } else {
          input.len() - arg_input.len()
        };

        // Run the completer and adjust ranges
        let mut completions = completer(editor, arg_input);

        // Adjust completion ranges to account for the command prefix
        for completion in &mut completions {
          completion.range = arg_start_offset..;
        }

        completions
      } else {
        // Unknown command - no completions
        Vec::new()
      }
    }
  }

  /// Register all built-in commands
  fn register_builtin_commands(&mut self) {
    self.register(TypableCommand::new(
      "quit",
      &["q"],
      "Close the editor",
      quit,
      CommandCompleter::none(),
    ));

    self.register(TypableCommand::new(
      "quit!",
      &["q!"],
      "Force close the editor without saving",
      force_quit,
      CommandCompleter::none(),
    ));

    self.register(TypableCommand::new(
      "write",
      &["w"],
      "Write buffer to file",
      write_buffer,
      CommandCompleter::all(completers::filename),
    ));

    self.register(TypableCommand::new(
      "write-quit",
      &["wq", "x"],
      "Write buffer to file and close the editor",
      write_quit,
      CommandCompleter::all(completers::filename),
    ));

    self.register(TypableCommand::new(
      "open",
      &["o", "e", "edit"],
      "Open a file for editing",
      open_file,
      CommandCompleter::all(completers::filename),
    ));

    self.register(TypableCommand::new(
      "new",
      &["n"],
      "Create a new buffer",
      new_file,
      CommandCompleter::none(),
    ));

    self.register(TypableCommand::new(
      "buffer-close",
      &["bc", "bclose"],
      "Close the current buffer",
      buffer_close,
      CommandCompleter::none(),
    ));

    self.register(TypableCommand::new(
      "buffer-next",
      &["bn", "bnext"],
      "Go to next buffer",
      buffer_next,
      CommandCompleter::none(),
    ));

    self.register(TypableCommand::new(
      "buffer-previous",
      &["bp", "bprev"],
      "Go to previous buffer",
      buffer_previous,
      CommandCompleter::none(),
    ));

    self.register(TypableCommand::new(
      "help",
      &["h"],
      "Show help for commands",
      show_help,
      CommandCompleter::all(completers::command),
    ));

    self.register(TypableCommand::new(
      "theme",
      &[],
      "Change the editor theme (show current theme if no name specified)",
      theme,
      CommandCompleter::all(completers::theme),
    ));

    self.register(TypableCommand::new(
      "format",
      &["fmt"],
      "Format the current buffer using formatter or language server",
      format,
      CommandCompleter::none(),
    ));
  }
}

impl Default for CommandRegistry {
  fn default() -> Self {
    Self::new()
  }
}

/// Completer functions for command arguments
pub mod completers {
  use super::*;

  /// No completion
  pub fn none(_editor: &Editor, _input: &str) -> Vec<Completion> {
    Vec::new()
  }

  /// Filename completer with fuzzy matching
  pub fn filename(_editor: &Editor, input: &str) -> Vec<Completion> {
    use std::{
      fs,
      path::Path,
    };

    let input_path = Path::new(input);
    let (dir, prefix) = if input.ends_with('/') || input.is_empty() {
      (input_path, "")
    } else {
      (
        input_path.parent().unwrap_or(Path::new(".")),
        input_path
          .file_name()
          .and_then(|s| s.to_str())
          .unwrap_or(""),
      )
    };

    let Ok(entries) = fs::read_dir(dir) else {
      return Vec::new();
    };

    let mut completions = Vec::new();
    for entry in entries.flatten() {
      let Ok(file_name) = entry.file_name().into_string() else {
        continue;
      };

      // Fuzzy match against prefix
      if file_name.to_lowercase().contains(&prefix.to_lowercase()) {
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let mut path = if dir == Path::new(".") {
          file_name.clone()
        } else {
          dir.join(&file_name).to_string_lossy().to_string()
        };

        if is_dir {
          path.push('/');
        }

        // Range starts from beginning of input (replace entire path)
        completions.push(Completion {
          range: 0..,
          text:  path,
          doc:   None,
        });
      }
    }

    // Sort completions: directories first, then alphabetically
    completions.sort_by(|a, b| {
      let a_is_dir = a.text.ends_with('/');
      let b_is_dir = b.text.ends_with('/');
      match (a_is_dir, b_is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.text.cmp(&b.text),
      }
    });

    completions
  }

  /// Theme name completer
  pub fn theme(_editor: &Editor, input: &str) -> Vec<Completion> {
    use std::path::PathBuf;

    use crate::core::theme;

    // Get theme directories from runtime paths
    let runtime_dirs = vec![PathBuf::from("runtime/themes")];

    let mut theme_names: Vec<String> = Vec::new();
    for dir in &runtime_dirs {
      let names = theme::Loader::read_names(dir);
      theme_names.extend(names);
    }

    theme_names.sort();
    theme_names.dedup();

    let input_lower = input.to_lowercase();
    theme_names
      .into_iter()
      .filter(|name| name.to_lowercase().contains(&input_lower))
      .map(|name| {
        Completion {
          range: 0..,
          text:  name,
          doc:   None,
        }
      })
      .collect()
  }

  /// Command name completer
  pub fn command(editor: &Editor, input: &str) -> Vec<Completion> {
    let input_lower = input.to_lowercase();

    editor
      .command_registry
      .command_names()
      .into_iter()
      .filter(|name| name.to_lowercase().contains(&input_lower))
      .map(|name| {
        Completion {
          range: 0..,
          text:  name.to_string(),
          doc:   None,
        }
      })
      .collect()
  }
}

// Built-in command implementations

fn quit(cx: &mut Context, _args: &[&str]) -> Result<()> {
  if cx.editor.documents.values().any(|doc| doc.is_modified()) {
    cx.editor
      .set_error("unsaved changes, use :q! to force quit".to_string());
    return Ok(());
  }
  std::process::exit(0);
}

fn force_quit(_cx: &mut Context, _args: &[&str]) -> Result<()> {
  std::process::exit(0);
}

fn write_buffer(cx: &mut Context, args: &[&str]) -> Result<()> {
  use std::path::PathBuf;

  use crate::current;

  let (view, doc) = current!(cx.editor);
  let doc_id = view.doc;

  let path = if args.is_empty() {
    doc.path().map(|p| p.to_path_buf())
  } else {
    Some(PathBuf::from(args[0]))
  };

  if let Some(path) = path {
    match cx.editor.save(doc_id, Some(path.clone()), true) {
      Ok(_) => {
        cx.editor.set_status(format!("written: {}", path.display()));
      },
      Err(err) => {
        cx.editor.set_error(format!("failed to save: {}", err));
      },
    }
  } else {
    cx.editor.set_error("no file name".to_string());
  }

  Ok(())
}

fn write_quit(cx: &mut Context, args: &[&str]) -> Result<()> {
  write_buffer(cx, args)?;

  // Only quit if write was successful (no error was set)
  if cx.editor.documents.values().all(|doc| !doc.is_modified()) {
    std::process::exit(0);
  }

  Ok(())
}

fn open_file(cx: &mut Context, args: &[&str]) -> Result<()> {
  if args.is_empty() {
    cx.editor.set_error("expected file path".to_string());
    return Ok(());
  }

  let path = std::path::PathBuf::from(args[0]);
  match cx.editor.open(&path, crate::editor::Action::Load) {
    Ok(_) => {
      cx.editor.set_status(format!("opened: {}", path.display()));
    },
    Err(err) => {
      cx.editor.set_error(format!("failed to open: {}", err));
    },
  }

  Ok(())
}

fn new_file(cx: &mut Context, _args: &[&str]) -> Result<()> {
  cx.editor.new_file(crate::editor::Action::Load);
  cx.editor.set_status("new buffer created".to_string());
  Ok(())
}

fn buffer_close(cx: &mut Context, _args: &[&str]) -> Result<()> {
  use crate::view_mut;

  let view_id = view_mut!(cx.editor).id;
  let doc_id = cx
    .editor
    .documents
    .iter()
    .find(|(_, doc)| doc.selections().contains_key(&view_id))
    .map(|(id, _)| *id);

  if let Some(doc_id) = doc_id {
    if let Some(doc) = cx.editor.documents.get(&doc_id)
      && doc.is_modified()
    {
      cx.editor
        .set_error("unsaved changes, save first".to_string());
      return Ok(());
    }

    match cx.editor.close_document(doc_id, false) {
      Ok(_) => {},
      Err(_) => {
        cx.editor.set_error("failed to close buffer".to_string());
        return Ok(());
      },
    }
    cx.editor.set_status("buffer closed".to_string());
  }

  Ok(())
}

fn buffer_next(cx: &mut Context, _args: &[&str]) -> Result<()> {
  // Simplified buffer switching - in a real implementation you'd maintain a
  // buffer list
  cx.editor
    .set_status("buffer next (not implemented)".to_string());
  Ok(())
}

fn buffer_previous(cx: &mut Context, _args: &[&str]) -> Result<()> {
  // Simplified buffer switching - in a real implementation you'd maintain a
  // buffer list
  cx.editor
    .set_status("buffer previous (not implemented)".to_string());
  Ok(())
}

fn show_help(cx: &mut Context, args: &[&str]) -> Result<()> {
  if args.is_empty() {
    // Show general help
    let help_text = "Available commands:\n:quit, :q - Close the editor\n:quit!, :q! - Force close \
                     without saving\n:write, :w [file] - Write buffer to file\n:write-quit, :wq, \
                     :x - Write and quit\n:open, :o, :e, :edit <file> - Open a file\n:new, :n - \
                     Create new buffer\n:buffer-close, :bc - Close current buffer\n:theme <name> \
                     - Change the editor theme\n:help, :h [command] - Show help";

    cx.editor.set_status(help_text.to_string());
  } else {
    // Show help for specific command
    if let Some(cmd) = cx.editor.command_registry.get(args[0]) {
      cx.editor.set_status(format!("{}: {}", cmd.name, cmd.doc));
    } else {
      cx.editor.set_error(format!("unknown command: {}", args[0]));
    }
  }

  Ok(())
}

fn theme(cx: &mut Context, args: &[&str]) -> Result<()> {
  let config = cx.editor.config();
  let true_color = config.true_color;

  if args.is_empty() {
    // Show current theme name
    let current_theme = cx.editor.theme.name();
    cx.editor.set_status(current_theme.to_string());
    return Ok(());
  }

  let theme_name = args[0];

  // Try to load the theme
  match cx.editor.theme_loader.load(theme_name) {
    Ok(theme) => {
      // Check if theme is compatible with current color mode
      if !true_color && !theme.is_16_color() {
        cx.editor
          .set_error("theme requires true color support".to_string());
        return Ok(());
      }

      cx.editor.set_theme(theme);
      cx.editor
        .set_status(format!("theme changed to: {}", theme_name));
    },
    Err(err) => {
      cx.editor
        .set_error(format!("failed to load theme '{}': {}", theme_name, err));
    },
  }

  Ok(())
}

fn format(cx: &mut Context, _args: &[&str]) -> Result<()> {
  use crate::current;

  // Get IDs first before any borrows
  let (view, doc) = current!(cx.editor);
  let doc_id = view.doc;
  let view_id = view.id;
  let doc_version = doc.version();

  // Get the document as a static reference (required by format method)
  let doc_ptr = doc as *const _;
  let doc_static = unsafe { &*(doc_ptr as *const crate::core::document::Document) };

  let Some(format_future) = doc_static.format(cx.editor) else {
    cx.editor.set_error(
      "No formatter available (check languages.toml or language server support)".to_string(),
    );
    return Ok(());
  };

  // Spawn async task to format and apply changes
  cx.jobs.callback(async move {
    let transaction_result = format_future.await;

    let callback = move |editor: &mut crate::editor::Editor,
                         _compositor: &mut crate::ui::compositor::Compositor| {
      // Check if document and view still exist
      let Some(doc) = editor.documents.get_mut(&doc_id) else {
        return;
      };

      if !editor.tree.contains(view_id) {
        return;
      }

      match transaction_result {
        Ok(transaction) => {
          // Check if document version hasn't changed
          if doc.version() == doc_version {
            // Apply the formatting transaction
            doc.apply(&transaction, view_id);

            // Detect indent and line ending after formatting
            doc.detect_indent_and_line_ending();

            // Ensure cursor stays in view
            let view = editor.tree.get_mut(view_id);
            crate::core::view::align_view(doc, view, crate::core::view::Align::Center);

            editor.set_status("Buffer formatted".to_string());
          } else {
            log::info!("Discarded formatting changes because the document changed");
            editor.set_status("Formatting discarded (document changed)".to_string());
          }
        },
        Err(err) => {
          log::error!("Formatting failed: {:?}", err);
          editor.set_error(format!("Formatting failed: {:?}", err));
        },
      }
    };

    Ok(crate::ui::job::Callback::EditorCompositor(Box::new(
      callback,
    )))
  });

  Ok(())
}

/// Results in an error if there are modified buffers remaining and sets editor
/// error, otherwise returns `Ok(())`. If the current document is unmodified,
/// and there are modified documents, switches focus to one of them.
pub(super) fn buffers_remaining_impl(editor: &mut Editor) -> anyhow::Result<()> {
  let modified_ids: Vec<_> = editor
    .documents()
    .filter(|doc| doc.is_modified())
    .map(|doc| doc.id())
    .collect();

  if let Some(first) = modified_ids.first() {
    let current = doc!(editor);
    // If the current document is unmodified, and there are modified
    // documents, switch focus to the first modified doc.
    if !modified_ids.contains(&current.id()) {
      editor.switch(*first, Action::Replace);
    }

    let modified_names: Vec<_> = modified_ids
      .iter()
      .map(|doc_id| doc!(editor, doc_id).display_name())
      .collect();

    bail!(
      "{} unsaved buffer{} remaining: {:?}",
      modified_names.len(),
      if modified_names.len() == 1 { "" } else { "s" },
      modified_names,
    );
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_command_registry() {
    let registry = CommandRegistry::new();

    // Test getting a command
    assert!(registry.get("quit").is_some());
    assert!(registry.get("q").is_some()); // alias
    assert!(registry.get("nonexistent").is_none());

    // Test completions
    let completions = registry.completions("q");
    assert!(completions.contains(&"quit"));
  }

  #[test]
  fn test_typable_command() {
    fn test_cmd(_cx: &mut Context, _args: &[&str]) -> Result<()> {
      Ok(())
    }

    let cmd = TypableCommand::new(
      "test",
      &["t"],
      "Test command",
      test_cmd,
      CommandCompleter::none(),
    );

    assert_eq!(cmd.name, "test");
    assert_eq!(cmd.aliases, &["t"]);
    assert_eq!(cmd.doc, "Test command");
  }
}
