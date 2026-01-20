use smartstring::{LazyCompact, SmartString};

pub mod movement;
pub mod selection;
pub mod transaction;

pub type Tendril = SmartString<LazyCompact>;
