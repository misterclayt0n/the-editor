#![no_main]

mod common;

use std::mem;

use libfuzzer_sys::fuzz_target;
use the_lib::syntax::generate_edits;

use crate::common::{
  apply_edit,
  session_from_bytes,
  short_timeout,
  warm_highlights,
};

fuzz_target!(|data: &[u8]| {
  let Some(mut session) = session_from_bytes(data) else {
    return;
  };

  let mut from_changeset = session.syntax.clone();
  let mut from_edits = session.syntax;

  for (index, op) in mem::take(&mut session.ops).into_iter().enumerate() {
    let Some((old_text, changes)) = apply_edit(&mut session.text, &op) else {
      continue;
    };
    let edits = generate_edits(old_text.slice(..), &changes);

    from_changeset.interpolate(old_text.slice(..), &changes);
    from_edits.interpolate_with_edits(&edits);

    let timeout = short_timeout(session.seed, index);
    let _ = from_changeset.try_update_with_short_timeout(
      session.text.slice(..),
      &edits,
      session.loader.as_ref(),
      timeout,
    );
    let _ = from_edits.try_update_with_short_timeout(
      session.text.slice(..),
      &edits,
      session.loader.as_ref(),
      timeout,
    );

    warm_highlights(&from_changeset, &session.text, session.loader.as_ref());
    warm_highlights(&from_edits, &session.text, session.loader.as_ref());
  }
});
