//! Minimal plugin API for composing dispatch behavior at runtime.
//!
//! This keeps the-dispatch generic while enabling a simple middleware-style
//! chain (user plugin → default plugin) without hard-coding any UI events.

/// Result of a dispatch call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult<T = ()> {
  /// Pass the input to the next handler.
  Continue,
  /// Input was handled; stop the chain.
  Handled,
  /// Emit a single output value.
  Emit(T),
  /// Emit multiple output values.
  EmitMany(Vec<T>),
}

impl DispatchResult<()> {
  /// Convenience for a handled result.
  pub const fn handled() -> Self {
    Self::Handled
  }

  /// Convenience for a continue result.
  pub const fn r#continue() -> Self {
    Self::Continue
  }
}

/// Minimal plugin interface for dispatch pipelines.
///
/// This is intentionally generic: it does not know about key or mouse events.
/// Higher layers define the input types (e.g., commands) and outputs (if any).
pub trait DispatchPlugin<Ctx, Input, Output = ()> {
  fn dispatch(&mut self, ctx: &mut Ctx, input: Input) -> DispatchResult<Output>;
}
