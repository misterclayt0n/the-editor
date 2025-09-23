use std::{
  collections::HashMap,
  fmt,
  sync::Arc,
};

use anyhow::{Result, anyhow};

use super::{
  commands::Context,
};

/// Type alias for a command function that takes a context and arguments
pub type CommandFn = fn(&mut Context, &[&str]) -> Result<()>;

/// A typable command that can be executed in command mode
#[derive(Clone)]
pub struct TypableCommand {
    /// Command name (primary identifier)
    pub name: &'static str,
    /// Command aliases (alternative names)
    pub aliases: &'static [&'static str],
    /// Short documentation string
    pub doc: &'static str,
    /// The function to execute
    pub fun: CommandFn,
}

impl TypableCommand {
    /// Create a new typable command
    pub const fn new(
        name: &'static str,
        aliases: &'static [&'static str],
        doc: &'static str,
        fun: CommandFn,
    ) -> Self {
        Self {
            name,
            aliases,
            doc,
            fun,
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
        let mut names: Vec<_> = self.commands
            .values()
            .map(|cmd| cmd.name)
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Get command completions that start with the given prefix
    pub fn completions(&self, prefix: &str) -> Vec<&str> {
        self.command_names()
            .into_iter()
            .filter(|name| name.starts_with(prefix))
            .collect()
    }

    /// Register all built-in commands
    fn register_builtin_commands(&mut self) {
        self.register(TypableCommand::new(
            "quit",
            &["q"],
            "Close the editor",
            quit,
        ));

        self.register(TypableCommand::new(
            "quit!",
            &["q!"],
            "Force close the editor without saving",
            force_quit,
        ));

        self.register(TypableCommand::new(
            "write",
            &["w"],
            "Write buffer to file",
            write_buffer,
        ));

        self.register(TypableCommand::new(
            "write-quit",
            &["wq", "x"],
            "Write buffer to file and close the editor",
            write_quit,
        ));

        self.register(TypableCommand::new(
            "open",
            &["o", "e", "edit"],
            "Open a file for editing",
            open_file,
        ));

        self.register(TypableCommand::new(
            "new",
            &["n"],
            "Create a new buffer",
            new_file,
        ));

        self.register(TypableCommand::new(
            "buffer-close",
            &["bc", "bclose"],
            "Close the current buffer",
            buffer_close,
        ));

        self.register(TypableCommand::new(
            "buffer-next",
            &["bn", "bnext"],
            "Go to next buffer",
            buffer_next,
        ));

        self.register(TypableCommand::new(
            "buffer-previous",
            &["bp", "bprev"],
            "Go to previous buffer",
            buffer_previous,
        ));

        self.register(TypableCommand::new(
            "help",
            &["h"],
            "Show help for commands",
            show_help,
        ));
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Built-in command implementations

fn quit(cx: &mut Context, _args: &[&str]) -> Result<()> {
    if cx.editor.documents.values().any(|doc| doc.is_modified()) {
        cx.editor.set_error("unsaved changes, use :q! to force quit".to_string());
        return Ok(());
    }
    std::process::exit(0);
}

fn force_quit(_cx: &mut Context, _args: &[&str]) -> Result<()> {
    std::process::exit(0);
}

fn write_buffer(cx: &mut Context, args: &[&str]) -> Result<()> {
    use crate::current;
    use std::path::PathBuf;

    let (_view, doc) = current!(cx.editor);

    let path = if args.is_empty() {
        doc.path().map(|p| p.to_path_buf())
    } else {
        Some(PathBuf::from(args[0]))
    };

    if let Some(path) = path {
        match doc.save(Some(path.clone()), true) {
            Ok(_) => {
                cx.editor.set_status(format!("written: {}", path.display()));
            }
            Err(err) => {
                cx.editor.set_error(format!("failed to save: {}", err));
            }
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
        }
        Err(err) => {
            cx.editor.set_error(format!("failed to open: {}", err));
        }
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
    let doc_id = cx.editor.documents
        .iter()
        .find(|(_, doc)| doc.selections().contains_key(&view_id))
        .map(|(id, _)| *id);

    if let Some(doc_id) = doc_id {
        if let Some(doc) = cx.editor.documents.get(&doc_id) {
            if doc.is_modified() {
                cx.editor.set_error("unsaved changes, save first".to_string());
                return Ok(());
            }
        }

        match cx.editor.close_document(doc_id, false) {
            Ok(_) => {},
            Err(err) => {
                cx.editor.set_error("failed to close buffer".to_string());
                return Ok(());
            }
        }
        cx.editor.set_status("buffer closed".to_string());
    }

    Ok(())
}

fn buffer_next(cx: &mut Context, _args: &[&str]) -> Result<()> {
    // Simplified buffer switching - in a real implementation you'd maintain a buffer list
    cx.editor.set_status("buffer next (not implemented)".to_string());
    Ok(())
}

fn buffer_previous(cx: &mut Context, _args: &[&str]) -> Result<()> {
    // Simplified buffer switching - in a real implementation you'd maintain a buffer list
    cx.editor.set_status("buffer previous (not implemented)".to_string());
    Ok(())
}

fn show_help(cx: &mut Context, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        // Show general help
        let help_text = "Available commands:\n\
            :quit, :q - Close the editor\n\
            :quit!, :q! - Force close without saving\n\
            :write, :w [file] - Write buffer to file\n\
            :write-quit, :wq, :x - Write and quit\n\
            :open, :o, :e, :edit <file> - Open a file\n\
            :new, :n - Create new buffer\n\
            :buffer-close, :bc - Close current buffer\n\
            :help, :h [command] - Show help";

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
        );

        assert_eq!(cmd.name, "test");
        assert_eq!(cmd.aliases, &["t"]);
        assert_eq!(cmd.doc, "Test command");
    }
}