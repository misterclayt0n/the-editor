use std::collections::HashMap;

/// Cache for shaped text lines to avoid reshaping unchanged content
/// Currently simplified - just tracks which lines have been rendered
pub struct TextShapeCache {
  /// Map from line content hash to last rendered version
  rendered_lines: HashMap<u64, String>,
  /// Maximum number of cached lines
  max_cache_size: usize,
}

impl TextShapeCache {
  pub fn new(max_cache_size: usize) -> Self {
    Self {
      rendered_lines: HashMap::with_capacity(max_cache_size),
      max_cache_size,
    }
  }

  /// Check if a line has changed since last render
  pub fn has_line_changed(&mut self, line_idx: usize, line_text: &str) -> bool {
    let content_hash = Self::hash_line_with_idx(line_idx, line_text);

    // Check if content changed
    if let Some(cached) = self.rendered_lines.get(&content_hash) {
      cached != line_text
    } else {
      true // Not in cache, needs rendering
    }
  }

  /// Mark a line as rendered
  pub fn mark_rendered(&mut self, line_idx: usize, line_text: &str) {
    let content_hash = Self::hash_line_with_idx(line_idx, line_text);

    // Evict if at capacity
    if self.rendered_lines.len() >= self.max_cache_size {
      // Simple eviction - remove first item (could be improved with LRU)
      if let Some(key) = self.rendered_lines.keys().next().cloned() {
        self.rendered_lines.remove(&key);
      }
    }

    self
      .rendered_lines
      .insert(content_hash, line_text.to_string());
  }

  /// Clear cache entries for a specific line
  pub fn invalidate_line(&mut self, line_idx: usize) {
    // Remove any cached version of this line
    self.rendered_lines.retain(|k, _| {
      // Extract line index from hash (simplified approach)
      *k >> 32 != line_idx as u64
    });
  }

  /// Clear entire cache
  pub fn clear(&mut self) {
    self.rendered_lines.clear();
  }

  fn hash_line_with_idx(line_idx: usize, text: &str) -> u64 {
    use std::{
      collections::hash_map::DefaultHasher,
      hash::{Hash, Hasher},
    };

    let mut hasher = DefaultHasher::new();
    line_idx.hash(&mut hasher);
    text.hash(&mut hasher);
    hasher.finish()
  }
}

/// Dirty region tracking for partial redraws
#[derive(Debug, Clone)]
pub struct DirtyRegion {
  /// Dirty lines that need redrawing
  dirty_lines: Vec<usize>,
  /// Whether the entire viewport is dirty
  pub full_redraw: bool,
  /// Viewport bounds for optimization
  viewport_start: usize,
  viewport_end: usize,
}

impl DirtyRegion {
  pub fn new() -> Self {
    Self {
      dirty_lines: Vec::new(),
      full_redraw: true, // Start with full redraw - IMPORTANT for initial render
      viewport_start: 0,
      viewport_end: 0,
    }
  }

  /// Mark a specific line as dirty
  pub fn mark_line_dirty(&mut self, line_idx: usize) {
    if !self.full_redraw && !self.dirty_lines.contains(&line_idx) {
      self.dirty_lines.push(line_idx);
    }
  }

  /// Mark a range of lines as dirty
  pub fn mark_range_dirty(&mut self, start: usize, end: usize) {
    if self.full_redraw {
      return;
    }

    for line in start..=end {
      if !self.dirty_lines.contains(&line) {
        self.dirty_lines.push(line);
      }
    }

    // If too many lines are dirty, just do a full redraw
    if self.dirty_lines.len() > 100 {
      self.mark_all_dirty();
    }
  }

  /// Mark everything as dirty
  pub fn mark_all_dirty(&mut self) {
    self.full_redraw = true;
    self.dirty_lines.clear();
  }

  /// Check if a line needs redrawing
  pub fn is_line_dirty(&self, line_idx: usize) -> bool {
    self.full_redraw || self.dirty_lines.contains(&line_idx)
  }

  /// Clear dirty state after redraw
  pub fn clear(&mut self) {
    self.full_redraw = false;
    self.dirty_lines.clear();
  }

  /// Set the viewport bounds for optimization
  pub fn set_viewport(&mut self, start: usize, end: usize) {
    if self.viewport_start != start || self.viewport_end != end {
      self.viewport_start = start;
      self.viewport_end = end;
      // Viewport change requires full redraw
      self.mark_all_dirty();
    }
  }

  /// Check if we need any redraw at all
  pub fn needs_redraw(&self) -> bool {
    self.full_redraw || !self.dirty_lines.is_empty()
  }

  /// Get all dirty lines within the viewport
  pub fn get_dirty_lines_in_viewport(&self) -> Vec<usize> {
    if self.full_redraw {
      // Return all lines in viewport
      (self.viewport_start..=self.viewport_end).collect()
    } else {
      // Return only dirty lines within viewport
      self
        .dirty_lines
        .iter()
        .filter(|&&line| line >= self.viewport_start && line <= self.viewport_end)
        .copied()
        .collect()
    }
  }

  /// Optimize dirty lines by merging adjacent lines into ranges
  pub fn optimize(&mut self) {
    if self.full_redraw || self.dirty_lines.is_empty() {
      return;
    }

    self.dirty_lines.sort_unstable();
    self.dirty_lines.dedup();
  }

  /// Get viewport bounds
  pub fn viewport_bounds(&self) -> (usize, usize) {
    (self.viewport_start, self.viewport_end)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_dirty_region_starts_with_full_redraw() {
    let region = DirtyRegion::new();
    assert!(region.full_redraw);
    assert!(region.needs_redraw());
  }

  #[test]
  fn test_mark_line_dirty() {
    let mut region = DirtyRegion::new();
    region.clear(); // Clear initial full redraw

    assert!(!region.needs_redraw());

    region.mark_line_dirty(5);
    assert!(region.needs_redraw());
    assert!(region.is_line_dirty(5));
    assert!(!region.is_line_dirty(4));
  }

  #[test]
  fn test_mark_range_dirty() {
    let mut region = DirtyRegion::new();
    region.clear();

    region.mark_range_dirty(10, 15);
    for line in 10..=15 {
      assert!(region.is_line_dirty(line));
    }
    assert!(!region.is_line_dirty(9));
    assert!(!region.is_line_dirty(16));
  }

  #[test]
  fn test_too_many_dirty_lines_triggers_full_redraw() {
    let mut region = DirtyRegion::new();
    region.clear();

    // Mark 101 lines dirty - should trigger full redraw at 100
    region.mark_range_dirty(0, 100);
    assert!(region.full_redraw);
  }

  #[test]
  fn test_viewport_change_marks_all_dirty() {
    let mut region = DirtyRegion::new();
    region.clear();
    assert!(!region.full_redraw);

    // First viewport set - changes from (0,0) to (0,50)
    region.set_viewport(0, 50);
    assert!(region.full_redraw); // Viewport changed, marks dirty

    region.clear();

    // Setting same viewport doesn't mark dirty
    region.set_viewport(0, 50);
    assert!(!region.full_redraw);

    // Changing viewport marks dirty
    region.set_viewport(10, 60);
    assert!(region.full_redraw);
  }

  #[test]
  fn test_clear_resets_dirty_state() {
    let mut region = DirtyRegion::new();
    region.mark_line_dirty(5);

    region.clear();
    assert!(!region.full_redraw);
    assert!(!region.needs_redraw());
  }

  #[test]
  fn test_get_dirty_lines_in_viewport() {
    let mut region = DirtyRegion::new();
    region.clear();
    region.set_viewport(10, 20);
    region.clear(); // Clear the full redraw from viewport change

    region.mark_line_dirty(5); // Outside viewport
    region.mark_line_dirty(15); // Inside viewport
    region.mark_line_dirty(25); // Outside viewport

    let dirty_in_viewport = region.get_dirty_lines_in_viewport();
    assert_eq!(dirty_in_viewport, vec![15]);
  }

  #[test]
  fn test_full_redraw_returns_all_viewport_lines() {
    let mut region = DirtyRegion::new();
    region.set_viewport(5, 10);

    let dirty_in_viewport = region.get_dirty_lines_in_viewport();
    assert_eq!(dirty_in_viewport, (5..=10).collect::<Vec<_>>());
  }

  #[test]
  fn test_optimize_sorts_and_deduplicates() {
    let mut region = DirtyRegion::new();
    region.clear();

    region.mark_line_dirty(15);
    region.mark_line_dirty(10);
    region.mark_line_dirty(15); // Duplicate
    region.mark_line_dirty(12);

    region.optimize();

    assert_eq!(region.dirty_lines, vec![10, 12, 15]);
  }
}
