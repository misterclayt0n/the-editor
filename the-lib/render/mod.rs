pub mod damage;
pub mod display_list;
pub mod doc_formatter;
pub mod layers;
pub mod surface;
pub mod text_annotations;
pub mod text_format;
pub mod visual_position;

pub use damage::Damage;
pub use display_list::{
  FrameRequest,
  RenderFrameDelta,
  RenderRow,
  RowId,
  RowMove,
  RowPlacement,
  SurfaceId,
  Viewport,
};
pub use doc_formatter::{
  DocumentFormatter,
  FormattedGrapheme,
  GraphemeSource,
  Position,
};
pub use layers::{
  CursorLayerDelta,
  GutterLayerDelta,
  LayerDeltaSet,
  OverlayLayerDelta,
  SelectionLayerDelta,
  SelectionSpan,
  WidgetLayerDelta,
};
pub use surface::RenderSurface;
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
