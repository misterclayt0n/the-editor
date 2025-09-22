//! GPU renderer implementation
//!
//! This module contains the core rendering functionality using wgpu.
//! **NOTE**: We may end up switching backends in the future.

use std::sync::Arc;

use wgpu::util::DeviceExt;
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

/// A single rectangle instance to be rendered
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectInstance {
  position:      [f32; 2], // 0  .. 8
  size:          [f32; 2], // 8  .. 16
  color:         [f32; 4], // 16 .. 32
  corner_radius: f32,      // 32 .. 36
  _pad0:         [f32; 2], // 36 .. 44 (pad to 8-byte boundary for next vec2)
  glow_center:   [f32; 2], // 44 .. 52
  glow_radius:   f32,      // 52 .. 56
  effect_kind:   f32,      // 56 .. 60
}

/// Vertex data for a rectangle (using instanced rendering)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
  position: [f32; 2],
}

/// Uniform data for transforming coordinates
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectUniforms {
  screen_size: [f32; 2],
}

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

  // Primitive rendering data
  rect_render_pipeline: wgpu::RenderPipeline,
  rect_vertex_buffer:   wgpu::Buffer,
  rect_uniform_buffer:  wgpu::Buffer,
  rect_bind_group:      wgpu::BindGroup,

  // Primitive drawing commands for this frame
  rect_instances: Vec<RectInstance>,
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

    // Set up rectangle rendering pipeline
    let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label:  Some("Rectangle Shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("rect.wgsl").into()),
    });

    // Create uniform buffer for screen size
    let rect_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label:              Some("Rectangle Uniform Buffer"),
      size:               std::mem::size_of::<RectUniforms>() as u64,
      usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });

    // Create bind group layout for uniforms
    let rect_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label:   Some("Rectangle Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
          binding:    0,
          visibility: wgpu::ShaderStages::VERTEX,
          ty:         wgpu::BindingType::Buffer {
            ty:                 wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size:   None,
          },
          count:      None,
        }],
      });

    // Create bind group
    let rect_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label:   Some("Rectangle Bind Group"),
      layout:  &rect_bind_group_layout,
      entries: &[wgpu::BindGroupEntry {
        binding:  0,
        resource: rect_uniform_buffer.as_entire_binding(),
      }],
    });

    // Create pipeline layout
    let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label:                Some("Rectangle Pipeline Layout"),
      bind_group_layouts:   &[&rect_bind_group_layout],
      push_constant_ranges: &[],
    });

    // Rectangle vertex buffer (a quad)
    let rect_vertices = [
      RectVertex {
        position: [0.0, 0.0],
      }, // Top-left
      RectVertex {
        position: [1.0, 0.0],
      }, // Top-right
      RectVertex {
        position: [0.0, 1.0],
      }, // Bottom-left
      RectVertex {
        position: [1.0, 1.0],
      }, // Bottom-right
    ];

    let rect_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label:    Some("Rectangle Vertex Buffer"),
      contents: bytemuck::cast_slice(&rect_vertices),
      usage:    wgpu::BufferUsages::VERTEX,
    });

    // Create render pipeline
    let rect_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label:         Some("Rectangle Render Pipeline"),
      layout:        Some(&rect_pipeline_layout),
      vertex:        wgpu::VertexState {
        module:              &rect_shader,
        entry_point:         Some("vs_main"),
        buffers:             &[
          // Vertex buffer
          wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectVertex>() as u64,
            step_mode:    wgpu::VertexStepMode::Vertex,
            attributes:   &[wgpu::VertexAttribute {
              offset:          0,
              shader_location: 0,
              format:          wgpu::VertexFormat::Float32x2,
            }],
          },
          // Instance buffer
          wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectInstance>() as u64,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes:   &[
              // rect_position
              wgpu::VertexAttribute {
                offset:          0,
                shader_location: 1,
                format:          wgpu::VertexFormat::Float32x2,
              },
              // rect_size
              wgpu::VertexAttribute {
                offset:          (std::mem::size_of::<[f32; 2]>()) as u64, // 8
                shader_location: 2,
                format:          wgpu::VertexFormat::Float32x2,
              },
              // rect_color
              wgpu::VertexAttribute {
                offset:          (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<[f32; 2]>())
                  as u64, // 16
                shader_location: 3,
                format:          wgpu::VertexFormat::Float32x4,
              },
              // corner_radius
              wgpu::VertexAttribute {
                offset:          (std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 4]>()) as u64, // 32
                shader_location: 4,
                format:          wgpu::VertexFormat::Float32,
              },
              // glow_center (after an internal pad of 8 bytes)
              wgpu::VertexAttribute {
                offset:          (std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 4]>()
                  + std::mem::size_of::<f32>()
                  + std::mem::size_of::<[f32; 2]>()) as u64, // 44
                shader_location: 5,
                format:          wgpu::VertexFormat::Float32x2,
              },
              // glow_radius
              wgpu::VertexAttribute {
                offset:          (std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 4]>()
                  + std::mem::size_of::<f32>()
                  + std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 2]>()) as u64, // 52
                shader_location: 6,
                format:          wgpu::VertexFormat::Float32,
              },
              // effect_kind
              wgpu::VertexAttribute {
                offset:          (std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 4]>()
                  + std::mem::size_of::<f32>()
                  + std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<[f32; 2]>()
                  + std::mem::size_of::<f32>()) as u64, // 56
                shader_location: 7,
                format:          wgpu::VertexFormat::Float32,
              },
            ],
          },
        ],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
      },
      fragment:      Some(wgpu::FragmentState {
        module:              &rect_shader,
        entry_point:         Some("fs_main"),
        targets:             &[Some(wgpu::ColorTargetState {
          format:     config.format,
          blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
          write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
      }),
      primitive:     wgpu::PrimitiveState {
        topology:           wgpu::PrimitiveTopology::TriangleStrip,
        strip_index_format: None,
        front_face:         wgpu::FrontFace::Ccw,
        cull_mode:          None,
        unclipped_depth:    false,
        polygon_mode:       wgpu::PolygonMode::Fill,
        conservative:       false,
      },
      depth_stencil: None,
      multisample:   wgpu::MultisampleState {
        count:                     1,
        mask:                      !0,
        alpha_to_coverage_enabled: false,
      },
      multiview:     None,
      cache:         None,
    });

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
      rect_render_pipeline,
      rect_vertex_buffer,
      rect_uniform_buffer,
      rect_bind_group,
      rect_instances: Vec::new(),
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
    self.rect_instances.clear();

    // Update uniform buffer with current screen size
    let uniforms = RectUniforms {
      screen_size: [self.size.width as f32, self.size.height as f32],
    };
    self.queue.write_buffer(
      &self.rect_uniform_buffer,
      0,
      bytemuck::cast_slice(&[uniforms]),
    );

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

    // Draw rectangles
    if !self.rect_instances.is_empty() {
      // Create instance buffer for this frame
      let instance_buffer = self
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
          label:    Some("Rectangle Instance Buffer"),
          contents: bytemuck::cast_slice(&self.rect_instances),
          usage:    wgpu::BufferUsages::VERTEX,
        });

      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Rectangle Render Pass"),
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

      render_pass.set_pipeline(&self.rect_render_pipeline);
      render_pass.set_bind_group(0, &self.rect_bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
      render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
      render_pass.draw(0..4, 0..self.rect_instances.len() as u32);
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

  /// Draw a rectangle at the specified position with the given size and color
  ///
  /// # Example
  ///
  /// ```rust,no_run
  /// # use the_editor_renderer::{Renderer, Color};
  /// # fn draw(renderer: &mut Renderer) {
  /// renderer.draw_rect(10.0, 10.0, 100.0, 50.0, Color::new(1.0, 0.0, 0.0, 1.0));
  /// # }
  /// ```
  pub fn draw_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Color) {
    self.rect_instances.push(RectInstance {
      position:      [x, y],
      size:          [width, height],
      color:         [color.r, color.g, color.b, color.a],
      corner_radius: 0.0,
      _pad0:         [0.0, 0.0],
      glow_center:   [0.0, 0.0],
      glow_radius:   0.0,
      effect_kind:   0.0,
    });
  }

  /// Draw a rounded rectangle at the specified position with the given size,
  /// color, and corner radius
  ///
  /// # Example
  ///
  /// ```rust,no_run
  /// # use the_editor_renderer::{Renderer, Color};
  /// # fn draw(renderer: &mut Renderer) {
  /// renderer.draw_rounded_rect(10.0, 10.0, 100.0, 50.0, 8.0, Color::new(0.0, 1.0, 0.0, 1.0));
  /// # }
  /// ```
  pub fn draw_rounded_rect(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    color: Color,
  ) {
    self.rect_instances.push(RectInstance {
      position: [x, y],
      size: [width, height],
      color: [color.r, color.g, color.b, color.a],
      corner_radius,
      _pad0: [0.0, 0.0],
      glow_center: [0.0, 0.0],
      glow_radius: 0.0,
      effect_kind: 0.0,
    });
  }

  /// Draw a rounded rectangle glow overlay, clipped to the rounded rect.
  /// `center_x`, `center_y` are absolute pixels; `radius` in pixels controls
  /// falloff size.
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
    self.rect_instances.push(RectInstance {
      position: [x, y],
      size: [width, height],
      color: [color.r, color.g, color.b, color.a],
      corner_radius,
      _pad0: [0.0, 0.0],
      glow_center: [center_x - x, center_y - y],
      glow_radius: radius,
      effect_kind: 1.0,
    });
  }

  /// Draw only the rounded-rect outline (stroke), with given thickness in
  /// pixels.
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
    self.rect_instances.push(RectInstance {
      position: [x, y],
      size: [width, height],
      color: [color.r, color.g, color.b, color.a],
      corner_radius,
      _pad0: [0.0, 0.0],
      glow_center: [0.0, 0.0],
      glow_radius: thickness.max(0.5),
      effect_kind: 2.0,
    });
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
