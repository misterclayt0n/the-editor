pub type GraphemeIndex = usize;
pub type LineIndex = usize;
pub type ByteIndex = usize;
pub type ColIndex = usize;
pub type RowIndex = usize;

mod position;
mod location;
mod size;

pub use position::Position;
pub use location::Location;
pub use size::Size;
pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";
