use std::{
  borrow::Borrow,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
};

use filesentry::{
  Events,
  ShutdownOnDrop,
};
use ignore::gitignore::{
  Gitignore,
  GitignoreBuilder,
};
use serde::{
  Deserialize,
  Serialize,
};
use the_editor_event::events;

events! {
  FileSystemDidChange {
    fs_events: Events
  }
}

/// Config for file watching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Config {
  pub enable:            bool,
  pub watch_vcs:         bool,
  pub require_workspace: bool,
  pub hidden:            bool,
  pub ignore:            bool,
  pub git_ignore:        bool,
  pub git_global:        bool,
  pub max_depth:         Option<usize>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      enable:            true,
      watch_vcs:         true,
      require_workspace: true,
      hidden:            true,
      ignore:            true,
      git_ignore:        true,
      git_global:        true,
      max_depth:         Some(10),
    }
  }
}

pub struct Watcher {
  watcher: Option<(filesentry::Watcher, ShutdownOnDrop)>,
  filter:  Arc<WatchFilter>,
  roots:   Vec<(PathBuf, usize)>,
  config:  Config,
}

impl Watcher {
  pub fn new(config: &Config) -> Self {
    let mut watcher = Watcher {
      watcher: None,
      filter:  Arc::new(WatchFilter {
        filesentry_ignores: Gitignore::empty(),
        ignore_files:       Vec::new(),
        global_ignores:     Vec::new(),
        hidden:             true,
        watch_vcs:          true,
      }),
      roots:   Vec::new(),
      config:  config.clone(),
    };

    watcher.reload(config);
    watcher
  }
}

struct IgnoreFiles {
  root:    PathBuf,
  ignores: Vec<Arc<Gitignore>>,
}

impl IgnoreFiles {
  fn new(
    workspace_ignore: Option<Arc<Gitignore>>,
    config: &Config,
    root: &Path,
    globals: &[Arc<Gitignore>],
  ) -> Self {
    let filenames: &[&str] = match (config.ignore, config.git_ignore) {
      (true, true) => &[".gitignore", ".ignore"],
      (true, false) => &[".ignore"],
      (false, true) => &[".gitignore"],
      _ => &[],
    };

    let mut ignores = Vec::with_capacity(8);
    if let Some(ws) = workspace_ignore {
      ignores.push(ws)
    }

    for ancestor in root.ancestors() {
      if filenames.is_empty() {
        break;
      }

      let paths = filenames.iter().map(|name| ancestor.join(name));

      if let Some(ignore) = build_ignore(paths, ancestor) {
        ignores.push(Arc::new(ignore));
      }
    }

    ignores.extend(globals.iter().cloned());

    Self {
      root: root.into(),
      ignores,
    }
  }

  fn shared_ignores(
    workspace: &Path,
    config: &Config,
  ) -> (Vec<Arc<Gitignore>>, Option<Arc<Gitignore>>) {
    let mut ignores = Vec::new();
    let workspace_ignores = build_ignore(
      [
        the_editor_loader::config_dir().join("ignore"),
        workspace.join(".the-editor/ignore"),
      ],
      workspace,
    )
    .map(Arc::new);

    if config.git_global {
      let (gi, err) = Gitignore::global();

      if let Some(err) = err.filter(|e| !e.is_io()) {
        log::error!("failed to read global global ignorefile: {err}");
      }

      if !gi.is_empty() {
        ignores.push(Arc::new(gi));
      }
    }

    // TODO
    // if config.git_exclude { }

    (ignores, workspace_ignores)
  }

  fn filesentry_ignores(workspace: &Path) -> Gitignore {
    // Second path takes priority here.
    build_ignore(
      [
        the_editor_loader::config_dir().join("filesentryignore"),
        workspace.join(".the-editor/filesentryignore"),
      ],
      workspace,
    )
    .unwrap_or(Gitignore::empty())
  }

  fn is_ignored(
    ignores: &[impl Borrow<Gitignore>],
    path: &Path,
    is_dir: Option<bool>,
  ) -> Option<bool> {
    match is_dir {
      Some(is_dir) => {
        for ignore in ignores {
          match ignore.borrow().matched(path, is_dir) {
            ignore::Match::None => continue,
            ignore::Match::Ignore(_) => return Some(true),
            ignore::Match::Whitelist(_) => return Some(false),
          }
        }
      },

      None => {
        // If we don't know whether this is a directory (on windows)
        // then we are conservative and allow the dirs.
        for ignore in ignores {
          match ignore.borrow().matched(path, true) {
            ignore::Match::None => continue,
            ignore::Match::Ignore(glob) => {
              if glob.is_only_dir() {
                match ignore.borrow().matched(path, false) {
                  ignore::Match::None => continue,
                  ignore::Match::Ignore(_) => return Some(true),
                  ignore::Match::Whitelist(_) => return Some(false),
                }
              } else {
                return Some(true);
              }
            },

            ignore::Match::Whitelist(_) => return Some(false),
          }
        }
      },
    }

    None
  }
}

/// A filter to ignore hidden/ignored files. The point of this
/// is to avoid  overwhelming the watcher with a ton of files/directories
/// ("target/", "node_modules/", etc.). So, this is very much a perf
/// optimization.
///
/// By default we ignore ignored.
pub struct WatchFilter {
  filesentry_ignores: Gitignore,
  ignore_files:       Vec<IgnoreFiles>,
  global_ignores:     Vec<Arc<Gitignore>>,
  hidden:             bool,
  watch_vcs:          bool,
}

impl WatchFilter {
  fn new<'a>(
    config: &Config,
    workspace: &'a Path,
    roots: impl Iterator<Item = &'a Path> + Clone,
  ) -> Self {
    let filesentry_ignores = IgnoreFiles::filesentry_ignores(workspace);
    let (global_ignores, workspace_ignore) = IgnoreFiles::shared_ignores(workspace, config);

    let ignore_files = roots
      .chain([workspace])
      .map(|root| IgnoreFiles::new(workspace_ignore.clone(), config, root, &global_ignores))
      .collect();

    Self {
      filesentry_ignores,
      ignore_files,
      global_ignores,
      hidden: config.hidden,
      watch_vcs: config.watch_vcs,
    }
  }
}

// Helpers

fn build_ignore(paths: impl IntoIterator<Item = PathBuf> + Clone, dir: &Path) -> Option<Gitignore> {
  let mut builder = GitignoreBuilder::new(dir);
  for path in paths.clone() {
    if let Some(err) = builder.add(&path)
      && !err.is_io()
    {
      log::error!("failed to read ignorefile at {path:?}: {err}")
    }
  }

  match builder.build() {
    Ok(ignore) => (!ignore.is_empty()).then_some(ignore),
    Err(err) => {
      if !err.is_io() {
        log::error!(
          "failed to read ignorefile at {:?}: {err}",
          paths.into_iter().collect::<Vec<_>>()
        );
      }

      None
    },
  }
}
