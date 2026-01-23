//! Render-adjacent helpers and visual layout utilities.
//!
//! This module will host visual layout computations (soft-wrap, annotations)
//! that depend on formatting and rendering state. It intentionally lives
//! alongside core logic so consumers can access `the_lib::render::*` without
//! pulling a separate crate.

pub mod visual_position;
pub mod graphics;
pub mod text_format;
pub mod grapheme;
pub mod text_annotations;
pub mod doc_formatter;

pub use grapheme::{FormattedGrapheme, GraphemeSource};
