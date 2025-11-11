use std::{
  collections::HashSet,
  io::{
    self,
    Write,
  },
};

use anyhow::Result;
use config::{
  default_lang_config,
  user_lang_config,
};
use crossterm::{
  style::{
    Color,
    StyledContent,
    Stylize,
  },
  terminal,
  tty::IsTty,
};
use the_editor_loader::grammar::{
  self,
  load_runtime_file,
};

use crate::core::config::{
  self,
  Config,
  ConfigLoadError,
};

#[derive(Copy, Clone)]
pub enum TsFeature {
  Highlight,
  TextObject,
  AutoIndent,
  Tags,
  RainbowBracket,
}

impl TsFeature {
  pub fn all() -> &'static [Self] {
    &[
      Self::Highlight,
      Self::TextObject,
      Self::AutoIndent,
      Self::Tags,
      Self::RainbowBracket,
    ]
  }

  pub fn runtime_filename(&self) -> &'static str {
    match self {
      Self::Highlight => "highlights.scm",
      Self::TextObject => "textobjects.scm",
      Self::AutoIndent => "indents.scm",
      Self::Tags => "tags.scm",
      Self::RainbowBracket => "rainbows.scm",
    }
  }

  pub fn short_title(&self) -> &'static str {
    match self {
      Self::Highlight => "Highlight",
      Self::TextObject => "Textobject",
      Self::AutoIndent => "Indent",
      Self::Tags => "Tags",
      Self::RainbowBracket => "Rainbow",
    }
  }
}

pub fn run(category: Option<&str>) -> Result<()> {
  match category {
    Some("languages") => languages_selection()?,
    Some("all-languages") => languages_all()?,
    Some("clipboard") => clipboard()?,
    Some("all") => {
      general()?;
      clipboard()?;
      writeln!(io::stdout().lock())?;
      languages_all()?;
    },
    Some(lang) => language(lang)?,
    None => {
      general()?;
      clipboard()?;
      writeln!(io::stdout().lock())?;
      languages_selection()?;
    },
  }

  Ok(())
}

fn general() -> io::Result<()> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  let config_file = the_editor_loader::config_file();
  let lang_file = the_editor_loader::lang_config_file();
  let log_file = the_editor_loader::log_file();
  let rt_dirs = the_editor_loader::runtime_dirs();

  if config_file.exists() {
    writeln!(stdout, "Config file: {}", config_file.display())?;
  } else {
    writeln!(stdout, "Config file: default")?;
  }

  if lang_file.exists() {
    writeln!(stdout, "Language file: {}", lang_file.display())?;
  } else {
    writeln!(stdout, "Language file: default")?;
  }

  writeln!(stdout, "Log file: {}", log_file.display())?;
  writeln!(
    stdout,
    "Runtime directories: {}",
    rt_dirs
      .iter()
      .map(|dir| dir.to_string_lossy())
      .collect::<Vec<_>>()
      .join(";")
  )?;

  for rt_dir in rt_dirs.iter() {
    if let Ok(target) = std::fs::read_link(rt_dir) {
      let msg = format!(
        "Runtime directory {} is symlinked to: {}",
        rt_dir.display(),
        target.display()
      );
      writeln!(stdout, "{}", msg.yellow())?;
    }
    if !rt_dir.exists() {
      let msg = format!("Runtime directory does not exist: {}", rt_dir.display());
      writeln!(stdout, "{}", msg.yellow())?;
    } else if rt_dir
      .read_dir()
      .ok()
      .map(|mut it| it.next().is_none())
      .unwrap_or(false)
    {
      let msg = format!("Runtime directory is empty: {}", rt_dir.display());
      writeln!(stdout, "{}", msg.yellow())?;
    }
  }

  Ok(())
}

fn clipboard() -> io::Result<()> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  let config = match Config::load_user() {
    Ok(config) => config,
    Err(ConfigLoadError::Error(err)) if err.kind() == io::ErrorKind::NotFound => Config::default(),
    Err(ConfigLoadError::Error(err)) => {
      writeln!(stdout, "{}", "Configuration file malformed".red())?;
      writeln!(stdout, "{}", err)?;
      return Ok(());
    },
    Err(ConfigLoadError::BadConfig(err)) => {
      writeln!(stdout, "{}", "Configuration file malformed".red())?;
      writeln!(stdout, "{}", err)?;
      return Ok(());
    },
  };

  match config.editor.clipboard_provider.name().as_ref() {
    "none" => {
      writeln!(
        stdout,
        "{}",
        "System clipboard provider: Not installed".red()
      )?;
      writeln!(
        stdout,
        "    {}",
        "For troubleshooting system clipboard issues, refer".red()
      )?;
      writeln!(
        stdout,
        "    {}",
        "https://github.com/helix-editor/helix/wiki/Troubleshooting#copypaste-fromto-system-clipboard-not-working"
          .red()
          .underlined()
      )?;
    },
    name => writeln!(stdout, "System clipboard provider: {}", name)?,
  }

  Ok(())
}

fn languages_all() -> io::Result<()> {
  languages(None)
}

fn languages_selection() -> io::Result<()> {
  let selection = grammar::get_grammar_names().unwrap_or_default();
  languages(selection)
}

fn languages(selection: Option<HashSet<String>>) -> io::Result<()> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  let mut syn_loader_conf = match user_lang_config() {
    Ok(conf) => conf,
    Err(err) => {
      let stderr = io::stderr();
      let mut stderr = stderr.lock();

      writeln!(
        stderr,
        "{}: {}",
        "Error parsing user language config".red(),
        err
      )?;
      writeln!(stderr, "{}", "Using default language config".yellow())?;
      default_lang_config()
    },
  };

  let mut headings = vec!["Language", "Language servers", "Debug adapter", "Formatter"];
  for feature in TsFeature::all() {
    headings.push(feature.short_title());
  }

  let terminal_cols = terminal::size().map(|(cols, _)| cols).unwrap_or(80);
  let column_width = terminal_cols as usize / headings.len();
  let is_terminal = io::stdout().is_tty();

  let fit = |text: &str| -> StyledContent<String> {
    format!(
      "{:column_width$}",
      text
        .get(..column_width.saturating_sub(2))
        .map(|s| format!("{}…", s))
        .unwrap_or_else(|| text.to_string())
    )
    .stylize()
  };
  let color = |content: StyledContent<String>, tone: Color| {
    if is_terminal {
      content.with(tone)
    } else {
      content
    }
  };
  let bold = |content: StyledContent<String>| if is_terminal { content.bold() } else { content };

  for heading in headings {
    write!(stdout, "{}", bold(fit(heading)))?;
  }
  writeln!(stdout)?;

  syn_loader_conf
    .language
    .sort_unstable_by_key(|language| language.language_id.clone());

  let check_binary_with_name = |cmd: Option<(&str, &str)>| {
    match cmd {
      Some((name, cmd)) => {
        match the_editor_stdx::env::which(cmd) {
          Ok(_) => color(fit(&format!("✓ {}", name)), Color::Green),
          Err(_) => color(fit(&format!("✘ {}", name)), Color::Red),
        }
      },
      None => color(fit("None"), Color::Yellow),
    }
  };
  let check_binary = |cmd: Option<&str>| check_binary_with_name(cmd.map(|c| (c, c)));

  for lang in &syn_loader_conf.language {
    if selection
      .as_ref()
      .is_some_and(|choices| !choices.contains(&lang.language_id))
    {
      continue;
    }

    write!(stdout, "{}", fit(&lang.language_id))?;

    let mut servers = lang.language_servers.iter().filter_map(|ls| {
      syn_loader_conf
        .language_server
        .get(&ls.name)
        .map(|cfg| (ls.name.as_str(), cfg.command.as_str()))
    });

    write!(stdout, "{}", check_binary_with_name(servers.next()))?;

    let dap = lang.debugger.as_ref().map(|dap| dap.command.as_str());
    write!(stdout, "{}", check_binary(dap))?;

    let formatter = lang
      .formatter
      .as_ref()
      .map(|formatter| formatter.command.as_str());
    write!(stdout, "{}", check_binary(formatter))?;

    for feature in TsFeature::all() {
      if load_runtime_file(&lang.language_id, feature.runtime_filename()).is_ok() {
        write!(stdout, "{}", color(fit("✓"), Color::Green))?;
      } else {
        write!(stdout, "{}", color(fit("✘"), Color::Red))?;
      }
    }

    writeln!(stdout)?;

    for cmd in servers {
      write!(stdout, "{}", fit(""))?;
      writeln!(stdout, "{}", check_binary_with_name(Some(cmd)))?;
    }
  }

  if selection.is_some() {
    writeln!(
      stdout,
      "\nThis list is filtered according to the 'use-grammars' option in languages.toml file.\nTo \
       see the full list, use the '--health all' or '--health all-languages' option."
    )?;
  }

  Ok(())
}

fn language(lang: &str) -> io::Result<()> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  let syn_loader_conf = match user_lang_config() {
    Ok(conf) => conf,
    Err(err) => {
      let stderr = io::stderr();
      let mut stderr = stderr.lock();

      writeln!(
        stderr,
        "{}: {}",
        "Error parsing user language config".red(),
        err
      )?;
      writeln!(stderr, "{}", "Using default language config".yellow())?;
      default_lang_config()
    },
  };

  let Some(language) = syn_loader_conf
    .language
    .iter()
    .find(|l| l.language_id == lang)
  else {
    let msg = format!("Language '{lang}' not found");
    writeln!(stdout, "{}", msg.red())?;

    let mut suggestions = syn_loader_conf
      .language
      .iter()
      .filter(|l| {
        l.language_id
          .starts_with(lang.chars().next().unwrap_or_default())
      })
      .map(|l| l.language_id.as_str())
      .collect::<Vec<_>>();
    suggestions.sort_unstable();
    if !suggestions.is_empty() {
      writeln!(
        stdout,
        "Did you mean one of these: {} ?",
        suggestions.join(", ").yellow()
      )?;
    }
    return Ok(());
  };

  probe_protocols(
    "language server",
    language.language_servers.iter().filter_map(|ls| {
      syn_loader_conf
        .language_server
        .get(&ls.name)
        .map(|config| (ls.name.as_str(), config.command.as_str()))
    }),
  )?;

  probe_protocol(
    "debug adapter",
    language
      .debugger
      .as_ref()
      .map(|dap| dap.command.to_string()),
  )?;

  probe_protocol(
    "formatter",
    language
      .formatter
      .as_ref()
      .map(|formatter| formatter.command.to_string()),
  )?;

  probe_parser(language.grammar.as_deref().unwrap_or(&language.language_id))?;

  for feature in TsFeature::all() {
    probe_treesitter_feature(lang, *feature)?;
  }

  Ok(())
}

fn probe_parser(grammar_name: &str) -> io::Result<()> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  write!(stdout, "Tree-sitter parser: ")?;

  match grammar::get_language(grammar_name) {
    Ok(Some(_)) => writeln!(stdout, "{}", "✓".green())?,
    Ok(None) => writeln!(stdout, "{}", "None".yellow())?,
    Err(err) => writeln!(stdout, "{}", err.to_string().red())?,
  }

  Ok(())
}

fn probe_protocols<'a, I>(protocol_name: &str, server_cmds: I) -> io::Result<()>
where
  I: Iterator<Item = (&'a str, &'a str)> + 'a,
{
  let stdout = io::stdout();
  let mut stdout = stdout.lock();
  let mut server_cmds = server_cmds.peekable();

  write!(stdout, "Configured {}s:", protocol_name)?;
  if server_cmds.peek().is_none() {
    writeln!(stdout, "{}", " None".yellow())?;
    return Ok(());
  }
  writeln!(stdout)?;

  for (name, cmd) in server_cmds {
    let (diagnostic, icon) = match the_editor_stdx::env::which(cmd) {
      Ok(path) => (path.display().to_string().green(), "✓".green()),
      Err(_) => (format!("'{}' not found in $PATH", cmd).red(), "✘".red()),
    };
    writeln!(stdout, "  {} {}: {}", icon, name, diagnostic)?;
  }

  Ok(())
}

fn probe_protocol(protocol_name: &str, server_cmd: Option<String>) -> io::Result<()> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  write!(stdout, "Configured {}:", protocol_name)?;
  let Some(cmd) = server_cmd else {
    writeln!(stdout, "{}", " None".yellow())?;
    return Ok(());
  };
  writeln!(stdout)?;

  let (diagnostic, icon) = match the_editor_stdx::env::which(&cmd) {
    Ok(path) => (path.display().to_string().green(), "✓".green()),
    Err(_) => (format!("'{}' not found in $PATH", cmd).red(), "✘".red()),
  };
  writeln!(stdout, "  {} {}", icon, diagnostic)?;

  Ok(())
}

fn probe_treesitter_feature(lang: &str, feature: TsFeature) -> io::Result<()> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  let found = if load_runtime_file(lang, feature.runtime_filename()).is_ok() {
    "✓".green()
  } else {
    "✘".red()
  };

  writeln!(stdout, "{} queries: {}", feature.short_title(), found)?;

  Ok(())
}
