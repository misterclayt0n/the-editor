use std::{
  borrow::Cow,
  fs,
  path::Path,
  sync::Arc,
};

use anyhow::anyhow;
use glyphon::{
  Attrs,
  AttrsOwned,
  Buffer,
  Cache,
  Color as GlyphColor,
  Family,
  FontSystem,
  Metrics,
  Resolution,
  Shaping,
  SwashCache,
  TextArea,
  TextAtlas,
  TextBounds,
  TextRenderer,
  Viewport,
  Wrap,
};
use wgpu::{
  self,
  CompositeAlphaMode,
  util::DeviceExt,
};
use winit::{
  dpi::PhysicalSize,
  window::Window,
};

use crate::{
  Color,
  RendererError,
  Result,
  TextSection,
};

const LINE_HEIGHT_FACTOR: f32 = 1.2;

/// A single rectangle instance to be rendered
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectInstance {
  position:      [f32; 2],
  size:          [f32; 2],
  color:         [f32; 4],
  corner_radius: f32,
  _pad0:         [f32; 2],
  glow_center:   [f32; 2],
  glow_radius:   f32,
  effect_kind:   f32,
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

struct TextCommand {
  position:  (f32, f32),
  cache_key: crate::text_cache::ShapedTextKey, // Key to retrieve buffer from cache
  bounds:    TextBounds,
}

/// Pool of reusable text buffers for better performance
struct BufferPool {
  buffers: Vec<Buffer>,
  metrics: Metrics,
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
  surface:        wgpu::Surface<'static>,
  device:         wgpu::Device,
  queue:          wgpu::Queue,
  config:         wgpu::SurfaceConfiguration,
  size:           PhysicalSize<u32>,
  /// Tracks a resize that needs a surface reconfigure. Applied in begin_frame.
  pending_resize: Option<PhysicalSize<u32>>,

  cache:         Cache,
  font_system:   FontSystem,
  swash_cache:   SwashCache,
  viewport:      Viewport,
  text_atlas:    TextAtlas,
  text_renderer: TextRenderer,

  // Frame state.
  current_output:  Option<wgpu::SurfaceTexture>,
  current_view:    Option<wgpu::TextureView>,
  current_encoder: Option<wgpu::CommandEncoder>,

  // Configuration.
  background_color: Color,

  // Pending draw data for the current frame.
  rect_instances: Vec<RectInstance>,
  text_commands:  Vec<TextCommand>,

  // Text batching for performance
  pending_text_batch: Option<(TextSection, f32, f32)>, // Accumulate text segments

  // Rectangle pipeline resources.
  rect_render_pipeline: wgpu::RenderPipeline,
  rect_vertex_buffer:   wgpu::Buffer,
  rect_uniform_buffer:  wgpu::Buffer,
  rect_bind_group:      wgpu::BindGroup,

  // Text metrics / font tracking.
  font_family: String,
  font_size:   f32,
  cell_width:  f32,
  cell_height: f32,

  // Performance optimization: disable ligature protection for better performance
  disable_ligature_protection: bool,

  // Buffer pool for text rendering performance
  buffer_pool: BufferPool,

  // Shaped text cache for avoiding re-shaping
  shaped_text_cache: crate::text_cache::ShapedTextCache,
}

impl Renderer {
  /// Create a new renderer with the given window
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
      .map_err(|e| RendererError::Configuration(format!("Failed to get adapter: {e}")))?;

    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits:   wgpu::Limits::default(),
        label:             None,
        memory_hints:      Default::default(),
        trace:             Default::default(),
      })
      .await
      .map_err(|e| RendererError::Configuration(format!("Failed to create device: {e}")))?;

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
      .formats
      .iter()
      .find(|f| f.is_srgb())
      .copied()
      .unwrap_or(surface_caps.formats[0]);

    // Prefer low-latency present mode when available.
    let present_mode = surface_caps
      .present_modes
      .iter()
      .copied()
      .find(|m| *m == wgpu::PresentMode::Mailbox)
      .or_else(|| {
        surface_caps
          .present_modes
          .iter()
          .copied()
          .find(|m| *m == wgpu::PresentMode::Immediate)
      })
      .unwrap_or(wgpu::PresentMode::Fifo);

    let config = wgpu::SurfaceConfiguration {
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
      format: surface_format,
      width: size.width.max(1),
      height: size.height.max(1),
      present_mode,
      alpha_mode: CompositeAlphaMode::Auto,
      view_formats: vec![],
      desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Rectangle pipeline setup.
    let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label:  Some("Rectangle Shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("rect.wgsl").into()),
    });

    let rect_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label:              Some("Rectangle Uniform Buffer"),
      size:               std::mem::size_of::<RectUniforms>() as u64,
      usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });

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

    let rect_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label:   Some("Rectangle Bind Group"),
      layout:  &rect_bind_group_layout,
      entries: &[wgpu::BindGroupEntry {
        binding:  0,
        resource: rect_uniform_buffer.as_entire_binding(),
      }],
    });

    let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label:                Some("Rectangle Pipeline Layout"),
      bind_group_layouts:   &[&rect_bind_group_layout],
      push_constant_ranges: &[],
    });

    let rect_vertices = [
      RectVertex {
        position: [0.0, 0.0],
      },
      RectVertex {
        position: [1.0, 0.0],
      },
      RectVertex {
        position: [0.0, 1.0],
      },
      RectVertex {
        position: [1.0, 1.0],
      },
    ];

    let rect_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label:    Some("Rectangle Vertex Buffer"),
      contents: bytemuck::cast_slice(&rect_vertices),
      usage:    wgpu::BufferUsages::VERTEX,
    });

    let rect_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label:         Some("Rectangle Render Pipeline"),
      layout:        Some(&rect_pipeline_layout),
      vertex:        wgpu::VertexState {
        module:              &rect_shader,
        entry_point:         Some("vs_main"),
        buffers:             &[
          wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectVertex>() as u64,
            step_mode:    wgpu::VertexStepMode::Vertex,
            attributes:   &[wgpu::VertexAttribute {
              offset:          0,
              shader_location: 0,
              format:          wgpu::VertexFormat::Float32x2,
            }],
          },
          wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectInstance>() as u64,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes:   &[
              wgpu::VertexAttribute {
                offset:          0,
                shader_location: 1,
                format:          wgpu::VertexFormat::Float32x2,
              },
              wgpu::VertexAttribute {
                offset:          8,
                shader_location: 2,
                format:          wgpu::VertexFormat::Float32x2,
              },
              wgpu::VertexAttribute {
                offset:          16,
                shader_location: 3,
                format:          wgpu::VertexFormat::Float32x4,
              },
              wgpu::VertexAttribute {
                offset:          32,
                shader_location: 4,
                format:          wgpu::VertexFormat::Float32,
              },
              wgpu::VertexAttribute {
                offset:          44,
                shader_location: 5,
                format:          wgpu::VertexFormat::Float32x2,
              },
              wgpu::VertexAttribute {
                offset:          52,
                shader_location: 6,
                format:          wgpu::VertexFormat::Float32,
              },
              wgpu::VertexAttribute {
                offset:          56,
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
          format:     surface_format,
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
      multisample:   wgpu::MultisampleState::default(),
      multiview:     None,
      cache:         None,
    });

    // Glyphon initialization.
    let cache = Cache::new(&device);
    let mut font_system = FontSystem::new();
    let swash_cache = SwashCache::new();
    let mut viewport = Viewport::new(&device, &cache);
    let mut text_atlas = TextAtlas::new(&device, &queue, &cache, surface_format);
    let text_renderer = TextRenderer::new(
      &mut text_atlas,
      &device,
      wgpu::MultisampleState::default(),
      None,
    );

    viewport.update(&queue, Resolution {
      width:  config.width,
      height: config.height,
    });

    // Load default font and configure metrics.
    const FONT_BYTES: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");
    let default_family =
      resolve_family_name(FONT_BYTES).unwrap_or_else(|| "JetBrains Mono".to_string());
    font_system.db_mut().load_font_data(FONT_BYTES.to_vec());

    let mut renderer = Self {
      surface,
      device,
      queue,
      config,
      size,
      pending_resize: None,
      cache,
      font_system,
      swash_cache,
      viewport,
      text_atlas,
      text_renderer,
      current_output: None,
      current_view: None,
      current_encoder: None,
      background_color: Color::new(0.1, 0.1, 0.15, 1.0),
      rect_instances: Vec::new(),
      text_commands: Vec::new(),
      rect_render_pipeline,
      rect_vertex_buffer,
      rect_uniform_buffer,
      rect_bind_group,
      font_family: default_family,
      font_size: 16.0,
      cell_width: 8.0,
      cell_height: 16.0,
      disable_ligature_protection: false,
      buffer_pool: BufferPool {
        buffers: Vec::with_capacity(4),
        metrics: Metrics::new(16.0, 16.0 * LINE_HEIGHT_FACTOR),
      },
      shaped_text_cache: crate::text_cache::ShapedTextCache::new(1000), /* Cache up to 1000 text
                                                                         * runs */
      pending_text_batch: None,
    };

    renderer.recalculate_metrics();

    Ok(renderer)
  }

  fn recalculate_metrics(&mut self) {
    let metrics = Metrics::new(self.font_size, self.font_size * LINE_HEIGHT_FACTOR);
    let mut buffer = Buffer::new(&mut self.font_system, metrics);
    buffer.set_wrap(&mut self.font_system, Wrap::None);

    let attrs = Attrs::new()
      .family(Family::Name(self.font_family.as_str()))
      .metrics(metrics);
    buffer.set_text(&mut self.font_system, "0", &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut self.font_system, false);

    if let Some(run) = buffer.layout_runs().next() {
      self.cell_width = run.line_w.max(1.0);
      self.cell_height = run.line_height.max(self.font_size);
    } else {
      self.cell_height = self.font_size * LINE_HEIGHT_FACTOR;
      self.cell_width = (self.font_size * 0.6).max(1.0);
    }
  }

  /// Resize the renderer viewport
  pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
    if new_size.width > 0 && new_size.height > 0 {
      // Defer heavy surface reconfiguration until the next frame to
      // coalesce rapid resize events into a single reconfigure.
      self.size = new_size;
      self.pending_resize = Some(new_size);
    }
  }

  /// Update viewport dimensions. Returns true if the size changed.
  pub fn update_viewport(&mut self, width: u32, height: u32) -> bool {
    if self.size.width == width && self.size.height == height {
      return false;
    }

    self.resize(PhysicalSize::new(width, height));
    true
  }

  /// Begin a new rendering frame
  pub fn begin_frame(&mut self) -> Result<()> {
    // Apply any pending resize before acquiring the next frame.
    if let Some(new_size) = self.pending_resize.take() {
      self.config.width = new_size.width.max(1);
      self.config.height = new_size.height.max(1);
      self.surface.configure(&self.device, &self.config);
      self.viewport.update(&self.queue, Resolution {
        width:  self.config.width,
        height: self.config.height,
      });
    }

    // Acquire the surface texture with robust error handling during resizes.
    let output = match self.surface.get_current_texture() {
      Ok(o) => o,
      Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
        // Reconfigure and retry once.
        self.surface.configure(&self.device, &self.config);
        match self.surface.get_current_texture() {
          Ok(o2) => o2,
          Err(wgpu::SurfaceError::Timeout) => {
            // Skip this frame quietly.
            return Err(RendererError::SkipFrame);
          },
          Err(e) => {
            return Err(RendererError::Runtime(format!(
              "Failed to acquire frame after reconfigure: {e}"
            )));
          },
        }
      },
      Err(wgpu::SurfaceError::Timeout) => {
        // Skip this frame quietly.
        return Err(RendererError::SkipFrame);
      },
      Err(e) => {
        return Err(RendererError::Runtime(format!(
          "Failed to acquire frame: {e}"
        )));
      },
    };
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
    self.rect_instances.clear();

    // Advance frame counter for cache LRU tracking
    self.shaped_text_cache.next_frame();

    // Clear text commands (buffers are now kept in cache)
    self.text_commands.clear();

    // Clear any pending text batch
    self.pending_text_batch = None;

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

  /// End the current frame and present it to the screen
  pub fn end_frame(&mut self) -> Result<()> {
    // Flush any pending text batch before rendering
    self.flush_text_batch();
    let output = self
      .current_output
      .take()
      .ok_or_else(|| RendererError::Runtime("end_frame called without begin_frame".into()))?;
    let view = self
      .current_view
      .take()
      .ok_or_else(|| RendererError::Runtime("end_frame called without begin_frame".into()))?;
    let mut encoder = self
      .current_encoder
      .take()
      .ok_or_else(|| RendererError::Runtime("end_frame called without begin_frame".into()))?;

    // Clear background
    {
      let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Clear Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           &view,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Clear(linear_clear_color(self.background_color)),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });
    }

    // Draw instanced rectangles
    if !self.rect_instances.is_empty() {
      let instance_buffer = self
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
          label:    Some("Rectangle Instance Buffer"),
          contents: bytemuck::cast_slice(&self.rect_instances),
          usage:    wgpu::BufferUsages::VERTEX,
        });

      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

      pass.set_pipeline(&self.rect_render_pipeline);
      pass.set_bind_group(0, &self.rect_bind_group, &[]);
      pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
      pass.set_vertex_buffer(1, instance_buffer.slice(..));
      pass.draw(0..4, 0..self.rect_instances.len() as u32);
    }

    if !self.text_commands.is_empty() {
      self
        .text_renderer
        .prepare(
          &self.device,
          &self.queue,
          &mut self.font_system,
          &mut self.text_atlas,
          &self.viewport,
          self.text_commands.iter().filter_map(|command| {
            // Get buffer from cache
            self
              .shaped_text_cache
              .entries
              .get(&command.cache_key)
              .map(|entry| {
                TextArea {
                  buffer:        &entry.buffer,
                  left:          command.position.0,
                  top:           command.position.1,
                  scale:         1.0,
                  bounds:        command.bounds,
                  default_color: GlyphColor::rgba(255, 255, 255, 255),
                  custom_glyphs: &[],
                }
              })
          }),
          &mut self.swash_cache,
        )
        .map_err(|e| RendererError::TextRendering(e.to_string()))?;

      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

      self
        .text_renderer
        .render(&self.text_atlas, &self.viewport, &mut pass)
        .map_err(|e| RendererError::TextRendering(e.to_string()))?;
    }

    self.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
  }

  /// Set the background color for clearing the screen
  pub fn set_background_color(&mut self, color: Color) {
    self.background_color = color;
  }

  /// Batch multiple text segments for efficient rendering.
  /// Call this instead of draw_text for better performance when rendering many
  /// small text segments.
  pub fn draw_text_batched(&mut self, section: TextSection) {
    if section.texts.is_empty() {
      return;
    }

    // Check if we can batch with existing pending text
    if let Some((ref mut batch, batch_x, batch_y)) = self.pending_text_batch {
      let x_diff = (section.position.0 - batch_x).abs();
      let y_diff = (section.position.1 - batch_y).abs();

      // Batch if: same Y position (same line) AND text is adjacent or very close
      if y_diff < 1.0 && x_diff < self.cell_width * 50.0 {
        // Within 50 chars on same line
        // Merge into existing batch
        if x_diff < 1.0 {
          // Same position, just append
          batch.texts.extend(section.texts);
        } else {
          // Different x position on same line - merge with appropriate spacing
          batch.texts.extend(section.texts);
        }
        return;
      } else {
        // Different line or too far apart, flush existing batch
        let batch_to_flush = self.pending_text_batch.take().unwrap().0;
        self.draw_text_internal(batch_to_flush);
      }
    }

    // Start new batch
    let pos_x = section.position.0;
    let pos_y = section.position.1;
    self.pending_text_batch = Some((section, pos_x, pos_y));
  }

  /// Flush any pending batched text
  pub fn flush_text_batch(&mut self) {
    if let Some((batch, ..)) = self.pending_text_batch.take() {
      self.draw_text_internal(batch);
    }
  }

  /// Draw text using glyphon buffers (immediate mode)
  pub fn draw_text(&mut self, section: TextSection) {
    // For compatibility, immediately draw without batching
    self.draw_text_internal(section);
  }

  /// Internal text drawing implementation using cached shaped text
  fn draw_text_internal(&mut self, section: TextSection) {
    if section.texts.is_empty() {
      return;
    }

    let width = self.config.width as f32;
    let height = self.config.height as f32;

    // Build the full text string
    let mut full_text = String::new();
    for segment in &section.texts {
      if !segment.content.is_empty() {
        full_text.push_str(&segment.content);
      }
    }

    if full_text.is_empty() {
      return;
    }

    // Get first color for cache key
    let first_color = section.texts[0].style.color;

    // Create cache key
    let cache_key = crate::text_cache::ShapedTextKey {
      text:          full_text.clone(),
      metrics:       (
        (self.font_size * 100.0) as u32,
        (self.cell_height * 100.0) as u32,
      ),
      color:         [
        (first_color.r * 255.0) as u8,
        (first_color.g * 255.0) as u8,
        (first_color.b * 255.0) as u8,
        (first_color.a * 255.0) as u8,
      ],
      position_hash: {
        use std::hash::{
          Hash,
          Hasher,
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        section.position.0.to_bits().hash(&mut hasher);
        section.position.1.to_bits().hash(&mut hasher);
        hasher.finish()
      },
    };

    let base_metrics = Metrics::new(self.font_size, self.cell_height);

    // Check if we already have this text shaped in cache
    if !self.shaped_text_cache.entries.contains_key(&cache_key) {
      // Need to create and shape new buffer
      self.shaped_text_cache.misses += 1;

      let mut buffer = if let Some(mut pooled) = self.buffer_pool.buffers.pop() {
        pooled.set_metrics(&mut self.font_system, base_metrics);
        pooled.set_size(&mut self.font_system, Some(width), Some(height));
        pooled
      } else {
        let mut buffer = Buffer::new(&mut self.font_system, base_metrics);
        buffer.set_wrap(&mut self.font_system, Wrap::None);
        buffer.set_size(&mut self.font_system, Some(width), Some(height));
        buffer
      };

      // Build spans for styled text
      let mut spans = Vec::new();
      let mut cursor = 0usize;
      let family = self.font_family.clone();

      for segment in section.texts {
        if segment.content.is_empty() {
          continue;
        }

        let start = cursor;
        cursor += segment.content.len();

        let seg_metrics = Metrics::new(segment.style.size, segment.style.size * LINE_HEIGHT_FACTOR);
        let attrs = Attrs::new()
          .family(Family::Name(family.as_str()))
          .metrics(seg_metrics)
          .color(to_glyph_color(segment.style.color));

        spans.push((start..cursor, AttrsOwned::new(&attrs)));
      }

      let default_attrs = Attrs::new()
        .family(Family::Name(family.as_str()))
        .metrics(base_metrics);

      buffer.set_rich_text(
        &mut self.font_system,
        spans
          .iter()
          .map(|(range, attrs_owned)| (&full_text[range.clone()], attrs_owned.as_attrs())),
        &default_attrs,
        Shaping::Advanced,
        None,
      );

      buffer.shape_until_scroll(&mut self.font_system, false);

      // Store in cache
      if self.shaped_text_cache.entries.len() >= 1000 {
        self.shaped_text_cache.evict_lru();
      }

      let entry = crate::text_cache::CachedShapedText {
        buffer,
        last_used_frame: self.shaped_text_cache.current_frame,
        generation: self.shaped_text_cache.current_generation,
      };

      self
        .shaped_text_cache
        .entries
        .insert(cache_key.clone(), entry);
    } else {
      // Update cache hit stats
      self.shaped_text_cache.hits += 1;
      if let Some(entry) = self.shaped_text_cache.entries.get_mut(&cache_key) {
        entry.last_used_frame = self.shaped_text_cache.current_frame;
      }
    }

    let bounds = TextBounds {
      left:   0,
      top:    0,
      right:  self.config.width as i32,
      bottom: self.config.height as i32,
    };

    // Store the command with cache key for deferred rendering
    let bounds = TextBounds {
      left:   0,
      top:    0,
      right:  self.config.width as i32,
      bottom: self.config.height as i32,
    };

    self.text_commands.push(TextCommand {
      position: section.position,
      cache_key,
      bounds,
    });
  }

  /// Draw a rectangle at the specified position with the given size and color
  pub fn draw_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Color) {
    self.rect_instances.push(RectInstance {
      position:      [x, y],
      size:          [width, height],
      color:         color_to_linear(color),
      corner_radius: 0.0,
      _pad0:         [0.0, 0.0],
      glow_center:   [0.0, 0.0],
      glow_radius:   0.0,
      effect_kind:   0.0,
    });
  }

  /// Draw a filled rounded rectangle
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
      color: color_to_linear(color),
      corner_radius,
      _pad0: [0.0, 0.0],
      glow_center: [0.0, 0.0],
      glow_radius: 0.0,
      effect_kind: 0.0,
    });
  }

  /// Draw a rounded rectangle glow overlay, clipped to the rounded rect
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
      color: color_to_linear(color),
      corner_radius,
      _pad0: [0.0, 0.0],
      glow_center: [center_x - x, center_y - y],
      glow_radius: radius,
      effect_kind: 1.0,
    });
  }

  /// Draw only the rounded-rect outline (stroke)
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
      color: color_to_linear(color),
      corner_radius,
      _pad0: [0.0, 0.0],
      glow_center: [0.0, 0.0],
      glow_radius: thickness.max(0.5),
      effect_kind: 2.0,
    });
  }

  /// Configure the monospaced font family and size used for layout calculations
  pub fn configure_font(&mut self, family: &str, size: f32) {
    self.font_family = family.to_string();
    self.font_size = size.max(1.0);
    self.recalculate_metrics();
  }

  /// Configure the font by reading TTF/OTF/TTC bytes and installing them into
  /// the font system
  pub fn configure_font_from_bytes(&mut self, bytes: Vec<u8>, size: f32) -> anyhow::Result<()> {
    let family = resolve_family_name(&bytes)
      .ok_or_else(|| anyhow!("could not resolve font family from provided bytes"))?;

    self.font_system.db_mut().load_font_data(bytes);
    self.font_family = family;
    self.font_size = size.max(1.0);
    self.recalculate_metrics();
    Ok(())
  }

  /// Configure the font by reading the specified font file path
  pub fn configure_font_from_path<P: AsRef<Path>>(
    &mut self,
    path: P,
    size: f32,
  ) -> anyhow::Result<()> {
    let bytes = fs::read(path)?;
    self.configure_font_from_bytes(bytes, size)
  }

  /// Current family name configured for text rendering.
  pub fn current_font_family(&self) -> &str {
    &self.font_family
  }

  /// Width in physical pixels.
  pub fn width(&self) -> u32 {
    self.size.width
  }

  /// Height in physical pixels.
  pub fn height(&self) -> u32 {
    self.size.height
  }

  /// Current cell width used for layout heuristics.
  pub fn cell_width(&self) -> f32 {
    self.cell_width
  }

  /// Current cell height used for layout heuristics.
  pub fn cell_height(&self) -> f32 {
    self.cell_height
  }

  /// Enable or disable ligature protection for performance.
  /// When disabled, ligatures will render normally which may cause visual
  /// artifacts with some fonts, but will significantly improve performance.
  pub fn set_ligature_protection(&mut self, enabled: bool) {
    self.disable_ligature_protection = !enabled;
  }
}

fn to_glyph_color(color: Color) -> GlyphColor {
  let r = (color.r.clamp(0.0, 1.0) * 255.0).round() as u8;
  let g = (color.g.clamp(0.0, 1.0) * 255.0).round() as u8;
  let b = (color.b.clamp(0.0, 1.0) * 255.0).round() as u8;
  let a = (color.a.clamp(0.0, 1.0) * 255.0).round() as u8;
  GlyphColor::rgba(r, g, b, a)
}

fn color_to_linear(color: Color) -> [f32; 4] {
  [
    srgb_to_linear(color.r),
    srgb_to_linear(color.g),
    srgb_to_linear(color.b),
    color.a.clamp(0.0, 1.0),
  ]
}

fn srgb_to_linear(channel: f32) -> f32 {
  let c = channel.clamp(0.0, 1.0);
  if c <= 0.04045 {
    c / 12.92
  } else {
    ((c + 0.055) / 1.055).powf(2.4)
  }
}

fn linear_clear_color(color: Color) -> wgpu::Color {
  let [r, g, b, a] = color_to_linear(color);
  wgpu::Color {
    r: r as f64,
    g: g as f64,
    b: b as f64,
    a: a as f64,
  }
}

/// Try to resolve a readable family name from raw font bytes using fontdb.
fn resolve_family_name(bytes: &[u8]) -> Option<String> {
  let mut db = fontdb::Database::new();
  let before = db.faces().len();
  db.load_font_data(bytes.to_vec());
  let faces = db.faces();
  let face = faces.get(before)?;

  if let Some((name, _lang)) = face.families.first() {
    return Some(name.clone());
  }

  if !face.post_script_name.is_empty() {
    return Some(face.post_script_name.clone());
  }

  None
}

fn protect_problematic_ligatures(text: &str) -> Cow<'_, str> {
  let mut protected: Option<String> = None;

  for pattern in PROBLEM_LIGATURES {
    let current = protected.as_deref().unwrap_or(text);
    if !current.contains(pattern) {
      continue;
    }

    let replacement = join_with_word_joiner(pattern);
    let updated = current.replace(pattern, replacement.as_str());
    protected = Some(updated);
  }

  match protected {
    Some(result) => Cow::Owned(result),
    None => Cow::Borrowed(text),
  }
}

fn join_with_word_joiner(pattern: &str) -> String {
  let mut chars = pattern.chars();
  let Some(first) = chars.next() else {
    return String::new();
  };

  let mut joined =
    String::with_capacity(pattern.len() + (pattern.len().saturating_sub(1) * WORD_JOINER.len()));
  joined.push(first);
  for ch in chars {
    joined.push_str(WORD_JOINER);
    joined.push(ch);
  }

  joined
}

const WORD_JOINER: &str = "\u{2060}";

const PROBLEM_LIGATURES: &[&str] = &[
  "<!--", ":?>", "|->", "<-|", "<=>", "<|>", "</>", "..<", ">..", "||=", "&&=", "+++", "---",
  "-->", ">>=", "<<=", "=/=", "===", "!==", "=!=", "/**", "<->", "...", "??", "||", "&&", "|=",
  "&=", "++", "--", "/*", "*/", "!=", "==", "->", "<-", "=>", "<=", ">=", "::", ":?", "?:", "<|",
  "|>", ">>", "<<", "~>", "<~", "-~", "~-", "</", "/>", "?.", "..", "><", "<>", "|-", "-|",
];

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn protects_simple_two_char_pattern() {
    let result = protect_problematic_ligatures("->");
    let expected = format!("-{}>", WORD_JOINER);
    match result {
      Cow::Owned(owned) => assert_eq!(owned, expected),
      Cow::Borrowed(_) => panic!("expected owned cow for protected ligature"),
    }
  }

  #[test]
  fn protects_longer_pattern_before_shorter_variants() {
    let result = protect_problematic_ligatures("<->");
    let expected = format!("<{}-{}>", WORD_JOINER, WORD_JOINER);
    match result {
      Cow::Owned(owned) => assert_eq!(owned, expected),
      Cow::Borrowed(_) => panic!("expected owned cow for protected ligature"),
    }
  }

  #[test]
  fn leaves_non_ligature_text_unmodified() {
    let result = protect_problematic_ligatures("hello");
    match result {
      Cow::Borrowed(text) => assert_eq!(text, "hello"),
      Cow::Owned(_) => panic!("non-ligature text should remain borrowed"),
    }
  }
}
