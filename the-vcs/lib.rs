//! `the-vcs` provides types for working with diffs from a Version Control
//! System (VCS). Currently `git` and `jj` providers are supported for diffs.

use std::{
  cell::RefCell,
  env,
  fs::OpenOptions,
  io::Write,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
  thread,
  time::Instant,
};

use arc_swap::ArcSwap;
use eyre::{
  Result,
  bail,
  eyre,
};

#[cfg(feature = "git")] mod git;

#[cfg(feature = "jj")] mod jj;

#[cfg(feature = "diff")] mod diff;

#[cfg(feature = "diff")]
pub use diff::{
  DiffHandle,
  DiffSignKind,
  Hunk,
};

mod status;

pub use status::FileChange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcsStatuslineInfo {
  Jj {
    description: String,
    bookmark:    Option<String>,
  },
  Git {
    branch: String,
  },
}

impl VcsStatuslineInfo {
  pub fn statusline_text(&self) -> String {
    match self {
      Self::Jj {
        description,
        bookmark,
      } => {
        match bookmark {
          Some(bookmark) if !bookmark.is_empty() => {
            format!("{description} · {bookmark}")
          },
          _ => description.clone(),
        }
      },
      Self::Git { branch } => branch.clone(),
    }
  }
}

/// Contains all active diff providers. Diff providers are compiled in via
/// features.
#[derive(Clone)]
pub struct DiffProviderRegistry {
  providers: Vec<DiffProvider>,
}

impl DiffProviderRegistry {
  /// Get the given file from the VCS. This provides the unedited document as a
  /// "base" for a diff to be created.
  pub fn get_diff_base(&self, file: &Path) -> Option<Vec<u8>> {
    self.providers.iter().find_map(|provider| {
      match provider.get_diff_base(file) {
        Ok(res) => Some(res),
        Err(err) => {
          log::debug!("{err:#?}");
          log::debug!("failed to open diff base for {}", file.display());
          None
        },
      }
    })
  }

  /// Get the current name of the current [HEAD](https://stackoverflow.com/questions/2304087/what-is-head-in-git).
  pub fn get_current_head_name(&self, file: &Path) -> Option<Arc<ArcSwap<Box<str>>>> {
    self.providers.iter().find_map(|provider| {
      match provider.get_current_head_name(file) {
        Ok(res) => Some(res),
        Err(err) => {
          log::debug!("{err:#?}");
          log::debug!("failed to obtain current head name for {}", file.display());
          None
        },
      }
    })
  }

  /// Get statusline metadata for the active VCS provider.
  pub fn get_statusline_info(&self, file: &Path) -> Option<VcsStatuslineInfo> {
    self.providers.iter().find_map(|provider| {
      match provider.get_statusline_info(file) {
        Ok(res) => Some(res),
        Err(err) => {
          log::debug!("{err:#?}");
          log::debug!(
            "failed to obtain vcs statusline info for {}",
            file.display()
          );
          None
        },
      }
    })
  }

  /// Fire-and-forget changed file iteration. Runs everything in a background
  /// task. Keeps iteration until `on_change` returns `false`.
  pub fn for_each_changed_file(
    self,
    cwd: PathBuf,
    f: impl Fn(Result<FileChange>) -> bool + Send + 'static,
  ) {
    thread::spawn(move || {
      if self
        .providers
        .iter()
        .find_map(|provider| provider.for_each_changed_file(&cwd, &f).ok())
        .is_none()
      {
        f(Err(eyre!("no diff provider returns success")));
      }
    });
  }

  /// Collect changed files synchronously from the first provider that succeeds.
  pub fn collect_changed_files(&self, cwd: &Path) -> Result<Vec<FileChange>> {
    for provider in &self.providers {
      let changes = RefCell::new(Vec::new());
      let collect_start = Instant::now();
      let provider_result = provider.for_each_changed_file(cwd, |entry| {
        match entry {
          Ok(change) => {
            changes.borrow_mut().push(change);
            true
          },
          Err(err) => {
            log::debug!("{err:#?}");
            false
          },
        }
      });

      match provider_result {
        Ok(()) => {
          let changes = changes.into_inner();
          log_vcs_collect_event(
            provider.label(),
            cwd,
            collect_start.elapsed().as_secs_f64() * 1000.0,
            changes.len(),
          );
          return Ok(changes);
        },
        Err(err) => {
          log::debug!("{err:#?}");
        },
      }
    }

    bail!("no diff provider returns success")
  }
}

impl Default for DiffProviderRegistry {
  fn default() -> Self {
    // Keep a deterministic provider order. Prefer jj in colocated jj+git repos.
    let mut providers = Vec::new();
    #[cfg(feature = "jj")]
    if jj::is_available() {
      providers.push(DiffProvider::Jj);
    }
    #[cfg(feature = "git")]
    providers.push(DiffProvider::Git);
    providers.push(DiffProvider::None);
    DiffProviderRegistry { providers }
  }
}

/// A union type that includes all types that implement [DiffProvider]. We need
/// this type to allow cloning [DiffProviderRegistry] as `Clone` cannot be used
/// in trait objects.
///
/// `Copy` is simply to ensure the `clone()` call is the simplest it can be.
#[derive(Copy, Clone)]
enum DiffProvider {
  #[cfg(feature = "git")]
  Git,
  #[cfg(feature = "jj")]
  Jj,
  None,
}

impl DiffProvider {
  const fn label(self) -> &'static str {
    match self {
      #[cfg(feature = "git")]
      Self::Git => "git",
      #[cfg(feature = "jj")]
      Self::Jj => "jj",
      Self::None => "none",
    }
  }

  fn get_diff_base(&self, _file: &Path) -> Result<Vec<u8>> {
    match self {
      #[cfg(feature = "git")]
      Self::Git => git::get_diff_base(_file),
      #[cfg(feature = "jj")]
      Self::Jj => jj::get_diff_base(_file),
      Self::None => bail!("No diff support compiled in"),
    }
  }

  fn get_current_head_name(&self, _file: &Path) -> Result<Arc<ArcSwap<Box<str>>>> {
    match self {
      #[cfg(feature = "git")]
      Self::Git => git::get_current_head_name(_file),
      #[cfg(feature = "jj")]
      Self::Jj => jj::get_current_head_name(_file),
      Self::None => bail!("No diff support compiled in"),
    }
  }

  fn get_statusline_info(&self, _file: &Path) -> Result<VcsStatuslineInfo> {
    match self {
      #[cfg(feature = "git")]
      Self::Git => git::get_statusline_info(_file),
      #[cfg(feature = "jj")]
      Self::Jj => jj::get_statusline_info(_file),
      Self::None => bail!("No diff support compiled in"),
    }
  }

  fn for_each_changed_file(
    &self,
    _cwd: &Path,
    _f: impl Fn(Result<FileChange>) -> bool,
  ) -> Result<()> {
    match self {
      #[cfg(feature = "git")]
      Self::Git => git::for_each_changed_file(_cwd, _f),
      #[cfg(feature = "jj")]
      Self::Jj => jj::for_each_changed_file(_cwd, _f),
      Self::None => bail!("No diff support compiled in"),
    }
  }
}

fn log_vcs_collect_event(provider: &str, cwd: &Path, command_ms: f64, change_count: usize) {
  if env::var("THE_TERM_DEBUG_RENDER_PERF").ok().as_deref() != Some("1") {
    return;
  }
  let Some(path) = env::var("THE_TERM_DEBUG_RENDER_PERF_FILE")
    .ok()
    .map(|raw| raw.trim().to_string())
    .filter(|raw| !raw.is_empty())
    .map(PathBuf::from)
  else {
    return;
  };
  if let Some(parent) = path.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  let line = format!(
    "kind=vcs_collect provider={} cwd={} command={:.2}ms changes={}\n",
    provider,
    cwd.display(),
    command_ms,
    change_count,
  );
  if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
    let _ = file.write_all(line.as_bytes());
  }
}
