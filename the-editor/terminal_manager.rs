//! Terminal management for the editor
//!
//! Manages the lifecycle of terminal sessions and provides an interface
//! for spawning, switching, and closing terminals.

use std::collections::HashMap;
use crate::ui::components::TerminalView;

/// Manages multiple terminal instances
pub struct TerminalManager {
  /// Active terminal sessions, mapped by ID
  terminals: HashMap<u32, TerminalView>,
  /// Currently focused terminal ID
  active_terminal: Option<u32>,
  /// Counter for generating unique terminal IDs
  next_id: u32,
}

impl TerminalManager {
  /// Create a new terminal manager
  pub fn new() -> Self {
    Self {
      terminals: HashMap::new(),
      active_terminal: None,
      next_id: 0,
    }
  }

  /// Spawn a new terminal with the given dimensions
  pub fn spawn(&mut self, cols: u16, rows: u16, shell: Option<&str>) -> anyhow::Result<u32> {
    let id = self.next_id;
    self.next_id += 1;

    let terminal = TerminalView::new(cols, rows, shell, id)?;
    self.terminals.insert(id, terminal);
    self.active_terminal = Some(id);

    Ok(id)
  }

  /// Get the currently active terminal
  pub fn active(&self) -> Option<&TerminalView> {
    self.active_terminal.and_then(|id| self.terminals.get(&id))
  }

  /// Get a mutable reference to the currently active terminal
  pub fn active_mut(&mut self) -> Option<&mut TerminalView> {
    self.active_terminal.and_then(|id| self.terminals.get_mut(&id))
  }

  /// Switch to a different terminal by ID
  pub fn switch(&mut self, id: u32) -> bool {
    if self.terminals.contains_key(&id) {
      self.active_terminal = Some(id);
      true
    } else {
      false
    }
  }

  /// Close a terminal by ID
  pub fn close(&mut self, id: u32) -> Option<TerminalView> {
    let removed = self.terminals.remove(&id);
    if self.active_terminal == Some(id) {
      // Switch to another terminal if available
      self.active_terminal = self.terminals.keys().copied().next();
    }
    removed
  }

  /// Close the active terminal
  pub fn close_active(&mut self) -> bool {
    if let Some(id) = self.active_terminal {
      self.close(id);
      true
    } else {
      false
    }
  }

  /// Get the number of active terminals
  pub fn count(&self) -> usize {
    self.terminals.len()
  }

  /// List all terminal IDs
  pub fn list(&self) -> Vec<u32> {
    self.terminals.keys().copied().collect()
  }
}

impl Default for TerminalManager {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_terminal_manager_creation() {
    let manager = TerminalManager::new();
    assert_eq!(manager.count(), 0);
    assert!(manager.active().is_none());
  }

  #[test]
  fn test_terminal_manager_switch() {
    let mut manager = TerminalManager::new();
    // Manually add a fake terminal ID to the map for testing
    // (Can't create real terminals in unit tests due to Tokio requirement)
    // Just verify the API structure
    manager.active_terminal = Some(99);
    assert!(!manager.switch(999)); // Non-existent terminal
    assert_eq!(manager.active_terminal, Some(99)); // Active terminal unchanged
  }
}
