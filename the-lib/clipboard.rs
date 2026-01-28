//! Clipboard abstraction for `the-lib`.
//!
//! The lib only defines the interface and error types. Runtime hosts should
//! provide concrete implementations (see `the-runtime`).

use std::borrow::Cow;

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardType {
  Clipboard,
  Selection,
}

#[derive(Debug, Error)]
pub enum ClipboardError {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error("could not convert provider output to UTF-8: {0}")]
  FromUtf8(#[from] std::string::FromUtf8Error),
  #[error("clipboard provider command failed")]
  CommandFailed,
  #[error("failed to write to clipboard provider's stdin")]
  StdinWriteFailed,
  #[error("clipboard provider did not return any contents")]
  MissingStdout,
  #[error("clipboard provider does not support reading")]
  ReadingNotSupported,
  #[error("clipboard error: {0}")]
  Platform(String),
}

pub type Result<T> = std::result::Result<T, ClipboardError>;

pub trait ClipboardProvider: Send + Sync {
  fn name(&self) -> Cow<'_, str>;
  fn get_contents(&self, clipboard_type: ClipboardType) -> Result<String>;
  fn set_contents(&self, content: &str, clipboard_type: ClipboardType) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct NoClipboard;

impl ClipboardProvider for NoClipboard {
  fn name(&self) -> Cow<'_, str> {
    "none".into()
  }

  fn get_contents(&self, _clipboard_type: ClipboardType) -> Result<String> {
    Err(ClipboardError::ReadingNotSupported)
  }

  fn set_contents(&self, _content: &str, _clipboard_type: ClipboardType) -> Result<()> {
    Ok(())
  }
}
