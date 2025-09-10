pub mod active;
pub mod render;
pub mod elaborate;
pub mod parser;

#[derive(PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Clone, Copy)]
pub struct TabstopIdx(usize);
pub const LAST_TABSTOP_IDX: TabstopIdx = TabstopIdx(usize::MAX);
