pub mod damage;
pub mod display_list;
pub mod doc_formatter;
pub(crate) mod layer_projection;
pub mod layers;
pub(crate) mod layout_index;
pub mod manager;
pub mod measurement;
pub mod presentation;
pub(crate) mod row_diff;
pub mod surface;
pub mod text_annotations;
pub mod text_format;
pub(crate) mod visible_layout;
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
pub use measurement::CellGridMetrics;
pub use presentation::{
  FrameRow,
  PresentUpdate,
  RetainedFrame,
  SurfaceFrame,
};
pub use surface::CellGridRenderSurface;
pub use text_annotations::{
  GraphemeSubstitution,
  InlineAnnotation,
  TextAnnotationError,
  TextAnnotations,
};
pub use text_format::TextFormat;
