//! Quick slots for binding views to Alt+0-9 hotkeys.
//!
//! This module provides functionality to bind documents or terminals to quick
//! slots (0-9) that can be toggled with Alt+N keys. When a slotted view is
//! hidden, its exact layout position is captured and restored when shown again.

use the_terminal::TerminalId;

use super::{
  DocumentId,
  tree::Layout,
};

/// Content bound to a quick slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotContent {
  Document(DocumentId),
  Terminal(TerminalId),
}

/// A single step in the path from root to view.
#[derive(Clone, Debug)]
pub struct SplitStep {
  /// The layout direction of this container.
  pub layout: Layout,
  /// Which child index within this container (0-based).
  pub index: usize,
  /// Total number of children in this container at capture time.
  pub child_count: usize,
}

/// A recipe for restoring a view's position in the tree.
///
/// This describes the path from root to the view's position, allowing
/// the exact layout structure to be recreated.
#[derive(Clone, Debug)]
pub struct PositionRecipe {
  /// Path from root to the target position.
  /// Each step describes a container in the path.
  pub path: Vec<SplitStep>,
  /// The custom size this view had, if any (None = Fill).
  pub custom_size: Option<u16>,
}

/// A quick slot binding.
#[derive(Clone, Debug)]
pub struct QuickSlot {
  /// The content bound to this slot.
  pub content: SlotContent,
  /// Position recipe for restoration (captured when hiding).
  pub position: Option<PositionRecipe>,
  /// Whether the slotted view is currently visible.
  pub visible: bool,
  /// Whether this was the only view when hidden (full-screen mode).
  pub was_only_view: bool,
  /// Content that was displayed before showing this slot (for fullscreen restore).
  /// Only set when `was_only_view` is true and we replaced another view.
  pub previous_content: Option<SlotContent>,
}

/// Quick slots for Alt+0-9 bindings.
#[derive(Debug, Default)]
pub struct QuickSlots {
  /// Slots 0-9 (indexed by slot number).
  slots: [Option<QuickSlot>; 10],
}

impl QuickSlots {
  /// Create a new empty quick slots collection.
  pub fn new() -> Self {
    Self::default()
  }

  /// Get a slot by number (0-9).
  pub fn get(&self, slot: u8) -> Option<&QuickSlot> {
    self.slots.get(slot as usize).and_then(|s| s.as_ref())
  }

  /// Get a mutable slot by number (0-9).
  pub fn get_mut(&mut self, slot: u8) -> Option<&mut QuickSlot> {
    self.slots.get_mut(slot as usize).and_then(|s| s.as_mut())
  }

  /// Set a slot binding.
  pub fn set(&mut self, slot: u8, quick_slot: QuickSlot) {
    if let Some(s) = self.slots.get_mut(slot as usize) {
      *s = Some(quick_slot);
    }
  }

  /// Clear a slot binding.
  pub fn clear(&mut self, slot: u8) {
    if let Some(s) = self.slots.get_mut(slot as usize) {
      *s = None;
    }
  }

  /// Find the slot number for a given content, if bound.
  pub fn find_slot_for_content(&self, content: &SlotContent) -> Option<u8> {
    self
      .slots
      .iter()
      .enumerate()
      .find_map(|(i, slot)| {
        slot.as_ref().and_then(|s| {
          if &s.content == content {
            Some(i as u8)
          } else {
            None
          }
        })
      })
  }

  /// Unbind any slot containing the given content.
  pub fn unbind_content(&mut self, content: &SlotContent) {
    for slot in &mut self.slots {
      if let Some(s) = slot {
        if &s.content == content {
          *slot = None;
          break;
        }
      }
    }
  }

  /// Iterate over all bound slots with their slot numbers.
  pub fn iter(&self) -> impl Iterator<Item = (u8, &QuickSlot)> {
    self
      .slots
      .iter()
      .enumerate()
      .filter_map(|(i, slot)| slot.as_ref().map(|s| (i as u8, s)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn doc_id(n: usize) -> DocumentId {
    DocumentId(std::num::NonZeroUsize::new(n).unwrap())
  }

  #[test]
  fn test_quick_slots_basic() {
    let mut slots = QuickSlots::new();

    // Initially empty
    assert!(slots.get(0).is_none());
    assert!(slots.get(5).is_none());

    // Set a slot
    slots.set(
      3,
      QuickSlot {
        content:          SlotContent::Document(doc_id(1)),
        position:         None,
        visible:          true,
        was_only_view:    false,
        previous_content: None,
      },
    );

    assert!(slots.get(3).is_some());
    assert_eq!(slots.get(3).unwrap().visible, true);

    // Clear the slot
    slots.clear(3);
    assert!(slots.get(3).is_none());
  }

  #[test]
  fn test_find_slot_for_content() {
    let mut slots = QuickSlots::new();

    let content = SlotContent::Document(doc_id(42));
    slots.set(
      7,
      QuickSlot {
        content,
        position:         None,
        visible:          true,
        was_only_view:    false,
        previous_content: None,
      },
    );

    assert_eq!(slots.find_slot_for_content(&content), Some(7));
    assert_eq!(
      slots.find_slot_for_content(&SlotContent::Document(doc_id(99))),
      None
    );
  }

  #[test]
  fn test_unbind_content() {
    let mut slots = QuickSlots::new();

    let content = SlotContent::Document(doc_id(1));
    slots.set(
      5,
      QuickSlot {
        content,
        position:         None,
        visible:          true,
        was_only_view:    false,
        previous_content: None,
      },
    );

    assert!(slots.get(5).is_some());
    slots.unbind_content(&content);
    assert!(slots.get(5).is_none());
  }
}
