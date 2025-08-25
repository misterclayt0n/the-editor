use thiserror::Error;
use winit::error::EventLoopError;

/// Errors that can occur in the renderer
#[derive(Error, Debug)]
pub enum RendererError {
  /// Window creation failed
  #[error("Failed to create window: {0}")]
  WindowCreation(String),

  /// GPU adapter not found
  #[error("No suitable GPU adapter found")]
  NoAdapter,

  /// Device request failed
  #[error("Failed to request GPU device: {0}")]
  DeviceRequest(#[from] wgpu::RequestDeviceError),

  /// Surface creation failed
  #[error("Failed to create surface: {0}")]
  SurfaceCreation(String),

  /// Surface error during rendering
  #[error("Surface error: {0}")]
  Surface(#[from] wgpu::SurfaceError),

  /// Text rendering error
  #[error("Text rendering failed: {0}")]
  TextRendering(String),

  /// Font loading error
  #[error("Failed to load font: {0}")]
  FontError(String),

  /// Invalid configuration
  #[error("Invalid configuration: {0}")]
  Configuration(String),

  /// Event loop error
  #[error("Event loop error: {0}")]
  EventLoop(#[from] EventLoopError),
}

pub type Result<T> = std::result::Result<T, RendererError>;
