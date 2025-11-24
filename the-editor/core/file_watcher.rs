use core::slice;
use std::{
  borrow::Borrow,
  mem::replace,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
};

use filesentry::{
  Events,
  Filter,
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
use the_editor_event::{
  dispatch,
  events,
};

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

  pub fn reload(&mut self, config: &Config) {
    let old_config = replace(&mut self.config, config.clone());
    let (workspace, no_workspace) = the_editor_loader::find_workspace();

    if !config.enable || config.require_workspace && no_workspace {
      self.watcher = None;
      return;
    }

    self.filter = Arc::new(WatchFilter::new(
      config,
      &workspace,
      self.roots.iter().map(|(it, _)| &**it),
    ));

    let watcher = match &mut self.watcher {
      Some((watcher, _)) => {
        // TODO: more fine grained detection of when recrawl is nedded
        watcher.set_filter(self.filter.clone(), old_config != self.config);
        watcher
      },
      None => {
        match filesentry::Watcher::new() {
          Ok(watcher) => {
            watcher.set_filter(self.filter.clone(), false);
            watcher.add_handler(move |events| {
              dispatch(FileSystemDidChange { fs_events: events });
              true
            });
            let shutdown_guard = watcher.shutdown_guard();
            &mut self.watcher.insert((watcher, shutdown_guard)).0
          },
          Err(err) => {
            log::error!("failed to start file-watcher: {err}");
            return;
          },
        }
      },
    };

    if let Err(err) = watcher.add_root(&workspace, true, |_| ()) {
      log::error!("failed to start file-watcher: {err}");
    }

    for (root, _) in &self.roots {
      if let Err(err) = watcher.add_root(root, true, |_| ()) {
        log::error!("failed to start file-watcher: {err}");
      }
    }

    watcher.start();
  }

  pub fn remove_root(&mut self, root: PathBuf) {
    let i = self.roots.partition_point(|(it, _)| it < &root);

    if self.roots.get(i).is_none_or(|(it, _)| it != &root) {
      log::error!("tried to remove root {root:?} from watch list that does not exist!");
      return;
    }

    if self.roots[i].1 <= 1 {
      self.roots.remove(i);
    } else {
      self.roots[i].1 -= 1;
    }
  }

  pub fn is_tracking_root(&self, root: &Path) -> bool {
    self.roots.iter().any(|(it, _)| it == root)
  }

  pub fn add_root(&mut self, root: &Path) {
    let root = match root.canonicalize() {
      Ok(root) => root,
      Err(err) => {
        log::error!("failed to watch {root:?}: {err}");
        return;
      },
    };
    let i = self.roots.partition_point(|(it, _)| it < &root);

    if let Some((_, refcnt)) = self.roots.get_mut(i).filter(|(path, _)| path == &root) {
      *refcnt += 1;
      return;
    }

    if self.roots[..i]
      .iter()
      .rev()
      .find(|(it, _)| it.parent().is_none_or(|it| root.starts_with(it)))
      .is_some_and(|(it, _)| root.starts_with(it))
      && !self.filter.ignore_path_rec(&root, Some(true))
    {
      return;
    }
    let (workspace, _) = the_editor_loader::find_workspace();

    if root.starts_with(&workspace) {
      return;
    }

    self.roots.insert(i, (root.clone(), 1));
    self.filter = Arc::new(WatchFilter::new(
      &self.config,
      &workspace,
      self.roots.iter().map(|(it, _)| &**it),
    ));

    if let Some((watcher, _)) = &self.watcher {
      watcher.set_filter(self.filter.clone(), false);

      if let Err(err) = watcher.add_root(&root, true, |_| ()) {
        log::error!("failed to watch {root:?}: {err}");
      }
    }
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

  fn ignore_path_impl(
    &self,
    path: &Path,
    is_dir: Option<bool>,
    ignore_files: &[Arc<Gitignore>],
  ) -> bool {
    if let Some(ignore) =
      IgnoreFiles::is_ignored(slice::from_ref(&self.filesentry_ignores), path, is_dir)
    {
      return ignore;
    }

    if is_hardcoded_whitelist(path) {
      return false;
    }

    if is_hardcoded_blacklist(path, is_dir.unwrap_or(false)) {
      return true;
    }

    if let Some(ignore) = IgnoreFiles::is_ignored(ignore_files, path, is_dir) {
      return ignore;
    }

    // Ignore .git dircectory except .git/HEAD (and .git itself).
    if is_vcs_ignore(path, self.watch_vcs) {
      return true;
    }

    !self.hidden && is_hidden(path)
  }
}

impl filesentry::Filter for WatchFilter {
  fn ignore_path(&self, path: &Path, is_dir: Option<bool>) -> bool {
    let i = self
      .ignore_files
      .partition_point(|ignore_files| path < ignore_files.root);

    let (root, ignore_files) = self
      .ignore_files
      .get(i)
      .map_or((Path::new(""), &self.global_ignores), |files| {
        (&files.root, &files.ignores)
      });

    if path == root {
      return false;
    }

    self.ignore_path_impl(path, is_dir, ignore_files)
  }

  fn ignore_path_rec(&self, mut path: &Path, is_dir: Option<bool>) -> bool {
    let i = self
      .ignore_files
      .partition_point(|ignore_files| path < ignore_files.root);

    let (root, ignore_files) = self
      .ignore_files
      .get(i)
      .map_or((Path::new(""), &self.global_ignores), |files| {
        (&files.root, &files.ignores)
      });

    loop {
      if path == root {
        return false;
      }

      if self.ignore_path_impl(path, is_dir, ignore_files) {
        return true;
      }

      let Some(parent) = path.parent() else {
        break;
      };

      path = parent;
    }

    false
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

fn is_hidden(path: &Path) -> bool {
  path.file_name().is_some_and(|it| {
    it.as_encoded_bytes().first() == Some(&b'.')
        // Handled by vcs ignore rules.
        && it != ".git"
  })
}

// Hidden directories we want to watch by default.
fn is_hardcoded_whitelist(path: &Path) -> bool {
  path.ends_with(".the-editor")
    | path.ends_with(".github")
    | path.ends_with(".cargo")
    | path.ends_with(".envrc")
}

fn is_hardcoded_blacklist(path: &Path, is_dir: bool) -> bool {
  // Don't descend into the cargo regstiry and similar.
  path
    .parent()
    .is_some_and(|parent| parent.ends_with(".cargo"))
    && is_dir
}

fn file_name(path: &Path) -> Option<&str> {
  path.file_name().and_then(|it| it.to_str())
}

fn is_vcs_ignore(path: &Path, watch_vcs: bool) -> bool {
  // Ignore .git dircectory except .git/HEAD (and .git itself).
  if watch_vcs
    && path.parent().is_some_and(|it| it.ends_with(".git"))
    && !path.ends_with(".git/HEAD")
  {
    return true;
  }
  match file_name(path) {
    Some(".jj" | ".svn" | ".hg") => true,
    Some(".git") => !watch_vcs,
    _ => false,
  }
}
