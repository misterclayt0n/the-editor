use smartstring::{LazyCompact, SmartString};

pub mod auto_pairs;
pub mod movement;
pub mod selection;
pub mod transaction;
pub mod case_convention;

pub type Tendril = SmartString<LazyCompact>;
