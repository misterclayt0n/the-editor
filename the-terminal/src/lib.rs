//! Terminal emulation wrapper around libghostty-vt.
//!
//! This crate provides safe Rust bindings to the ghostty virtual terminal library,
//! enabling terminal emulation capabilities within the editor.

pub mod ffi;
pub mod terminal;
pub mod pty;
pub mod terminal_session;

pub use terminal::Terminal;
pub use pty::PtySession;
pub use terminal_session::TerminalSession;
