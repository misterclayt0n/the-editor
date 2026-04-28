#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GutterLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SelectionLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CursorLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OverlayLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WidgetLayerDelta;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LayerDeltaSet {
  pub gutters:    Option<GutterLayerDelta>,
  pub selections: Option<SelectionLayerDelta>,
  pub cursors:    Option<CursorLayerDelta>,
  pub overlays:   Option<OverlayLayerDelta>,
  pub widgets:    Option<WidgetLayerDelta>,
}
