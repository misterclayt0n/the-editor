use std::{
  collections::HashMap,
  hash::{Hash, Hasher},
};

use glyphon::{Buffer, FontSystem, Metrics, Shaping};

/// Key for caching shaped text buffers
#[derive(Clone, Debug)]
pub struct ShapedTextKey {
  /// The text content
  pub text: String,
  /// Font metrics (size and line height)
  pub metrics: (u32, u32), // Store as fixed point
  /// Text color as RGBA bytes
  pub color: [u8; 4],
}

impl Hash for ShapedTextKey {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.text.hash(state);
    self.metrics.hash(state);
    self.color.hash(state);
  }
}

impl PartialEq for ShapedTextKey {
  fn eq(&self, other: &Self) -> bool {
    self.text == other.text && self.metrics == other.metrics && self.color == other.color
  }
}

impl Eq for ShapedTextKey {}

/// A cached shaped text buffer
pub struct CachedShapedText {
  /// The shaped buffer ready for rendering
  pub buffer: Buffer,
  /// Frame when this was last used
  pub last_used_frame: u64,
  /// Generation number for invalidation
  pub generation: u64,
}

/// Cache for shaped text to avoid re-shaping identical text
pub struct ShapedTextCache {
  /// Map from text key to cached shaped buffer
  pub entries: HashMap<ShapedTextKey, CachedShapedText>,
  /// Current frame number for LRU tracking
  pub current_frame: u64,
  /// Generation counter for cache invalidation
  pub current_generation: u64,
  /// Maximum number of cached entries
  max_entries: usize,
  /// Number of cache hits for statistics
  pub hits: u64,
  /// Number of cache misses for statistics
  pub misses: u64,
}

impl ShapedTextCache {
  /// Create a new shaped text cache
  pub fn new(max_entries: usize) -> Self {
    Self {
      entries: HashMap::with_capacity(max_entries / 2),
      current_frame: 0,
      current_generation: 0,
      max_entries,
      hits: 0,
      misses: 0,
    }
  }

  /// Get a shaped buffer from cache or create a new one
  pub fn get_or_shape(
    &mut self,
    key: ShapedTextKey,
    font_system: &mut FontSystem,
    metrics: Metrics,
    width: f32,
    height: f32,
  ) -> &mut Buffer {
    // Check if we need to create a new entry
    if !self.entries.contains_key(&key) {
      self.misses += 1;

      // Create new buffer and shape the text
      let mut buffer = Buffer::new(font_system, metrics);
      buffer.set_size(font_system, Some(width), Some(height));

      // Set the text and shape it
      use glyphon::{Attrs, Family};
      let attrs = Attrs::new().family(Family::SansSerif).metrics(metrics);

      buffer.set_text(font_system, &key.text, &attrs, Shaping::Advanced);
      buffer.shape_until_scroll(font_system, false);

      // Evict old entries if needed
      if self.entries.len() >= self.max_entries {
        self.evict_lru();
      }

      // Insert into cache
      let entry = CachedShapedText {
        buffer,
        last_used_frame: self.current_frame,
        generation: self.current_generation,
      };

      self.entries.insert(key.clone(), entry);
    } else {
      self.hits += 1;
    }

    // Get the entry and update its metadata
    let entry = self.entries.get_mut(&key).unwrap();
    entry.last_used_frame = self.current_frame;

    // Update buffer size in case window resized
    entry
      .buffer
      .set_size(font_system, Some(width), Some(height));

    &mut entry.buffer
  }

  /// Advance to the next frame
  pub fn next_frame(&mut self) {
    self.current_frame += 1;

    // Periodically clean up stale entries
    if self.current_frame.is_multiple_of(60) {
      self.cleanup_stale_entries();
    }
  }

  /// Evict the least recently used entry
  pub fn evict_lru(&mut self) {
    if let Some((key, _)) = self
      .entries
      .iter()
      .min_by_key(|(_, entry)| entry.last_used_frame)
      .map(|(k, e)| (k.clone(), e.last_used_frame))
    {
      self.entries.remove(&key);
    }
  }

  /// Remove entries that haven't been used recently
  fn cleanup_stale_entries(&mut self) {
    let stale_threshold = self.current_frame.saturating_sub(300); // 5 seconds at 60 FPS
    self
      .entries
      .retain(|_, entry| entry.last_used_frame > stale_threshold);
  }

  /// Invalidate the cache (e.g., when font changes)
  pub fn invalidate(&mut self) {
    self.current_generation += 1;
    self.entries.clear();
  }

  /// Clear the entire cache
  pub fn clear(&mut self) {
    self.entries.clear();
    self.hits = 0;
    self.misses = 0;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_cache_key_equality_ignores_position() {
    // Same text, color, and metrics should be equal regardless of position
    let key1 = ShapedTextKey {
      text: "Hello".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    let key2 = ShapedTextKey {
      text: "Hello".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    assert_eq!(key1, key2);
  }

  #[test]
  fn test_cache_key_different_text() {
    let key1 = ShapedTextKey {
      text: "Hello".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    let key2 = ShapedTextKey {
      text: "World".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    assert_ne!(key1, key2);
  }

  #[test]
  fn test_cache_key_different_color() {
    let key1 = ShapedTextKey {
      text: "Hello".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    let key2 = ShapedTextKey {
      text: "Hello".to_string(),
      metrics: (1400, 1680),
      color: [255, 0, 0, 255],
    };

    assert_ne!(key1, key2);
  }

  #[test]
  fn test_cache_eviction() {
    let mut cache = ShapedTextCache::new(2); // Small cache for testing

    let key1 = ShapedTextKey {
      text: "First".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    let key2 = ShapedTextKey {
      text: "Second".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    let key3 = ShapedTextKey {
      text: "Third".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    // Add two entries
    let mut font_system = FontSystem::new();
    let metrics = Metrics::new(14.0, 16.8);

    cache.get_or_shape(key1.clone(), &mut font_system, metrics, 100.0, 100.0);
    cache.next_frame(); // Advance frame so key1 is older
    cache.get_or_shape(key2.clone(), &mut font_system, metrics, 100.0, 100.0);

    assert_eq!(cache.entries.len(), 2);
    assert_eq!(cache.misses, 2);

    cache.next_frame(); // Advance frame again
    // Add third entry - should evict least recently used (key1)
    cache.get_or_shape(key3.clone(), &mut font_system, metrics, 100.0, 100.0);

    // Cache should still be at max size
    assert_eq!(cache.entries.len(), 2);

    // First key should have been evicted (oldest frame)
    assert!(!cache.entries.contains_key(&key1));
    assert!(cache.entries.contains_key(&key2));
    assert!(cache.entries.contains_key(&key3));
  }

  #[test]
  fn test_cache_hit_tracking() {
    let mut cache = ShapedTextCache::new(10);
    let mut font_system = FontSystem::new();
    let metrics = Metrics::new(14.0, 16.8);

    let key = ShapedTextKey {
      text: "Test".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    // First access is a miss
    cache.get_or_shape(key.clone(), &mut font_system, metrics, 100.0, 100.0);
    assert_eq!(cache.hits, 0);
    assert_eq!(cache.misses, 1);

    // Second access is a hit
    cache.get_or_shape(key.clone(), &mut font_system, metrics, 100.0, 100.0);
    assert_eq!(cache.hits, 1);
    assert_eq!(cache.misses, 1);

    // Third access is a hit
    cache.get_or_shape(key.clone(), &mut font_system, metrics, 100.0, 100.0);
    assert_eq!(cache.hits, 2);
    assert_eq!(cache.misses, 1);
  }

  #[test]
  fn test_cache_frame_tracking() {
    let mut cache = ShapedTextCache::new(10);
    let mut font_system = FontSystem::new();
    let metrics = Metrics::new(14.0, 16.8);

    let key = ShapedTextKey {
      text: "Test".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    cache.get_or_shape(key.clone(), &mut font_system, metrics, 100.0, 100.0);

    let entry = cache.entries.get(&key).unwrap();
    assert_eq!(entry.last_used_frame, 0);

    // Advance frame
    cache.next_frame();
    assert_eq!(cache.current_frame, 1);

    // Access again
    cache.get_or_shape(key.clone(), &mut font_system, metrics, 100.0, 100.0);

    let entry = cache.entries.get(&key).unwrap();
    assert_eq!(entry.last_used_frame, 1);
  }

  #[test]
  fn test_cache_invalidation() {
    let mut cache = ShapedTextCache::new(10);
    let mut font_system = FontSystem::new();
    let metrics = Metrics::new(14.0, 16.8);

    let key = ShapedTextKey {
      text: "Test".to_string(),
      metrics: (1400, 1680),
      color: [255, 255, 255, 255],
    };

    cache.get_or_shape(key.clone(), &mut font_system, metrics, 100.0, 100.0);
    assert_eq!(cache.entries.len(), 1);

    let gen_before = cache.current_generation;

    // Invalidate cache
    cache.invalidate();

    assert_eq!(cache.entries.len(), 0);
    assert_eq!(cache.current_generation, gen_before + 1);
  }
}
