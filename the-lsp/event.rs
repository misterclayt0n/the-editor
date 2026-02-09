use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspEvent {
  Started { workspace_root: PathBuf },
  Stopped,
  Error(String),
}
