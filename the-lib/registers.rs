use std::collections::BTreeMap;

use crate::Tendril;

/// Pure register storage keyed by a single character.
///
/// This module intentionally models only ordinary register storage semantics.
/// Clipboard integration, document-derived virtual registers, and other special
/// register policies belong above this layer.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Registers {
  inner: BTreeMap<char, Vec<Tendril>>,
}

impl Registers {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn len(&self) -> usize {
    self.inner.len()
  }

  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }

  pub fn contains(&self, name: char) -> bool {
    self.inner.contains_key(&name)
  }

  pub fn read(&self, name: char) -> Option<&[Tendril]> {
    self.inner.get(&name).map(Vec::as_slice)
  }

  pub fn first(&self, name: char) -> Option<&Tendril> {
    self.read(name).and_then(|values| values.first())
  }

  pub fn last(&self, name: char) -> Option<&Tendril> {
    self.read(name).and_then(|values| values.last())
  }

  pub fn write(&mut self, name: char, values: Vec<Tendril>) -> Option<Vec<Tendril>> {
    self.inner.insert(name, values)
  }

  pub fn push(&mut self, name: char, value: Tendril) {
    self.inner.entry(name).or_default().push(value);
  }

  pub fn remove(&mut self, name: char) -> Option<Vec<Tendril>> {
    self.inner.remove(&name)
  }

  pub fn clear(&mut self) {
    self.inner.clear();
  }

  pub fn iter(&self) -> impl Iterator<Item = (char, &[Tendril])> {
    self
      .inner
      .iter()
      .map(|(name, values)| (*name, values.as_slice()))
  }
}

#[cfg(test)]
mod tests {
  use super::Registers;
  use crate::Tendril;

  fn tendril(text: &str) -> Tendril {
    text.into()
  }

  #[test]
  fn new_registers_are_empty() {
    let registers = Registers::new();

    assert!(registers.is_empty());
    assert_eq!(registers.len(), 0);
    assert_eq!(registers.read('a'), None);
  }

  #[test]
  fn write_and_read_preserve_fragment_order() {
    let mut registers = Registers::new();

    registers.write('a', vec![tendril("first"), tendril("second")]);

    assert_eq!(registers.read('a').unwrap(), &[
      tendril("first"),
      tendril("second")
    ]);
    assert_eq!(registers.first('a').unwrap(), "first");
    assert_eq!(registers.last('a').unwrap(), "second");
  }

  #[test]
  fn push_appends_to_existing_register() {
    let mut registers = Registers::new();

    registers.write('a', vec![tendril("first")]);
    registers.push('a', tendril("second"));
    registers.push('a', tendril("third"));

    assert_eq!(registers.read('a').unwrap(), &[
      tendril("first"),
      tendril("second"),
      tendril("third")
    ]);
  }

  #[test]
  fn write_returns_previous_value() {
    let mut registers = Registers::new();

    assert_eq!(registers.write('a', vec![tendril("first")]), None);
    assert_eq!(
      registers.write('a', vec![tendril("second")]),
      Some(vec![tendril("first")])
    );
    assert_eq!(registers.read('a').unwrap(), &[tendril("second")]);
  }

  #[test]
  fn remove_and_clear_drop_stored_registers() {
    let mut registers = Registers::new();

    registers.write('a', vec![tendril("one")]);
    registers.write('b', vec![tendril("two")]);

    assert_eq!(registers.remove('a'), Some(vec![tendril("one")]));
    assert!(!registers.contains('a'));
    assert!(registers.contains('b'));

    registers.clear();
    assert!(registers.is_empty());
  }

  #[test]
  fn iterates_in_key_order() {
    let mut registers = Registers::new();

    registers.write('z', vec![tendril("last")]);
    registers.write('a', vec![tendril("first")]);
    registers.write('m', vec![tendril("middle")]);

    let names = registers.iter().map(|(name, _)| name).collect::<Vec<_>>();
    assert_eq!(names, vec!['a', 'm', 'z']);
  }
}
