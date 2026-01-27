//! # the-dispatch
//!
//! A zero-cost, generic dispatch system for building overridable, composable
//! behavior graphs.
//!
//! This crate provides a macro-based dispatch system that generates
//! statically-dispatched handler graphs with optional dynamic registry support.
//!
//! ## Core Concepts
//!
//! - **Dispatch Points**: Named entry points where handlers are invoked
//! - **Handlers**: Functions or closures that receive a context, input, and
//!   return an output
//! - **Builder Pattern**: Replace default no-op handlers with custom
//!   implementations
//! - **Zero-Cost**: Static dispatch via generics, no virtual calls by default
//! - **Dynamic Registry** (opt-in): String-keyed handler lookup for
//!   plugins/scripting
//! - **COW Handlers** (opt-in): Shared handler slots for cheap cloning
//!
//! ## Basic Usage
//!
//! ```rust
//! use the_dispatch::define;
//!
//! // Define dispatch points with their input and output types
//! define! {
//!     Editor {
//!         on_keypress: char => (),
//!         on_action: String => (),
//!     }
//! }
//!
//! // Create a context type
//! struct Ctx {
//!   buffer: String,
//! }
//!
//! // Build a dispatch with custom handlers
//! let dispatch = EditorDispatch::<Ctx, _, _>::new()
//!   .with_on_keypress(|ctx: &mut Ctx, key: char| {
//!     ctx.buffer.push(key);
//!   })
//!   .with_on_action(|ctx: &mut Ctx, action: String| {
//!     if action == "clear" {
//!       ctx.buffer.clear();
//!     }
//!   });
//!
//! let mut ctx = Ctx {
//!   buffer: String::new(),
//! };
//!
//! dispatch.on_keypress(&mut ctx, 'h');
//! dispatch.on_keypress(&mut ctx, 'i');
//! assert_eq!(ctx.buffer, "hi");
//!
//! dispatch.on_action(&mut ctx, "clear".to_string());
//! assert!(ctx.buffer.is_empty());
//! ```
//!
//! ## Generated API Trait
//!
//! The macro also generates a trait for ergonomic bounds without exposing
//! handler generics:
//!
//! ```rust
//! # use the_dispatch::define;
//! define! {
//!     Editor {
//!         on_keypress: char => (),
//!         on_action: String => (),
//!     }
//! }
//!
//! fn run_line<Ctx>(dispatch: &impl EditorApi<Ctx>, ctx: &mut Ctx, input: char) {
//!   dispatch.on_keypress(ctx, input);
//! }
//! ```
//!
//! ## COW Handlers (Feature: `cow-handlers`)
//!
//! Enable `cow-handlers` to store handlers behind `Arc` so dispatch values are
//! cheap to clone:
//!
//! ```rust,ignore
//! // In Cargo.toml: the-dispatch = { features = ["cow-handlers"] }
//! let base = EditorDispatch::<Ctx, _, _>::new();
//! let custom = base.clone().with_on_keypress(|ctx, key| {
//!     ctx.buffer.push(key);
//! });
//! ```
//!
//! ## Handler Chaining
//!
//! Handlers can call other dispatch points by having access to the dispatch
//! through external coordination (the dispatch system doesn't impose control
//! flow):
//!
//! ```rust
//! use std::{
//!   cell::RefCell,
//!   rc::Rc,
//! };
//!
//! use the_dispatch::define;
//!
//! define! {
//!     Pipeline {
//!         step1: i32,
//!         step2: i32,
//!         step3: i32,
//!     }
//! }
//!
//! let log = Rc::new(RefCell::new(Vec::new()));
//!
//! let dispatch = PipelineDispatch::<(), _, _, _>::new()
//!   .with_step1({
//!     let log = log.clone();
//!     move |_: &mut (), val: i32| log.borrow_mut().push(format!("step1: {}", val))
//!   })
//!   .with_step2({
//!     let log = log.clone();
//!     move |_: &mut (), val: i32| log.borrow_mut().push(format!("step2: {}", val))
//!   })
//!   .with_step3({
//!     let log = log.clone();
//!     move |_: &mut (), val: i32| log.borrow_mut().push(format!("step3: {}", val))
//!   });
//!
//! // Orchestrate the chain externally
//! let mut ctx = ();
//! dispatch.step1(&mut ctx, 1);
//! dispatch.step2(&mut ctx, 2);
//! dispatch.step3(&mut ctx, 3);
//!
//! assert_eq!(*log.borrow(), vec!["step1: 1", "step2: 2", "step3: 3"]);
//! ```
//!
//! ## Dynamic Registry (Feature: `dynamic-registry`)
//!
//! Enable the `dynamic-registry` feature for string-keyed handler lookup:
//!
//! ```rust,ignore
//! // In Cargo.toml: the-dispatch = { features = ["dynamic-registry"] }
//!
//! let mut dispatch = EditorDispatch::<Ctx, _, _>::new();
//!
//! // Register a dynamic handler
//! dispatch.registry_mut().set("plugin_handler", std::sync::Arc::new(|ctx, input| {
//!     // Dynamic handler logic
//!     Box::new(())
//! }));
//!
//! // Look up and call dynamic handlers
//! if let Some(handler) = dispatch.registry().get("plugin_handler") {
//!     handler(&mut ctx, Box::new(input_value));
//! }
//! ```

mod define;
#[cfg(feature = "editor-hooks")]
pub mod editor;
mod plugin;
mod registry;

pub use paste;
pub use plugin::{
  DispatchPlugin,
  DispatchResult,
};
pub use registry::{
  DispatchRegistry,
  DynHandler,
  DynValue,
};

/// Storage type for a handler slot.
///
/// With `cow-handlers` enabled, handler slots are shared via `Arc` for cheap
/// cloning. Without it, the slot stores the handler value directly.
#[cfg(feature = "cow-handlers")]
pub type HandlerSlot<T> = std::sync::Arc<T>;

/// Storage type for a handler slot.
///
/// With `cow-handlers` disabled, the slot stores the handler value directly.
#[cfg(not(feature = "cow-handlers"))]
pub type HandlerSlot<T> = T;

/// Wrap a handler in a slot.
///
/// With `cow-handlers` enabled this allocates an `Arc`; otherwise it returns
/// the handler.
#[cfg(feature = "cow-handlers")]
pub fn handler_slot<T>(handler: T) -> HandlerSlot<T> {
  std::sync::Arc::new(handler)
}

/// Wrap a handler in a slot.
///
/// With `cow-handlers` disabled this returns the handler unchanged.
#[cfg(not(feature = "cow-handlers"))]
pub fn handler_slot<T>(handler: T) -> HandlerSlot<T> {
  handler
}

/// Storage type for the dynamic registry.
///
/// With `cow-handlers` enabled, this is an `Arc` to allow clone-on-write
/// updates.
#[cfg(all(feature = "dynamic-registry", feature = "cow-handlers"))]
pub type RegistrySlot<Ctx> = std::sync::Arc<DispatchRegistry<Ctx>>;

/// Storage type for the dynamic registry.
///
/// With `cow-handlers` disabled, this stores the registry value directly.
#[cfg(all(feature = "dynamic-registry", not(feature = "cow-handlers")))]
pub type RegistrySlot<Ctx> = DispatchRegistry<Ctx>;

/// Construct a new registry slot.
///
/// With `cow-handlers` enabled, this allocates an `Arc`.
#[cfg(all(feature = "dynamic-registry", feature = "cow-handlers"))]
pub fn registry_slot<Ctx>() -> RegistrySlot<Ctx> {
  std::sync::Arc::new(DispatchRegistry::new())
}

/// Construct a new registry slot.
///
/// With `cow-handlers` disabled, this returns the registry value directly.
#[cfg(all(feature = "dynamic-registry", not(feature = "cow-handlers")))]
pub fn registry_slot<Ctx>() -> RegistrySlot<Ctx> {
  DispatchRegistry::new()
}

/// Type alias for a simple function pointer handler.
///
/// Handlers receive a mutable reference to the context and an input value.
/// They return the output type defined for the dispatch point.
pub type Handler<Ctx, Input, Output> = fn(&mut Ctx, Input) -> Output;

/// Trait for callable handlers.
///
/// This trait is automatically implemented for any `Fn(&mut Ctx, Input) ->
/// Output`, allowing both function pointers and closures to be used as
/// handlers. When `cow-handlers` is enabled, `Arc<T>` also implements
/// `HandlerFn`.
pub trait HandlerFn<Ctx, Input, Output> {
  /// Call the handler with the given context and input.
  fn call(&self, ctx: &mut Ctx, input: Input) -> Output;
}

impl<Ctx, Input, Output, F> HandlerFn<Ctx, Input, Output> for F
where
  F: Fn(&mut Ctx, Input) -> Output,
{
  fn call(&self, ctx: &mut Ctx, input: Input) -> Output {
    (self)(ctx, input)
  }
}

#[cfg(feature = "cow-handlers")]
impl<Ctx, Input, Output, F> HandlerFn<Ctx, Input, Output> for std::sync::Arc<F>
where
  F: HandlerFn<Ctx, Input, Output> + ?Sized,
{
  fn call(&self, ctx: &mut Ctx, input: Input) -> Output {
    (**self).call(ctx, input)
  }
}
