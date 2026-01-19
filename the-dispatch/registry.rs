use std::any::Any;
use std::collections::HashMap;

pub type DynValue = Box<dyn Any + Send + Sync>;
pub type DynHandler<Ctx> = Box<dyn Fn(&mut Ctx, DynValue) -> DynValue + Send + Sync>;

pub struct DispatchRegistry<Ctx> {
  handlers: HashMap<&'static str, DynHandler<Ctx>>,
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
