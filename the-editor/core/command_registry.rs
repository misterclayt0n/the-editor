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
use crate::{
  doc,
  editor::{
    Action,
    Editor,
  },
  ui::components::prompt::{
    Completion,
    PromptEvent,
  },
};

/// Type alias for a command function that takes a context, parsed arguments,
/// and prompt event
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
        (min, Some(max)) if min == max => {
          writeln!(doc, "{} argument{}", min, if min == 1 { "" } else { "s" }).unwrap()
        },
        (min, Some(max)) => writeln!(doc, "{}-{} arguments", min, max).unwrap(),
        (min, None) => writeln!(doc, "{} or more arguments", min).unwrap(),
      }
    }

    // Flags
    if !self.signature.flags.is_empty() {
      writeln!(doc, "Flags:").unwrap();

      // Calculate max flag name length for alignment
      let max_flag_len = self
        .signature
        .flags
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
  /// The args_line is parsed according to the command's signature with variable
  /// expansion
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
        let args = Args::parse(
          args_line,
          command.signature,
          event == PromptEvent::Validate,
          |token| {
            expansion::expand(cx.editor, token)
              .map_err(|e| Box::from(e.to_string()) as Box<dyn std::error::Error>)
          },
        )
        .map_err(|e| anyhow!("{}", e))?;

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
                (
                  token.content.as_ref(),
                  first_word.len() + 1 + token.content_start,
                )
              },
              _ => ("", input.len()),
            };

            // Run completer and adjust ranges
            let mut completions = completer(editor, arg_input);
            for completion in &mut completions {
              // Adjust the range by adding the offset
              // The completer returns ranges relative to arg_input,
              // we need to make them relative to the full input
              let relative_start = completion.range.start;
              completion.range = (arg_start_offset + relative_start)..;
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
            cmd
              .signature
              .flags
              .iter()
              .filter(|flag| flag.name.contains(flag_input))
              .map(|flag| {
                Completion {
                  range: flag_start_offset..,
                  text:  format!("--{}", flag.name),
                  doc:   Some(flag.doc.to_string()),
                }
              })
              .collect()
          },
          CompletionState::FlagArgument(flag) => {
            // Complete flag argument
            if let Some(completions) = flag.completions {
              let (arg_input, arg_start_offset) = match &last_token {
                Some(token) if !token.is_terminated => {
                  (
                    token.content.as_ref(),
                    first_word.len() + 1 + token.content_start,
                  )
                },
                _ => ("", input.len()),
              };

              completions
                .iter()
                .filter(|val| val.contains(arg_input))
                .map(|val| {
                  Completion {
                    range: arg_start_offset..,
                    text:  val.to_string(),
                    doc:   None,
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
      CommandCompleter::all(completers::filename_with_current_dir),
      Signature {
        positionals: (1, None),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "files",
      &[],
      "Open file manager buffer for the current directory",
      files_command,
      CommandCompleter::all(completers::filename_with_current_dir),
      Signature {
        positionals: (0, Some(1)),
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

    self.register(TypableCommand::new(
      "sh",
      &["shell", "shell-command"],
      "Run shell command asynchronously and stream output to the compilation buffer",
      crate::core::commands::cmd_shell_spawn,
      CommandCompleter::none(),
      Signature {
        positionals: (0, None),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "write!",
      &["w!"],
      "Force write buffer to file (creates directories)",
      force_write,
      CommandCompleter::all(completers::filename),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "buffer-close!",
      &["bc!", "bclose!"],
      "Force close the current buffer without saving",
      force_buffer_close,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "quit-all",
      &["qa", "qall"],
      "Close all views and quit",
      quit_all,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "quit-all!",
      &["qa!", "qall!"],
      "Force close all views and quit without saving",
      force_quit_all,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "vsplit",
      &["vs", "vsp"],
      "Open file in vertical split (current buffer if no file)",
      vsplit,
      CommandCompleter::all(completers::filename),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "hsplit",
      &["hs", "sp", "split"],
      "Open file in horizontal split (current buffer if no file)",
      hsplit,
      CommandCompleter::all(completers::filename),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "write-all",
      &["wa", "wall"],
      "Write all modified buffers to disk",
      write_all,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "write-all-quit",
      &["wqa", "wqall", "xa", "xall"],
      "Write all modified buffers and quit",
      write_all_quit,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "goto",
      &["g"],
      "Go to line number (with preview)",
      goto_line_number,
      CommandCompleter::none(),
      Signature {
        positionals: (1, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "lsp-restart",
      &[],
      "Restart language server(s) for current document",
      lsp_restart,
      CommandCompleter::none(),
      Signature {
        positionals: (0, None),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "lsp-stop",
      &[],
      "Stop language server(s) for current document",
      lsp_stop,
      CommandCompleter::none(),
      Signature {
        positionals: (0, None),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "noop",
      &[],
      "Toggle visual effects mode for inserts/deletes",
      noop,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    // ACP (Agent Client Protocol) commands
    self.register(TypableCommand::new(
      "acp-new-session",
      &[],
      "Create a new ACP session with Claude Code",
      crate::acp::commands::cmd_acp_new_session,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "acp-send-prompt",
      &[],
      "Send selected text as prompt to active ACP session",
      crate::acp::commands::cmd_acp_send_prompt,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "acp-close-session",
      &[],
      "Close the current ACP session",
      crate::acp::commands::cmd_acp_close_session,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    // Context fade commands
    self.register(TypableCommand::new(
      "fade",
      &[],
      "Toggle fade mode to highlight code context",
      |cx, _args, _event| {
        crate::core::commands::toggle_fade_mode(cx);
        Ok(())
      },
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "change-current-directory",
      &["cd"],
      "Change the current working directory",
      change_current_directory,
      CommandCompleter::positional(&[completers::directory], completers::none),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "show-directory",
      &["pwd"],
      "Show the current working directory",
      show_current_directory,
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

  /// Filename completer with enhanced path handling (Helix-inspired)
  pub fn filename(_editor: &Editor, input: &str) -> Vec<Completion> {
    filename_impl(_editor, input, true)
  }

  /// Filename completer that inserts current file's directory on empty input
  pub fn filename_with_current_dir(editor: &Editor, input: &str) -> Vec<Completion> {
    use crate::current_ref;

    // If input is empty, suggest the current file's directory
    if input.is_empty() {
      let (_view, doc) = current_ref!(editor);
      if let Some(path) = doc.path() {
        if let Some(parent) = path.parent() {
          let dir_path = parent.to_string_lossy().to_string() + "/";
          return vec![Completion {
            range: 0..,
            text:  dir_path,
            doc:   Some("Current file's directory".to_string()),
          }];
        }
      }
      // Fall through to normal completion if no current file
    }

    // Otherwise, use normal filename completion
    filename_impl(editor, input, true)
  }

  /// Filename completer implementation with optional gitignore support
  fn filename_impl(_editor: &Editor, input: &str, git_ignore: bool) -> Vec<Completion> {
    use std::{
      borrow::Cow,
      path::Path,
    };

    use ignore::WalkBuilder;
    use the_editor_stdx::path::expand_tilde;

    // Expand tilde if present
    let is_tilde = input == "~";
    let path = expand_tilde(Path::new(input));

    // Split path into directory and file_name components
    let (dir, file_name) = if input.ends_with(std::path::MAIN_SEPARATOR) {
      (path, None)
    } else {
      // Handle special case for "." and "/."
      let is_period = (input.ends_with(format!("{}.", std::path::MAIN_SEPARATOR).as_str())
        && input.len() > 2)
        || input == ".";
      let file_name = if is_period {
        Some(String::from("."))
      } else {
        path
          .file_name()
          .and_then(|file| file.to_str().map(|path| path.to_owned()))
      };

      let path = if is_period {
        path
      } else {
        match path.parent() {
          Some(path) if !path.as_os_str().is_empty() => Cow::Borrowed(path),
          // Path::new("h")'s parent is Some("")...
          _ => Cow::Owned(the_editor_stdx::env::current_working_dir()),
        }
      };

      (path, file_name)
    };

    // Range for replacement
    // When input ends with /, we want to append to it (not replace from beginning)
    let range_for_no_prefix = input.len()..;

    // Use WalkBuilder for gitignore-aware traversal
    let entries = WalkBuilder::new(&*dir)
      .hidden(false) // Show hidden files
      .follow_links(false) // Don't follow symlinks
      .git_ignore(git_ignore)
      .max_depth(Some(1)) // Only scan immediate directory
      .build()
      .filter_map(|entry| {
        let entry = entry.ok()?;
        let entry_path = entry.path();

        // Skip the directory itself
        if entry_path == Path::new(&*dir) {
          return None;
        }

        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());

        // Get relative path from dir
        let mut path = if is_tilde {
          // If it's a single tilde, show absolute path so Tab expansion works
          entry_path.to_path_buf()
        } else {
          entry_path.strip_prefix(&*dir).unwrap_or(entry_path).to_path_buf()
        };

        // Add trailing slash for directories
        if is_dir {
          path.push("");
        }

        let path_str = path.into_os_string().into_string().ok()?;
        Some((path_str, is_dir))
      })
      .filter(|(path, _)| !path.is_empty());

    // If we have a file_name prefix, filter and fuzzy match
    let completions: Vec<Completion> = if let Some(file_name) = file_name {
      let file_name_lower = file_name.to_lowercase();
      let range_start = input.len().saturating_sub(file_name.len());
      let replace_range = range_start..;

      entries
        .filter(|(path, _)| {
          // Fuzzy match: check if file name contains the prefix
          path.to_lowercase().contains(&file_name_lower)
        })
        .map(|(path, _is_dir)| {
          Completion {
            range: replace_range.clone(),
            text:  path,
            doc:   None,
          }
        })
        .collect()
    } else {
      // No prefix - return all entries (append to current path)
      entries
        .map(|(path, _is_dir)| {
          Completion {
            range: range_for_no_prefix.clone(),
            text:  path,
            doc:   None,
          }
        })
        .collect()
    };

    // Sort: directories first, then alphabetically
    let mut completions = completions;
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
    use crate::core::theme;

    // Search all theme directories in priority order:
    // 1. User config directory (~/.config/the-editor/themes)
    // 2. Runtime directories (built-in themes)
    let mut theme_dirs = vec![the_editor_loader::config_dir()];
    theme_dirs.extend(the_editor_loader::runtime_dirs().iter().cloned());

    let mut theme_names: Vec<String> = Vec::new();
    for dir in &theme_dirs {
      let theme_dir = dir.join("themes");
      let names = theme::Loader::read_names(&theme_dir);
      theme_names.extend(names);
    }

    // Add default themes
    theme_names.push("default".to_string());
    theme_names.push("base16_default".to_string());

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

  /// Directory completer - only shows directories (not files)
  pub fn directory(_editor: &Editor, input: &str) -> Vec<Completion> {
    use std::{
      borrow::Cow,
      path::Path,
    };

    use ignore::WalkBuilder;
    use the_editor_stdx::path::expand_tilde;

    // Expand tilde if present
    let is_tilde = input == "~";
    let path = expand_tilde(Path::new(input));

    // Split path into directory and file_name components
    let (dir, file_name) = if input.ends_with(std::path::MAIN_SEPARATOR) {
      (path, None)
    } else {
      let is_period = (input.ends_with(format!("{}.", std::path::MAIN_SEPARATOR).as_str())
        && input.len() > 2)
        || input == ".";
      let file_name = if is_period {
        Some(String::from("."))
      } else {
        path
          .file_name()
          .and_then(|file| file.to_str().map(|path| path.to_owned()))
      };

      let path = if is_period {
        path
      } else {
        match path.parent() {
          Some(path) if !path.as_os_str().is_empty() => Cow::Borrowed(path),
          _ => Cow::Owned(the_editor_stdx::env::current_working_dir()),
        }
      };

      (path, file_name)
    };

    // Range for replacement
    let range_for_no_prefix = input.len()..;

    // Use WalkBuilder for gitignore-aware traversal
    let entries = WalkBuilder::new(&*dir)
      .hidden(false)
      .follow_links(false)
      .git_ignore(false)
      .max_depth(Some(1))
      .build()
      .filter_map(|entry| {
        let entry = entry.ok()?;
        let entry_path = entry.path();

        // Skip the directory itself
        if entry_path == Path::new(&*dir) {
          return None;
        }

        // Only include directories
        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
        if !is_dir {
          return None;
        }

        // Get relative path from dir
        let mut path = if is_tilde {
          entry_path.to_path_buf()
        } else {
          entry_path.strip_prefix(&*dir).unwrap_or(entry_path).to_path_buf()
        };

        // Add trailing slash for directories
        path.push("");

        let path_str = path.into_os_string().into_string().ok()?;
        Some(path_str)
      })
      .filter(|path| !path.is_empty());

    // If we have a file_name prefix, filter and fuzzy match
    let completions: Vec<Completion> = if let Some(file_name) = file_name {
      let file_name_lower = file_name.to_lowercase();
      let range_start = input.len().saturating_sub(file_name.len());
      let replace_range = range_start..;

      entries
        .filter(|path| path.to_lowercase().contains(&file_name_lower))
        .map(|path| {
          Completion {
            range: replace_range.clone(),
            text:  path,
            doc:   None,
          }
        })
        .collect()
    } else {
      // No prefix - return all entries (append to current path)
      entries
        .map(|path| {
          Completion {
            range: range_for_no_prefix.clone(),
            text:  path,
            doc:   None,
          }
        })
        .collect()
    };

    // Sort alphabetically
    let mut completions = completions;
    completions.sort_by(|a, b| a.text.cmp(&b.text));

    completions
  }

  #[cfg(test)]
  mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    // Test wrapper for filename_impl that doesn't require an Editor
    fn filename_impl_for_test(input: &str, git_ignore: bool) -> Vec<Completion> {
      use std::{
        borrow::Cow,
        path::Path,
      };

      use ignore::WalkBuilder;
      use the_editor_stdx::path::expand_tilde;

      // Copied logic from filename_impl
      let is_tilde = input == "~";
      let path = expand_tilde(Path::new(input));

      let (dir, file_name) = if input.ends_with(std::path::MAIN_SEPARATOR) {
        (path, None)
      } else {
        let is_period = (input.ends_with(format!("{}.", std::path::MAIN_SEPARATOR).as_str())
          && input.len() > 2)
          || input == ".";
        let file_name = if is_period {
          Some(String::from("."))
        } else {
          path
            .file_name()
            .and_then(|file| file.to_str().map(|path| path.to_owned()))
        };

        let path = if is_period {
          path
        } else {
          match path.parent() {
            Some(path) if !path.as_os_str().is_empty() => Cow::Borrowed(path),
            _ => Cow::Owned(the_editor_stdx::env::current_working_dir()),
          }
        };

        (path, file_name)
      };

      let range_for_no_prefix = input.len()..;

      let entries = WalkBuilder::new(&*dir)
        .hidden(false)
        .follow_links(false)
        .git_ignore(git_ignore)
        .max_depth(Some(1))
        .build()
        .filter_map(|entry| {
          let entry = entry.ok()?;
          let entry_path = entry.path();

          if entry_path == Path::new(&*dir) {
            return None;
          }

          let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());

          let mut path = if is_tilde {
            entry_path.to_path_buf()
          } else {
            entry_path
              .strip_prefix(&*dir)
              .unwrap_or(entry_path)
              .to_path_buf()
          };

          if is_dir {
            path.push("");
          }

          let path_str = path.into_os_string().into_string().ok()?;
          Some((path_str, is_dir))
        })
        .filter(|(path, _)| !path.is_empty());

      let completions: Vec<Completion> = if let Some(file_name) = file_name {
        let file_name_lower = file_name.to_lowercase();
        let range_start = input.len().saturating_sub(file_name.len());
        let replace_range = range_start..;

        entries
          .filter(|(path, _)| path.to_lowercase().contains(&file_name_lower))
          .map(|(path, _is_dir)| {
            Completion {
              range: replace_range.clone(),
              text:  path,
              doc:   None,
            }
          })
          .collect()
      } else {
        entries
          .map(|(path, _is_dir)| {
            Completion {
              range: range_for_no_prefix.clone(),
              text:  path,
              doc:   None,
            }
          })
          .collect()
      };

      let mut completions = completions;
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

    /// Create a temporary directory structure for testing
    fn create_test_dir_structure() -> TempDir {
      let temp = TempDir::new().unwrap();
      let base = temp.path();

      // Create directory structure:
      // temp/
      //   file1.txt
      //   file2.rs
      //   dir1/
      //     nested.txt
      //   dir2/
      //     another.rs
      fs::write(base.join("file1.txt"), "test").unwrap();
      fs::write(base.join("file2.rs"), "test").unwrap();
      fs::create_dir(base.join("dir1")).unwrap();
      fs::write(base.join("dir1/nested.txt"), "test").unwrap();
      fs::create_dir(base.join("dir2")).unwrap();
      fs::write(base.join("dir2/another.rs"), "test").unwrap();

      temp
    }

    // Note: These tests use a minimal mock since filename_impl doesn't actually use
    // the editor parameter
    struct MockEditor;

    #[test]
    fn test_filename_completer_lists_directory_contents() {
      let temp = create_test_dir_structure();

      // Test listing directory contents
      // Note: filename_impl takes &Editor but doesn't use it, so we create a mock
      use ignore::WalkBuilder;
      let entries: Vec<_> = WalkBuilder::new(temp.path())
        .hidden(false)
        .max_depth(Some(1))
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.path() != temp.path())
        .collect();

      // Should have at least 4 entries (2 files + 2 dirs)
      assert!(entries.len() >= 4);
    }

    #[test]
    fn test_filename_completer_path_parsing() {
      use std::path::Path;

      // Test path parsing logic
      let input = "/tmp/test/";
      let _path = the_editor_stdx::path::expand_tilde(Path::new(input));

      // Path should end with separator
      assert!(input.ends_with(std::path::MAIN_SEPARATOR));

      // Test non-separator ending
      let input2 = "/tmp/test";
      assert!(!input2.ends_with(std::path::MAIN_SEPARATOR));
    }

    #[test]
    fn test_completion_range_for_partial_path() {
      // Test that range calculation works for partial paths
      let input = "/tmp/test/fil";
      let file_name = "file.txt";

      // Range should start from where the partial name begins
      let range_start = input.len().saturating_sub(file_name.len());
      assert!(range_start <= input.len());

      // For partial match "fil", range should capture it
      let partial = "fil";
      let partial_start = input.len().saturating_sub(partial.len());
      assert_eq!(&input[partial_start..], partial);
    }

    #[test]
    fn test_directory_sorting_logic() {
      // Test that our sorting logic puts directories before files
      let mut items = vec![
        ("file1.txt", false),
        ("dir1/", true),
        ("file2.rs", false),
        ("dir2/", true),
      ];

      items.sort_by(|a, b| {
        match (a.1, b.1) {
          (true, false) => std::cmp::Ordering::Less,
          (false, true) => std::cmp::Ordering::Greater,
          _ => a.0.cmp(b.0),
        }
      });

      // First two should be directories
      assert!(items[0].1); // dir1/
      assert!(items[1].1); // dir2/
      // Last two should be files
      assert!(!items[2].1); // file1.txt
      assert!(!items[3].1); // file2.rs
    }

    #[test]
    fn test_directory_path_composition() {
      // This test verifies that completing directories composes paths correctly
      // e.g., "runtime/" + "queries" should become "runtime/queries"
      let temp = create_test_dir_structure();

      // Add a nested directory structure: dir1/subdir/
      fs::create_dir(temp.path().join("dir1/subdir")).unwrap();
      fs::write(temp.path().join("dir1/subdir/nested.txt"), "test").unwrap();

      // Test 1: Get completions for temp directory
      let path_str = temp.path().to_str().unwrap();
      let input = format!("{}/", path_str);

      // Note: We'll call filename_impl_for_test which doesn't need an editor
      let completions = filename_impl_for_test(&input, false);

      // Should have dir1/ in completions
      let dir1_completion = completions
        .iter()
        .find(|c| c.text == "dir1/")
        .expect("dir1/ not found");

      println!("Input: {}", input);
      println!("dir1/ completion range: {:?}", dir1_completion.range);
      println!("dir1/ completion text: {}", dir1_completion.text);

      // When we apply this completion, it should append to the input
      // Range should be from input.len() onwards (to append)
      assert_eq!(
        dir1_completion.range.start,
        input.len(),
        "Range should start at end of input to append, not replace"
      );

      // Test 2: Now simulate being inside dir1/
      let input2 = format!("{}/dir1/", path_str);
      let completions2 = filename_impl_for_test(&input2, false);

      // Should have subdir/ in completions
      let subdir_completion = completions2
        .iter()
        .find(|c| c.text == "subdir/")
        .expect("subdir/ not found");

      println!("\nInput: {}", input2);
      println!("subdir/ completion range: {:?}", subdir_completion.range);
      println!("subdir/ completion text: {}", subdir_completion.text);

      // When we apply this completion, it should append to dir1/
      assert_eq!(
        subdir_completion.range.start,
        input2.len(),
        "Range should start at end of input2 to append subdir/ to dir1/"
      );

      // Simulate applying the completion
      let mut result = input2.clone();
      result.replace_range(subdir_completion.range.clone(), &subdir_completion.text);

      println!("Result after applying completion: {}", result);
      println!("Expected: {}/dir1/subdir/", path_str);

      // The result should be the full path: temp/dir1/subdir/
      assert!(
        result.ends_with("/dir1/subdir/"),
        "Result should be 'dir1/subdir/' appended to parent path, got: {}",
        result
      );
    }
  }
}

// Built-in command implementations

fn quit(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  cx.block_try_flush_writes()?;

  if cx.editor.documents.values().any(|doc| doc.is_modified()) {
    cx.editor
      .set_error("unsaved changes, use :q! to force quit".to_string());
    return Ok(());
  }
  std::process::exit(0);
}

fn force_quit(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  cx.block_try_flush_writes()?;

  std::process::exit(0);
}

fn handle_file_manager_save(cx: &mut Context) -> Result<()> {
  let (_view, doc) = crate::current!(cx.editor);

  // Get current directory from file manager metadata
  let current_path = crate::file_manager::buffer::current_path(doc);
  if current_path.is_none() {
    cx.editor.set_error("File manager: no current directory".to_string());
    return Ok(());
  }
  let directory = current_path.unwrap();

  // Compute operations
  let operations = match crate::file_manager::compute_operations(doc) {
    Ok(ops) => ops,
    Err(err) => {
      log::error!("File manager: failed to compute operations: {err}");
      cx.editor
        .set_status("File manager: failed to compute operations (see logs)".to_string());
      return Ok(());
    },
  };

  if operations.is_empty() {
    cx.editor.set_status("File manager: no changes to apply".to_string());
    return Ok(());
  }

  // Format summary
  let summary = crate::file_manager::format_operations_summary(&operations);

  // Show confirmation prompt
  cx.editor.set_status(format!("File manager operations:\n{}\nExecute? (y/n)", summary));

  // For now, we'll just show the summary and not execute
  // In a full implementation, we'd need to add a confirmation dialog component
  // TODO: Add proper confirmation dialog with callbacks

  // Execute operations immediately for now (skip confirmation)
  match crate::file_manager::execute_operations(&directory, &operations) {
    Ok(results) => {
      let mut errors = Vec::new();
      for (op, result) in results {
        if let Err(err) = result {
          errors.push(format!("{}: {}", op.description(), err));
        }
      }

      if errors.is_empty() {
        cx.editor.set_status(format!("File manager: {} operations completed", operations.len()));
        // Refresh the buffer
        let (view, doc) = crate::current!(cx.editor);
        let view_id = view.id;
        if let Err(err) = crate::file_manager::refresh_buffer(doc, view_id) {
          log::error!("File manager: failed to refresh buffer: {err}");
          cx
            .editor
            .set_status("File manager: failed to refresh buffer (see logs)".to_string());
        }
      } else {
        for error in &errors {
          log::error!("File manager operation error: {error}");
        }
        cx
          .editor
          .set_status("File manager: some operations failed (see logs)".to_string());
      }
    },
    Err(err) => {
      log::error!("File manager: failed to execute operations: {err}");
      cx
        .editor
        .set_status("File manager: failed to execute operations (see logs)".to_string());
    },
  }

  Ok(())
}

fn write_buffer(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  use std::path::PathBuf;

  use crate::current;

  let (view, doc) = current!(cx.editor);
  let doc_id = view.doc;

  // Special handling for file manager buffers
  if doc.is_file_manager_buffer() {
    return handle_file_manager_save(cx);
  }

  let path = if args.is_empty() {
    doc.path().map(|p| p.to_path_buf())
  } else {
    Some(PathBuf::from(&args[0]))
  };

  if let Some(path) = path {
    match cx.editor.save(doc_id, Some(path.clone()), false) {
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

fn force_write(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
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
  cx.block_try_flush_writes()?;

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
  match cx.editor.open(&path, crate::editor::Action::Replace) {
    Ok(_) => {
      cx.editor.set_status(format!("opened: {}", path.display()));
    },
    Err(err) => {
      cx.editor.set_error(format!("failed to open: {}", err));
    },
  }

  Ok(())
}

fn files_command(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  use crate::{
    core::special_buffer::SpecialBufferKind,
    editor::Action,
    file_manager,
  };

  // Determine the directory to open
  let path = if args.is_empty() {
    // Use current file's directory or cwd
    let (_, doc) = crate::current!(cx.editor);
    doc
      .path()
      .and_then(|p| p.parent())
      .map(|p| p.to_path_buf())
      .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
  } else {
    std::path::PathBuf::from(&args[0])
  };

  // Get the current view ID
  let view_id = cx.editor.tree.focus;

  // Check if there's already a file manager buffer open
  if let Some(doc_id) = cx.editor.special_buffers.last_for(SpecialBufferKind::FileManager) {
    // Navigate to the new path in the existing buffer
    if let Some(doc) = cx.editor.documents.get_mut(&doc_id) {
      match file_manager::navigate_to(doc, path.clone(), view_id) {
        Ok(_) => {
          // Focus the existing file manager buffer
          cx.editor.switch(doc_id, Action::Replace);
          cx
            .editor
            .set_status(format!("file manager: {}", path.display()));
          return Ok(());
        },
        Err(err) => {
          // Failed to navigate, create new buffer
          log::warn!("Failed to navigate file manager: {}", err);
        },
      }
    }
  }

  // Create new file manager buffer
  let doc_id = cx.editor.new_file(Action::Replace);

  // Set up the file manager buffer
  if let Some(doc) = cx.editor.documents.get_mut(&doc_id) {
    match file_manager::refresh_to_path(doc, path.clone(), false, view_id) {
      Ok(_) => {
        cx.editor.mark_special_buffer(doc_id, SpecialBufferKind::FileManager);
        cx
          .editor
          .set_status(format!("file manager: {}", path.display()));
      },
      Err(err) => {
        cx
          .editor
          .set_error(format!("failed to open file manager: {}", err));
        // Close the buffer if initialization failed
        let _ = cx.editor.close_document(doc_id, true);
      },
    }
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
      cx.editor
        .set_error(format!("unknown command: {}", &args[0]));
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
      cx.editor.unset_theme_preview();
      return Ok(());
    },
    PromptEvent::Update => {
      // Preview theme while typing
      if args.is_empty() {
        cx.editor.unset_theme_preview();
        return Ok(());
      }

      let theme_name = &args[0];
      if let Ok(theme) = cx.editor.theme_loader.load(theme_name) {
        if true_color || theme.is_16_color() {
          cx.editor.set_theme_preview(theme);
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

fn force_buffer_close(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  use crate::current;

  let (view, _doc) = current!(cx.editor);
  let doc_id = view.doc;

  // Force close by ignoring unsaved changes
  match cx.editor.close_document(doc_id, true) {
    Ok(_) => {},
    Err(_) => {
      // Ignore close errors when forcing
    },
  }

  Ok(())
}

fn quit_all(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  cx.block_try_flush_writes()?;

  // Check for unsaved buffers
  buffers_remaining_impl(cx.editor)?;

  // Close all views
  let view_ids: Vec<_> = cx.editor.tree.views().map(|(view, _)| view.id).collect();
  for view_id in view_ids {
    cx.editor.close(view_id);
  }

  // Exit the application
  std::process::exit(0);
}

fn force_quit_all(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  cx.block_try_flush_writes()?;

  // Close all views without checking for unsaved buffers
  let view_ids: Vec<_> = cx.editor.tree.views().map(|(view, _)| view.id).collect();
  for view_id in view_ids {
    cx.editor.close(view_id);
  }

  // Exit the application
  std::process::exit(0);
}

fn vsplit(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  use crate::editor::Action;

  if event != PromptEvent::Validate {
    return Ok(());
  }

  if args.is_empty() {
    // Split with current buffer
    use crate::current;
    let (view, _doc) = current!(cx.editor);
    let doc_id = view.doc;
    cx.editor.switch(doc_id, Action::VerticalSplit);
  } else {
    // Open file in split
    open_impl(cx, args, Action::VerticalSplit)?;
  }

  Ok(())
}

fn hsplit(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  use crate::editor::Action;

  if event != PromptEvent::Validate {
    return Ok(());
  }

  if args.is_empty() {
    // Split with current buffer
    use crate::current;
    let (view, _doc) = current!(cx.editor);
    let doc_id = view.doc;
    cx.editor.switch(doc_id, Action::HorizontalSplit);
  } else {
    // Open file in split
    open_impl(cx, args, Action::HorizontalSplit)?;
  }

  Ok(())
}

fn write_all(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  let doc_ids: Vec<_> = cx
    .editor
    .documents()
    .filter(|doc| doc.is_modified() && doc.path().is_some())
    .map(|doc| doc.id())
    .collect();

  let mut errors = Vec::new();
  for doc_id in doc_ids {
    if let Err(err) = cx.editor.save::<std::path::PathBuf>(doc_id, None, false) {
      errors.push(format!("{}", err));
    }
  }

  if errors.is_empty() {
    cx.editor.set_status("All buffers written".to_string());
  } else {
    cx.editor.set_error(format!(
      "Failed to save some buffers: {}",
      errors.join(", ")
    ));
  }

  Ok(())
}

fn write_all_quit(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  // Write all and then quit all
  let doc_ids: Vec<_> = cx
    .editor
    .documents()
    .filter(|doc| doc.is_modified() && doc.path().is_some())
    .map(|doc| doc.id())
    .collect();

  let mut errors = Vec::new();
  for doc_id in doc_ids {
    if let Err(err) = cx.editor.save::<std::path::PathBuf>(doc_id, None, false) {
      errors.push(format!("{}", err));
    }
  }

  if !errors.is_empty() {
    cx.editor.set_error(format!(
      "Failed to save some buffers: {}",
      errors.join(", ")
    ));
    return Ok(());
  }

  cx.block_try_flush_writes()?;

  // Check for unsaved buffers
  buffers_remaining_impl(cx.editor)?;

  // Close all views
  let view_ids: Vec<_> = cx.editor.tree.views().map(|(view, _)| view.id).collect();
  for view_id in view_ids {
    cx.editor.close(view_id);
  }

  // Exit the application
  std::process::exit(0);
}

fn lsp_restart(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  use crate::doc;

  // Get editor configuration first
  let editor_config = cx.editor.config();
  let root_dirs = &editor_config.workspace_lsp_roots;
  let enable_snippets = editor_config.lsp.snippets;

  // Get document and language config (immutable borrows only)
  let current_doc = doc!(cx.editor);
  let config = current_doc
    .language_config()
    .ok_or_else(|| anyhow!("LSP not defined for the current document"))?;
  let doc_path = current_doc.path();

  // Get the list of language servers configured for this document
  let language_servers: Vec<_> = config
    .language_servers
    .iter()
    .map(|ls| ls.name.as_str())
    .collect();

  // If args provided, use those; otherwise restart all servers
  let language_servers = if args.is_empty() {
    language_servers
  } else {
    let (valid, invalid): (Vec<_>, Vec<_>) = args
      .iter()
      .map(|arg| arg.as_ref())
      .partition(|name| language_servers.contains(name));
    if !invalid.is_empty() {
      let s = if invalid.len() == 1 { "" } else { "s" };
      bail!("Unknown language server{}: {}", s, invalid.join(", "));
    }
    valid
  };

  // Restart each language server
  let mut errors = Vec::new();
  for server in language_servers.iter() {
    match cx
      .editor
      .language_servers
      .restart_server(server, config, doc_path, root_dirs, enable_snippets)
      .transpose()
    {
      Err(err) => errors.push(err.to_string()),
      _ => {},
    }
  }

  // Collect document IDs that need language server refresh
  let language_servers_to_match = language_servers
    .iter()
    .map(|s| s.to_string())
    .collect::<Vec<_>>();
  let document_ids_to_refresh: Vec<_> = cx
    .editor
    .documents()
    .filter_map(|doc| {
      match doc.language_config() {
        Some(doc_config)
          if doc_config
            .language_servers
            .iter()
            .any(|ls| language_servers_to_match.contains(&ls.name.to_string())) =>
        {
          Some(doc.id())
        },
        _ => None,
      }
    })
    .collect();

  // Refresh language servers for affected documents
  for document_id in document_ids_to_refresh {
    cx.editor.refresh_language_servers(document_id);
  }

  if errors.is_empty() {
    cx.editor
      .set_status("Language server(s) restarted".to_string());
    Ok(())
  } else {
    Err(anyhow!(
      "Error restarting language servers: {}",
      errors.join(", ")
    ))
  }
}

fn lsp_stop(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  use crate::current;

  let (_view, doc) = current!(cx.editor);

  // Get the list of language servers running for this document
  let language_servers: Vec<_> = doc
    .language_servers()
    .map(|ls| ls.name().to_string())
    .collect();

  // If args provided, use those; otherwise stop all servers
  let language_servers = if args.is_empty() {
    language_servers
  } else {
    let (valid, invalid): (Vec<_>, Vec<_>) = args
      .iter()
      .map(|arg| arg.to_string())
      .partition(|name| language_servers.contains(name));
    if !invalid.is_empty() {
      let s = if invalid.len() == 1 { "" } else { "s" };
      bail!("Unknown language server{}: {}", s, invalid.join(", "));
    }
    valid
  };

  // Stop each language server
  for ls_name in &language_servers {
    cx.editor.language_servers.stop(ls_name);

    // Remove from all documents and clear diagnostics
    let doc_ids: Vec<_> = cx.editor.documents().map(|d| d.id()).collect();
    for doc_id in doc_ids {
      let doc = cx.editor.documents.get_mut(&doc_id).unwrap();
      if let Some(client) = doc.remove_language_server_by_name(ls_name) {
        // Clear diagnostics for this language server
        doc.clear_diagnostics_for_language_server(client.id());
      }
    }
  }

  cx.editor
    .set_status("Language server(s) stopped".to_string());
  Ok(())
}

fn goto_line_number(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  use crate::current;

  match event {
    PromptEvent::Abort => {
      // Restore original selection
      if let Some(last_selection) = cx.editor.last_selection.take() {
        let (view, doc) = current!(cx.editor);
        doc.set_selection(view.id, last_selection);
      }
    },
    PromptEvent::Validate => {
      // Save to jumplist
      if let Some(last_selection) = cx.editor.last_selection.take() {
        let (view, doc) = current!(cx.editor);
        view.jumps.push((doc.id(), last_selection));
      }
    },
    PromptEvent::Update => {
      if args.is_empty() {
        // Restore original selection if no input
        if let Some(last_selection) = cx.editor.last_selection.as_ref() {
          let (view, doc) = current!(cx.editor);
          doc.set_selection(view.id, last_selection.clone());
        }
        return Ok(());
      }

      // Save original selection on first update
      if cx.editor.last_selection.is_none() {
        let (view, doc) = current!(cx.editor);
        cx.editor.last_selection = Some(doc.selection(view.id).clone());
      }

      // Parse line number and navigate
      if let Ok(line) = args[0].parse::<usize>() {
        use crate::core::selection::Selection;

        let (view, doc) = current!(cx.editor);
        let text = doc.text();

        // Convert to 0-indexed
        let line = line
          .saturating_sub(1)
          .min(text.len_lines().saturating_sub(1));

        // Get position at start of line - find the start character of the line
        let line_start = text.line_to_char(line);

        // Create selection at the line
        let selection = Selection::single(line_start, line_start);
        doc.set_selection(view.id, selection);

        // Ensure cursor in view
        crate::core::view::align_view(doc, view, crate::core::view::Align::Center);
      }
    },
  }

  Ok(())
}

/// Helper function to open a file with a specific action
fn open_impl(cx: &mut Context, args: Args, action: crate::editor::Action) -> Result<()> {
  use std::path::PathBuf;

  let path = PathBuf::from(&args[0]);

  // Expand tilde to home directory
  let path = if path.starts_with("~") {
    if let Ok(home) = std::env::var("HOME") {
      PathBuf::from(home).join(path.strip_prefix("~").unwrap())
    } else {
      path
    }
  } else {
    path
  };

  // Make path absolute if it's relative
  let path = if path.is_relative() {
    std::env::current_dir()?.join(path)
  } else {
    path
  };

  match cx.editor.open(&path, action) {
    Ok(doc_id) => {
      cx.editor.set_status(format!("opened: {}", path.display()));
      // Set focus to the new document
      let view_id = cx.editor.tree.focus;
      let view = cx.editor.tree.get_mut(view_id);
      view.doc = doc_id;
    },
    Err(err) => {
      cx.editor
        .set_error(format!("failed to open {}: {}", path.display(), err));
    },
  }

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

fn noop(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  // Toggle noop effect mode
  cx.editor.noop_effect_pending = !cx.editor.noop_effect_pending;

  if cx.editor.noop_effect_pending {
    cx.editor.set_status(
      "Noop effect mode enabled - all inserts/deletes will trigger visual effects".to_string(),
    );
  } else {
    cx.editor
      .set_status("Noop effect mode disabled".to_string());
  }

  Ok(())
}

fn change_current_directory(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  use std::path::PathBuf;

  use the_editor_stdx::path::expand_tilde;

  if event != PromptEvent::Validate {
    return Ok(());
  }

  let dir = match args.first().map(AsRef::as_ref) {
    Some("-") => {
      // Switch to previous working directory
      cx
        .editor
        .get_last_cwd()
        .map(|path| std::borrow::Cow::Owned(path.to_path_buf()))
        .ok_or_else(|| anyhow!("No previous working directory"))?
    },
    Some(path) => expand_tilde(std::path::Path::new(path)),
    None => {
      // No argument - go to home directory
      let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| anyhow!("Could not determine home directory"))?;
      std::borrow::Cow::Owned(PathBuf::from(home))
    },
  };

  cx.editor.set_cwd(&dir).map_err(|err| {
    anyhow!(
      "Could not change working directory to '{}': {}",
      dir.display(),
      err
    )
  })?;

  cx.editor.set_status(format!(
    "Current working directory is now {}",
    the_editor_stdx::env::current_working_dir().display()
  ));

  Ok(())
}

fn show_current_directory(cx: &mut Context, _args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }

  let cwd = the_editor_stdx::env::current_working_dir();
  let message = format!("Current working directory is {}", cwd.display());

  if cwd.exists() {
    cx.editor.set_status(message);
  } else {
    cx.editor.set_error(format!("{} (deleted)", message));
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
    use crate::core::command_line::{
      Args,
      Signature,
    };

    // Test parsing simple positional arguments
    let sig = Signature {
      positionals: (1, Some(3)),
      ..Signature::DEFAULT
    };

    let args = Args::parse("arg1 arg2 arg3", sig, true, |token| Ok(token.content)).unwrap();

    assert_eq!(args.len(), 3);
    assert_eq!(&args[0], "arg1");
    assert_eq!(&args[1], "arg2");
    assert_eq!(&args[2], "arg3");
  }

  #[test]
  fn test_args_parsing_quoted() {
    use crate::core::command_line::{
      Args,
      Signature,
    };

    // Test parsing quoted arguments
    let sig = Signature::DEFAULT;

    let args = Args::parse(r#""quoted arg" 'another one' normal"#, sig, true, |token| {
      Ok(token.content)
    })
    .unwrap();

    assert_eq!(args.len(), 3);
    assert_eq!(&args[0], "quoted arg");
    assert_eq!(&args[1], "another one");
    assert_eq!(&args[2], "normal");
  }

  #[test]
  fn test_args_parsing_flags() {
    use crate::core::command_line::{
      Args,
      Flag,
      Signature,
    };

    const FLAGS: &[Flag] = &[
      Flag {
        name:        "force",
        alias:       Some('f'),
        doc:         "Force operation",
        completions: None,
      },
      Flag {
        name:        "verbose",
        alias:       Some('v'),
        doc:         "Verbose output",
        completions: None,
      },
    ];

    let sig = Signature {
      positionals: (0, None),
      flags: FLAGS,
      ..Signature::DEFAULT
    };

    let args = Args::parse("--force arg1 -v arg2", sig, true, |token| Ok(token.content)).unwrap();

    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "arg1");
    assert_eq!(&args[1], "arg2");
    assert!(args.has_flag("force"));
    assert!(args.has_flag("verbose"));
  }

  #[test]
  fn test_args_parsing_flag_with_argument() {
    use crate::core::command_line::{
      Args,
      Flag,
      Signature,
    };

    const FLAGS: &[Flag] = &[Flag {
      name:        "output",
      alias:       Some('o'),
      doc:         "Output file",
      completions: Some(&["file.txt", "output.txt"]),
    }];

    let sig = Signature {
      positionals: (0, None),
      flags: FLAGS,
      ..Signature::DEFAULT
    };

    let args = Args::parse("--output file.txt input.txt", sig, true, |token| {
      Ok(token.content)
    })
    .unwrap();

    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "input.txt");
    assert_eq!(args.get_flag("output"), Some("file.txt"));
  }

  #[test]
  fn test_command_documentation_generation() {
    use crate::core::command_line::{
      Flag,
      Signature,
    };

    fn test_cmd(_cx: &mut Context, _args: Args, _event: PromptEvent) -> Result<()> {
      Ok(())
    }

    const FLAGS: &[Flag] = &[Flag {
      name:        "force",
      alias:       Some('f'),
      doc:         "Force the operation",
      completions: None,
    }];

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
    use crate::core::command_line::{
      Args,
      Signature,
    };

    // Require exactly 1 positional argument
    let sig = Signature {
      positionals: (1, Some(1)),
      ..Signature::DEFAULT
    };

    // Try to parse with no arguments - should fail in validation mode
    let result = Args::parse("", sig, true, |token| Ok(token.content));

    assert!(result.is_err());

    // Try to parse with too many arguments - should fail
    let result = Args::parse("arg1 arg2", sig, true, |token| Ok(token.content));

    assert!(result.is_err());
  }

  #[test]
  fn test_args_completion_state() {
    use crate::core::command_line::{
      Args,
      CompletionState,
      Flag,
      Signature,
    };

    const FLAGS: &[Flag] = &[Flag {
      name:        "output",
      alias:       Some('o'),
      doc:         "Output file",
      completions: Some(&["file.txt"]),
    }];

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
