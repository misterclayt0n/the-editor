//! Tests for the dynamic registry feature
//! Run with: cargo test -p the-dispatch --features dynamic-registry

#![cfg(feature = "dynamic-registry")]

use std::cell::RefCell;
use std::rc::Rc;

use the_dispatch::{DispatchRegistry, DynHandler, DynValue, define};

// Define a dispatch for testing registry
define! {
    App {
        on_input: i32,
        on_output: String,
    }
}

struct AppCtx {
  log: Rc<RefCell<Vec<String>>>,
}

impl AppCtx {
  fn new() -> Self {
    Self {
      log: Rc::new(RefCell::new(Vec::new())),
    }
  }

  fn push(&self, msg: &str) {
    self.log.borrow_mut().push(msg.to_string());
  }

  fn logs(&self) -> Vec<String> {
    self.log.borrow().clone()
  }
}

#[test]
fn test_dispatch_has_registry_when_feature_enabled() {
  let dispatch = AppDispatch::<AppCtx, _, _>::new();

  // registry() and registry_mut() should be available
  let _registry: &DispatchRegistry<AppCtx> = dispatch.registry();
}

#[test]
fn test_registry_set_and_get_handler() {
  let mut registry = DispatchRegistry::<AppCtx>::new();

  let handler: DynHandler<AppCtx> = Box::new(|_ctx: &mut AppCtx, input: DynValue| {
    // Echo back the input
    input
  });

  registry.set("test_handler", handler);

  assert!(registry.get("test_handler").is_some());
  assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_registry_remove_handler() {
  let mut registry = DispatchRegistry::<AppCtx>::new();

  let handler: DynHandler<AppCtx> = Box::new(|_ctx, input| input);
  registry.set("removable", handler);

  assert!(registry.get("removable").is_some());

  let removed = registry.remove("removable");
  assert!(removed.is_some());
  assert!(registry.get("removable").is_none());
}

#[test]
fn test_registry_call_dynamic_handler() {
  let mut registry = DispatchRegistry::<AppCtx>::new();

  let handler: DynHandler<AppCtx> = Box::new(|ctx: &mut AppCtx, input: DynValue| {
    if let Some(val) = input.downcast_ref::<i32>() {
      ctx.push(&format!("received: {}", val));
    }
    Box::new(()) as DynValue
  });

  registry.set("logger", handler);

  let mut ctx = AppCtx::new();
  let input: DynValue = Box::new(42i32);

  if let Some(handler) = registry.get("logger") {
    handler(&mut ctx, input);
  }

  assert_eq!(ctx.logs(), vec!["received: 42"]);
}

#[test]
fn test_dispatch_with_registry_integration() {
  let mut dispatch =
    AppDispatch::<AppCtx, _, _>::new().with_on_input(|ctx: &mut AppCtx, val: i32| {
      ctx.push(&format!("static handler: {}", val));
    });

  // Register a dynamic handler
  let dyn_handler: DynHandler<AppCtx> = Box::new(|ctx: &mut AppCtx, input: DynValue| {
    if let Some(val) = input.downcast_ref::<i32>() {
      ctx.push(&format!("dynamic handler: {}", val));
    }
    Box::new(()) as DynValue
  });

  dispatch.registry_mut().set("on_input_dyn", dyn_handler);

  let mut ctx = AppCtx::new();

  // Call static handler
  dispatch.on_input(&mut ctx, 10);

  // Call dynamic handler via registry
  if let Some(handler) = dispatch.registry().get("on_input_dyn") {
    handler(&mut ctx, Box::new(20i32) as DynValue);
  }

  assert_eq!(
    ctx.logs(),
    vec!["static handler: 10", "dynamic handler: 20"]
  );
}

#[test]
fn test_registry_handler_can_transform_input() {
  let mut registry = DispatchRegistry::<()>::new();

  let double_handler: DynHandler<()> = Box::new(|_ctx: &mut (), input: DynValue| {
    if let Some(val) = input.downcast_ref::<i32>() {
      Box::new(val * 2) as DynValue
    } else {
      input
    }
  });

  registry.set("double", double_handler);

  let mut ctx = ();
  let input: DynValue = Box::new(21i32);

  if let Some(handler) = registry.get("double") {
    let result = handler(&mut ctx, input);
    if let Some(val) = result.downcast_ref::<i32>() {
      assert_eq!(*val, 42);
    } else {
      panic!("Expected i32 result");
    }
  }
}

#[test]
fn test_registry_multiple_handlers() {
  let mut registry = DispatchRegistry::<()>::new();

  registry.set("handler1", Box::new(|_, input| input));
  registry.set("handler2", Box::new(|_, input| input));
  registry.set("handler3", Box::new(|_, input| input));

  assert!(registry.get("handler1").is_some());
  assert!(registry.get("handler2").is_some());
  assert!(registry.get("handler3").is_some());

  registry.remove("handler2");

  assert!(registry.get("handler1").is_some());
  assert!(registry.get("handler2").is_none());
  assert!(registry.get("handler3").is_some());
}
