use std::{
  any::{
    Any,
    TypeId,
    type_name,
  },
  collections::HashMap,
};

struct ExtensionStateSlot {
  type_name: &'static str,
  value:     Box<dyn Any>,
}

#[derive(Default)]
pub struct ExtensionStateStore {
  slots: HashMap<TypeId, ExtensionStateSlot>,
}

impl std::fmt::Debug for ExtensionStateStore {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut type_names = self.type_names();
    type_names.sort_unstable();
    f.debug_struct("ExtensionStateStore")
      .field("type_names", &type_names)
      .finish()
  }
}

impl ExtensionStateStore {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn is_empty(&self) -> bool {
    self.slots.is_empty()
  }

  pub fn len(&self) -> usize {
    self.slots.len()
  }

  pub fn type_names(&self) -> Vec<&'static str> {
    self.slots.values().map(|slot| slot.type_name).collect()
  }

  pub fn contains<T>(&self) -> bool
  where
    T: 'static,
  {
    self.slots.contains_key(&TypeId::of::<T>())
  }

  pub fn insert<T>(&mut self, value: T) -> Option<T>
  where
    T: 'static,
  {
    let previous = self.slots.insert(TypeId::of::<T>(), ExtensionStateSlot {
      type_name: type_name::<T>(),
      value:     Box::new(value),
    })?;
    previous.value.downcast::<T>().ok().map(|boxed| *boxed)
  }

  pub fn get<T>(&self) -> Option<&T>
  where
    T: 'static,
  {
    self
      .slots
      .get(&TypeId::of::<T>())
      .and_then(|slot| slot.value.downcast_ref::<T>())
  }

  pub fn get_mut<T>(&mut self) -> Option<&mut T>
  where
    T: 'static,
  {
    self
      .slots
      .get_mut(&TypeId::of::<T>())
      .and_then(|slot| slot.value.downcast_mut::<T>())
  }

  pub fn get_or_insert_with<T, F>(&mut self, init: F) -> &mut T
  where
    T: 'static,
    F: FnOnce() -> T,
  {
    let type_id = TypeId::of::<T>();
    let slot = self.slots.entry(type_id).or_insert_with(|| {
      ExtensionStateSlot {
        type_name: type_name::<T>(),
        value:     Box::new(init()),
      }
    });
    slot
      .value
      .downcast_mut::<T>()
      .expect("extension state slot should match requested type")
  }

  pub fn get_or_default<T>(&mut self) -> &mut T
  where
    T: Default + 'static,
  {
    self.get_or_insert_with(T::default)
  }

  pub fn remove<T>(&mut self) -> Option<T>
  where
    T: 'static,
  {
    let slot = self.slots.remove(&TypeId::of::<T>())?;
    slot.value.downcast::<T>().ok().map(|boxed| *boxed)
  }
}

#[cfg(test)]
mod tests {
  use super::ExtensionStateStore;

  #[derive(Debug, PartialEq, Eq)]
  struct CounterState(usize);

  #[test]
  fn extension_state_store_round_trips_typed_values() {
    let mut store = ExtensionStateStore::new();
    assert!(store.is_empty());

    assert_eq!(store.insert(CounterState(2)), None);
    assert!(store.contains::<CounterState>());
    assert_eq!(store.get::<CounterState>(), Some(&CounterState(2)));

    store.get_mut::<CounterState>().unwrap().0 += 1;
    assert_eq!(store.remove::<CounterState>(), Some(CounterState(3)));
    assert!(!store.contains::<CounterState>());
  }

  #[test]
  fn extension_state_store_get_or_default_initializes_once() {
    let mut store = ExtensionStateStore::new();

    let counter = store.get_or_default::<CounterState>();
    assert_eq!(counter.0, 0);
    counter.0 = 9;

    let counter = store.get_or_default::<CounterState>();
    assert_eq!(counter.0, 9);
  }

  impl Default for CounterState {
    fn default() -> Self {
      Self(0)
    }
  }
}
