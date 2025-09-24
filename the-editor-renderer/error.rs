use thiserror::Error;

/// Errors that can occur in the renderer
#[derive(Error, Debug, Clone)]
pub enum RendererError {
  /// Window creation failed
  #[error("Failed to create window: {0}")]
  WindowCreation(String),

  /// General runtime error
  #[error("Renderer error: {0}")]
  Runtime(String),
}

pub type Result<T> = std::result::Result<T, RendererError>;
