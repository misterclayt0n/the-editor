use std::time::{
  Duration,
  Instant,
};

use the_editor_renderer::Color;

/// Animation configuration presets for common use cases
pub mod presets {
  use super::*;

  /// Fast, snappy animation (150ms, EaseOutQuad)
  pub const FAST: (Duration, Easing) = (Duration::from_millis(100), Easing::EaseOutQuad);

  /// Medium speed animation (250ms, EaseInOutQuad)
  pub const MEDIUM: (Duration, Easing) = (Duration::from_millis(150), Easing::EaseInOutQuad);

  /// Slow, smooth animation (400ms, EaseInOutCubic)
  pub const SLOW: (Duration, Easing) = (Duration::from_millis(200), Easing::EaseInOutCubic);

  /// Very fast, instant-like (50ms, Linear)
  pub const INSTANT: (Duration, Easing) = (Duration::from_millis(50), Easing::Linear);

  /// Cursor animation (150ms, EaseOutQuad) - snappy and responsive
  pub const CURSOR: (Duration, Easing) = (Duration::from_millis(25), Easing::Linear);

  /// Scroll animation (300ms, EaseOutQuart) - smooth and natural
  pub const SCROLL: (Duration, Easing) = (Duration::from_millis(300), Easing::EaseOutQuart);

  /// Fade animation (200ms, EaseInOut)
  pub const FADE: (Duration, Easing) = (Duration::from_millis(200), Easing::EaseInOut);

  /// Popup animation (180ms, EaseOutCubic)
  pub const POPUP: (Duration, Easing) = (Duration::from_millis(180), Easing::EaseOutCubic);
}

pub mod selection {
  use std::f32::consts::TAU;

  use super::*;

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum SelectionPulseKind {
    SearchMatch,
    FilteredSelection,
    YankHighlight,
  }

  #[derive(Debug, Clone, Copy)]
  pub struct PulseSample {
    pub elapsed: f32,
    pub energy:  f32,
  }

  #[derive(Debug, Clone)]
  pub struct SelectionPulse {
    started_at: Instant,
    duration:   Duration,
    kind:       SelectionPulseKind,
  }

  impl SelectionPulse {
    const SEARCH_DURATION: Duration = Duration::from_millis(2100);
    const FILTER_DURATION: Duration = Duration::from_millis(2400);
    const YANK_DURATION: Duration = Duration::from_millis(100);
    const FADE_OUT_DURATION: Duration = Duration::from_millis(420);

    pub fn new(kind: SelectionPulseKind) -> Self {
      Self {
        started_at: Instant::now(),
        duration: Self::duration_for(kind),
        kind,
      }
    }

    pub fn kind(&self) -> SelectionPulseKind {
      self.kind
    }

    pub fn duration(&self) -> Duration {
      self.duration
    }

    pub fn sample(&self, now: Instant) -> Option<PulseSample> {
      let elapsed = now.saturating_duration_since(self.started_at);
      let total_duration = self.duration + Self::FADE_OUT_DURATION;

      if elapsed >= total_duration {
        return None;
      }

      let energy = if elapsed <= self.duration {
        1.0
      } else {
        let fade_elapsed = elapsed - self.duration;
        let fade_window = Self::FADE_OUT_DURATION.as_secs_f32().max(f32::EPSILON);
        (1.0 - fade_elapsed.as_secs_f32() / fade_window).clamp(0.0, 1.0)
      };

      Some(PulseSample {
        elapsed: elapsed.as_secs_f32(),
        energy,
      })
    }

    fn duration_for(kind: SelectionPulseKind) -> Duration {
      match kind {
        SelectionPulseKind::SearchMatch => Self::SEARCH_DURATION,
        SelectionPulseKind::FilteredSelection => Self::FILTER_DURATION,
        SelectionPulseKind::YankHighlight => Self::YANK_DURATION,
      }
    }
  }

  #[derive(Debug, Clone, Copy)]
  pub struct GlowFrame {
    pub color:  Color,
    pub active: bool,
  }

  pub fn evaluate_glow(
    kind: SelectionPulseKind,
    base: Color,
    glow: Color,
    sample: PulseSample,
  ) -> GlowFrame {
    let (frequency, mix_bias, alpha_floor, glow_override) = match kind {
      SelectionPulseKind::SearchMatch => (TAU, 1.1, 0.38, None),
      SelectionPulseKind::FilteredSelection => (TAU * 0.75, 1.25, 0.32, None),
      SelectionPulseKind::YankHighlight => {
        (
          TAU * 1.35,
          1.4,
          0.42,
          Some(Color::rgba(1.0, 0.86, 0.4, (glow.a * 1.2).min(1.0))),
        )
      },
    };

    let wave = (sample.elapsed * frequency).sin().mul_add(0.5, 0.5);
    let amplitude = sample.energy;
    let blend = (wave * mix_bias * amplitude).clamp(0.0, 1.0);
    let glow_target = glow_override.unwrap_or(glow);
    let mut blended = base.lerp(glow_target, blend);
    let alpha_wave = alpha_floor + (1.0 - alpha_floor) * wave * amplitude;
    let alpha_source = if glow_override.is_some() {
      base.a.max(glow_target.a)
    } else {
      base.a
    };
    blended.a = alpha_source * alpha_wave;

    GlowFrame {
      color:  blended,
      active: amplitude > 0.0,
    }
  }
}

/// Easing functions for animations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
  /// Linear interpolation (no easing)
  Linear,
  /// Ease in (slow start)
  EaseIn,
  /// Ease out (slow end)
  EaseOut,
  /// Ease in and out (slow start and end)
  EaseInOut,
  /// Quadratic ease in
  EaseInQuad,
  /// Quadratic ease out
  EaseOutQuad,
  /// Quadratic ease in and out
  EaseInOutQuad,
  /// Cubic ease in
  EaseInCubic,
  /// Cubic ease out
  EaseOutCubic,
  /// Cubic ease in and out
  EaseInOutCubic,
  /// Quartic ease in
  EaseInQuart,
  /// Quartic ease out
  EaseOutQuart,
  /// Quartic ease in and out
  EaseInOutQuart,
}

impl Easing {
  /// Apply the easing function to a linear time value (0.0 to 1.0)
  pub fn apply(self, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match self {
      Easing::Linear => t,
      Easing::EaseIn => t * t,
      Easing::EaseOut => t * (2.0 - t),
      Easing::EaseInOut => {
        if t < 0.5 {
          2.0 * t * t
        } else {
          -1.0 + (4.0 - 2.0 * t) * t
        }
      },
      Easing::EaseInQuad => t * t,
      Easing::EaseOutQuad => t * (2.0 - t),
      Easing::EaseInOutQuad => {
        if t < 0.5 {
          2.0 * t * t
        } else {
          -1.0 + (4.0 - 2.0 * t) * t
        }
      },
      Easing::EaseInCubic => t * t * t,
      Easing::EaseOutCubic => {
        let t = t - 1.0;
        t * t * t + 1.0
      },
      Easing::EaseInOutCubic => {
        if t < 0.5 {
          4.0 * t * t * t
        } else {
          let t = 2.0 * t - 2.0;
          1.0 + t * t * t / 2.0
        }
      },
      Easing::EaseInQuart => t * t * t * t,
      Easing::EaseOutQuart => {
        let t = t - 1.0;
        1.0 - t * t * t * t
      },
      Easing::EaseInOutQuart => {
        if t < 0.5 {
          8.0 * t * t * t * t
        } else {
          let t = t - 1.0;
          1.0 - 8.0 * t * t * t * t
        }
      },
    }
  }
}

/// Trait for types that can be animated (interpolated)
pub trait Animatable: Clone {
  /// Linear interpolation between self and target
  /// t is in the range [0.0, 1.0] where 0.0 = self, 1.0 = target
  fn lerp(&self, target: &Self, t: f32) -> Self;
}

// Implement Animatable for common numeric types
impl Animatable for f32 {
  fn lerp(&self, target: &Self, t: f32) -> Self {
    self + (target - self) * t
  }
}

impl Animatable for f64 {
  fn lerp(&self, target: &Self, t: f32) -> Self {
    self + (target - self) * t as f64
  }
}

impl Animatable for usize {
  fn lerp(&self, target: &Self, t: f32) -> Self {
    let start = *self as f32;
    let end = *target as f32;
    (start + (end - start) * t) as usize
  }
}

// Implement for tuples (useful for cursor positions, etc.)
impl Animatable for (f32, f32) {
  fn lerp(&self, target: &Self, t: f32) -> Self {
    (self.0.lerp(&target.0, t), self.1.lerp(&target.1, t))
  }
}

impl Animatable for (usize, usize) {
  fn lerp(&self, target: &Self, t: f32) -> Self {
    (self.0.lerp(&target.0, t), self.1.lerp(&target.1, t))
  }
}

// Implement for Color (from the-renderer)
impl Animatable for Color {
  fn lerp(&self, target: &Self, t: f32) -> Self {
    Color {
      r: self.r.lerp(&target.r, t),
      g: self.g.lerp(&target.g, t),
      b: self.b.lerp(&target.b, t),
      a: self.a.lerp(&target.a, t),
    }
  }
}

/// Represents an active animation
pub struct Animation<T: Animatable> {
  /// Starting value
  start:    T,
  /// Target value
  target:   T,
  /// Current value (updated each frame)
  current:  T,
  /// Duration of the animation in seconds
  duration: f32,
  /// Time elapsed since animation started (in seconds)
  elapsed:  f32,
  /// Easing function to use
  easing:   Easing,
}

impl<T: Animatable> Animation<T> {
  /// Create a new animation
  pub fn new(start: T, target: T, duration: Duration, easing: Easing) -> Self {
    let duration_secs = duration.as_secs_f32();
    Self {
      current: start.clone(),
      start,
      target,
      duration: duration_secs,
      elapsed: 0.0,
      easing,
    }
  }

  /// Update the animation with the time delta
  /// Returns true if the animation is complete
  pub fn update(&mut self, dt: f32) -> bool {
    self.elapsed += dt;

    if self.elapsed >= self.duration {
      // Animation complete
      self.current = self.target.clone();
      true
    } else {
      // Calculate interpolation factor with easing
      let t = self.elapsed / self.duration;
      let eased_t = self.easing.apply(t);
      self.current = self.start.lerp(&self.target, eased_t);
      false
    }
  }

  /// Get the current value
  pub fn current(&self) -> &T {
    &self.current
  }

  /// Check if animation is complete
  pub fn is_complete(&self) -> bool {
    self.elapsed >= self.duration
  }

  /// Get the target value
  pub fn target(&self) -> &T {
    &self.target
  }

  /// Update the target value (useful for retargeting animations)
  pub fn retarget(&mut self, new_target: T) {
    self.start = self.current.clone();
    self.target = new_target;
    self.elapsed = 0.0;
  }
}

/// Unique identifier for an animation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(usize);

/// Manages all active animations
pub struct AnimationManager {
  next_id:    usize,
  animations: Vec<(AnimationId, Box<dyn AnimationTrait>)>,
}

/// Trait object wrapper for animations of different types
trait AnimationTrait {
  fn update(&mut self, dt: f32) -> bool;
  fn is_complete(&self) -> bool;
}

impl<T: Animatable + 'static> AnimationTrait for Animation<T> {
  fn update(&mut self, dt: f32) -> bool {
    Animation::update(self, dt)
  }

  fn is_complete(&self) -> bool {
    Animation::is_complete(self)
  }
}

impl Default for AnimationManager {
  fn default() -> Self {
    Self::new()
  }
}

impl AnimationManager {
  /// Create a new animation manager
  pub fn new() -> Self {
    Self {
      next_id:    0,
      animations: Vec::new(),
    }
  }

  /// Start a new animation and return its ID
  pub fn animate<T: Animatable + 'static>(
    &mut self,
    from: T,
    to: T,
    duration: Duration,
    easing: Easing,
  ) -> AnimationId {
    let id = AnimationId(self.next_id);
    self.next_id += 1;

    let animation = Animation::new(from, to, duration, easing);
    self.animations.push((id, Box::new(animation)));

    id
  }

  /// Update all animations with the time delta
  /// Removes completed animations
  pub fn update(&mut self, dt: f32) {
    // Update all animations and remove completed ones
    self.animations.retain_mut(|(_, anim)| !anim.update(dt));
  }

  /// Cancel an animation by ID
  pub fn cancel(&mut self, id: AnimationId) {
    self.animations.retain(|(anim_id, _)| *anim_id != id);
  }

  /// Cancel all animations
  pub fn cancel_all(&mut self) {
    self.animations.clear();
  }

  /// Check if there are any active animations
  pub fn has_active_animations(&self) -> bool {
    !self.animations.is_empty()
  }

  /// Get the number of active animations
  pub fn active_count(&self) -> usize {
    self.animations.len()
  }
}

/// Typed animation handle that provides type-safe access to animation values
pub struct AnimationHandle<T: Animatable + 'static> {
  animation: Animation<T>,
}

impl<T: Animatable + 'static> AnimationHandle<T> {
  /// Create a new animation handle
  pub fn new(from: T, to: T, duration: Duration, easing: Easing) -> Self {
    Self {
      animation: Animation::new(from, to, duration, easing),
    }
  }

  /// Update the animation with the time delta
  /// Returns true if the animation is complete
  pub fn update(&mut self, dt: f32) -> bool {
    self.animation.update(dt)
  }

  /// Get the current value
  pub fn current(&self) -> &T {
    self.animation.current()
  }

  /// Check if animation is complete
  pub fn is_complete(&self) -> bool {
    self.animation.is_complete()
  }

  /// Retarget the animation to a new value
  pub fn retarget(&mut self, new_target: T) {
    self.animation.retarget(new_target)
  }

  /// Get the target value
  pub fn target(&self) -> &T {
    self.animation.target()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_easing_linear() {
    assert_eq!(Easing::Linear.apply(0.0), 0.0);
    assert_eq!(Easing::Linear.apply(0.5), 0.5);
    assert_eq!(Easing::Linear.apply(1.0), 1.0);
  }

  #[test]
  fn test_easing_ease_out() {
    let result = Easing::EaseOut.apply(0.5);
    assert!(result > 0.5); // Should be past halfway
  }

  #[test]
  fn test_f32_lerp() {
    let start = 0.0_f32;
    let end = 10.0_f32;
    assert_eq!(start.lerp(&end, 0.0), 0.0);
    assert_eq!(start.lerp(&end, 0.5), 5.0);
    assert_eq!(start.lerp(&end, 1.0), 10.0);
  }

  #[test]
  fn test_animation_update() {
    let mut anim = Animation::new(0.0_f32, 10.0_f32, Duration::from_secs(1), Easing::Linear);

    // At t=0, current should be start
    assert_eq!(*anim.current(), 0.0);

    // At t=0.5s, current should be halfway
    let complete = anim.update(0.5);
    assert!(!complete);
    assert!((anim.current() - 5.0).abs() < 0.001);

    // At t=1.0s, current should be end
    let complete = anim.update(0.5);
    assert!(complete);
    assert_eq!(*anim.current(), 10.0);
  }

  #[test]
  fn test_animation_manager() {
    let mut manager = AnimationManager::new();

    let _id = manager.animate(0.0_f32, 10.0_f32, Duration::from_secs(1), Easing::Linear);

    assert!(manager.has_active_animations());
    assert_eq!(manager.active_count(), 1);

    // Update halfway
    manager.update(0.5);
    assert!(manager.has_active_animations());

    // Update to completion
    manager.update(0.5);
    assert!(!manager.has_active_animations());
  }

  #[test]
  fn test_animation_handle() {
    let mut handle =
      AnimationHandle::new(0.0_f32, 10.0_f32, Duration::from_secs(1), Easing::Linear);

    assert_eq!(*handle.current(), 0.0);
    assert!(!handle.is_complete());

    handle.update(0.5);
    assert!((handle.current() - 5.0).abs() < 0.001);

    handle.update(0.5);
    assert_eq!(*handle.current(), 10.0);
    assert!(handle.is_complete());
  }

  #[test]
  fn test_animation_retarget() {
    let mut handle =
      AnimationHandle::new(0.0_f32, 10.0_f32, Duration::from_secs(1), Easing::Linear);

    handle.update(0.5);
    assert!((handle.current() - 5.0).abs() < 0.001);

    // Retarget to 20.0 from current position
    handle.retarget(20.0);
    assert_eq!(*handle.current(), 5.0); // Should stay at current position

    handle.update(0.5);
    assert!((handle.current() - 12.5).abs() < 0.001); // Halfway from 5.0 to 20.0
  }
}
