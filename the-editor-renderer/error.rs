use thiserror::Error;

/// Errors that can occur in the renderer
#[derive(Error, Debug, Clone)]
pub enum RendererError {
  /// Window creation failed
  #[error("Failed to create window: {0}")]
  WindowCreation(String),

  /// Unable to create a rendering surface
  #[error("Failed to create surface: {0}")]
  SurfaceCreation(String),

  /// GPU configuration error (adapter/device)
  #[error("Renderer configuration error: {0}")]
  Configuration(String),

  /// Text rendering failure
  #[error("Text rendering error: {0}")]
  TextRendering(String),

  /// General runtime error
  #[error("Renderer error: {0}")]
  Runtime(String),
}

pub type Result<T> = std::result::Result<T, RendererError>;
