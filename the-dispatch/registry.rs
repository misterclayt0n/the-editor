use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

pub type DynValue = Box<dyn Any + Send + Sync>;
pub type DynHandler<Ctx> = Arc<dyn Fn(&mut Ctx, DynValue) -> DynValue + Send + Sync>;

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

impl<Ctx> DispatchRegistry<Ctx> {
  pub fn new() -> Self {
    Self {
      handlers: HashMap::new(),
    }
  }

  pub fn set(&mut self, name: &'static str, handler: DynHandler<Ctx>) {
    self.handlers.insert(name, handler);
  }

  pub fn get(&self, name: &'static str) -> Option<&DynHandler<Ctx>> {
    self.handlers.get(name)
  }

  pub fn remove(&mut self, name: &'static str) -> Option<DynHandler<Ctx>> {
    self.handlers.remove(name)
  }
}
