use std::{
  fmt,
  path::{Path, PathBuf},
};

use anyhow::Result;
use clap::{ArgAction, Parser, ValueEnum};
use indexmap::IndexMap;

use crate::core::position::Position;

#[derive(Clone, Debug)]
pub struct CliOptions {
  pub display_version: bool,
  pub load_tutor: bool,
  pub fetch_grammars: bool,
  pub build_grammars: bool,
  pub health: bool,
  pub health_category: Option<String>,
  pub split: Option<SplitMode>,
  pub verbosity: u8,
  pub log_file: Option<PathBuf>,
  pub config_file: Option<PathBuf>,
  pub working_dir: Option<PathBuf>,
  pub files: IndexMap<PathBuf, Vec<Position>>,
}

impl CliOptions {
  pub fn parse() -> Result<Self> {
    let raw = RawCli::parse();
    raw.try_into()
  }
}

#[derive(Parser, Debug)]
#[command(
  name = "the-editor",
  about,
  long_about = None,
  version = the_editor_loader::VERSION_AND_GIT_HASH,
  disable_version_flag = true
)]
struct RawCli {
  /// Print version information
  #[arg(short = 'V', long = "version")]
  display_version: bool,

  /// Load the interactive tutor
  #[arg(long = "tutor")]
  load_tutor: bool,

  /// Run health diagnostics (optionally specifying a category)
  #[arg(long = "health", value_name = "CATEGORY", num_args = 0..=1)]
  health: Option<Option<String>>,

  /// Grammar maintenance tasks
  #[arg(short = 'g', long = "grammar", value_enum, value_name = "TASK")]
  grammar: Option<GrammarCommand>,

  /// Split all opened files vertically
  #[arg(long = "vsplit", conflicts_with = "hsplit")]
  vsplit: bool,

  /// Split all opened files horizontally
  #[arg(long = "hsplit")]
  hsplit: bool,

  /// Increase logging verbosity (repeat for more detail)
  #[arg(short = 'v', action = ArgAction::Count)]
  verbosity: u8,

  /// Save logs to a specific file
  #[arg(long = "log", value_name = "FILE", value_parser = parse_pathbuf)]
  log_file: Option<PathBuf>,

  /// Load configuration from a specific file
  #[arg(short = 'c', long = "config", value_name = "FILE", value_parser = parse_pathbuf)]
  config_file: Option<PathBuf>,

  /// Set the initial working directory
  #[arg(short = 'w', long = "working-dir", value_name = "PATH", value_parser = parse_working_dir)]
  working_dir: Option<PathBuf>,

  /// Files to open (optionally with :row[:col] suffix)
  #[arg(value_name = "files", trailing_var_arg = true)]
  inputs: Vec<String>,
}

#[derive(Copy, Clone, Debug)]
pub enum SplitMode {
  Vertical,
  Horizontal,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum GrammarCommand {
  Fetch,
  Build,
}

impl fmt::Display for GrammarCommand {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Fetch => write!(f, "fetch"),
      Self::Build => write!(f, "build"),
    }
  }
}

impl TryFrom<RawCli> for CliOptions {
  type Error = anyhow::Error;

  fn try_from(raw: RawCli) -> Result<Self> {
    let (health, health_category) = match raw.health {
      Some(category) => (true, category.filter(|s| !s.is_empty())),
      None => (false, None),
    };

    let (fetch_grammars, build_grammars) = match raw.grammar {
      Some(GrammarCommand::Fetch) => (true, false),
      Some(GrammarCommand::Build) => (false, true),
      None => (false, false),
    };

    let split = if raw.vsplit {
      Some(SplitMode::Vertical)
    } else if raw.hsplit {
      Some(SplitMode::Horizontal)
    } else {
      None
    };

    let mut files = IndexMap::new();
    let mut line_override: Option<usize> = None;

    for input in raw.inputs {
      if let Some(line) = parse_line_override(&input) {
        line_override = Some(line);
        continue;
      }
      insert_file_argument(&mut files, &input);
    }

    if let Some(line) = line_override {
      if let Some((_path, positions)) = files.first_mut() {
        if let Some(first) = positions.first_mut() {
          first.row = line;
        } else {
          positions.push(Position::new(line, 0));
        }
      }
    }

    Ok(Self {
      display_version: raw.display_version,
      load_tutor: raw.load_tutor,
      fetch_grammars,
      build_grammars,
      health,
      health_category,
      split,
      verbosity: raw.verbosity,
      log_file: raw.log_file,
      config_file: raw.config_file,
      working_dir: raw.working_dir,
      files,
    })
  }
}

fn parse_pathbuf(value: &str) -> std::result::Result<PathBuf, String> {
  Ok(the_editor_stdx::path::canonicalize(value))
}

fn parse_working_dir(value: &str) -> std::result::Result<PathBuf, String> {
  let path = the_editor_stdx::path::canonicalize(value);
  if path.is_dir() {
    Ok(path)
  } else {
    Err(format!(
      "working directory '{value}' does not exist or is not a directory"
    ))
  }
}

fn parse_line_override(value: &str) -> Option<usize> {
  let stripped = value.strip_prefix('+')?;
  if stripped.is_empty() {
    return None;
  }

  stripped
    .parse::<usize>()
    .ok()
    .map(|line| line.saturating_sub(1))
}

fn insert_file_argument(files: &mut IndexMap<PathBuf, Vec<Position>>, argument: &str) {
  let (path, position) = parse_file(argument);
  let path = the_editor_stdx::path::canonicalize(path);
  files
    .entry(path)
    .and_modify(|positions| positions.push(position))
    .or_insert_with(|| vec![position]);
}

fn parse_file(argument: &str) -> (PathBuf, Position) {
  let default_position = || (PathBuf::from(argument), Position::default());

  if Path::new(argument).exists() {
    return default_position();
  }

  split_path_row_col(argument)
    .or_else(|| split_path_row(argument))
    .unwrap_or_else(default_position)
}

fn split_path_row_col(argument: &str) -> Option<(PathBuf, Position)> {
  let mut parts = argument.trim_end_matches(':').rsplitn(3, ':');
  let col: usize = parts.next()?.parse().ok()?;
  let row: usize = parts.next()?.parse().ok()?;
  let path = parts.next()?.into();
  let position = Position::new(row.saturating_sub(1), col.saturating_sub(1));
  Some((path, position))
}

fn split_path_row(argument: &str) -> Option<(PathBuf, Position)> {
  let (path, row) = argument.trim_end_matches(':').rsplit_once(':')?;
  let row: usize = row.parse().ok()?;
  let position = Position::new(row.saturating_sub(1), 0);
  Some((path.into(), position))
}
