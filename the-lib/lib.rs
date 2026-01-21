use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod auto_pairs;
pub mod case_convention;
pub mod command_line;
pub mod history;
pub mod movement;
pub mod selection;
pub mod transaction;

pub type Tendril = SmartString<LazyCompact>;
