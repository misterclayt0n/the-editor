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
  TextSegment,
  TextStyle,
  powerline_glyphs::PowerlineGlyph,
};

const LINE_HEIGHT_FACTOR: f32 = 1.3;

/// Atlas for Powerline glyph textures
struct PowerlineAtlas {
  textures:   std::collections::HashMap<PowerlineGlyph, wgpu::Texture>,
  views:      std::collections::HashMap<PowerlineGlyph, wgpu::TextureView>,
  sampler:    wgpu::Sampler,
  bind_group: Option<wgpu::BindGroup>,
}

impl PowerlineAtlas {
  fn new(device: &wgpu::Device, queue: &wgpu::Queue, cell_width: f32, cell_height: f32) -> Self {
    use crate::powerline_glyphs;

    let mut textures = std::collections::HashMap::new();
    let mut views = std::collections::HashMap::new();

    let width = cell_width.ceil() as u32;
    let height = cell_height.ceil() as u32;

    // Pre-render all Powerline glyphs
    let glyphs = [
      PowerlineGlyph::RightTriangle,
      PowerlineGlyph::LeftTriangle,
      PowerlineGlyph::RightRounded,
      PowerlineGlyph::LeftRounded,
    ];

    for glyph in glyphs {
      if let Some(pixmap) = powerline_glyphs::render_powerline_glyph(glyph, width, height) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
          label:           Some("Powerline Glyph"),
          size:            wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
          },
          mip_level_count: 1,
          sample_count:    1,
          dimension:       wgpu::TextureDimension::D2,
          format:          wgpu::TextureFormat::Rgba8Unorm,
          usage:           wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
          view_formats:    &[],
        });

        queue.write_texture(
          wgpu::ImageCopyTexture {
            texture:   &texture,
            mip_level: 0,
            origin:    wgpu::Origin3d::ZERO,
            aspect:    wgpu::TextureAspect::All,
          },
          pixmap.data(), // RGBA8 bytes
          wgpu::ImageDataLayout {
            offset:         0,
            bytes_per_row:  Some(4 * width),
            rows_per_image: Some(height),
          },
          wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
          },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        textures.insert(glyph, texture);
        views.insert(glyph, view);
      }
    }

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("Powerline Sampler"),
      address_mode_u: wgpu::AddressMode::ClampToEdge,
      address_mode_v: wgpu::AddressMode::ClampToEdge,
      address_mode_w: wgpu::AddressMode::ClampToEdge,
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      ..Default::default()
    });

    Self {
      textures,
      views,
      sampler,
      bind_group: None,
    }
  }
}

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
  effect_time:   f32,      // Animation progress [0..1]
  _pad1:         [f32; 3], // Padding to align to 16 bytes
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

/// Uniform data for blur shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BlurUniforms {
  direction:  [f32; 2],
  resolution: [f32; 2],
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

/// Saved font state for save/restore operations
#[derive(Clone, Debug)]
pub struct FontState {
  pub family:      String,
  pub size:        f32,
  pub cell_width:  f32,
  pub cell_height: f32,
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

  cache:                 Cache,
  font_system:           FontSystem,
  swash_cache:           SwashCache,
  viewport:              Viewport,
  text_atlas:            TextAtlas,
  text_renderer:         TextRenderer, // For regular text with stencil masking
  overlay_text_renderer: TextRenderer, // For overlay text without stencil masking

  // Frame state.
  current_output:  Option<wgpu::SurfaceTexture>,
  current_view:    Option<wgpu::TextureView>,
  current_encoder: Option<wgpu::CommandEncoder>,

  // Stencil buffer for masking regions
  stencil_texture: wgpu::Texture,
  stencil_view:    wgpu::TextureView,

  // Configuration.
  background_color: Color,

  // Pending draw data for the current frame.
  rect_instances:        Vec<RectInstance>,
  text_commands:         Vec<TextCommand>,
  overlay_text_commands: Vec<TextCommand>, // Text that ignores stencil mask (for UI overlays)

  // Text batching for performance
  pending_text_batch: Option<(TextSection, f32, f32)>, // Accumulate text segments
  is_overlay_mode:    bool,                            /* Whether current text should be added
                                                        * to overlay_text_commands */

  // Scissor rect stack for clipping text to specific regions
  scissor_rect_stack: Vec<(f32, f32, f32, f32)>, // (x, y, width, height)

  // Stencil mask rects for excluding text from specific regions (supports multiple overlays)
  stencil_mask_rects: Vec<(f32, f32, f32, f32)>, // (x, y, width, height)

  // Rectangle pipeline resources.
  rect_render_pipeline:   wgpu::RenderPipeline,
  rect_vertex_buffer:     wgpu::Buffer,
  rect_uniform_buffer:    wgpu::Buffer,
  rect_bind_group:        wgpu::BindGroup,
  // Reusable instance buffers for performance
  rect_instance_buffer:   Option<wgpu::Buffer>,
  rect_instance_capacity: usize,
  mask_instance_buffer:   Option<wgpu::Buffer>,
  mask_instance_capacity: usize,

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

  // Blur effect resources
  blur_pipeline:          wgpu::RenderPipeline,
  blur_bind_group_layout: wgpu::BindGroupLayout,
  blur_sampler:           wgpu::Sampler,
  blur_uniform_buffer:    wgpu::Buffer,
  intermediate_texture_1: Option<wgpu::Texture>,
  intermediate_view_1:    Option<wgpu::TextureView>,
  intermediate_texture_2: Option<wgpu::Texture>,
  intermediate_view_2:    Option<wgpu::TextureView>,
  blur_vertex_buffer:     wgpu::Buffer,

  // Powerline glyph atlas
  powerline_atlas: PowerlineAtlas,

  // Cursor icon tracking
  pending_cursor_icon: Option<winit::window::CursorIcon>,
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

    // Select an opaque alpha mode (prefer Opaque, then fallback to others)
    let alpha_mode = surface_caps
      .alpha_modes
      .iter()
      .copied()
      .find(|m| *m == CompositeAlphaMode::Opaque)
      .or_else(|| {
        surface_caps
          .alpha_modes
          .iter()
          .copied()
          .find(|m| *m == CompositeAlphaMode::Inherit || *m == CompositeAlphaMode::PreMultiplied)
      })
      .unwrap_or(surface_caps.alpha_modes[0]);

    let config = wgpu::SurfaceConfiguration {
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
      format: surface_format,
      width: size.width.max(1),
      height: size.height.max(1),
      present_mode,
      alpha_mode,
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
              wgpu::VertexAttribute {
                offset:          60,
                shader_location: 8,
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
      depth_stencil: Some(wgpu::DepthStencilState {
        format:              wgpu::TextureFormat::Stencil8,
        depth_write_enabled: false,
        depth_compare:       wgpu::CompareFunction::Always,
        stencil:             wgpu::StencilState {
          front:      wgpu::StencilFaceState {
            compare:       wgpu::CompareFunction::Always,
            fail_op:       wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op:       wgpu::StencilOperation::Replace,
          },
          back:       wgpu::StencilFaceState {
            compare:       wgpu::CompareFunction::Always,
            fail_op:       wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op:       wgpu::StencilOperation::Replace,
          },
          read_mask:  0xFF,
          write_mask: 0xFF,
        },
        bias:                wgpu::DepthBiasState::default(),
      }),
      multisample:   wgpu::MultisampleState::default(),
      multiview:     None,
      cache:         None,
    });

    // Blur pipeline setup
    let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label:  Some("Blur Shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
    });

    let blur_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("Blur Sampler"),
      address_mode_u: wgpu::AddressMode::ClampToEdge,
      address_mode_v: wgpu::AddressMode::ClampToEdge,
      address_mode_w: wgpu::AddressMode::ClampToEdge,
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      mipmap_filter: wgpu::FilterMode::Nearest,
      ..Default::default()
    });

    let blur_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label:              Some("Blur Uniform Buffer"),
      size:               std::mem::size_of::<BlurUniforms>() as u64,
      usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });

    let blur_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label:   Some("Blur Bind Group Layout"),
        entries: &[
          wgpu::BindGroupLayoutEntry {
            binding:    0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty:         wgpu::BindingType::Texture {
              sample_type:    wgpu::TextureSampleType::Float { filterable: true },
              view_dimension: wgpu::TextureViewDimension::D2,
              multisampled:   false,
            },
            count:      None,
          },
          wgpu::BindGroupLayoutEntry {
            binding:    1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty:         wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count:      None,
          },
          wgpu::BindGroupLayoutEntry {
            binding:    2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty:         wgpu::BindingType::Buffer {
              ty:                 wgpu::BufferBindingType::Uniform,
              has_dynamic_offset: false,
              min_binding_size:   None,
            },
            count:      None,
          },
        ],
      });

    let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label:                Some("Blur Pipeline Layout"),
      bind_group_layouts:   &[&blur_bind_group_layout],
      push_constant_ranges: &[],
    });

    // Full-screen triangle vertices for blur
    let blur_vertices = [
      RectVertex {
        position: [0.0, 0.0],
      },
      RectVertex {
        position: [2.0, 0.0],
      },
      RectVertex {
        position: [0.0, 2.0],
      },
    ];

    let blur_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label:    Some("Blur Vertex Buffer"),
      contents: bytemuck::cast_slice(&blur_vertices),
      usage:    wgpu::BufferUsages::VERTEX,
    });

    let blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label:         Some("Blur Render Pipeline"),
      layout:        Some(&blur_pipeline_layout),
      vertex:        wgpu::VertexState {
        module:              &blur_shader,
        entry_point:         Some("vs_main"),
        buffers:             &[wgpu::VertexBufferLayout {
          array_stride: std::mem::size_of::<RectVertex>() as u64,
          step_mode:    wgpu::VertexStepMode::Vertex,
          attributes:   &[wgpu::VertexAttribute {
            offset:          0,
            shader_location: 0,
            format:          wgpu::VertexFormat::Float32x2,
          }],
        }],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
      },
      fragment:      Some(wgpu::FragmentState {
        module:              &blur_shader,
        entry_point:         Some("fs_main"),
        targets:             &[Some(wgpu::ColorTargetState {
          format:     surface_format,
          blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
          write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
      }),
      primitive:     wgpu::PrimitiveState {
        topology:           wgpu::PrimitiveTopology::TriangleList,
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
    // Configure text renderer with stencil test to exclude masked regions
    let text_stencil_state = wgpu::DepthStencilState {
      format:              wgpu::TextureFormat::Stencil8,
      depth_write_enabled: false,
      depth_compare:       wgpu::CompareFunction::Always,
      stencil:             wgpu::StencilState {
        front:      wgpu::StencilFaceState {
          compare:       wgpu::CompareFunction::Equal, // Only render where stencil == 0
          fail_op:       wgpu::StencilOperation::Keep,
          depth_fail_op: wgpu::StencilOperation::Keep,
          pass_op:       wgpu::StencilOperation::Keep,
        },
        back:       wgpu::StencilFaceState {
          compare:       wgpu::CompareFunction::Equal, // Only render where stencil == 0
          fail_op:       wgpu::StencilOperation::Keep,
          depth_fail_op: wgpu::StencilOperation::Keep,
          pass_op:       wgpu::StencilOperation::Keep,
        },
        read_mask:  0xFF,
        write_mask: 0x00, // Don't write to stencil, only read
      },
      bias:                wgpu::DepthBiasState::default(),
    };

    let text_renderer = TextRenderer::new(
      &mut text_atlas,
      &device,
      wgpu::MultisampleState::default(),
      Some(text_stencil_state),
    );

    // Create overlay text renderer without stencil (for UI overlays)
    let overlay_text_renderer = TextRenderer::new(
      &mut text_atlas,
      &device,
      wgpu::MultisampleState::default(),
      None, // No stencil masking for overlay text
    );

    viewport.update(&queue, Resolution {
      width:  config.width,
      height: config.height,
    });

    // Create stencil buffer for masking regions
    let stencil_texture = device.create_texture(&wgpu::TextureDescriptor {
      label:           Some("Stencil Texture"),
      size:            wgpu::Extent3d {
        width:                 config.width.max(1),
        height:                config.height.max(1),
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count:    1,
      dimension:       wgpu::TextureDimension::D2,
      format:          wgpu::TextureFormat::Stencil8,
      usage:           wgpu::TextureUsages::RENDER_ATTACHMENT,
      view_formats:    &[],
    });
    let stencil_view = stencil_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Load default font and configure metrics.
    const FONT_BYTES: &[u8] = include_bytes!("../assets/Iosevka-Regular.ttc");
    let default_family =
      resolve_family_name(FONT_BYTES).unwrap_or_else(|| "JetBrains Mono".to_string());
    font_system.db_mut().load_font_data(FONT_BYTES.to_vec());

    // Create temporary powerline atlas (will be recreated with correct dimensions
    // after metrics calculation)
    let temp_powerline_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      label: Some("Powerline Sampler (temp)"),
      address_mode_u: wgpu::AddressMode::ClampToEdge,
      address_mode_v: wgpu::AddressMode::ClampToEdge,
      address_mode_w: wgpu::AddressMode::ClampToEdge,
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Linear,
      ..Default::default()
    });

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
      overlay_text_renderer,
      current_output: None,
      current_view: None,
      current_encoder: None,
      stencil_texture,
      stencil_view,
      background_color: Color::new(0.1, 0.1, 0.15, 1.0),
      rect_instances: Vec::new(),
      text_commands: Vec::new(),
      overlay_text_commands: Vec::new(),
      scissor_rect_stack: Vec::new(),
      stencil_mask_rects: Vec::new(),
      pending_text_batch: None,
      is_overlay_mode: false,
      rect_render_pipeline,
      rect_vertex_buffer,
      rect_uniform_buffer,
      rect_bind_group,
      rect_instance_buffer: None,
      rect_instance_capacity: 0,
      mask_instance_buffer: None,
      mask_instance_capacity: 0,
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
      blur_pipeline,
      blur_bind_group_layout,
      blur_sampler,
      blur_uniform_buffer,
      blur_vertex_buffer,
      intermediate_texture_1: None,
      intermediate_view_1: None,
      intermediate_texture_2: None,
      intermediate_view_2: None,
      powerline_atlas: PowerlineAtlas {
        textures:   std::collections::HashMap::new(),
        views:      std::collections::HashMap::new(),
        sampler:    temp_powerline_sampler,
        bind_group: None,
      },
      pending_cursor_icon: None,
    };

    renderer.recalculate_metrics();

    // Recreate powerline atlas with correct cell dimensions
    renderer.powerline_atlas = PowerlineAtlas::new(
      &renderer.device,
      &renderer.queue,
      renderer.cell_width,
      renderer.cell_height,
    );

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

      // Recreate stencil texture for new size
      self.stencil_texture = self.device.create_texture(&wgpu::TextureDescriptor {
        label:           Some("Stencil Texture"),
        size:            wgpu::Extent3d {
          width:                 self.config.width,
          height:                self.config.height,
          depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          wgpu::TextureFormat::Stencil8,
        usage:           wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats:    &[],
      });
      self.stencil_view = self
        .stencil_texture
        .create_view(&wgpu::TextureViewDescriptor::default());
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
    self.overlay_text_commands.clear();

    // Clear any pending text batch
    self.pending_text_batch = None;

    // Clear stencil mask rects (per-frame state)
    self.stencil_mask_rects.clear();
    self.is_overlay_mode = false;

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

    // Clear background and stencil buffer
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
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
          view:        &self.stencil_view,
          depth_ops:   None,
          stencil_ops: Some(wgpu::Operations {
            load:  wgpu::LoadOp::Clear(0), // Clear stencil to 0
            store: wgpu::StoreOp::Store,
          }),
        }),
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });
    }

    // Draw instanced rectangles
    if !self.rect_instances.is_empty() {
      // Get or create buffer with sufficient capacity
      self.get_or_create_rect_instance_buffer(self.rect_instances.len());

      // Now we can safely use the buffer
      let instance_buffer = self.rect_instance_buffer.as_ref().unwrap();
      self.queue.write_buffer(
        instance_buffer,
        0,
        bytemuck::cast_slice(&self.rect_instances),
      );

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
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
          view:        &self.stencil_view,
          depth_ops:   None,
          stencil_ops: Some(wgpu::Operations {
            load:  wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          }),
        }),
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });

      pass.set_pipeline(&self.rect_render_pipeline);
      pass.set_bind_group(0, &self.rect_bind_group, &[]);
      pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
      pass.set_vertex_buffer(1, instance_buffer.slice(..));
      pass.draw(0..4, 0..self.rect_instances.len() as u32);
    }

    // If there are stencil mask rects, write them to the stencil buffer before
    // rendering text (supports multiple overlay regions per frame)
    if !self.stencil_mask_rects.is_empty() {
      // Create instances for all mask rects
      let mask_instances: Vec<RectInstance> = self
        .stencil_mask_rects
        .iter()
        .map(|(mask_x, mask_y, mask_width, mask_height)| {
          RectInstance {
            position:      [*mask_x, *mask_y],
            size:          [*mask_width, *mask_height],
            color:         [0.0, 0.0, 0.0, 0.0], // Invisible, we're only writing to stencil
            corner_radius: 0.0,
            _pad0:         [0.0, 0.0],
            glow_center:   [0.0, 0.0],
            glow_radius:   0.0,
            effect_kind:   0.0,
            effect_time:   0.0,
            _pad1:         [0.0, 0.0, 0.0],
          }
        })
        .collect();

      // Get or create buffer with sufficient capacity
      self.get_or_create_mask_instance_buffer(mask_instances.len());

      // Now we can safely use the buffer
      let mask_buffer = self.mask_instance_buffer.as_ref().unwrap();
      self
        .queue
        .write_buffer(mask_buffer, 0, bytemuck::cast_slice(&mask_instances));

      // Render pass that writes to stencil
      let mut mask_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Stencil Write Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           &view,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
          view:        &self.stencil_view,
          depth_ops:   None,
          stencil_ops: Some(wgpu::Operations {
            load:  wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          }),
        }),
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });

      mask_pass.set_pipeline(&self.rect_render_pipeline);
      mask_pass.set_bind_group(0, &self.rect_bind_group, &[]);
      mask_pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
      mask_pass.set_vertex_buffer(1, mask_buffer.slice(..));
      mask_pass.set_stencil_reference(1); // Write 1 to stencil
      mask_pass.draw(0..4, 0..mask_instances.len() as u32);
      drop(mask_pass);
    }

    if !self.text_commands.is_empty() {
      // Prepare text areas for rendering
      let text_areas: Vec<_> = self
        .text_commands
        .iter()
        .filter(|command| {
          // Check if text is within the active scissor rect (if any)
          if let Some(&(scissor_x, scissor_y, scissor_width, scissor_height)) =
            self.scissor_rect_stack.last()
          {
            let text_x = command.position.0;
            let text_y = command.position.1;
            let text_width = (command.bounds.right - command.bounds.left) as f32;
            let text_height = (command.bounds.bottom - command.bounds.top) as f32;

            // Check if text intersects with scissor rect
            !(text_x + text_width < scissor_x
              || text_x > scissor_x + scissor_width
              || text_y + text_height < scissor_y
              || text_y > scissor_y + scissor_height)
          } else {
            // No scissor rect active, render all text
            true
          }
        })
        .filter_map(|command| {
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
        })
        .collect();

      // Try to prepare text, with atlas recovery on failure
      let prepare_result = self.text_renderer.prepare(
        &self.device,
        &self.queue,
        &mut self.font_system,
        &mut self.text_atlas,
        &self.viewport,
        text_areas.iter().map(|ta| ta.clone()),
        &mut self.swash_cache,
      );

      if let Err(e) = prepare_result {
        let error_msg = e.to_string();
        // Check if atlas is full
        if error_msg.contains("glyph texture atlas is full") {
          log::warn!("Glyph texture atlas full, trimming and retrying...");
          // Trim unused glyphs from atlas
          self.text_atlas.trim();
          // Retry prepare after trimming
          self
            .text_renderer
            .prepare(
              &self.device,
              &self.queue,
              &mut self.font_system,
              &mut self.text_atlas,
              &self.viewport,
              text_areas.iter().map(|ta| ta.clone()),
              &mut self.swash_cache,
            )
            .map_err(|e| RendererError::TextRendering(e.to_string()))?;
        } else {
          // Other error, propagate it
          return Err(RendererError::TextRendering(error_msg));
        }
      }

      // Always attach stencil buffer (text renderer pipeline expects it)
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
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
          view:        &self.stencil_view,
          depth_ops:   None,
          stencil_ops: Some(wgpu::Operations {
            load:  wgpu::LoadOp::Load, /* Load existing stencil values (will be 0 without mask,
                                        * 1 in mask area) */
            store: wgpu::StoreOp::Store,
          }),
        }),
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });

      // Set stencil reference to 0 (text renders where stencil == 0, i.e., not in
      // masked regions)
      pass.set_stencil_reference(0);

      self
        .text_renderer
        .render(&self.text_atlas, &self.viewport, &mut pass)
        .map_err(|e| RendererError::TextRendering(e.to_string()))?;
    }

    // Render overlay text (UI text that ignores stencil masking)
    if !self.overlay_text_commands.is_empty() {
      // Prepare overlay text areas
      let overlay_text_areas: Vec<_> = self
        .overlay_text_commands
        .iter()
        .filter_map(|command| {
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
        })
        .collect();

      // Try to prepare overlay text, with atlas recovery on failure
      let prepare_result = self.overlay_text_renderer.prepare(
        &self.device,
        &self.queue,
        &mut self.font_system,
        &mut self.text_atlas,
        &self.viewport,
        overlay_text_areas.iter().map(|ta| ta.clone()),
        &mut self.swash_cache,
      );

      if let Err(e) = prepare_result {
        let error_msg = e.to_string();
        // Check if atlas is full
        if error_msg.contains("glyph texture atlas is full") {
          log::warn!("Glyph texture atlas full (overlay), trimming and retrying...");
          // Trim unused glyphs from atlas
          self.text_atlas.trim();
          // Retry prepare after trimming
          self
            .overlay_text_renderer
            .prepare(
              &self.device,
              &self.queue,
              &mut self.font_system,
              &mut self.text_atlas,
              &self.viewport,
              overlay_text_areas.iter().map(|ta| ta.clone()),
              &mut self.swash_cache,
            )
            .map_err(|e| RendererError::TextRendering(e.to_string()))?;
        } else {
          // Other error, propagate it
          return Err(RendererError::TextRendering(error_msg));
        }
      }

      // Render overlay text without stencil (overlay text renderer has no stencil
      // configured)
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Overlay Text Render Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           &view,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None, // No stencil for overlay text
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });

      self
        .overlay_text_renderer
        .render(&self.text_atlas, &self.viewport, &mut pass)
        .map_err(|e| RendererError::TextRendering(e.to_string()))?;
    }

    // Copy final frame to intermediate texture for blur effect on next frame
    self.ensure_intermediate_textures();
    if let Some(dest_texture) = self.intermediate_texture_1.as_ref() {
      encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
          texture:   &output.texture,
          mip_level: 0,
          origin:    wgpu::Origin3d::ZERO,
          aspect:    wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
          texture:   dest_texture,
          mip_level: 0,
          origin:    wgpu::Origin3d::ZERO,
          aspect:    wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
          width:                 self.config.width,
          height:                self.config.height,
          depth_or_array_layers: 1,
        },
      );
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
  /// BUGGY: Text batching with position offset handling
  ///
  /// BUG: When batching text at different X positions on the same line (lines
  /// 1186-1187), the code just appends text segments without accounting for
  /// position differences. This causes text to render at wrong positions,
  /// creating overlapping/garbled text.
  ///
  /// Example of the bug:
  /// - Text "func" at x=10
  /// - Text "main" at x=50
  /// - Batch merges them as ["func", "main"] at x=10
  /// - Renders as "funcmain" starting at x=10 (wrong!)
  /// - Should render "func" at x=10, "main" at x=50
  ///
  /// To fix: Either don't batch text at different positions, or insert proper
  /// spacing/padding to account for position offsets between text segments.
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
          // BUG: This doesn't account for position difference!
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

  /// Draw text using glyphon buffers (immediate mode - batching disabled due to
  /// bugs)
  pub fn draw_text(&mut self, section: TextSection) {
    // NOTE: Batching is currently buggy (doesn't handle position offsets correctly)
    // Use immediate mode until batching logic is fixed
    self.draw_text_internal(section);
  }

  /// Draw text immediately without batching
  pub fn draw_text_immediate(&mut self, section: TextSection) {
    self.draw_text_internal(section);
  }

  /// Draw a single decoration grapheme at the specified position
  /// This is a convenience method for rendering single characters with a
  /// specific color
  pub fn draw_decoration_grapheme(&mut self, grapheme: &str, color: Color, x: f32, y: f32) {
    self.draw_text(TextSection {
      position: (x, y),
      texts:    vec![TextSegment {
        content: grapheme.to_string(),
        style:   TextStyle {
          size: self.font_size,
          color,
        },
      }],
    });
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

    // Create cache key (position-independent for better cache reuse)
    let cache_key = crate::text_cache::ShapedTextKey {
      text:    full_text.clone(),
      metrics: (
        (self.font_size * 100.0) as u32,
        (self.cell_height * 100.0) as u32,
      ),
      color:   [
        (first_color.r * 255.0) as u8,
        (first_color.g * 255.0) as u8,
        (first_color.b * 255.0) as u8,
        (first_color.a * 255.0) as u8,
      ],
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

        // Use consistent cell_height for all segments to prevent accumulated
        // positioning errors
        let seg_metrics = Metrics::new(segment.style.size, self.cell_height);
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

    // Store the command with cache key for deferred rendering
    let bounds = TextBounds {
      left:   0,
      top:    0,
      right:  self.config.width as i32,
      bottom: self.config.height as i32,
    };

    let text_command = TextCommand {
      position: section.position,
      cache_key,
      bounds,
    };

    // Add to appropriate list based on overlay mode
    if self.is_overlay_mode {
      self.overlay_text_commands.push(text_command);
    } else {
      self.text_commands.push(text_command);
    }
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
      effect_time:   0.0,
      _pad1:         [0.0, 0.0, 0.0],
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
      effect_time: 0.0,
      _pad1: [0.0, 0.0, 0.0],
    });
  }

  /// Draw a rounded rectangle glow overlay, clipped to the rounded rect
  #[allow(clippy::too_many_arguments)]
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
      effect_time: 0.0,
      _pad1: [0.0, 0.0, 0.0],
    });
  }

  /// Draw only the rounded-rect outline (stroke)
  #[allow(clippy::too_many_arguments)]
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
      effect_time: 0.0,
      _pad1: [0.0, 0.0, 0.0],
    });
  }

  /// Draw a rounded-rect outline with directional thickness that fades from
  /// top (thickest) → sides → bottom (thinnest).
  #[allow(clippy::too_many_arguments)]
  pub fn draw_rounded_rect_stroke_fade(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    top_thickness: f32,
    side_thickness: f32,
    bottom_thickness: f32,
    color: Color,
  ) {
    self.rect_instances.push(RectInstance {
      position: [x, y],
      size: [width, height],
      color: color_to_linear(color),
      corner_radius,
      _pad0: [0.0, 0.0],
      glow_center: [top_thickness.max(0.0), bottom_thickness.max(0.0)],
      glow_radius: side_thickness.max(0.0),
      effect_kind: 3.0,
      effect_time: 0.0,
      _pad1: [0.0, 0.0, 0.0],
    });
  }

  /// Draw a custom shader effect (explosion or laser)
  /// effect_kind: 4.0 = explosion, 5.0 = laser
  /// effect_time: animation progress from 0.0 to 1.0
  #[allow(clippy::too_many_arguments)]
  pub fn draw_effect(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    effect_kind: f32,
    effect_time: f32,
    radius: f32,
    color: Color,
  ) {
    self.rect_instances.push(RectInstance {
      position: [x, y],
      size: [width, height],
      color: color_to_linear(color),
      corner_radius: 0.0,
      _pad0: [0.0, 0.0],
      glow_center: [0.0, 0.0],
      glow_radius: radius,
      effect_kind,
      effect_time,
      _pad1: [0.0, 0.0, 0.0],
    });
  }

  /// Draw a Powerline separator glyph
  /// This renders the glyph using tiny-skia and draws it as colored rectangles
  pub fn draw_powerline_glyph(
    &mut self,
    ch: char,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
  ) {
    use crate::powerline_glyphs;

    // Check if this is a known Powerline glyph
    if let Some(glyph) = PowerlineGlyph::from_char(ch) {
      let w = width.ceil() as u32;
      let h = height.ceil() as u32;

      // Render the glyph with tiny-skia
      if let Some(pixmap) = powerline_glyphs::render_powerline_glyph(glyph, w, h) {
        // Draw the glyph by sampling pixels and rendering colored rectangles
        // We only draw pixels with alpha > threshold for efficiency
        const ALPHA_THRESHOLD: u8 = 32;

        for py in 0..h {
          for px in 0..w {
            let pixel_idx = ((py * w + px) * 4) as usize;
            let pixel_data = pixmap.data();

            if pixel_idx + 3 < pixel_data.len() {
              let alpha = pixel_data[pixel_idx + 3];

              if alpha > ALPHA_THRESHOLD {
                // Draw a 1x1 rectangle for this pixel with the glyph's alpha
                let pixel_x = x + px as f32;
                let pixel_y = y + py as f32;

                // Blend the color with the glyph's alpha
                let alpha_f = alpha as f32 / 255.0;
                let pixel_color = Color::new(color.r, color.g, color.b, color.a * alpha_f);

                self.draw_rect(pixel_x, pixel_y, 1.0, 1.0, pixel_color);
              }
            }
          }
        }
      }
    }
  }

  /// Configure the monospaced font family and size used for layout calculations
  pub fn configure_font(&mut self, family: &str, size: f32) {
    self.font_family = family.to_string();
    self.font_size = size.max(1.0);
    self.recalculate_metrics();
  }

  /// Save the current font state for later restoration
  pub fn save_font_state(&self) -> FontState {
    FontState {
      family:      self.font_family.clone(),
      size:        self.font_size,
      cell_width:  self.cell_width,
      cell_height: self.cell_height,
    }
  }

  /// Restore a previously saved font state
  pub fn restore_font_state(&mut self, state: FontState) {
    self.font_family = state.family;
    self.font_size = state.size;
    self.cell_width = state.cell_width;
    self.cell_height = state.cell_height;
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

  /// Clear text commands that would appear in the specified area.
  /// This prevents text from rendering on top of UI elements like popups.
  pub fn clear_text_in_area(&mut self, x: f32, y: f32, width: f32, height: f32) {
    self.text_commands.retain(|cmd| {
      // Check if the text command intersects with the area
      let text_x = cmd.position.0;
      let text_y = cmd.position.1;
      let text_width = (cmd.bounds.right - cmd.bounds.left) as f32;
      let text_height = (cmd.bounds.bottom - cmd.bounds.top) as f32;

      // No intersection if completely outside the area
      text_x + text_width < x
        || text_x > x + width
        || text_y + text_height < y
        || text_y > y + height
    });
  }

  /// Push a scissor rect onto the stack. Text outside this rect will not be
  /// rendered. Call `pop_scissor_rect()` when done to restore the previous
  /// scissor state.
  pub fn push_scissor_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
    self.scissor_rect_stack.push((x, y, width, height));
  }

  /// Pop the most recent scissor rect from the stack.
  pub fn pop_scissor_rect(&mut self) {
    self.scissor_rect_stack.pop();
  }

  /// Add a stencil mask rect. Text will NOT be rendered in this region.
  /// Private: used internally by `with_overlay_region`.
  fn add_stencil_mask_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
    self.stencil_mask_rects.push((x, y, width, height));
  }

  /// Enable overlay text mode. Text rendered in this mode will ignore stencil
  /// masks. Private: used internally by `with_overlay_region`.
  fn begin_overlay_text(&mut self) {
    self.is_overlay_mode = true;
  }

  /// Disable overlay text mode, returning to normal text rendering.
  /// Private: used internally by `with_overlay_region`.
  fn end_overlay_text(&mut self) {
    self.is_overlay_mode = false;
  }

  /// Render content in an overlay region with automatic masking.
  ///
  /// This combines stencil masking and overlay text mode into a single call.
  /// The specified region will be masked from normal rendering (preventing the
  /// editor from drawing into it), and all drawing within the callback will be
  /// in overlay mode (bypassing the mask).
  ///
  /// This is the preferred API for UI components that need to overlap the
  /// editor.
  pub fn with_overlay_region<F>(&mut self, x: f32, y: f32, width: f32, height: f32, f: F)
  where
    F: FnOnce(&mut Self),
  {
    // Add this overlay region to the stencil mask (supports multiple overlays per
    // frame)
    self.add_stencil_mask_rect(x, y, width, height);
    self.begin_overlay_text();

    // Execute the drawing code
    f(self);

    // Clean up overlay text mode (mask rect stays active for the frame)
    self.end_overlay_text();
  }

  /// Ensure intermediate textures are created for the current viewport size
  fn ensure_intermediate_textures(&mut self) {
    let width = self.config.width;
    let height = self.config.height;

    // Check if we need to recreate textures
    let needs_recreate = if let Some(ref tex) = self.intermediate_texture_1 {
      tex.width() != width || tex.height() != height
    } else {
      true
    };

    if needs_recreate {
      // Create two intermediate textures for ping-pong blur
      let texture_desc = wgpu::TextureDescriptor {
        label:           Some("Intermediate Blur Texture 1"),
        size:            wgpu::Extent3d {
          width,
          height,
          depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          self.config.format,
        usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
          | wgpu::TextureUsages::TEXTURE_BINDING
          | wgpu::TextureUsages::COPY_DST,
        view_formats:    &[],
      };

      let texture_1 = self.device.create_texture(&texture_desc);
      let view_1 = texture_1.create_view(&wgpu::TextureViewDescriptor::default());

      let texture_2 = self.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Intermediate Blur Texture 2"),
        ..texture_desc
      });
      let view_2 = texture_2.create_view(&wgpu::TextureViewDescriptor::default());

      self.intermediate_texture_1 = Some(texture_1);
      self.intermediate_view_1 = Some(view_1);
      self.intermediate_texture_2 = Some(texture_2);
      self.intermediate_view_2 = Some(view_2);
    }
  }

  /// Prepare for blur by rendering pending rectangles
  /// Uses the previous frame's content (saved in end_frame) for blur
  /// This creates a one-frame delay but avoids TextRenderer conflicts
  pub fn capture_for_blur(&mut self) -> Result<()> {
    // Flush any pending text batch
    self.flush_text_batch();

    // Prepare instance buffer before getting view/encoder
    let has_rect_instances = !self.rect_instances.is_empty();
    if has_rect_instances {
      self.get_or_create_rect_instance_buffer(self.rect_instances.len());
      let instance_buffer = self.rect_instance_buffer.as_ref().unwrap();
      self.queue.write_buffer(
        instance_buffer,
        0,
        bytemuck::cast_slice(&self.rect_instances),
      );
    }

    let view = self
      .current_view
      .as_ref()
      .ok_or_else(|| RendererError::Runtime("No view available".into()))?;

    let encoder = self
      .current_encoder
      .as_mut()
      .ok_or_else(|| RendererError::Runtime("No encoder available".into()))?;

    // Render all pending rectangles to the current frame
    // (Text will be rendered normally in end_frame)
    if has_rect_instances {
      let instance_buffer = self.rect_instance_buffer.as_ref().unwrap();

      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Pre-blur Rectangle Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view,
          resolve_target: None,
          ops: wgpu::Operations {
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

      // Clear rect instances so they don't render again in end_frame
      drop(pass);
      self.rect_instances.clear();
    }

    // Note: intermediate_texture_1 already contains the previous frame's content
    // from the copy that happened in end_frame(), so blur will use that

    Ok(())
  }

  /// Apply Gaussian blur to the captured frame and render to specified rect
  pub fn apply_blur(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    blur_strength: f32,
  ) -> Result<()> {
    self.ensure_intermediate_textures();

    let encoder = self
      .current_encoder
      .as_mut()
      .ok_or_else(|| RendererError::Runtime("No encoder available for blur".into()))?;

    let intermediate_view_1 = self
      .intermediate_view_1
      .as_ref()
      .ok_or_else(|| RendererError::Runtime("No intermediate texture 1".into()))?;

    let intermediate_view_2 = self
      .intermediate_view_2
      .as_ref()
      .ok_or_else(|| RendererError::Runtime("No intermediate texture 2".into()))?;

    let current_view = self
      .current_view
      .as_ref()
      .ok_or_else(|| RendererError::Runtime("No current view".into()))?;

    let resolution = [self.config.width as f32, self.config.height as f32];

    // First pass: Horizontal blur (texture 1 -> texture 2)
    {
      let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:   Some("Blur Horizontal Bind Group"),
        layout:  &self.blur_bind_group_layout,
        entries: &[
          wgpu::BindGroupEntry {
            binding:  0,
            resource: wgpu::BindingResource::TextureView(intermediate_view_1),
          },
          wgpu::BindGroupEntry {
            binding:  1,
            resource: wgpu::BindingResource::Sampler(&self.blur_sampler),
          },
          wgpu::BindGroupEntry {
            binding:  2,
            resource: self.blur_uniform_buffer.as_entire_binding(),
          },
        ],
      });

      let uniforms = BlurUniforms {
        direction: [blur_strength, 0.0], // Horizontal
        resolution,
      };
      self.queue.write_buffer(
        &self.blur_uniform_buffer,
        0,
        bytemuck::cast_slice(&[uniforms]),
      );

      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Blur Horizontal Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           intermediate_view_2,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });

      pass.set_pipeline(&self.blur_pipeline);
      pass.set_bind_group(0, &bind_group, &[]);
      pass.set_vertex_buffer(0, self.blur_vertex_buffer.slice(..));
      pass.draw(0..3, 0..1);
    }

    // Second pass: Vertical blur (texture 2 -> current view, clipped to rect)
    {
      let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:   Some("Blur Vertical Bind Group"),
        layout:  &self.blur_bind_group_layout,
        entries: &[
          wgpu::BindGroupEntry {
            binding:  0,
            resource: wgpu::BindingResource::TextureView(intermediate_view_2),
          },
          wgpu::BindGroupEntry {
            binding:  1,
            resource: wgpu::BindingResource::Sampler(&self.blur_sampler),
          },
          wgpu::BindGroupEntry {
            binding:  2,
            resource: self.blur_uniform_buffer.as_entire_binding(),
          },
        ],
      });

      let uniforms = BlurUniforms {
        direction: [0.0, blur_strength], // Vertical
        resolution,
      };
      self.queue.write_buffer(
        &self.blur_uniform_buffer,
        0,
        bytemuck::cast_slice(&[uniforms]),
      );

      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Blur Vertical Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           current_view,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Load, // Preserve existing content
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes:         None,
        occlusion_query_set:      None,
      });

      pass.set_pipeline(&self.blur_pipeline);
      pass.set_bind_group(0, &bind_group, &[]);
      pass.set_vertex_buffer(0, self.blur_vertex_buffer.slice(..));

      // Set viewport to render only to the specified rect
      pass.set_viewport(x, y, width, height, 0.0, 1.0);

      pass.draw(0..3, 0..1);
    }

    Ok(())
  }

  /// Get or create a rect instance buffer with sufficient capacity
  fn get_or_create_rect_instance_buffer(&mut self, required_count: usize) {
    // Check if we need to create or resize the buffer
    if self.rect_instance_buffer.is_none() || self.rect_instance_capacity < required_count {
      // Allocate with 50% headroom to reduce reallocations
      let new_capacity = (required_count as f32 * 1.5).ceil() as usize;
      let buffer_size = (new_capacity * std::mem::size_of::<RectInstance>()) as u64;

      self.rect_instance_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
        label:              Some("Rect Instance Buffer"),
        size:               buffer_size,
        usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
      }));
      self.rect_instance_capacity = new_capacity;
    }
  }

  /// Get or create a mask instance buffer with sufficient capacity
  fn get_or_create_mask_instance_buffer(&mut self, required_count: usize) {
    // Check if we need to create or resize the buffer
    if self.mask_instance_buffer.is_none() || self.mask_instance_capacity < required_count {
      // Allocate with 50% headroom to reduce reallocations
      let new_capacity = (required_count as f32 * 1.5).ceil() as usize;
      let buffer_size = (new_capacity * std::mem::size_of::<RectInstance>()) as u64;

      self.mask_instance_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
        label:              Some("Mask Instance Buffer"),
        size:               buffer_size,
        usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
      }));
      self.mask_instance_capacity = new_capacity;
    }
  }

  /// Set the cursor icon to be applied after the current frame
  pub fn set_cursor_icon(&mut self, icon: winit::window::CursorIcon) {
    self.pending_cursor_icon = Some(icon);
  }

  /// Clear the cursor icon (reset to default)
  pub fn reset_cursor_icon(&mut self) {
    self.pending_cursor_icon = Some(winit::window::CursorIcon::Default);
  }

  /// Take the pending cursor icon, leaving None in its place
  pub(crate) fn take_cursor_icon(&mut self) -> Option<winit::window::CursorIcon> {
    self.pending_cursor_icon.take()
  }

  fn release_in_flight_frame(&mut self) {
    let command_buffer = self.current_encoder.take().map(|encoder| encoder.finish());

    // Drop any references to the view before presenting
    self.current_view.take();

    if let Some(cb) = command_buffer {
      self.queue.submit(std::iter::once(cb));
    }

    if let Some(output) = self.current_output.take() {
      output.present();
    }
  }

  /// Gracefully shutdown the renderer, flushing all GPU operations.
  /// This should be called before dropping the renderer to ensure clean
  /// resource cleanup.
  pub fn shutdown(&mut self) {
    self.release_in_flight_frame();
    // Flush any pending command buffers to the GPU queue.
    // This is critical for preventing Wayland compositor freezes and audio glitches
    // caused by incomplete GPU resource cleanup.
    // An empty submit flushes pending operations without adding new work.
    self.queue.submit(vec![]);

    log::debug!("Renderer shutdown complete - GPU operations flushed");
  }
}

impl Drop for Renderer {
  fn drop(&mut self) {
    self.release_in_flight_frame();
    // Attempt graceful shutdown during cleanup to catch any missed explicit calls.
    // This is a safety net - proper shutdown should call shutdown() explicitly.
    // Flushing the queue ensures pending operations are sent to the GPU.
    self.queue.submit(vec![]);
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
