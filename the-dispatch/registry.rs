use std::{
  any::Any,
  collections::HashMap,
  sync::Arc,
};

/// Type-erased value used for dynamic handler inputs and outputs.
pub type DynValue = Box<dyn Any + Send + Sync>;

/// Dynamic handler type stored in the registry.
///
/// Accepts a type-erased input and returns a type-erased output.
pub type DynHandler<Ctx> = Arc<dyn Fn(&mut Ctx, DynValue) -> DynValue + Send + Sync>;

/// String-keyed registry for dynamic handlers.
pub struct DispatchRegistry<Ctx> {
  handlers: HashMap<&'static str, DynHandler<Ctx>>,
}

impl<Ctx> Clone for DispatchRegistry<Ctx> {
  fn clone(&self) -> Self {
    Self {
      handlers: self.handlers.clone(),
    }
  }
}

impl<Ctx> Default for DispatchRegistry<Ctx> {
  fn default() -> Self {
    Self::new()
  }
}

impl<Ctx> DispatchRegistry<Ctx> {
  /// Create an empty registry.
  pub fn new() -> Self {
    Self {
      handlers: HashMap::new(),
    }
  }

  /// Set or replace the handler for the given name.
  pub fn set(&mut self, name: &'static str, handler: DynHandler<Ctx>) {
    self.handlers.insert(name, handler);
  }

  /// Get a handler by name.
  pub fn get(&self, name: &'static str) -> Option<&DynHandler<Ctx>> {
    self.handlers.get(name)
  }

  /// Remove a handler by name and return it if present.
  pub fn remove(&mut self, name: &'static str) -> Option<DynHandler<Ctx>> {
    self.handlers.remove(name)
  }
}
