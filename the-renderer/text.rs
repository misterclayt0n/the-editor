//! Text rendering structures and utilities
//!
//! This module provides types for styling and positioning text.

use crate::Color;

/// Styling options for text segments
#[derive(Debug, Clone)]
pub struct TextStyle {
  /// Text color
  pub color: Color,
  /// Font size in pixels
  pub size: f32,
}

impl Default for TextStyle {
  fn default() -> Self {
    Self {
      color: Color::WHITE,
      size: 16.0,
    }
  }
}

/// A single segment of text with its own style
///
/// Multiple segments can be combined in a [`TextSection`] to create
/// text with mixed styles.
#[derive(Debug, Clone)]
pub struct TextSegment {
  /// The text content
  pub content: String,
  /// Style to apply to this segment
  pub style: TextStyle,
}

impl TextSegment {
  /// Create a new text segment with default style
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      content: content.into(),
      style: TextStyle::default(),
    }
  }

  /// Set the style for this segment
  pub fn with_style(mut self, style: TextStyle) -> Self {
    self.style = style;
    self
  }

  /// Set the color for this segment
  pub fn with_color(mut self, color: Color) -> Self {
    self.style.color = color;
    self
  }

  /// Set the font size for this segment
  pub fn with_size(mut self, size: f32) -> Self {
    self.style.size = size;
    self
  }
}

/// A section of text positioned on screen
///
/// A section can contain multiple [`TextSegment`]s, each with their own style.
/// All segments in a section share the same baseline position.
#[derive(Debug, Clone)]
pub struct TextSection {
  /// Screen position (x, y) in pixels from top-left
  pub position: (f32, f32),
  /// Text segments in this section
  pub texts: Vec<TextSegment>,
}

impl TextSection {
  /// Create a new empty text section at the given position
  pub fn new(x: f32, y: f32) -> Self {
    Self {
      position: (x, y),
      texts: Vec::new(),
    }
  }

  /// Add a text segment to this section
  pub fn add_text(mut self, text: TextSegment) -> Self {
    self.texts.push(text);
    self
  }

  /// Create a section with a single segment of text
  ///
  /// This is a convenience method for creating simple text without
  /// needing to manually create segments.
  pub fn simple(x: f32, y: f32, content: impl Into<String>, size: f32, color: Color) -> Self {
    Self {
      position: (x, y),
      texts: vec![TextSegment {
        content: content.into(),
        style: TextStyle { color, size },
      }],
    }
  }
}

/// Font data container
///
/// Currently unused - the renderer uses an embedded font.
/// This type is reserved for future font loading functionality.
#[derive(Debug, Clone)]
pub struct Font {
  pub data: Vec<u8>,
}

impl Font {
  /// Create a font from raw TTF/OTF data
  pub fn from_bytes(data: Vec<u8>) -> Self {
    Self { data }
  }
}
