//! Profiling support via Tracy.
//!
//! This module provides macros for integrating with the Tracy profiler.
//! When the `tracy` feature is enabled, these macros emit Tracy spans.
//! When disabled, they compile to no-ops.
//!
//! # Usage
//!
//! ```ignore
//! use crate::profiling::{profile_scope, profile_function};
//!
//! fn my_function() {
//!     profile_function!();
//!     // ... function body
//! }
//!
//! fn another_function() {
//!     {
//!         profile_scope!("expensive_operation");
//!         // ... expensive code
//!     }
//! }
//! ```

/// Create a Tracy span for the current scope with the given name.
///
/// When the `tracy` feature is disabled, this is a no-op.
#[macro_export]
macro_rules! profile_scope {
  ($name:expr) => {
    #[cfg(feature = "tracy")]
    let _tracy_span = tracy_client::span!($name);
    #[cfg(not(feature = "tracy"))]
    let _ = $name;
  };
}

/// Create a Tracy span for the current function.
///
/// Uses the function name as the span name.
/// When the `tracy` feature is disabled, this is a no-op.
#[macro_export]
macro_rules! profile_function {
  () => {
    #[cfg(feature = "tracy")]
    let _tracy_span = tracy_client::span!();
  };
}

/// Mark a frame boundary for Tracy.
///
/// Call this once per frame to enable Tracy's frame timing features.
/// When the `tracy` feature is disabled, this is a no-op.
#[macro_export]
macro_rules! profile_frame {
  () => {
    #[cfg(feature = "tracy")]
    tracy_client::frame_mark();
  };
  ($name:expr) => {
    #[cfg(feature = "tracy")]
    tracy_client::secondary_frame_mark!($name);
  };
}

/// Plot a value to Tracy.
///
/// Useful for tracking metrics over time (e.g., frame times, cache sizes).
/// When the `tracy` feature is disabled, this is a no-op.
#[macro_export]
macro_rules! profile_plot {
  ($name:expr, $value:expr) => {
    #[cfg(feature = "tracy")]
    tracy_client::plot!($name, $value as f64);
  };
}

/// Send a message to Tracy.
///
/// Useful for logging events that should appear in the Tracy timeline.
/// When the `tracy` feature is disabled, this is a no-op.
#[macro_export]
macro_rules! profile_message {
  ($msg:expr) => {
    #[cfg(feature = "tracy")]
    if let Some(client) = tracy_client::Client::running() {
      client.message($msg, 0);
    }
  };
}
