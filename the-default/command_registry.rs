use std::{
  collections::{
    HashMap,
    HashSet,
  },
  fs::OpenOptions,
  fmt,
  ops::RangeFrom,
  sync::Arc,
};

use the_core::chars::{
  next_char_boundary,
  prev_char_boundary,
};
use the_lib::{
  command_line::{
    Args,
    CompletionState,
    Signature,
    Token,
    Tokenizer,
    split,
  },
  render::{
    GutterConfig,
    GutterType,
    LineNumberMode,
  },
};

use crate::{
  Command,
  DefaultContext,
  Key,
  KeyEvent,
  Mode,
  command_palette_default_selected,
  command_palette_filtered_indices,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEvent {
  Preview,
  Validate,
  Cancel,
}

#[derive(Debug, Clone)]
pub struct Completion {
  pub range: RangeFrom<usize>,
  pub text:  String,
  pub doc:   Option<String>,
}

pub type CommandFn<Ctx> = fn(&mut Ctx, Args, CommandEvent) -> CommandResult;

pub type CommandResult = Result<(), CommandError>;

#[derive(Debug, Clone)]
pub struct CommandError {
  pub message: String,
}

impl fmt::Display for CommandError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

impl std::error::Error for CommandError {}

impl CommandError {
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

pub type Completer<Ctx> = fn(&Ctx, &str) -> Vec<Completion>;

#[derive(Clone, Copy)]
pub struct CommandCompleter<Ctx: 'static> {
  pub positional: &'static [Completer<Ctx>],
  pub variadic:   Completer<Ctx>,
}

impl<Ctx: 'static> CommandCompleter<Ctx> {
  pub const fn none() -> Self {
    Self {
      positional: &[],
      variadic:   completers::none,
    }
  }

  pub const fn all(completer: Completer<Ctx>) -> Self {
    Self {
      positional: &[],
      variadic:   completer,
    }
  }

  pub const fn positional(positional: &'static [Completer<Ctx>], variadic: Completer<Ctx>) -> Self {
    Self {
      positional,
      variadic,
    }
  }

  pub fn get(&self, index: usize) -> Completer<Ctx> {
    self.positional.get(index).copied().unwrap_or(self.variadic)
  }
}

#[derive(Clone)]
pub struct TypableCommand<Ctx: 'static> {
  pub name:      &'static str,
  pub aliases:   &'static [&'static str],
  pub doc:       &'static str,
  pub fun:       CommandFn<Ctx>,
  pub completer: CommandCompleter<Ctx>,
  pub signature: Signature,
}

impl<Ctx: 'static> TypableCommand<Ctx> {
  pub const fn new(
    name: &'static str,
    aliases: &'static [&'static str],
    doc: &'static str,
    fun: CommandFn<Ctx>,
    completer: CommandCompleter<Ctx>,
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

  pub fn execute(&self, ctx: &mut Ctx, args: Args, event: CommandEvent) -> CommandResult {
    (self.fun)(ctx, args, event)
  }

  pub fn generate_doc(&self) -> String {
    use std::fmt::Write;

    let mut doc = String::new();

    writeln!(doc, ":{} - {}", self.name, self.doc).unwrap();
    if !self.aliases.is_empty() {
      writeln!(doc, "Aliases: {}", self.aliases.join(", ")).unwrap();
    }

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
          writeln!(doc, "{min} argument{}", if min == 1 { "" } else { "s" }).unwrap()
        },
        (min, Some(max)) => writeln!(doc, "{min}-{max} arguments").unwrap(),
        (min, None) => writeln!(doc, "{min} or more arguments").unwrap(),
      }
    }

    if !self.signature.flags.is_empty() {
      writeln!(doc, "Flags:").unwrap();
      let max_flag_len = self
        .signature
        .flags
        .iter()
        .map(|flag| {
          let name_len = flag.name.len();
          let alias_len = if flag.alias.is_some() { 3 } else { 0 };
          let arg_len = if flag.takes_value { 6 } else { 0 };
          name_len + alias_len + arg_len
        })
        .max()
        .unwrap_or(0);

      for flag in self.signature.flags {
        write!(doc, "  --{}", flag.name).unwrap();
        let mut current_len = flag.name.len();

        if let Some(alias) = flag.alias {
          write!(doc, "/-{}", alias).unwrap();
          current_len += 3;
        }

        if flag.takes_value {
          write!(doc, " <arg>").unwrap();
          current_len += 6;
        }

        let padding = max_flag_len.saturating_sub(current_len);
        write!(doc, "{:width$}  {}", "", flag.doc, width = padding).unwrap();
        writeln!(doc).unwrap();
      }
    }

    doc
  }
}

impl<Ctx: 'static> fmt::Debug for TypableCommand<Ctx> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TypableCommand")
      .field("name", &self.name)
      .field("aliases", &self.aliases)
      .field("doc", &self.doc)
      .finish()
  }
}

#[derive(Debug, Clone)]
pub struct CommandRegistry<Ctx: 'static> {
  commands: HashMap<String, Arc<TypableCommand<Ctx>>>,
}

impl<Ctx: DefaultContext + 'static> CommandRegistry<Ctx> {
  pub fn new() -> Self {
    let mut registry = Self {
      commands: HashMap::new(),
    };
    registry.register_builtin_commands();
    registry
  }

  pub fn register(&mut self, command: TypableCommand<Ctx>) {
    let cmd = Arc::new(command);
    self.commands.insert(cmd.name.to_string(), cmd.clone());
    for alias in cmd.aliases {
      self.commands.insert(alias.to_string(), cmd.clone());
    }
  }

  pub fn get(&self, name: &str) -> Option<&TypableCommand<Ctx>> {
    self.commands.get(name).map(|cmd| cmd.as_ref())
  }

  pub fn execute(
    &self,
    ctx: &mut Ctx,
    name: &str,
    args_line: &str,
    event: CommandEvent,
  ) -> CommandResult {
    match self.get(name) {
      Some(command) => {
        let args = Args::parse::<std::convert::Infallible, _>(
          args_line,
          command.signature,
          event == CommandEvent::Validate,
          |token| Ok(token.content),
        )
        .map_err(|e| CommandError::new(format!("{e}")))?;

        command.execute(ctx, args, event)
      },
      None => Err(CommandError::new(format!("command not found: {name}"))),
    }
  }

  pub fn command_names(&self) -> Vec<&str> {
    let mut names: Vec<_> = self.commands.values().map(|cmd| cmd.name).collect();
    names.sort();
    names.dedup();
    names
  }

  pub fn all_commands(&self) -> Vec<Arc<TypableCommand<Ctx>>> {
    let mut seen = HashSet::new();
    let mut commands: Vec<_> = self
      .commands
      .values()
      .filter(|cmd| seen.insert(cmd.name))
      .cloned()
      .collect();
    commands.sort_by(|a, b| a.name.cmp(b.name));
    commands
  }

  pub fn completions(&self, prefix: &str) -> Vec<&str> {
    self
      .command_names()
      .into_iter()
      .filter(|name| name.starts_with(prefix))
      .collect()
  }

  pub fn complete_command_line(&self, ctx: &Ctx, input: &str) -> Vec<Completion> {
    let (command, rest, complete_command_name) = split(input);

    if complete_command_name {
      let input_lower = command.to_lowercase();
      return self
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
        .collect();
    }

    let Some(cmd) = self.get(command) else {
      return Vec::new();
    };

    let mut args = Args::new(cmd.signature, false);
    let mut tokenizer = Tokenizer::new(rest, false);
    let mut last_token: Option<Token<'_>> = None;

    while let Ok(Some(token)) = args.read_token(&mut tokenizer) {
      last_token = Some(token.clone());
      let _ = args.push(token.content);
    }

    match args.completion_state() {
      CompletionState::Positional => {
        let arg_index = args.len();
        let completer = cmd.completer.get(arg_index);

        let (arg_input, arg_start_offset) = match &last_token {
          Some(token) if !token.is_terminated => {
            (
              token.content.as_ref(),
              command.len() + 1 + token.content_start,
            )
          },
          _ => ("", input.len()),
        };

        let mut completions = completer(ctx, arg_input);
        for completion in &mut completions {
          let relative_start = completion.range.start;
          completion.range = (arg_start_offset + relative_start)..;
        }
        completions
      },
      CompletionState::Flag(_) => {
        if cmd.signature.flags.is_empty() {
          return Vec::new();
        }

        let (flag_input, flag_start_offset) = match &last_token {
          Some(token) if !token.is_terminated => {
            let input = token.content.as_ref();
            let trimmed = input.trim_start_matches('-');
            (trimmed, command.len() + 1 + token.content_start)
          },
          _ => ("", input.len()),
        };

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
        if let Some(completions) = flag.completions {
          let (arg_input, arg_start_offset) = match &last_token {
            Some(token) if !token.is_terminated => {
              (
                token.content.as_ref(),
                command.len() + 1 + token.content_start,
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
  }

  fn register_builtin_commands(&mut self) {
    self.register(TypableCommand::new(
      "quit",
      &["q"],
      "Close the editor",
      cmd_quit::<Ctx>,
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
      cmd_write::<Ctx>,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "write-quit",
      &["wq", "x"],
      "Write buffer to file and close the editor",
      cmd_write_quit::<Ctx>,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "reload",
      &[],
      "Reload current file from disk (use :reload force to discard unsaved changes)",
      cmd_reload::<Ctx>,
      CommandCompleter::all(completers::reload_mode),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "help",
      &["h"],
      "Show help for commands",
      cmd_help::<Ctx>,
      CommandCompleter::all(completers::command),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "log-open",
      &["open-log"],
      "Open a debug log file (messages/lsp/watch)",
      cmd_log_open::<Ctx>,
      CommandCompleter::all(completers::log_target),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "format",
      &["fmt"],
      "Format current document via LSP",
      cmd_lsp_format::<Ctx>,
      CommandCompleter::none(),
      Signature {
        positionals: (0, Some(0)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "soft-wrap",
      &[],
      "Configure soft line wrapping (on/off/toggle/status)",
      cmd_wrap::<Ctx>,
      CommandCompleter::all(completers::wrap_mode),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "gutter",
      &[],
      "Configure gutter visibility (on/off/toggle/status)",
      cmd_gutter::<Ctx>,
      CommandCompleter::all(completers::gutter_mode),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));

    self.register(TypableCommand::new(
      "line-number",
      &["line-numbers"],
      "Configure line-number mode (absolute/relative/off/status)",
      cmd_line_number::<Ctx>,
      CommandCompleter::all(completers::line_number_mode),
      Signature {
        positionals: (0, Some(1)),
        ..Signature::DEFAULT
      },
    ));
  }
}

impl<Ctx> Default for CommandRegistry<Ctx>
where
  Ctx: DefaultContext,
{
  fn default() -> Self {
    Self::new()
  }
}

fn cmd_quit<Ctx: DefaultContext>(ctx: &mut Ctx, _args: Args, event: CommandEvent) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  ctx.dispatch().pre_on_action(ctx, Command::Quit);
  Ok(())
}

fn cmd_write<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: Args,
  event: CommandEvent,
) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  ctx.dispatch().pre_on_action(ctx, Command::Save);
  Ok(())
}

fn cmd_write_quit<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: Args,
  event: CommandEvent,
) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  ctx.dispatch().pre_on_action(ctx, Command::Save);
  ctx.dispatch().pre_on_action(ctx, Command::Quit);
  Ok(())
}

fn cmd_reload<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: Args,
  event: CommandEvent,
) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let force = match args.first() {
    None => false,
    Some("force") | Some("!") => true,
    Some(other) => {
      return Err(CommandError::new(format!(
        "invalid reload mode '{other}' (expected force)"
      )));
    },
  };

  let Some(path) = ctx.file_path().map(|path| path.to_path_buf()) else {
    return Err(CommandError::new("no file path set for current buffer"));
  };

  if ctx.editor().document().flags().modified && !force {
    return Err(CommandError::new(
      "buffer has unsaved changes; run :reload force to discard them",
    ));
  }

  ctx.reload_file_preserving_view(&path).map_err(|err| {
    CommandError::new(format!(
      "failed to reload '{}': {err}",
      path.display()
    ))
  })?;

  let suffix = if force { " (discarded local changes)" } else { "" };
  ctx.push_info(
    "reload",
    format!("reloaded {} from disk{suffix}", path.display()),
  );
  Ok(())
}

fn cmd_help<Ctx: DefaultContext>(ctx: &mut Ctx, args: Args, event: CommandEvent) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let help = if let Some(name) = args.first() {
    match ctx.command_registry_ref().get(name) {
      Some(cmd) => cmd.generate_doc(),
      None => format!("Unknown command: {name}"),
    }
  } else {
    let mut result = String::new();
    for name in ctx.command_registry_ref().command_names() {
      result.push_str(name);
      result.push('\n');
    }
    result
  };

  let prompt = ctx.command_prompt_mut();
  prompt.help = Some(help);
  prompt.error = None;
  Ok(())
}

fn cmd_log_open<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: Args,
  event: CommandEvent,
) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let targets = ctx.log_target_names();
  let Some(target) = args.first().or_else(|| targets.first().copied()) else {
    return Err(CommandError::new(
      "no log targets configured for this editor client",
    ));
  };

  let Some(path) = ctx.log_path_for_target(target) else {
    if targets.contains(&target) {
      return Err(CommandError::new(format!(
        "log target '{target}' is disabled"
      )));
    }
    let available = if targets.is_empty() {
      "none".to_string()
    } else {
      targets.join(", ")
    };
    return Err(CommandError::new(format!(
      "unknown log target '{target}' (available: {available})"
    )));
  };

  if let Some(parent) = path.parent()
    && let Err(err) = std::fs::create_dir_all(parent)
  {
    return Err(CommandError::new(format!(
      "failed to create log directory '{}': {err}",
      parent.display()
    )));
  }

  if let Err(err) = OpenOptions::new().create(true).append(true).open(&path) {
    return Err(CommandError::new(format!(
      "failed to open log file '{}': {err}",
      path.display()
    )));
  }

  ctx
    .open_file(&path)
    .map_err(|err| CommandError::new(format!("failed to open log buffer: {err}")))?;
  ctx.push_info("editor", format!("opened {target} log: {}", path.display()));
  Ok(())
}

fn cmd_lsp_format<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: Args,
  event: CommandEvent,
) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  ctx.dispatch().pre_on_action(ctx, Command::LspFormat);
  Ok(())
}

fn cmd_wrap<Ctx: DefaultContext>(ctx: &mut Ctx, args: Args, event: CommandEvent) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let mode = args.first().unwrap_or("toggle");
  let next_state = match mode {
    "on" => Some(true),
    "off" => Some(false),
    "toggle" => Some(!ctx.soft_wrap_enabled()),
    "status" => None,
    other => {
      return Err(CommandError::new(format!(
        "invalid wrap mode '{other}' (expected on/off/toggle/status)"
      )));
    },
  };

  if let Some(enabled) = next_state {
    ctx.set_soft_wrap_enabled(enabled);
  }

  let state_label = if ctx.soft_wrap_enabled() { "on" } else { "off" };
  ctx.push_info("editor", format!("soft wrap: {state_label}"));
  Ok(())
}

fn default_gutter_layout() -> Vec<GutterType> {
  GutterConfig::default().layout
}

fn ensure_line_number_column(layout: &mut Vec<GutterType>) {
  if layout.contains(&GutterType::LineNumbers) {
    return;
  }

  if layout.is_empty() {
    *layout = default_gutter_layout();
  } else {
    layout.push(GutterType::Spacer);
    layout.push(GutterType::LineNumbers);
  }
}

fn cmd_gutter<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: Args,
  event: CommandEvent,
) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let mode = args.first().unwrap_or("toggle");
  let mut changed = false;
  let enabled = {
    let config = ctx.gutter_config_mut();
    match mode {
      "on" => {
        if config.layout.is_empty() {
          config.layout = default_gutter_layout();
          changed = true;
        }
      },
      "off" => {
        if !config.layout.is_empty() {
          config.layout.clear();
          changed = true;
        }
      },
      "toggle" => {
        if config.layout.is_empty() {
          config.layout = default_gutter_layout();
        } else {
          config.layout.clear();
        }
        changed = true;
      },
      "status" => {},
      other => {
        return Err(CommandError::new(format!(
          "invalid gutter mode '{other}' (expected on/off/toggle/status)"
        )));
      },
    }
    !config.layout.is_empty()
  };

  if changed {
    ctx.request_render();
  }

  let state = if enabled { "on" } else { "off" };
  ctx.push_info("editor", format!("gutter: {state}"));
  Ok(())
}

fn cmd_line_number<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: Args,
  event: CommandEvent,
) -> CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let mode = args.first().unwrap_or("status");
  let mut changed = false;
  let message = {
    let config = ctx.gutter_config_mut();
    match mode {
      "absolute" => {
        if config.line_numbers.mode != LineNumberMode::Absolute {
          config.line_numbers.mode = LineNumberMode::Absolute;
          changed = true;
        }
        let prev_len = config.layout.len();
        ensure_line_number_column(&mut config.layout);
        if config.layout.len() != prev_len {
          changed = true;
        }
      },
      "relative" => {
        if config.line_numbers.mode != LineNumberMode::Relative {
          config.line_numbers.mode = LineNumberMode::Relative;
          changed = true;
        }
        let prev_len = config.layout.len();
        ensure_line_number_column(&mut config.layout);
        if config.layout.len() != prev_len {
          changed = true;
        }
      },
      "off" => {
        let prev_len = config.layout.len();
        config
          .layout
          .retain(|column| *column != GutterType::LineNumbers);
        changed |= config.layout.len() != prev_len;
      },
      "status" => {},
      other => {
        return Err(CommandError::new(format!(
          "invalid line-number mode '{other}' (expected absolute/relative/off/status)"
        )));
      },
    }

    if config.layout.contains(&GutterType::LineNumbers) {
      let mode = match config.line_numbers.mode {
        LineNumberMode::Absolute => "absolute",
        LineNumberMode::Relative => "relative",
      };
      format!("line numbers: {mode}")
    } else {
      "line numbers: off".to_string()
    }
  };

  if changed {
    ctx.request_render();
  }
  ctx.push_info("editor", message);
  Ok(())
}

pub struct CommandPromptState {
  pub input:       String,
  pub cursor:      usize,
  pub completions: Vec<Completion>,
  pub help:        Option<String>,
  pub error:       Option<String>,
}

impl CommandPromptState {
  pub fn new() -> Self {
    Self {
      input:       String::new(),
      cursor:      0,
      completions: Vec::new(),
      help:        None,
      error:       None,
    }
  }

  pub fn clear(&mut self) {
    self.input.clear();
    self.cursor = 0;
    self.completions.clear();
    self.help = None;
    self.error = None;
  }
}

impl Default for CommandPromptState {
  fn default() -> Self {
    Self::new()
  }
}

pub fn handle_command_prompt_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if ctx.mode() != Mode::Command {
    return false;
  }

  let mut should_update = false;

  match key.key {
    Key::Escape => {
      ctx.set_mode(Mode::Normal);
      ctx.command_prompt_mut().clear();
      {
        let palette = ctx.command_palette_mut();
        palette.is_open = false;
        palette.query.clear();
        palette.selected = None;
      }
      ctx.request_render();
      return true;
    },
    Key::Enter | Key::NumpadEnter => {
      let mut line = {
        let prompt = ctx.command_prompt_ref();
        prompt.input.trim().trim_start_matches(':').to_string()
      };

      if line.is_empty() || !line.chars().any(char::is_whitespace) {
        let selected = {
          let palette = ctx.command_palette_mut();
          if let Some(sel) = palette.selected {
            let filtered = command_palette_filtered_indices(palette);
            if !filtered.contains(&sel) {
              palette.selected = filtered.first().copied();
            }
          } else {
            palette.selected = command_palette_default_selected(palette);
          }
          palette.selected
        };

        if let Some(sel) = selected {
          if let Some(item) = ctx.command_palette().items.get(sel) {
            line = item.title.clone();
          }
        }
      }

      if line.is_empty() {
        ctx.set_mode(Mode::Normal);
        ctx.command_prompt_mut().clear();
        {
          let palette = ctx.command_palette_mut();
          palette.is_open = false;
          palette.query.clear();
          palette.selected = None;
        }
        ctx.request_render();
        return true;
      }

      let (command, args, _) = split(&line);
      if command.is_empty() {
        let message = "empty command".to_string();
        ctx.command_prompt_mut().error = Some(message.clone());
        ctx.push_error("command", message);
        ctx.request_render();
        return true;
      }

      let registry = ctx.command_registry_ref() as *const CommandRegistry<Ctx>;
      let result = unsafe { (&*registry).execute(ctx, command, args, CommandEvent::Validate) };

      match result {
        Ok(()) => {
          ctx.set_mode(Mode::Normal);
          ctx.command_prompt_mut().clear();
          {
            let palette = ctx.command_palette_mut();
            palette.is_open = false;
            palette.query.clear();
            palette.selected = None;
          }
        },
        Err(err) => {
          let message = err.to_string();
          ctx.command_prompt_mut().error = Some(message.clone());
          ctx.push_error("command", message);
        },
      }

      ctx.request_render();
      return true;
    },
    Key::Backspace => {
      let prompt = ctx.command_prompt_mut();
      if prompt.cursor > 0 && prompt.cursor <= prompt.input.len() {
        let prev = prev_char_boundary(&prompt.input, prompt.cursor);
        prompt.input.replace_range(prev..prompt.cursor, "");
        prompt.cursor = prev;
        should_update = true;
      }
    },
    Key::Up => {
      let filtered = command_palette_filtered_indices(ctx.command_palette());
      if !filtered.is_empty() {
        let palette = ctx.command_palette_mut();
        let current = palette
          .selected
          .and_then(|sel| filtered.iter().position(|&idx| idx == sel))
          .unwrap_or(0);
        let next = if current == 0 {
          filtered.len() - 1
        } else {
          current - 1
        };
        palette.selected = Some(filtered[next]);
        ctx.request_render();
      }
      return true;
    },
    Key::Down => {
      let filtered = command_palette_filtered_indices(ctx.command_palette());
      if !filtered.is_empty() {
        let palette = ctx.command_palette_mut();
        let current = palette
          .selected
          .and_then(|sel| filtered.iter().position(|&idx| idx == sel))
          .unwrap_or(usize::MAX);
        let next = if current >= filtered.len() - 1 {
          0
        } else {
          current + 1
        };
        palette.selected = Some(filtered[next]);
        ctx.request_render();
      }
      return true;
    },
    Key::Delete => {
      let prompt = ctx.command_prompt_mut();
      if prompt.cursor < prompt.input.len() {
        let next = next_char_boundary(&prompt.input, prompt.cursor);
        prompt.input.replace_range(prompt.cursor..next, "");
        should_update = true;
      }
    },
    Key::Left => {
      let prompt = ctx.command_prompt_mut();
      prompt.cursor = prev_char_boundary(&prompt.input, prompt.cursor);
      should_update = true;
    },
    Key::Right => {
      let prompt = ctx.command_prompt_mut();
      prompt.cursor = next_char_boundary(&prompt.input, prompt.cursor);
      should_update = true;
    },
    Key::Home => {
      ctx.command_prompt_mut().cursor = 0;
      should_update = true;
    },
    Key::End => {
      let prompt = ctx.command_prompt_mut();
      prompt.cursor = prompt.input.len();
      should_update = true;
    },
    Key::Tab => {
      let apply = {
        let prompt = ctx.command_prompt_ref();
        prompt.completions.first().cloned()
      };

      if let Some(completion) = apply {
        let prompt = ctx.command_prompt_mut();
        let start = completion.range.start.min(prompt.input.len());
        prompt.input.replace_range(start.., &completion.text);
        prompt.cursor = prompt.input.len();
        should_update = true;
      }
    },
    Key::Char(ch) => {
      if key.modifiers.ctrl() || key.modifiers.alt() {
        return true;
      }
      let prompt = ctx.command_prompt_mut();
      prompt.input.insert(prompt.cursor, ch);
      prompt.cursor += ch.len_utf8();
      should_update = true;
    },
    _ => {},
  }

  if should_update {
    let input = {
      let prompt = ctx.command_prompt_ref();
      prompt.input.clone()
    };
    let completions = ctx
      .command_registry_ref()
      .complete_command_line(ctx, &input);
    let input = {
      let prompt = ctx.command_prompt_ref();
      prompt.input.clone()
    };
    let prompt = ctx.command_prompt_mut();
    prompt.completions = completions;
    prompt.error = None;
    {
      let palette = ctx.command_palette_mut();
      palette.query = input;
      palette.selected = command_palette_default_selected(palette);
    }
    ctx.request_render();
  }

  true
}

pub mod completers {
  use super::{
    Completion,
    DefaultContext,
  };

  pub fn none<Ctx>(_ctx: &Ctx, _input: &str) -> Vec<Completion> {
    Vec::new()
  }

  pub fn command<Ctx: DefaultContext>(ctx: &Ctx, input: &str) -> Vec<Completion> {
    let input_lower = input.to_lowercase();
    ctx
      .command_registry_ref()
      .command_names()
      .into_iter()
      .filter(|name| name.to_lowercase().contains(&input_lower))
      .map(|name| {
        Completion {
          range: 0..,
          text:  name.to_string(),
          doc:   ctx
            .command_registry_ref()
            .get(name)
            .map(|cmd| cmd.doc.to_string()),
        }
      })
      .collect()
  }

  pub fn wrap_mode<Ctx>(_ctx: &Ctx, input: &str) -> Vec<Completion> {
    const MODES: &[&str] = &["toggle", "on", "off", "status"];
    MODES
      .iter()
      .filter(|mode| mode.starts_with(input))
      .map(|mode| {
        Completion {
          range: 0..,
          text:  (*mode).to_string(),
          doc:   None,
        }
      })
      .collect()
  }

  pub fn gutter_mode<Ctx>(_ctx: &Ctx, input: &str) -> Vec<Completion> {
    const MODES: &[&str] = &["toggle", "on", "off", "status"];
    MODES
      .iter()
      .filter(|mode| mode.starts_with(input))
      .map(|mode| {
        Completion {
          range: 0..,
          text:  (*mode).to_string(),
          doc:   None,
        }
      })
      .collect()
  }

  pub fn line_number_mode<Ctx>(_ctx: &Ctx, input: &str) -> Vec<Completion> {
    const MODES: &[&str] = &["absolute", "relative", "off", "status"];
    MODES
      .iter()
      .filter(|mode| mode.starts_with(input))
      .map(|mode| {
        Completion {
          range: 0..,
          text:  (*mode).to_string(),
          doc:   None,
        }
      })
      .collect()
  }

  pub fn log_target<Ctx: DefaultContext>(ctx: &Ctx, input: &str) -> Vec<Completion> {
    ctx
      .log_target_names()
      .iter()
      .filter(|target| target.starts_with(input))
      .map(|target| Completion {
        range: 0..,
        text:  (*target).to_string(),
        doc:   None,
      })
      .collect()
  }

  pub fn reload_mode<Ctx>(_ctx: &Ctx, input: &str) -> Vec<Completion> {
    const MODES: &[&str] = &["force"];
    MODES
      .iter()
      .filter(|mode| mode.starts_with(input))
      .map(|mode| Completion {
        range: 0..,
        text:  (*mode).to_string(),
        doc:   None,
      })
      .collect()
  }
}
