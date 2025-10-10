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

use super::{
  command_line::{
    Args,
    CompletionState,
    Signature,
    Token,
    Tokenizer,
  },
  commands::Context,
  expansion,
};
use crate::ui::components::prompt::PromptEvent;
use crate::{
  doc,
  editor::{
    Action,
    Editor,
  },
  ui::components::prompt::Completion,
};

/// Type alias for a command function that takes a context, parsed arguments, and prompt event
pub type CommandFn = fn(&mut Context, Args, PromptEvent) -> Result<()>;

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
  /// Command signature (positional args, flags, etc.)
  pub signature: Signature,
}

impl TypableCommand {
  /// Create a new typable command
  pub const fn new(
    name: &'static str,
    aliases: &'static [&'static str],
    doc: &'static str,
    fun: CommandFn,
    completer: CommandCompleter,
    signature: Signature,
  ) -> Self {
    Self {
      name,
      aliases,
      doc,
      fun,
      completer,
      signature,
    }
  }

  /// Execute the command with given context, parsed arguments, and prompt event
  pub fn execute(&self, cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
    (self.fun)(cx, args, event)
  }

  /// Generate comprehensive documentation for this command
  pub fn generate_doc(&self) -> String {
    use std::fmt::Write;

    let mut doc = String::new();

    // Command name and basic doc
    writeln!(doc, ":{} - {}", self.name, self.doc).unwrap();

    // Aliases
    if !self.aliases.is_empty() {
      writeln!(doc, "Aliases: {}", self.aliases.join(", ")).unwrap();
    }

    // Positional arguments
    let (min, max) = self.signature.positionals;
    if min > 0 || max.is_some() {
      write!(doc, "Arguments: ").unwrap();
      match (min, max) {
        (0, Some(0)) => writeln!(doc, "none").unwrap(),
        (0, Some(1)) => writeln!(doc, "[arg] (optional)").unwrap(),
        (1, Some(1)) => writeln!(doc, "<arg> (required)").unwrap(),
        (0, None) => writeln!(doc, "[args...] (zero or more)").unwrap(),
        (1, None) => writeln!(doc, "<arg> [args...] (one or more)").unwrap(),
        (min, Some(max)) if min == max => writeln!(doc, "{} argument{}", min, if min == 1 { "" } else { "s" }).unwrap(),
        (min, Some(max)) => writeln!(doc, "{}-{} arguments", min, max).unwrap(),
        (min, None) => writeln!(doc, "{} or more arguments", min).unwrap(),
      }
    }

    // Flags
    if !self.signature.flags.is_empty() {
      writeln!(doc, "Flags:").unwrap();

      // Calculate max flag name length for alignment
      let max_flag_len = self.signature.flags
        .iter()
        .map(|flag| {
          let name_len = flag.name.len();
          let alias_len = if flag.alias.is_some() { 3 } else { 0 }; // "/-X"
          let arg_len = if flag.completions.is_some() { 6 } else { 0 }; // " <arg>"
          name_len + alias_len + arg_len
        })
        .max()
        .unwrap_or(0);

      for flag in self.signature.flags {
        write!(doc, "  --{}", flag.name).unwrap();
        let mut current_len = flag.name.len();

        // Add alias if present
        if let Some(alias) = flag.alias {
          write!(doc, "/-{}", alias).unwrap();
          current_len += 3;
        }

        // Add argument placeholder if flag takes an argument
        if flag.completions.is_some() {
          write!(doc, " <arg>").unwrap();
          current_len += 6;
        }

        // Padding for alignment
        let padding = max_flag_len - current_len;
        write!(doc, "{:width$}  {}", "", flag.doc, width = padding).unwrap();
        writeln!(doc).unwrap();
      }
    }

    doc
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

  /// Execute a command with the given name and arguments string
  /// The args_line is parsed according to the command's signature with variable expansion
  pub fn execute(
    &self,
    cx: &mut Context,
    name: &str,
    args_line: &str,
    event: PromptEvent,
  ) -> Result<()> {
    match self.get(name) {
      Some(command) => {
        // Parse arguments according to command signature with expansion
        let args = Args::parse(args_line, command.signature, event == PromptEvent::Validate, |token| {
          expansion::expand(cx.editor, token).map_err(|e| Box::from(e.to_string()) as Box<dyn std::error::Error>)
        }).map_err(|e| anyhow!("{}", e))?;

        command.execute(cx, args, event)
      },
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
        // Split command from args using command_line parser
        let (_, args_line, _) = super::command_line::split(input);

        // Parse arguments to determine completion state
        // Use non-validating mode so we can get partial parses for completion
        let mut args = Args::new(cmd.signature, false);
        let mut tokenizer = Tokenizer::new(args_line, false);
        let mut last_token: Option<Token> = None;

        // Parse all tokens to build up args state
        while let Ok(Some(token)) = args.read_token(&mut tokenizer) {
          last_token = Some(token.clone());
          let _ = args.push(token.content);
        }

        // Check completion state to determine what to complete
        match args.completion_state() {
          CompletionState::Positional => {
            // Complete positional argument
            let arg_index = args.len();
            let completer = cmd.completer.get(arg_index);

            // Get the text being completed
            let (arg_input, arg_start_offset) = match &last_token {
              Some(token) if !token.is_terminated => {
                (token.content.as_ref(), first_word.len() + 1 + token.content_start)
              },
              _ => ("", input.len()),
            };

            // Run completer and adjust ranges
            let mut completions = completer(editor, arg_input);
            for completion in &mut completions {
              completion.range = arg_start_offset..;
            }
            completions
          },
          CompletionState::Flag(_) => {
            // Complete flag names
            if cmd.signature.flags.is_empty() {
              return Vec::new();
            }

            let (flag_input, flag_start_offset) = match &last_token {
              Some(token) if !token.is_terminated => {
                let input = token.content.as_ref();
                let trimmed = input.trim_start_matches('-');
                (trimmed, first_word.len() + 1 + token.content_start)
              },
              _ => ("", input.len()),
            };

            // Fuzzy match flag names
            cmd.signature.flags
              .iter()
              .filter(|flag| flag.name.contains(flag_input))
              .map(|flag| {
                Completion {
                  range: flag_start_offset..,
                  text: format!("--{}", flag.name),
                  doc: Some(flag.doc.to_string()),
                }
              })
              .collect()
          },
          CompletionState::FlagArgument(flag) => {
            // Complete flag argument
            if let Some(completions) = flag.completions {
              let (arg_input, arg_start_offset) = match &last_token {
                Some(token) if !token.is_terminated => {
                  (token.content.as_ref(), first_word.len() + 1 + token.content_start)
                },
                _ => ("", input.len()),
              };

              completions
                .iter()
                .filter(|val| val.contains(arg_input))
                .map(|val| {
                  Completion {
                    range: arg_start_offset..,
                    text: val.to_string(),
                    doc: None,
                  }
                })
                .collect()
            } else {
              Vec::new()
            }
          },
        }
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
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "quit!",
      &["q!"],
      "Force close the editor without saving",
      force_quit,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "write",
      &["w"],
      "Write buffer to file",
      write_buffer,
      CommandCompleter::all(completers::filename),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "write-quit",
      &["wq", "x"],
      "Write buffer to file and close the editor",
      write_quit,
      CommandCompleter::all(completers::filename),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "open",
      &["o", "e", "edit"],
      "Open a file for editing",
      open_file,
      CommandCompleter::all(completers::filename),
      Signature {
        positionals: (1, None),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "new",
      &["n"],
      "Create a new buffer",
      new_file,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "buffer-close",
      &["bc", "bclose"],
      "Close the current buffer",
      buffer_close,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "buffer-next",
      &["bn", "bnext"],
      "Go to next buffer",
      buffer_next,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "buffer-previous",
      &["bp", "bprev"],
      "Go to previous buffer",
      buffer_previous,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "help",
      &["h"],
      "Show help for commands",
      show_help,
      CommandCompleter::all(completers::command),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "theme",
      &[],
      "Change the editor theme (show current theme if no name specified)",
      theme,
      CommandCompleter::all(completers::theme),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "format",
      &["fmt"],
      "Format the current buffer using formatter or language server",
      format,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
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

fn quit(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  if cx.editor.documents.values().any(|doc| doc.is_modified()) {
    cx.editor
      .set_error("unsaved changes, use :q! to force quit".to_string());
    return Ok(());
  }
  std::process::exit(0);
}

fn force_quit(_cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  std::process::exit(0);
}

fn write_buffer(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  use std::path::PathBuf;

  use crate::current;

  let (view, doc) = current!(cx.editor);
  let doc_id = view.doc;

  let path = if args.is_empty() {
    doc.path().map(|p| p.to_path_buf())
  } else {
    Some(PathBuf::from(&args[0]))
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

fn write_quit(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  write_buffer(cx, args, PromptEvent::Validate)?;

  // Only quit if write was successful (no error was set)
  if cx.editor.documents.values().all(|doc| !doc.is_modified()) {
    std::process::exit(0);
  }

  Ok(())
}

fn open_file(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  if args.is_empty() {
    cx.editor.set_error("expected file path".to_string());
    return Ok(());
  }

  let path = std::path::PathBuf::from(&args[0]);
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

fn new_file(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  cx.editor.new_file(crate::editor::Action::Load);
  cx.editor.set_status("new buffer created".to_string());
  Ok(())
}

fn buffer_close(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

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

fn buffer_next(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  // Simplified buffer switching - in a real implementation you'd maintain a
  // buffer list
  cx.editor
    .set_status("buffer next (not implemented)".to_string());
  Ok(())
}

fn buffer_previous(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  // Simplified buffer switching - in a real implementation you'd maintain a
  // buffer list
  cx.editor
    .set_status("buffer previous (not implemented)".to_string());
  Ok(())
}

fn show_help(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  if args.is_empty() {
    // Show list of all commands
    let mut help_text = String::from("Available commands (use :help <command> for details):\n");

    let mut commands = cx.editor.command_registry.command_names();
    commands.sort();
    commands.dedup();

    for cmd_name in commands {
      if let Some(cmd) = cx.editor.command_registry.get(cmd_name) {
        help_text.push_str(&format!("  :{} - {}\n", cmd.name, cmd.doc));
      }
    }

    cx.editor.set_status(help_text);
  } else {
    // Show detailed help for specific command using generated documentation
    if let Some(cmd) = cx.editor.command_registry.get(&args[0]) {
      let doc = cmd.generate_doc();
      cx.editor.set_status(doc);
    } else {
      cx.editor.set_error(format!("unknown command: {}", &args[0]));
    }
  }

  Ok(())
}

fn theme(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  let config = cx.editor.config();
  let true_color = config.true_color;

  match event {
    PromptEvent::Abort => {
      // Restore previous theme
      // TODO: Store and restore previous theme
      return Ok(());
    },
    PromptEvent::Update => {
      // Preview theme while typing
      if args.is_empty() {
        return Ok(());
      }

      let theme_name = &args[0];
      if let Ok(theme) = cx.editor.theme_loader.load(theme_name) {
        if true_color || theme.is_16_color() {
          cx.editor.theme = theme;
        }
      }

      return Ok(());
    },
    PromptEvent::Validate => {
      // Apply theme permanently
    },
  }

  if args.is_empty() {
    // Show current theme name
    let current_theme = cx.editor.theme.name();
    cx.editor.set_status(current_theme.to_string());
    return Ok(());
  }

  let theme_name = &args[0];

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

fn format(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

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
    fn test_cmd(_cx: &mut Context, _args: Args, _event: PromptEvent) -> Result<()> {
      Ok(())
    }

    let cmd = TypableCommand::new(
      "test",
      &["t"],
      "Test command",
      test_cmd,
      CommandCompleter::none(),
      Signature::DEFAULT,
    );

    assert_eq!(cmd.name, "test");
    assert_eq!(cmd.aliases, &["t"]);
    assert_eq!(cmd.doc, "Test command");
  }

  #[test]
  fn test_args_parsing_basic() {
    use crate::core::command_line::{Args, Signature};

    // Test parsing simple positional arguments
    let sig = Signature {
      positionals: (1, Some(3)),
      ..Signature::DEFAULT
    };

    let args = Args::parse("arg1 arg2 arg3", sig, true, |token| {
      Ok(token.content)
    }).unwrap();

    assert_eq!(args.len(), 3);
    assert_eq!(&args[0], "arg1");
    assert_eq!(&args[1], "arg2");
    assert_eq!(&args[2], "arg3");
  }

  #[test]
  fn test_args_parsing_quoted() {
    use crate::core::command_line::{Args, Signature};

    // Test parsing quoted arguments
    let sig = Signature::DEFAULT;

    let args = Args::parse(r#""quoted arg" 'another one' normal"#, sig, true, |token| {
      Ok(token.content)
    }).unwrap();

    assert_eq!(args.len(), 3);
    assert_eq!(&args[0], "quoted arg");
    assert_eq!(&args[1], "another one");
    assert_eq!(&args[2], "normal");
  }

  #[test]
  fn test_args_parsing_flags() {
    use crate::core::command_line::{Args, Flag, Signature};

    const FLAGS: &[Flag] = &[
      Flag {
        name: "force",
        alias: Some('f'),
        doc: "Force operation",
        completions: None,
      },
      Flag {
        name: "verbose",
        alias: Some('v'),
        doc: "Verbose output",
        completions: None,
      },
    ];

    let sig = Signature {
      positionals: (0, None),
      flags: FLAGS,
      ..Signature::DEFAULT
    };

    let args = Args::parse("--force arg1 -v arg2", sig, true, |token| {
      Ok(token.content)
    }).unwrap();

    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "arg1");
    assert_eq!(&args[1], "arg2");
    assert!(args.has_flag("force"));
    assert!(args.has_flag("verbose"));
  }

  #[test]
  fn test_args_parsing_flag_with_argument() {
    use crate::core::command_line::{Args, Flag, Signature};

    const FLAGS: &[Flag] = &[
      Flag {
        name: "output",
        alias: Some('o'),
        doc: "Output file",
        completions: Some(&["file.txt", "output.txt"]),
      },
    ];

    let sig = Signature {
      positionals: (0, None),
      flags: FLAGS,
      ..Signature::DEFAULT
    };

    let args = Args::parse("--output file.txt input.txt", sig, true, |token| {
      Ok(token.content)
    }).unwrap();

    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "input.txt");
    assert_eq!(args.get_flag("output"), Some("file.txt"));
  }

  #[test]
  fn test_command_documentation_generation() {
    use crate::core::command_line::{Flag, Signature};

    fn test_cmd(_cx: &mut Context, _args: Args, _event: PromptEvent) -> Result<()> {
      Ok(())
    }

    const FLAGS: &[Flag] = &[
      Flag {
        name: "force",
        alias: Some('f'),
        doc: "Force the operation",
        completions: None,
      },
    ];

    let cmd = TypableCommand::new(
      "write",
      &["w"],
      "Write buffer to file",
      test_cmd,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(1)),
        flags: FLAGS,
        ..Signature::DEFAULT
      },
    );

    let doc = cmd.generate_doc();

    // Check that doc contains command name, doc string, aliases, and flags
    assert!(doc.contains(":write"));
    assert!(doc.contains("Write buffer to file"));
    assert!(doc.contains("Aliases: w"));
    assert!(doc.contains("--force"));
    assert!(doc.contains("Force the operation"));
  }

  #[test]
  fn test_args_wrong_positional_count() {
    use crate::core::command_line::{Args, Signature};

    // Require exactly 1 positional argument
    let sig = Signature {
      positionals: (1, Some(1)),
      ..Signature::DEFAULT
    };

    // Try to parse with no arguments - should fail in validation mode
    let result = Args::parse("", sig, true, |token| {
      Ok(token.content)
    });

    assert!(result.is_err());

    // Try to parse with too many arguments - should fail
    let result = Args::parse("arg1 arg2", sig, true, |token| {
      Ok(token.content)
    });

    assert!(result.is_err());
  }

  #[test]
  fn test_args_completion_state() {
    use crate::core::command_line::{Args, CompletionState, Flag, Signature};

    const FLAGS: &[Flag] = &[
      Flag {
        name: "output",
        alias: Some('o'),
        doc: "Output file",
        completions: Some(&["file.txt"]),
      },
    ];

    let sig = Signature {
      positionals: (0, None),
      flags: FLAGS,
      ..Signature::DEFAULT
    };

    // Test completion state when typing a positional
    let args = Args::new(sig, false);
    match args.completion_state() {
      CompletionState::Positional => {},
      _ => panic!("Expected Positional state"),
    }
  }
}
