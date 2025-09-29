use glyphon::{Buffer, FontSystem, Metrics, Shaping};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Key for caching shaped text buffers
#[derive(Clone, Debug)]
pub struct ShapedTextKey {
    /// The text content
    pub text: String,
    /// Font metrics (size and line height)
    pub metrics: (u32, u32), // Store as fixed point
    /// Text color as RGBA bytes
    pub color: [u8; 4],
    /// Position hash for spatial caching
    pub position_hash: u64,
}

impl Hash for ShapedTextKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.metrics.hash(state);
        self.color.hash(state);
        self.position_hash.hash(state);
    }
}

impl PartialEq for ShapedTextKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.metrics == other.metrics
            && self.color == other.color
            && self.position_hash == other.position_hash
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
            let attrs = Attrs::new()
                .family(Family::SansSerif)
                .metrics(metrics);

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
        entry.buffer.set_size(font_system, Some(width), Some(height));

        &mut entry.buffer
    }

    /// Advance to the next frame
    pub fn next_frame(&mut self) {
        self.current_frame += 1;

        // Periodically clean up stale entries
        if self.current_frame % 60 == 0 {
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
        self.entries.retain(|_, entry| entry.last_used_frame > stale_threshold);
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