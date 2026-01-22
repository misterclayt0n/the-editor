//! Core editing primitives and utilities for the-editor.
//!
//! This crate provides the fundamental building blocks for text editing
//! operations, including transactions, selections, history management, and
//! various utilities. It is designed to be a pure library with no I/O, suitable
//! for embedding in different editor frontends.
//!
//! # Architecture
//!
//! The library follows a layered design where lower-level primitives compose
//! into higher-level operations:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Higher-Level Editor                      │
//! │              (view, commands, input handling)               │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        the-lib                              │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
//! │  │  Transaction │  │  Selection   │  │   History    │      │
//! │  │   (edits)    │  │  (cursors)   │  │ (undo/redo)  │      │
//! │  └──────────────┘  └──────────────┘  └──────────────┘      │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
//! │  │  Auto Pairs  │  │    Search    │  │    Fuzzy     │      │
//! │  └──────────────┘  └──────────────┘  └──────────────┘      │
//! │  ┌──────────────┐  ┌──────────────┐                        │
//! │  │ Command Line │  │Case Convent. │                        │
//! │  └──────────────┘  └──────────────┘                        │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                       the-core                              │
//! │                  (Rope, graphemes, etc.)                    │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Module Overview
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`transaction`] | Atomic text changes with position mapping |
//! | [`selection`] | Cursor and selection management |
//! | [`history`] | Undo/redo with branching history tree |
//! | [`auto_pairs`] | Automatic bracket/quote pairing |
//! | [`search`] | Single-character search within text |
//! | [`fuzzy`] | Fuzzy string matching for filtering |
//! | [`command_line`] | Command mode parsing (`:` commands) |
//! | [`case_convention`] | Case transformations (snake_case, etc.) |
//! | [`movement`] | Direction enum for cursor movement |
//!
//! # Design Principles
//!
//! - **Pure functions**: Operations return new state rather than mutating
//! - **Explicit errors**: `Result` types instead of panics for invalid input
//! - **No I/O**: All file/network operations belong in higher layers
//! - **Composable**: Small primitives combine into complex operations
//!
//! # The Tendril Type
//!
//! [`Tendril`] is a small-string-optimized type used throughout the library.
//! Strings up to ~23 bytes are stored inline without heap allocation, making
//! it efficient for the small text fragments typical in editing operations.

use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod auto_pairs;
pub mod case_convention;
pub mod command_line;
pub mod fuzzy;
pub mod history;
pub mod movement;
pub mod search;
pub mod selection;
pub mod transaction;

/// A small-string-optimized string type.
///
/// Strings up to ~23 bytes are stored inline without heap allocation.
/// This is the primary string type used throughout the library for
/// text fragments, insertions, and other small strings.
pub type Tendril = SmartString<LazyCompact>;
