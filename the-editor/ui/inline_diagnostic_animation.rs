//! Animation state and logic for inline diagnostics.
//!
//! Uses exponential decay for smooth, frame-rate independent animations.

/// Animation rate constants (exponential decay factors).
/// Formula: rate = 1.0 - 2.0^(-k * dt)
pub mod rates {
  /// Opacity fade rate (moderate speed)
  pub const OPACITY_K: f32 = 40.0;
  /// Vertical slide rate (fast, snappy)
  pub const SLIDE_K: f32 = 60.0;
  /// Debounce delay before animation starts (milliseconds)
  pub const DEBOUNCE_MS: u64 = 350;
}

/// Convergence threshold for animation completion.
const EPSILON: f32 = 0.01;

/// Animation state for a single inline diagnostic on a document line.
#[derive(Debug, Clone, Copy)]
pub struct InlineDiagnosticAnimState {
  /// Overall opacity of the diagnostic box (0 = invisible, 1 = fully visible)
  pub opacity: f32,
  /// Vertical slide offset in line-heights (starts > 0, animates to 0)
  /// Represents how many line-heights the box is offset upward from target
  pub slide_offset: f32,
}

impl Default for InlineDiagnosticAnimState {
  fn default() -> Self {
    Self {
      opacity:      0.0,
      slide_offset: 0.5, // Start half a line up, slide down to position
    }
  }
}

impl InlineDiagnosticAnimState {
  /// Check if animation is complete for fade-out (fully hidden)
  pub fn is_hidden(&self) -> bool {
    self.opacity < EPSILON
  }
}

/// Target values for animation (what we're animating toward)
#[derive(Debug, Clone, Copy)]
pub struct InlineDiagnosticAnimTarget {
  pub opacity: f32,
}

impl InlineDiagnosticAnimTarget {
  /// Target state for fully visible diagnostic
  pub fn visible() -> Self {
    Self { opacity: 1.0 }
  }

  /// Target state for hidden diagnostic (fading out)
  pub fn hidden() -> Self {
    Self { opacity: 0.0 }
  }
}

/// Update animation state toward target using exponential decay.
/// Returns `true` if any animation is still in progress.
pub fn update_animation(
  state: &mut InlineDiagnosticAnimState,
  target: InlineDiagnosticAnimTarget,
  dt: f32,
) -> bool {
  let mut animating = false;

  // Opacity animation
  let opacity_rate = 1.0 - 2.0_f32.powf(-rates::OPACITY_K * dt);
  let opacity_delta = target.opacity - state.opacity;
  if opacity_delta.abs() > EPSILON {
    state.opacity += opacity_rate * opacity_delta;
    animating = true;
  } else {
    state.opacity = target.opacity;
  }

  // Slide animation
  // When fading out (target opacity = 0), slide up instead of down
  let slide_target = if target.opacity < EPSILON {
    -0.5 // Slide up when fading out
  } else {
    0.0 // Slide to position when fading in
  };

  let slide_rate = 1.0 - 2.0_f32.powf(-rates::SLIDE_K * dt);
  let slide_delta = slide_target - state.slide_offset;
  if slide_delta.abs() > EPSILON {
    state.slide_offset += slide_rate * slide_delta;
    animating = true;
  } else {
    state.slide_offset = slide_target;
  }

  animating
}
