use std::{
  num::NonZeroU64,
  path::{
    Path,
    PathBuf,
  },
};

use thiserror::Error;

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct ResourceId(NonZeroU64);

impl ResourceId {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }

  pub(crate) fn next(counter: &mut u64) -> Self {
    *counter = counter.wrapping_add(1).max(1);
    Self::new(*counter).expect("resource id must be non-zero")
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
  path: PathBuf,
}

impl Resource {
  pub fn file(path: impl AsRef<Path>) -> std::io::Result<Self> {
    Ok(Self {
      path: normalize_file_path(path.as_ref())?,
    })
  }

  pub fn path(&self) -> &Path {
    &self.path
  }
}

#[derive(Debug, Error)]
pub enum ResourceError {
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

pub(crate) fn normalize_file_path(path: &Path) -> std::io::Result<PathBuf> {
  if path.exists() {
    return dunce::canonicalize(path);
  }

  if path.is_absolute() {
    Ok(path.to_path_buf())
  } else {
    Ok(std::env::current_dir()?.join(path))
  }
}
