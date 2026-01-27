use crate::define;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Up,
  Down,
  Left,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
  InsertChar(char),
  DeleteChar,
  Move(Direction),
  AddCursor(Direction),
  Save,
  Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
  Char(char),
  Enter,
  Backspace,
  Left,
  Right,
  Up,
  Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Modifiers {
  bits: u8,
}

impl Modifiers {
  pub const CTRL: u8 = 0b0000_0001;
  pub const ALT: u8 = 0b0000_0010;

  #[must_use]
  pub const fn empty() -> Self {
    Self { bits: 0 }
  }

  #[must_use]
  pub const fn is_empty(self) -> bool {
    self.bits == 0
  }

  #[must_use]
  pub const fn ctrl(self) -> bool {
    (self.bits & Self::CTRL) != 0
  }

  #[must_use]
  pub const fn alt(self) -> bool {
    (self.bits & Self::ALT) != 0
  }

  pub fn insert(&mut self, bits: u8) {
    self.bits |= bits;
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
  pub key:       Key,
  pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
  Continue,
  Handled,
  Command(Command),
}

impl Default for KeyOutcome {
  fn default() -> Self {
    Self::Continue
  }
}

fn default_key_hook<Ctx>(_ctx: &mut Ctx, _key: KeyEvent) -> KeyOutcome {
  KeyOutcome::Continue
}

define! {
  KeyPipeline {
    pre: KeyEvent => KeyOutcome,
    on: KeyEvent => KeyOutcome,
    post: KeyEvent => KeyOutcome,
  }
}

pub fn default_key_pipeline<Ctx>() -> KeyPipelineDispatch<Ctx, fn(&mut Ctx, KeyEvent) -> KeyOutcome, fn(&mut Ctx, KeyEvent) -> KeyOutcome, fn(&mut Ctx, KeyEvent) -> KeyOutcome> {
  KeyPipelineDispatch::new()
    .with_pre(default_key_hook::<Ctx> as fn(&mut Ctx, KeyEvent) -> KeyOutcome)
    .with_on(default_key_hook::<Ctx> as fn(&mut Ctx, KeyEvent) -> KeyOutcome)
    .with_post(default_key_hook::<Ctx> as fn(&mut Ctx, KeyEvent) -> KeyOutcome)
}

/// Build a key hook from a closure expression without fighting HRTB inference.
///
/// Example:
/// ```
/// use the_dispatch::editor::{KeyEvent, KeyOutcome};
/// use the_dispatch::key_hook;
///
/// let hook = key_hook!(|_ctx, _key: KeyEvent| KeyOutcome::Continue);
/// ```
#[macro_export]
macro_rules! key_hook {
  ($body:expr) => {{
    fn __key_hook<Ctx>(
      ctx: &mut Ctx,
      key: $crate::editor::KeyEvent,
    ) -> $crate::editor::KeyOutcome {
      let f = $body;
      f(ctx, key)
    }
    __key_hook::<Ctx> as fn(&mut Ctx, $crate::editor::KeyEvent) -> $crate::editor::KeyOutcome
  }};
}
