use std::{
  borrow::Cow,
  fs,
  path::Path,
};

use anyhow::anyhow;
use gpui::{
  Window,
  font,
  px,
};

use crate::{
  Color,
  TextSection,
};

/// Configuration options for the renderer
#[derive(Debug, Clone)]
pub struct RendererConfig {
  /// Background color for clearing the screen
  pub background_color: Color,
}

impl Default for RendererConfig {
  fn default() -> Self {
    Self {
      background_color: Color::new(0.1, 0.1, 0.15, 1.0),
    }
  }
}

/// The main renderer struct used by the editor.
///
/// In the GPUI implementation the renderer now acts as a command buffer that
/// collects drawing commands for the current frame. The actual GPU work happens
/// inside the GPUI canvas element once the frame data has been produced.
pub struct Renderer {
  config:       RendererConfig,
  width:        u32,
  height:       u32,
  commands:     Vec<DrawCommand>,
  font_family:  String,
  font_size:    f32,
  cell_width:   f32,
  cell_height:  f32,
  have_metrics: bool,
  pending_font_bytes: Option<Vec<u8>>, // Font bytes to register on next metrics refresh
}

impl Renderer {
  /// Create a new renderer for the provided viewport size.
  pub fn new(width: u32, height: u32) -> Self {
    Self {
      config: RendererConfig::default(),
      width,
      height,
      commands: Vec::new(),
      font_family: "Iosevka".to_string(),
      font_size: 16.0,
      cell_width: 8.0,
      cell_height: 16.0,
      have_metrics: false,
      pending_font_bytes: None,
    }
  }

  /// Update the viewport. Returns `true` when the size changed.
  pub fn update_viewport(&mut self, width: u32, height: u32) -> bool {
    if self.width != width || self.height != height {
      self.width = width;
      self.height = height;
      true
    } else {
      false
    }
  }

  /// Get the currently configured font family name.
  pub fn current_font_family(&self) -> &str {
    &self.font_family
  }

  /// Begin a new frame. This simply clears the recorded command list.
  pub fn begin_frame(&mut self) -> crate::Result<()> {
    self.commands.clear();
    Ok(())
  }

  /// Finish the current frame. Provided for API compatibility.
  pub fn end_frame(&mut self) -> crate::Result<()> {
    Ok(())
  }

  /// Consume the recorded commands and return the frame data for presentation.
  pub fn take_frame(&mut self) -> FrameData {
    FrameData {
      background_color: self.config.background_color,
      commands:         std::mem::take(&mut self.commands),
      font_family:      self.font_family.clone(),
    }
  }

  /// Set the background color for the current frame.
  pub fn set_background_color(&mut self, color: Color) {
    self.config.background_color = color;
  }

  /// Configure the monospaced font family and size used for layout
  /// calculations.
  pub fn configure_font(&mut self, family: &str, size: f32) {
    if self.font_family != family {
      self.font_family = family.to_string();
      self.have_metrics = false;
    }
    if (self.font_size - size).abs() > f32::EPSILON {
      self.font_size = size;
      self.have_metrics = false;
    }
  }

  /// Configure the font by reading TTF/OTF/TTC bytes, resolving its family name,
  /// and scheduling the bytes to be registered with the text system.
  pub fn configure_font_from_bytes(&mut self, bytes: Vec<u8>, size: f32) -> anyhow::Result<()> {
    let family = resolve_family_name(&bytes)
      .ok_or_else(|| anyhow!("could not resolve font family from provided bytes"))?;

    if self.font_family != family {
      self.font_family = family;
      self.have_metrics = false;
    }

    if (self.font_size - size).abs() > f32::EPSILON {
      self.font_size = size;
      self.have_metrics = false;
    }

    self.pending_font_bytes = Some(bytes);
    Ok(())
  }

  /// Configure the font by reading the specified font file path and resolving
  /// its family name automatically.
  pub fn configure_font_from_path<P: AsRef<Path>>(
    &mut self,
    path: P,
    size: f32,
  ) -> anyhow::Result<()> {
    let bytes = fs::read(path)?;
    self.configure_font_from_bytes(bytes, size)
  }

  /// Ensure cached font metrics are in sync with the GPUI text system.
  pub fn ensure_font_metrics(&mut self, window: &Window) {
    if self.have_metrics {
      return;
    }

    // If a new font file was configured, register it with the text system now.
    if let Some(bytes) = self.pending_font_bytes.take() {
      if let Err(err) = window.text_system().add_fonts(vec![Cow::Owned(bytes)]) {
        log::warn!("failed to register configured font bytes: {err}");
      }
    }

    let font = font(self.font_family.clone());
    let font_id = window.text_system().resolve_font(&font);
    let advance = window
      .text_system()
      .em_advance(font_id, px(self.font_size))
      .map(|px_val| px_val.0)
      .unwrap_or(self.cell_width);

    self.cell_width = advance.max(1.0);
    self.cell_height = self.font_size;
    self.have_metrics = true;

    // match window.text_system().resolve_font(&font) {
    //   Ok(font_id) => {
    // },
    // Err(err) => {
    // log::warn!("failed to resolve font metrics: {err}");
    // },
    // }
  }

  /// Report the configured monospaced character width in pixels.
  pub fn cell_width(&self) -> f32 {
    self.cell_width
  }

  /// Record a text section to be drawn this frame.
  pub fn draw_text(&mut self, section: TextSection) {
    self.commands.push(DrawCommand::Text(section));
  }

  /// Record a solid rectangle.
  pub fn draw_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Color) {
    self.commands.push(DrawCommand::Rect {
      x,
      y,
      width,
      height,
      color,
    });
  }

  /// Record a rounded rectangle fill.
  pub fn draw_rounded_rect(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    color: Color,
  ) {
    self.commands.push(DrawCommand::RoundedRect {
      x,
      y,
      width,
      height,
      corner_radius,
      color,
    });
  }

  /// Record a rounded rectangle glow overlay.
  pub fn draw_rounded_rect_glow(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    center_x: f32,
    center_y: f32,
    radius: f32,
    color: Color,
  ) {
    self.commands.push(DrawCommand::RoundedRectGlow {
      x,
      y,
      width,
      height,
      corner_radius,
      center_x,
      center_y,
      glow_radius: radius,
      color,
    });
  }

  /// Record a rounded rectangle stroke.
  pub fn draw_rounded_rect_stroke(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    thickness: f32,
    color: Color,
  ) {
    self.commands.push(DrawCommand::RoundedRectStroke {
      x,
      y,
      width,
      height,
      corner_radius,
      thickness,
      color,
    });
  }

  /// Width in physical pixels.
  pub fn width(&self) -> u32 {
    self.width
  }

  /// Height in physical pixels.
  pub fn height(&self) -> u32 {
    self.height
  }
}

/// Try to resolve a readable family name from raw font bytes using fontdb.
fn resolve_family_name(bytes: &[u8]) -> Option<String> {
  let mut db = fontdb::Database::new();
  let before = db.faces().len();
  db.load_font_data(bytes.to_vec());
  let faces = db.faces();
  let face = faces.get(before)?;

  // Prefer a named family from the face if present
  if let Some((name, _lang)) = face.families.first() {
    return Some(name.clone());
  }

  // Fallback: post script name if available
  if !face.post_script_name.is_empty() {
    return Some(face.post_script_name.clone());
  }

  None
}

/// Drawing commands captured for a frame. These are consumed by the GPUI canvas
/// when the frame is presented.
pub struct FrameData {
  pub background_color: Color,
  pub commands:         Vec<DrawCommand>,
  pub font_family:      String,
}

/// Individual drawing operations supported by the higher level UI code.
#[allow(clippy::large_enum_variant)]
pub enum DrawCommand {
  Text(TextSection),
  Rect {
    x:      f32,
    y:      f32,
    width:  f32,
    height: f32,
    color:  Color,
  },
  RoundedRect {
    x:             f32,
    y:             f32,
    width:         f32,
    height:        f32,
    corner_radius: f32,
    color:         Color,
  },
  RoundedRectGlow {
    x:             f32,
    y:             f32,
    width:         f32,
    height:        f32,
    corner_radius: f32,
    center_x:      f32,
    center_y:      f32,
    glow_radius:   f32,
    color:         Color,
  },
  RoundedRectStroke {
    x:             f32,
    y:             f32,
    width:         f32,
    height:        f32,
    corner_radius: f32,
    thickness:     f32,
    color:         Color,
  },
}
