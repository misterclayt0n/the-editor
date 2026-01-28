//! Register storage and clipboard integration.
//!
//! Registers are a small key-value store keyed by a `char`. Some registers have
//! special behavior (black hole, selection contents, clipboard, etc).

use std::{
  borrow::Cow,
  collections::HashMap,
  iter,
  sync::Arc,
};

use the_core::line_ending::NATIVE_LINE_ENDING;
use thiserror::Error;

use crate::{
  clipboard::{
    ClipboardError,
    ClipboardProvider,
    ClipboardType,
    NoClipboard,
  },
  document::Document,
};

#[derive(Debug, Error)]
pub enum RegisterError {
  #[error("register {0} does not support writing")]
  WriteNotSupported(char),
  #[error("register {0} does not support pushing")]
  PushNotSupported(char),
  #[error("clipboard contents do not match register {0}")]
  ClipboardMismatch(char),
  #[error(transparent)]
  Clipboard(#[from] ClipboardError),
}

pub type Result<T> = std::result::Result<T, RegisterError>;

/// A key-value store for saving sets of values.
///
/// Special registers:
/// - `_`: black hole (discard reads and writes)
/// - `#`: selection indices
/// - `.`: selection contents
/// - `%`: document display name
/// - `*`: primary clipboard
/// - `+`: system clipboard
pub struct Registers {
  inner:                    HashMap<char, Vec<String>>,
  clipboard_provider:       Arc<dyn ClipboardProvider>,
  pub last_search_register: char,
}

impl Registers {
  pub fn new() -> Self {
    Self::with_clipboard(Arc::new(NoClipboard))
  }

  pub fn with_clipboard(clipboard_provider: Arc<dyn ClipboardProvider>) -> Self {
    Self {
      inner: Default::default(),
      clipboard_provider,
      last_search_register: '/',
    }
  }

  pub fn set_clipboard_provider(&mut self, clipboard_provider: Arc<dyn ClipboardProvider>) {
    self.clipboard_provider = clipboard_provider;
  }

  pub fn clipboard_provider_name(&self) -> Cow<'_, str> {
    self.clipboard_provider.name()
  }

  pub fn read<'a>(&'a self, name: char, doc: &'a Document) -> Option<RegisterValues<'a>> {
    match name {
      '_' => Some(RegisterValues::new(iter::empty())),
      '#' => {
        let count = doc.selection().ranges().len();
        Some(RegisterValues::new(
          (0..count).map(|i| (i + 1).to_string().into()),
        ))
      },
      '.' => {
        let text = doc.text().slice(..);
        Some(RegisterValues::new(doc.selection().fragments(text)))
      },
      '%' => Some(RegisterValues::new(iter::once(doc.display_name()))),
      '*' | '+' => {
        Some(read_from_clipboard(
          &*self.clipboard_provider,
          self.inner.get(&name),
          match name {
            '+' => ClipboardType::Clipboard,
            '*' => ClipboardType::Selection,
            _ => unreachable!(),
          },
        ))
      },
      _ => {
        self
          .inner
          .get(&name)
          .map(|values| RegisterValues::new(values.iter().map(Cow::from).rev()))
      },
    }
  }

  pub fn write(&mut self, name: char, mut values: Vec<String>) -> Result<()> {
    match name {
      '_' => Ok(()),
      '#' | '.' | '%' => Err(RegisterError::WriteNotSupported(name)),
      '*' | '+' => {
        self.clipboard_provider.set_contents(
          &values.join(NATIVE_LINE_ENDING.as_str()),
          match name {
            '+' => ClipboardType::Clipboard,
            '*' => ClipboardType::Selection,
            _ => unreachable!(),
          },
        )?;
        values.reverse();
        self.inner.insert(name, values);
        Ok(())
      },
      _ => {
        values.reverse();
        self.inner.insert(name, values);
        Ok(())
      },
    }
  }

  pub fn push(&mut self, name: char, mut value: String) -> Result<()> {
    match name {
      '_' => Ok(()),
      '#' | '.' | '%' => Err(RegisterError::PushNotSupported(name)),
      '*' | '+' => {
        let clipboard_type = match name {
          '+' => ClipboardType::Clipboard,
          '*' => ClipboardType::Selection,
          _ => unreachable!(),
        };
        let contents = self.clipboard_provider.get_contents(clipboard_type)?;
        let saved_values = self.inner.entry(name).or_default();

        if !contents_are_saved(saved_values, &contents) {
          return Err(RegisterError::ClipboardMismatch(name));
        }

        saved_values.push(value.clone());
        if !contents.is_empty() {
          value.push_str(NATIVE_LINE_ENDING.as_str());
        }
        value.push_str(&contents);
        self
          .clipboard_provider
          .set_contents(&value, clipboard_type)?;

        Ok(())
      },
      _ => {
        self.inner.entry(name).or_default().push(value);
        Ok(())
      },
    }
  }

  pub fn first<'a>(&'a self, name: char, doc: &'a Document) -> Option<Cow<'a, str>> {
    self.read(name, doc).and_then(|mut values| values.next())
  }

  pub fn last<'a>(&'a self, name: char, doc: &'a Document) -> Option<Cow<'a, str>> {
    self
      .read(name, doc)
      .and_then(|mut values| values.next_back())
  }

  pub fn iter_preview(&self) -> impl Iterator<Item = (char, &str)> {
    self
      .inner
      .iter()
      .filter(|(name, _)| !matches!(name, '*' | '+'))
      .map(|(name, values)| {
        let preview = values
          .last()
          .and_then(|s| s.lines().next())
          .unwrap_or("<empty>");

        (*name, preview)
      })
      .chain(
        [
          ('_', "<empty>"),
          ('#', "<selection indices>"),
          ('.', "<selection contents>"),
          ('%', "<document name>"),
          ('+', "<system clipboard>"),
          ('*', "<primary clipboard>"),
        ]
        .iter()
        .copied(),
      )
  }

  pub fn clear(&mut self) {
    self.clear_clipboard(ClipboardType::Clipboard);
    self.clear_clipboard(ClipboardType::Selection);
    self.inner.clear()
  }

  pub fn remove(&mut self, name: char) -> bool {
    match name {
      '*' | '+' => {
        self.clear_clipboard(match name {
          '+' => ClipboardType::Clipboard,
          '*' => ClipboardType::Selection,
          _ => unreachable!(),
        });
        self.inner.remove(&name);

        true
      },
      '_' | '#' | '.' | '%' => false,
      _ => self.inner.remove(&name).is_some(),
    }
  }

  fn clear_clipboard(&mut self, clipboard_type: ClipboardType) {
    if let Err(err) = self.clipboard_provider.set_contents("", clipboard_type) {
      tracing::warn!(
        "failed to clear {} clipboard: {err}",
        match clipboard_type {
          ClipboardType::Clipboard => "system",
          ClipboardType::Selection => "primary",
        }
      )
    }
  }
}

fn read_from_clipboard<'a>(
  provider: &dyn ClipboardProvider,
  saved_values: Option<&'a Vec<String>>,
  clipboard_type: ClipboardType,
) -> RegisterValues<'a> {
  match provider.get_contents(clipboard_type) {
    Ok(contents) => {
      let Some(values) = saved_values else {
        return RegisterValues::new(iter::once(contents.into()));
      };

      if contents_are_saved(values, &contents) {
        RegisterValues::new(values.iter().map(Cow::from).rev())
      } else {
        RegisterValues::new(iter::once(contents.into()))
      }
    },
    Err(ClipboardError::ReadingNotSupported) => {
      match saved_values {
        Some(values) => RegisterValues::new(values.iter().map(Cow::from).rev()),
        None => RegisterValues::new(iter::empty()),
      }
    },
    Err(err) => {
      tracing::warn!("failed to read {} clipboard: {err}", match clipboard_type {
        ClipboardType::Clipboard => "system",
        ClipboardType::Selection => "primary",
      });
      RegisterValues::new(iter::empty())
    },
  }
}

fn contents_are_saved(saved_values: &[String], mut contents: &str) -> bool {
  let line_ending = NATIVE_LINE_ENDING.as_str();
  let mut values = saved_values.iter().rev();

  match values.next() {
    Some(first) if contents.starts_with(first) => {
      contents = &contents[first.len()..];
    },
    None if contents.is_empty() => return true,
    _ => return false,
  }

  for value in values {
    if contents.starts_with(line_ending) && contents[line_ending.len()..].starts_with(value) {
      contents = &contents[line_ending.len() + value.len()..];
    } else {
      return false;
    }
  }

  true
}

/// Iterator wrapper that is both double-ended and exact-size.
pub struct RegisterValues<'a> {
  iter: Box<dyn DoubleEndedExactSizeIterator<Item = Cow<'a, str>> + 'a>,
}

impl<'a> RegisterValues<'a> {
  fn new(
    iter: impl DoubleEndedIterator<Item = Cow<'a, str>> + ExactSizeIterator<Item = Cow<'a, str>> + 'a,
  ) -> Self {
    Self {
      iter: Box::new(iter),
    }
  }
}

impl<'a> Iterator for RegisterValues<'a> {
  type Item = Cow<'a, str>;

  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next()
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}

impl DoubleEndedIterator for RegisterValues<'_> {
  fn next_back(&mut self) -> Option<Self::Item> {
    self.iter.next_back()
  }
}

impl ExactSizeIterator for RegisterValues<'_> {
  fn len(&self) -> usize {
    self.iter.len()
  }
}

trait DoubleEndedExactSizeIterator: DoubleEndedIterator + ExactSizeIterator {}

impl<I: DoubleEndedIterator + ExactSizeIterator> DoubleEndedExactSizeIterator for I {}

#[cfg(test)]
mod tests {
  use std::num::NonZeroUsize;

  use ropey::Rope;

  use super::*;
  use crate::{
    document::DocumentId,
    selection::Selection,
  };

  #[test]
  fn read_special_registers() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(doc_id, Rope::from("hello"));
    let selection = Selection::single(0, doc.text().len_chars());
    doc.set_selection(selection).unwrap();
    let registers = Registers::new();

    let values = registers.read('#', &doc).unwrap().collect::<Vec<_>>();
    assert_eq!(values, vec![Cow::<str>::Owned("1".to_string())]);

    let values = registers.read('.', &doc).unwrap().collect::<Vec<_>>();
    assert_eq!(values, vec![Cow::Borrowed("hello")]);

    let values = registers.read('%', &doc).unwrap().collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
  }

  #[test]
  fn write_and_read_normal_register() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("hello"));
    let mut registers = Registers::new();

    registers
      .write('a', vec!["first".into(), "second".into()])
      .unwrap();
    let values = registers.read('a', &doc).unwrap().collect::<Vec<_>>();
    assert_eq!(values, vec![
      Cow::Borrowed("first"),
      Cow::Borrowed("second")
    ]);
  }
}
