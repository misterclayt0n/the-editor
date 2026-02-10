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

  for (index, op) in mem::take(&mut session.ops).into_iter().enumerate() {
    let Some((old_text, changes)) = apply_edit(&mut session.text, &op) else {
      continue;
    };
    let edits = generate_edits(old_text.slice(..), &changes);

    let _ =
      session
        .syntax
        .update_with_edits(session.text.slice(..), &edits, session.loader.as_ref());
    let timeout = short_timeout(session.seed, index);
    let _ = session.syntax.try_update_with_short_timeout(
      session.text.slice(..),
      &edits,
      session.loader.as_ref(),
      timeout,
    );

    warm_highlights(&session.syntax, &session.text, session.loader.as_ref());
  }
});
