use crate::{
  position::Position,
  render::graphics::{
    Rect,
    Style,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayRectKind {
  Panel,
  Divider,
  Highlight,
  Backdrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayRect {
  pub rect:   Rect,
  pub kind:   OverlayRectKind,
  pub radius: u16,
  pub style:  Style,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayText {
  pub pos:   Position,
  pub text:  String,
  pub style: Style,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayNode {
  Rect(OverlayRect),
  Text(OverlayText),
}
