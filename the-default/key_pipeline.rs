use the_dispatch::define;
use the_lib::input::{KeyEvent, KeyOutcome};

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

pub fn default_key_pipeline<Ctx>() -> KeyPipelineDispatch<Ctx,
  fn(&mut Ctx, KeyEvent) -> KeyOutcome,
  fn(&mut Ctx, KeyEvent) -> KeyOutcome,
  fn(&mut Ctx, KeyEvent) -> KeyOutcome,
> {
  KeyPipelineDispatch::new()
    .with_pre(default_key_hook::<Ctx> as fn(&mut Ctx, KeyEvent) -> KeyOutcome)
    .with_on(default_key_hook::<Ctx> as fn(&mut Ctx, KeyEvent) -> KeyOutcome)
    .with_post(default_key_hook::<Ctx> as fn(&mut Ctx, KeyEvent) -> KeyOutcome)
}

/// Build a key hook from a closure expression without fighting HRTB inference.
///
/// Example:
/// ```
/// use the_default::{KeyEvent, KeyOutcome, key_hook};
///
/// let hook = key_hook!(|_ctx, _key: KeyEvent| KeyOutcome::Continue);
/// ```
#[macro_export]
macro_rules! key_hook {
  ($body:expr) => {{
    fn __key_hook<Ctx>(
      ctx: &mut Ctx,
      key: $crate::KeyEvent,
    ) -> $crate::KeyOutcome {
      let f = $body;
      f(ctx, key)
    }
    __key_hook::<Ctx> as fn(&mut Ctx, $crate::KeyEvent) -> $crate::KeyOutcome
  }};
}
