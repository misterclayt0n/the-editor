//! Terminal emulation wrapper around libghostty-vt.
//!
//! This crate provides safe Rust bindings to the ghostty virtual terminal library,
//! enabling terminal emulation capabilities within the editor.

pub mod ffi;
pub mod terminal;

pub use terminal::Terminal;
