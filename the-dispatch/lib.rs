//! # the-dispatch
//!
//! A zero-cost, generic dispatch system for building overridable, composable behavior graphs.
//!
//! This crate provides a macro-based dispatch system that generates statically-dispatched
//! handler graphs with optional dynamic registry support.
//!
//! ## Core Concepts
//!
//! - **Dispatch Points**: Named entry points where handlers are invoked
//! - **Handlers**: Functions or closures that receive a context, input, and return an output
//! - **Builder Pattern**: Replace default no-op handlers with custom implementations
//! - **Zero-Cost**: Static dispatch via generics, no virtual calls by default
//! - **Dynamic Registry** (opt-in): String-keyed handler lookup for plugins/scripting
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
//!     buffer: String,
//! }
//!
//! // Build a dispatch with custom handlers
//! let dispatch = EditorDispatch::<Ctx, _, _>::new()
//!     .with_on_keypress(|ctx: &mut Ctx, key: char| {
//!         ctx.buffer.push(key);
//!     })
//!     .with_on_action(|ctx: &mut Ctx, action: String| {
//!         if action == "clear" {
//!             ctx.buffer.clear();
//!         }
//!     });
//!
//! let mut ctx = Ctx { buffer: String::new() };
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
//! The macro also generates a trait for ergonomic bounds without exposing handler generics:
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
//!     dispatch.on_keypress(ctx, input);
//! }
//! ```
//!
//! ## Handler Chaining
//!
//! Handlers can call other dispatch points by having access to the dispatch
//! through external coordination (the dispatch system doesn't impose control flow):
//!
//! ```rust
//! use the_dispatch::define;
//! use std::cell::RefCell;
//! use std::rc::Rc;
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
//!     .with_step1({
//!         let log = log.clone();
//!         move |_: &mut (), val: i32| log.borrow_mut().push(format!("step1: {}", val))
//!     })
//!     .with_step2({
//!         let log = log.clone();
//!         move |_: &mut (), val: i32| log.borrow_mut().push(format!("step2: {}", val))
//!     })
//!     .with_step3({
//!         let log = log.clone();
//!         move |_: &mut (), val: i32| log.borrow_mut().push(format!("step3: {}", val))
//!     });
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
//! dispatch.registry_mut().set("plugin_handler", Box::new(|ctx, input| {
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
mod registry;

pub use paste;
pub use registry::{DispatchRegistry, DynHandler, DynValue};

/// Type alias for a simple function pointer handler.
///
/// Handlers receive a mutable reference to the context and an input value.
/// They return the output type defined for the dispatch point.
pub type Handler<Ctx, Input, Output> = fn(&mut Ctx, Input) -> Output;

/// Trait for callable handlers.
///
/// This trait is automatically implemented for any `Fn(&mut Ctx, Input) -> Output`,
/// allowing both function pointers and closures to be used as handlers.
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
