pub mod doc_formatter;
pub mod text_annotations;
pub mod text_format;
pub mod visual_position;

pub use doc_formatter::{
  DocumentFormatter,
  FormattedGrapheme,
  GraphemeSource,
  Position,
};
pub use text_annotations::{
  GraphemeSubstitution,
  InlineAnnotation,
  TextAnnotationError,
  TextAnnotations,
  VirtualLineSpec,
};
pub use text_format::TextFormat;
pub use visual_position::{
  char_at_visual_pos,
  visual_pos_at_char,
};
