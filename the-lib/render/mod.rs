pub mod damage;
pub mod display_list;
pub mod doc_formatter;
pub mod layers;
pub mod manager;
pub mod measurement;
pub mod surface;
pub mod syntax_projection;
pub mod text_annotations;
pub mod text_format;
pub mod visual_position;

pub use damage::{
  Damage,
  DirtyFlag,
  DirtyRows,
  DirtyState,
};
pub use display_list::{
  FrameRequest,
  RenderFrameDelta,
  RenderRow,
  RowId,
  RowKind,
  RowMetadata,
  RowMetadataDelta,
  RowMove,
  RowPlacement,
  SurfaceId,
  Viewport,
  WidgetId,
};
pub use doc_formatter::{
  DocumentFormatter,
  FormattedGrapheme,
  GraphemeSource,
  Position,
};
pub use layers::{
  BlockWidget,
  CursorLayerDelta,
  CursorPlacement,
  GutterLayerDelta,
  LayerDeltaSet,
  OverlayLayerDelta,
  SelectionLayerDelta,
  SelectionSpan,
  SyntaxLayerDelta,
  SyntaxSpan,
  WidgetLayerDelta,
  WidgetPlacement,
};
pub use manager::{
  RenderManager,
  SourceId,
};
pub use measurement::{
  MeasurementBackendKind,
  SurfaceMeasurements,
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
