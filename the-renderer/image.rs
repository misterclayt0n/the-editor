//! Image decoding support for preview rendering.
//!
//! This module provides functionality to decode various image formats
//! (PNG, JPEG, GIF, WebP, BMP, ICO, TIFF) and SVG files into RGBA pixel
//! buffers suitable for GPU texture upload.
//!
//! Animated GIFs are supported via `decode_animated_gif()` which returns
//! all frames with their timing information.

use image::GenericImageView;

/// Maximum dimension for decoded images. Images larger than this will be
/// downscaled to fit within this size while preserving aspect ratio.
const MAX_IMAGE_DIMENSION: u32 = 1024;

/// Minimum frame delay in milliseconds.
/// GIFs with 0 or very small delays are treated as "as fast as possible",
/// but most viewers clamp this to ~100ms (10fps) to prevent CPU spikes.
/// We use 100ms as the default for 0-delay frames, matching browser behavior.
const MIN_FRAME_DELAY_MS: u32 = 20;

/// Default delay for frames with 0 delay (common in older GIFs)
const DEFAULT_FRAME_DELAY_MS: u32 = 100;

/// A decoded image ready for GPU upload.
#[derive(Clone)]
pub struct DecodedImage {
  /// RGBA pixel data (4 bytes per pixel)
  pub pixels: Vec<u8>,
  /// Width in pixels
  pub width:  u32,
  /// Height in pixels
  pub height: u32,
}

impl DecodedImage {
  /// Create a new decoded image from raw RGBA data.
  pub fn new(pixels: Vec<u8>, width: u32, height: u32) -> Self {
    debug_assert_eq!(pixels.len(), (width * height * 4) as usize);
    Self {
      pixels,
      width,
      height,
    }
  }
}

/// A single frame in an animation.
#[derive(Clone)]
pub struct AnimationFrame {
  /// RGBA pixel data (4 bytes per pixel), fully composited
  pub pixels:   Vec<u8>,
  /// Delay before showing the next frame, in milliseconds
  pub delay_ms: u32,
}

/// A decoded animation with multiple frames.
#[derive(Clone)]
pub struct DecodedAnimation {
  /// Animation frames in order
  pub frames: Vec<AnimationFrame>,
  /// Canvas width in pixels
  pub width:  u32,
  /// Canvas height in pixels
  pub height: u32,
}

impl DecodedAnimation {
  /// Check if this animation has multiple frames.
  pub fn is_animated(&self) -> bool {
    self.frames.len() > 1
  }

  /// Get total duration of one loop in milliseconds.
  pub fn total_duration_ms(&self) -> u32 {
    self.frames.iter().map(|f| f.delay_ms).sum()
  }
}

/// Decode a raster image (PNG, JPEG, GIF, WebP, BMP, ICO, TIFF) from bytes.
///
/// The image will be downscaled if either dimension exceeds
/// `MAX_IMAGE_DIMENSION` while preserving aspect ratio. Returns `None` if
/// decoding fails.
pub fn decode_image(data: &[u8]) -> Option<DecodedImage> {
  decode_image_with_max_size(data, MAX_IMAGE_DIMENSION)
}

/// Decode a raster image with a custom maximum dimension.
///
/// The image will be downscaled if either dimension exceeds `max_size`
/// while preserving aspect ratio.
pub fn decode_image_with_max_size(data: &[u8], max_size: u32) -> Option<DecodedImage> {
  let img = image::load_from_memory(data).ok()?;

  let (orig_width, orig_height) = img.dimensions();
  if orig_width == 0 || orig_height == 0 {
    return None;
  }

  // Calculate scale factor if image is too large
  let scale = if orig_width > max_size || orig_height > max_size {
    let scale_x = max_size as f32 / orig_width as f32;
    let scale_y = max_size as f32 / orig_height as f32;
    scale_x.min(scale_y)
  } else {
    1.0
  };

  let (width, height) = if scale < 1.0 {
    let new_width = (orig_width as f32 * scale).round() as u32;
    let new_height = (orig_height as f32 * scale).round() as u32;
    (new_width.max(1), new_height.max(1))
  } else {
    (orig_width, orig_height)
  };

  // Resize if needed, then convert to RGBA
  let rgba = if scale < 1.0 {
    img
      .resize(width, height, image::imageops::FilterType::Lanczos3)
      .into_rgba8()
  } else {
    img.into_rgba8()
  };

  Some(DecodedImage::new(rgba.into_raw(), width, height))
}

/// Decode an SVG file to RGBA pixels at a dynamic size.
///
/// The SVG will be rendered at its natural aspect ratio, scaled to fit
/// within `max_width` x `max_height` while preserving aspect ratio.
/// Colors are preserved (not tinted like icons).
pub fn decode_svg(data: &[u8], max_width: u32, max_height: u32) -> Option<DecodedImage> {
  if max_width == 0 || max_height == 0 {
    return None;
  }

  // Parse the SVG
  let tree = resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default()).ok()?;

  // Get the original SVG size
  let svg_size = tree.size();
  let svg_width = svg_size.width();
  let svg_height = svg_size.height();

  if svg_width <= 0.0 || svg_height <= 0.0 {
    return None;
  }

  // Calculate scale to fit within target size while preserving aspect ratio
  let scale_x = max_width as f32 / svg_width;
  let scale_y = max_height as f32 / svg_height;
  let scale = scale_x.min(scale_y);

  // Calculate actual rendered size
  let width = (svg_width * scale).ceil() as u32;
  let height = (svg_height * scale).ceil() as u32;

  if width == 0 || height == 0 {
    return None;
  }

  // Create pixmap at target size
  let mut pixmap = tiny_skia::Pixmap::new(width, height)?;

  // Create transform to scale the SVG
  let transform = tiny_skia::Transform::from_scale(scale, scale);

  // Render the SVG with original colors
  resvg::render(&tree, transform, &mut pixmap.as_mut());

  Some(DecodedImage::new(pixmap.take(), width, height))
}

/// Decode an animated GIF into multiple frames.
///
/// Each frame is fully composited (disposal methods are handled automatically).
/// The frames are downscaled if the GIF dimensions exceed
/// `MAX_IMAGE_DIMENSION`. Frame delays are clamped to a minimum of
/// `MIN_FRAME_DELAY_MS` (60fps cap).
///
/// Returns `None` if decoding fails. For non-animated GIFs (single frame),
/// this still returns a `DecodedAnimation` with one frame.
pub fn decode_animated_gif(data: &[u8]) -> Option<DecodedAnimation> {
  decode_animated_gif_with_max_size(data, MAX_IMAGE_DIMENSION)
}

/// Decode an animated GIF with a custom maximum dimension.
pub fn decode_animated_gif_with_max_size(data: &[u8], max_size: u32) -> Option<DecodedAnimation> {
  use std::io::Cursor;

  use image::{
    AnimationDecoder,
    codecs::gif::GifDecoder,
    imageops::FilterType,
  };

  let cursor = Cursor::new(data);
  let decoder = GifDecoder::new(cursor).ok()?;

  // Get the frames iterator
  let frames_iter = decoder.into_frames();

  let mut frames = Vec::new();
  let mut canvas_width = 0u32;
  let mut canvas_height = 0u32;
  let mut scale = 1.0f32;
  let mut scale_computed = false;

  for frame_result in frames_iter {
    let frame = frame_result.ok()?;

    let buffer = frame.buffer();
    let (w, h) = buffer.dimensions();

    // Compute scale on first frame (all frames have same canvas size)
    if !scale_computed {
      canvas_width = w;
      canvas_height = h;

      if w > max_size || h > max_size {
        let scale_x = max_size as f32 / w as f32;
        let scale_y = max_size as f32 / h as f32;
        scale = scale_x.min(scale_y);
      }
      scale_computed = true;
    }

    // Get delay in milliseconds
    // The image crate returns delay as numer/denom in milliseconds
    // GIF stores delay in centiseconds (10ms units), so the image crate
    // converts it to milliseconds by multiplying by 10
    let (numer, denom) = frame.delay().numer_denom_ms();
    let delay_ms = if denom == 0 || numer == 0 {
      // Many GIFs have 0 delay, meaning "as fast as possible"
      // Use a sensible default matching browser behavior
      DEFAULT_FRAME_DELAY_MS
    } else {
      let raw_delay = numer / denom;
      // Clamp very fast delays to minimum (prevents CPU spikes)
      if raw_delay < MIN_FRAME_DELAY_MS {
        DEFAULT_FRAME_DELAY_MS
      } else {
        raw_delay
      }
    };

    // Scale frame if needed
    let pixels = if scale < 1.0 {
      let new_w = (w as f32 * scale).round() as u32;
      let new_h = (h as f32 * scale).round() as u32;
      let resized = image::imageops::resize(buffer, new_w, new_h, FilterType::Lanczos3);
      resized.into_raw()
    } else {
      buffer.as_raw().clone()
    };

    frames.push(AnimationFrame { pixels, delay_ms });
  }

  if frames.is_empty() {
    return None;
  }

  // Compute final dimensions
  let final_width = if scale < 1.0 {
    (canvas_width as f32 * scale).round() as u32
  } else {
    canvas_width
  };
  let final_height = if scale < 1.0 {
    (canvas_height as f32 * scale).round() as u32
  } else {
    canvas_height
  };

  Some(DecodedAnimation {
    frames,
    width: final_width,
    height: final_height,
  })
}

/// Check if a file extension is GIF.
pub fn is_gif_extension(ext: &str) -> bool {
  ext.eq_ignore_ascii_case("gif")
}

/// Check if a file extension corresponds to a supported image format.
pub fn is_image_extension(ext: &str) -> bool {
  matches!(
    ext.to_lowercase().as_str(),
    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tiff" | "tif"
  )
}

/// Check if a file extension is SVG.
pub fn is_svg_extension(ext: &str) -> bool {
  ext.eq_ignore_ascii_case("svg")
}

#[cfg(test)]
mod tests {
  use super::*;

  // Simple 2x2 red PNG
  const TEST_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, // 2x2
    0x08, 0x02, 0x00, 0x00, 0x00, 0xFD, 0xD4, 0x9A, 0x73, // RGB, no filter
    0x00, 0x00, 0x00, 0x14, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
    0x78, 0x9C, 0x62, 0xF8, 0xCF, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0x00, 0x00, 0x00, 0x19, 0x00,
    0x05, 0xDF, 0x20, 0x9D, 0xA1, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, // IEND chunk
    0xAE, 0x42, 0x60, 0x82,
  ];

  const TEST_SVG: &[u8] = b"<svg width=\"16\" height=\"16\" viewBox=\"0 0 16 16\" xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"2\" y=\"2\" width=\"12\" height=\"12\" fill=\"#ff0000\"/></svg>";

  #[test]
  fn test_decode_svg() {
    let img = decode_svg(TEST_SVG, 32, 32);
    assert!(img.is_some());
    let img = img.unwrap();
    assert!(img.width > 0);
    assert!(img.height > 0);
    assert_eq!(img.pixels.len(), (img.width * img.height * 4) as usize);
  }

  #[test]
  fn test_decode_svg_preserves_aspect() {
    // 16x16 SVG rendered into 64x32 space should be 32x32 (square)
    let img = decode_svg(TEST_SVG, 64, 32).unwrap();
    assert_eq!(img.width, img.height); // Aspect ratio preserved
    assert!(img.width <= 32);
  }

  #[test]
  fn test_is_image_extension() {
    assert!(is_image_extension("png"));
    assert!(is_image_extension("PNG"));
    assert!(is_image_extension("jpg"));
    assert!(is_image_extension("jpeg"));
    assert!(is_image_extension("gif"));
    assert!(is_image_extension("webp"));
    assert!(!is_image_extension("svg"));
    assert!(!is_image_extension("txt"));
  }

  #[test]
  fn test_is_svg_extension() {
    assert!(is_svg_extension("svg"));
    assert!(is_svg_extension("SVG"));
    assert!(!is_svg_extension("png"));
  }
}
