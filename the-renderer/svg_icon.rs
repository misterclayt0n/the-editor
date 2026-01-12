//! SVG icon rendering support using resvg.
//!
//! This module provides functionality to render SVG icons to pixel buffers
//! that can be drawn by the renderer. Icons are cached at specific sizes
//! for efficient reuse.

use std::collections::HashMap;

use tiny_skia::Pixmap;

/// A cached SVG icon that has been rasterized at a specific size.
#[derive(Clone)]
pub struct RasterizedIcon {
  pub pixmap: Pixmap,
  pub width: u32,
  pub height: u32,
}

/// Cache key for rasterized icons: (svg_data_hash, width, height)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct IconCacheKey {
  svg_hash: u64,
  width: u32,
  height: u32,
}

/// Cache for rasterized SVG icons.
pub struct SvgIconCache {
  /// Cached rasterized icons keyed by (hash, size)
  cache: HashMap<IconCacheKey, RasterizedIcon>,
  /// Parsed SVG trees keyed by hash (to avoid re-parsing)
  svg_data: HashMap<u64, Vec<u8>>,
}

impl SvgIconCache {
  pub fn new() -> Self {
    Self {
      cache: HashMap::new(),
      svg_data: HashMap::new(),
    }
  }

  /// Hash SVG data for cache lookup.
  fn hash_svg(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
  }

  /// Rasterize an SVG at the specified size.
  ///
  /// The SVG will be scaled to fit within the given dimensions while
  /// preserving aspect ratio.
  pub fn rasterize(&mut self, svg_data: &[u8], width: u32, height: u32) -> Option<&RasterizedIcon> {
    let hash = Self::hash_svg(svg_data);
    let key = IconCacheKey {
      svg_hash: hash,
      width,
      height,
    };

    // Check cache first
    if self.cache.contains_key(&key) {
      return self.cache.get(&key);
    }

    // Store SVG data if not already stored
    if !self.svg_data.contains_key(&hash) {
      self.svg_data.insert(hash, svg_data.to_vec());
    }

    // Parse and render the SVG
    let icon = render_svg(svg_data, width, height)?;
    self.cache.insert(key.clone(), icon);
    self.cache.get(&key)
  }

  /// Clear the cache.
  pub fn clear(&mut self) {
    self.cache.clear();
    self.svg_data.clear();
  }

  /// Get cache statistics.
  pub fn stats(&self) -> (usize, usize) {
    (self.cache.len(), self.svg_data.len())
  }
}

impl Default for SvgIconCache {
  fn default() -> Self {
    Self::new()
  }
}

/// Render an SVG to a pixmap at the specified size.
///
/// The SVG will be scaled to fit within the given dimensions while
/// preserving aspect ratio. The icon will be centered if aspect ratios differ.
pub fn render_svg(svg_data: &[u8], width: u32, height: u32) -> Option<RasterizedIcon> {
  if width == 0 || height == 0 {
    return None;
  }

  // Parse the SVG
  let tree = resvg::usvg::Tree::from_data(svg_data, &resvg::usvg::Options::default()).ok()?;

  // Get the original SVG size
  let svg_size = tree.size();
  let svg_width = svg_size.width();
  let svg_height = svg_size.height();

  if svg_width <= 0.0 || svg_height <= 0.0 {
    return None;
  }

  // Calculate scale to fit within target size while preserving aspect ratio
  let scale_x = width as f32 / svg_width;
  let scale_y = height as f32 / svg_height;
  let scale = scale_x.min(scale_y);

  // Calculate actual rendered size
  let rendered_width = (svg_width * scale).ceil() as u32;
  let rendered_height = (svg_height * scale).ceil() as u32;

  // Create pixmap at target size
  let mut pixmap = Pixmap::new(width, height)?;

  // Calculate offset to center the icon
  let offset_x = (width as f32 - rendered_width as f32) / 2.0;
  let offset_y = (height as f32 - rendered_height as f32) / 2.0;

  // Create transform: translate to center, then scale
  let transform = tiny_skia::Transform::from_translate(offset_x, offset_y).post_scale(scale, scale);

  // Render the SVG
  resvg::render(&tree, transform, &mut pixmap.as_mut());

  Some(RasterizedIcon {
    pixmap,
    width,
    height,
  })
}

/// Render an SVG with a specific color applied.
///
/// This renders the SVG and then applies the color to all non-transparent
/// pixels, useful for monochrome icons that should match theme colors.
pub fn render_svg_with_color(
  svg_data: &[u8],
  width: u32,
  height: u32,
  color: (u8, u8, u8, u8),
) -> Option<RasterizedIcon> {
  let mut icon = render_svg(svg_data, width, height)?;

  // Apply color to all pixels, preserving alpha
  let data = icon.pixmap.data_mut();
  for chunk in data.chunks_exact_mut(4) {
    let alpha = chunk[3];
    if alpha > 0 {
      // Blend the color with the existing alpha
      chunk[0] = color.0;
      chunk[1] = color.1;
      chunk[2] = color.2;
      // Multiply alphas
      chunk[3] = ((alpha as u16 * color.3 as u16) / 255) as u8;
    }
  }

  Some(icon)
}

#[cfg(test)]
mod tests {
  use super::*;

  const TEST_SVG: &[u8] = br#"<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
<rect x="2" y="2" width="12" height="12" fill="black"/>
</svg>"#;

  #[test]
  fn test_render_svg() {
    let icon = render_svg(TEST_SVG, 16, 16);
    assert!(icon.is_some());
    let icon = icon.unwrap();
    assert_eq!(icon.width, 16);
    assert_eq!(icon.height, 16);
  }

  #[test]
  fn test_render_svg_scaled() {
    let icon = render_svg(TEST_SVG, 32, 32);
    assert!(icon.is_some());
    let icon = icon.unwrap();
    assert_eq!(icon.width, 32);
    assert_eq!(icon.height, 32);
  }

  #[test]
  fn test_cache() {
    let mut cache = SvgIconCache::new();

    // First call should parse and cache
    let icon1 = cache.rasterize(TEST_SVG, 16, 16);
    assert!(icon1.is_some());

    // Second call should hit cache
    let icon2 = cache.rasterize(TEST_SVG, 16, 16);
    assert!(icon2.is_some());

    // Different size should create new cache entry
    let icon3 = cache.rasterize(TEST_SVG, 32, 32);
    assert!(icon3.is_some());

    let (rasterized, parsed) = cache.stats();
    assert_eq!(rasterized, 2); // 16x16 and 32x32
    assert_eq!(parsed, 1); // Same SVG data
  }

  #[test]
  fn test_render_with_color() {
    let icon = render_svg_with_color(TEST_SVG, 16, 16, (255, 0, 0, 255));
    assert!(icon.is_some());
  }
}
