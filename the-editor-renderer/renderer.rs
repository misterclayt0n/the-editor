//! GPU renderer implementation
//!
//! This module contains the core rendering functionality using wgpu.
//! **NOTE**: We may end up switching backends in the future.

use std::sync::Arc;

use wgpu_text::{
  BrushBuilder,
  TextBrush,
  glyph_brush::{
    Section,
    ab_glyph::FontRef,
  },
};
use winit::window::Window;

use crate::{
  Color,
  RendererError,
  Result,
  TextSection,
};

/// Configuration options for the renderer
#[derive(Debug, Clone)]
pub struct RendererConfig {
  /// Background color for clearing the screen
  pub background_color: Color,
  /// Enable vertical sync
  pub vsync:            bool,
}

impl Default for RendererConfig {
  fn default() -> Self {
    Self {
      background_color: Color::new(0.1, 0.1, 0.15, 1.0),
      vsync:            true,
    }
  }
}

/// The main renderer struct that manages GPU resources and drawing operations
pub struct Renderer {
  surface: wgpu::Surface<'static>,
  device:  wgpu::Device,
  queue:   wgpu::Queue,
  config:  wgpu::SurfaceConfiguration,
  size:    winit::dpi::PhysicalSize<u32>,
  brush:   TextBrush<FontRef<'static>>,

  // Frame state.
  current_output:  Option<wgpu::SurfaceTexture>,
  current_view:    Option<wgpu::TextureView>,
  current_encoder: Option<wgpu::CommandEncoder>,

  // Configuration.
  background_color: Color,

  // Text sections to render this frame - store owned strings.
  text_strings:   Vec<Vec<(String, f32, [f32; 4])>>, // content, size, color.
  text_positions: Vec<(f32, f32)>,
  
}

impl Renderer {
  /// Create a new renderer with the given window
  ///
  /// # Errors
  ///
  /// Returns an error if GPU initialization fails or the adapter doesn't
  /// support the required features.
  pub async fn new(window: Arc<Window>) -> Result<Self> {
    let size = window.inner_size();

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
      backends: wgpu::Backends::PRIMARY,
      ..Default::default()
    });

    let surface = instance
      .create_surface(window.clone())
      .map_err(|e| RendererError::SurfaceCreation(e.to_string()))?;

    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        compatible_surface:     Some(&surface),
        force_fallback_adapter: false,
      })
      .await
      .map_err(|e| RendererError::Configuration(format!("Failed to get adapter: {}", e)))?;

    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits:   wgpu::Limits::default(),
        label:             None,
        memory_hints:      Default::default(),
        trace:             Default::default(),
      })
      .await?;

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
      .formats
      .iter()
      .find(|f| f.is_srgb())
      .copied()
      .unwrap_or(surface_caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
      usage:                         wgpu::TextureUsages::RENDER_ATTACHMENT,
      format:                        surface_format,
      width:                         size.width,
      height:                        size.height,
      present_mode:                  surface_caps.present_modes[0],
      alpha_mode:                    surface_caps.alpha_modes[0],
      view_formats:                  vec![],
      desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Load default font
    const FONT_BYTES: &[u8] = include_bytes!("assets/JetBrainsMono-Regular.ttf");
    let font =
      FontRef::try_from_slice(FONT_BYTES).map_err(|e| RendererError::FontError(e.to_string()))?;

    let brush =
      BrushBuilder::using_font(font).build(&device, size.width, size.height, config.format);

    Ok(Self {
      surface,
      device,
      queue,
      config,
      size,
      brush,
      current_output: None,
      current_view: None,
      current_encoder: None,
      background_color: Color::new(0.1, 0.1, 0.15, 1.0),
      text_strings: Vec::new(),
      text_positions: Vec::new(),
    })
  }

  /// Resize the renderer viewport
  ///
  /// This method should be called when the window is resized.
  /// It updates the surface configuration and text brush dimensions.
  pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    if new_size.width > 0 && new_size.height > 0 {
      self.size = new_size;
      self.config.width = new_size.width;
      self.config.height = new_size.height;
      self.surface.configure(&self.device, &self.config);

      self.brush.resize_view(
        self.config.width as f32,
        self.config.height as f32,
        &self.queue,
      );
    }
  }

  /// Begin a new rendering frame
  ///
  /// This must be called before any drawing operations.
  /// It acquires the next swap chain texture and prepares for rendering.
  ///
  /// # Panics
  ///
  /// Panics if called twice without calling `end_frame` in between.
  pub fn begin_frame(&mut self) -> Result<()> {
    let output = self.surface.get_current_texture()?;
    let view = output
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());

    let encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
      });

    self.current_output = Some(output);
    self.current_view = Some(view);
    self.current_encoder = Some(encoder);
    self.text_strings.clear();
    self.text_positions.clear();
    Ok(())
  }

  /// End the current rendering frame and present it to the screen
  ///
  /// This must be called after all drawing operations for the frame.
  /// It submits the command buffer and presents the rendered frame.
  ///
  /// # Errors
  ///
  /// Returns an error if the surface is lost or out of memory.
  ///
  /// # Panics
  ///
  /// Panics if called without first calling `begin_frame`.
  pub fn end_frame(&mut self) -> Result<()> {
    let output = self
      .current_output
      .take()
      .expect("end_frame called without begin_frame");
    let view = self
      .current_view
      .take()
      .expect("end_frame called without begin_frame");
    let mut encoder = self
      .current_encoder
      .take()
      .expect("end_frame called without begin_frame");

    // Clear the screen
    {
      let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Clear Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           &view,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Clear(wgpu::Color {
              r: self.background_color.r as f64,
              g: self.background_color.g as f64,
              b: self.background_color.b as f64,
              a: self.background_color.a as f64,
            }),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });
    }

    // Queue and draw text
    if !self.text_strings.is_empty() {
      use wgpu_text::glyph_brush::Text;

      // Build sections from stored data
      let sections: Vec<Section> = self
        .text_strings
        .iter()
        .zip(self.text_positions.iter())
        .map(|(texts, pos)| {
          let mut section = Section::default().with_screen_position(*pos);
          for (content, size, color) in texts {
            section = section.add_text(
              Text::new(content.as_str())
                .with_color(*color)
                .with_scale(*size),
            );
          }
          section
        })
        .collect();

      self
        .brush
        .queue(&self.device, &self.queue, sections)
        .map_err(|e| RendererError::TextRendering(e.to_string()))?;

      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Text Render Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           &view,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });

      self.brush.draw(&mut render_pass);
    }

    self.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
  }

  /// Set the background color for clearing the screen
  pub fn set_background_color(&mut self, color: Color) {
    self.background_color = color;
  }

  /// Draw text at the specified position
  ///
  /// Text is queued and will be rendered when `end_frame` is called.
  ///
  /// # Example
  ///
  /// ```rust,no_run
  /// # use the_editor_renderer::{Renderer, TextSection, Color};
  /// # fn draw(renderer: &mut Renderer) {
  /// renderer.draw_text(TextSection::simple(10.0, 10.0, "Hello", 16.0, Color::WHITE));
  /// # }
  /// ```
  pub fn draw_text(&mut self, section: TextSection) {
    let mut texts = Vec::new();

    for text in section.texts {
      texts.push((text.content, text.style.size, [
        text.style.color.r,
        text.style.color.g,
        text.style.color.b,
        text.style.color.a,
      ]));
    }

    self.text_strings.push(texts);
    self.text_positions.push(section.position);
  }

  /// Get the current viewport width in pixels
  pub fn width(&self) -> u32 {
    self.size.width
  }

  /// Get the current viewport height in pixels
  pub fn height(&self) -> u32 {
    self.size.height
  }
}
