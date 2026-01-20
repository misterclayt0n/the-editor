use std::{
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
};

use eyre::Result;
use thiserror::Error;

/// A generic pointer to a file location.
///
/// Currently this type only supports paths to local files.
///
/// Cloning this type is cheap: the internal representation uses an Arc.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Uri {
  File(Arc<Path>),
}

impl Uri {
  pub fn to_url(&self) -> Result<url::Url, UrlConversionError> {
    match self {
      Uri::File(path) => {
        url::Url::from_file_path(path)
          .map_err(|()| UrlConversionError::PathToUrlFailed(Arc::clone(path)))
      },
    }
  }

  pub fn as_path(&self) -> Option<&Path> {
    match self {
      Self::File(path) => Some(path),
    }
  }
}

impl From<PathBuf> for Uri {
  fn from(path: PathBuf) -> Self {
    Self::File(path.into())
  }
}

impl std::fmt::Display for Uri {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::File(path) => write!(f, "{}", path.display()),
    }
  }
}

#[derive(Debug, Error)]
pub enum UrlConversionError {
  #[error("unsupported scheme '{scheme}' in URL {url}")]
  UnsupportedScheme { scheme: String, url: url::Url },

  #[error("unable to convert URL to file path: {0}")]
  UnableToConvert(url::Url),

  #[error("unable to convert path to URL: {}", .0.display())]
  PathToUrlFailed(Arc<Path>),
}

fn convert_url_to_uri(url: &url::Url) -> Result<Uri, UrlConversionError> {
  if url.scheme() == "file" {
    url
      .to_file_path()
      .map(|path| Uri::File(the_stdx::path::normalize(path).into()))
      .map_err(|_| UrlConversionError::UnableToConvert(url.clone()))
  } else {
    Err(UrlConversionError::UnsupportedScheme {
      scheme: url.scheme().to_string(),
      url:    url.clone(),
    })
  }
}

impl TryFrom<url::Url> for Uri {
  type Error = UrlConversionError;

  fn try_from(url: url::Url) -> Result<Self, Self::Error> {
    convert_url_to_uri(&url)
  }
}

impl TryFrom<&url::Url> for Uri {
  type Error = UrlConversionError;

  fn try_from(url: &url::Url) -> Result<Self, Self::Error> {
    convert_url_to_uri(url)
  }
}

#[cfg(test)]
mod test {
  use url::Url;

  use super::*;

  #[test]
  fn unknown_scheme() {
    let url = Url::parse("csharp:/metadata/foo/bar/Baz.cs").unwrap();
    assert!(matches!(
      Uri::try_from(url),
      Err(UrlConversionError::UnsupportedScheme { .. })
    ));
  }
}
