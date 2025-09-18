pub mod active;
pub mod elaborate;
pub mod parser;
pub mod render;

#[derive(PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Clone, Copy)]
pub struct TabstopIdx(usize);
pub const LAST_TABSTOP_IDX: TabstopIdx = TabstopIdx(usize::MAX);
