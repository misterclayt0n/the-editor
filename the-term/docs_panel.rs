#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DocsPanelSource {
  #[default]
  Completion,
  Hover,
  Signature,
  CommandPalette,
}
