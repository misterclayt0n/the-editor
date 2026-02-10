use std::{
  sync::{
    Arc,
    OnceLock,
  },
  time::Duration,
};

use ropey::Rope;
use the_lib::{
  syntax::{
    Loader,
    Syntax,
    config::Configuration,
    runtime_loader::RuntimeLoader,
  },
  transaction::{
    ChangeSet,
    Transaction,
  },
};
use the_loader::config::user_lang_config;

const MAX_INITIAL_BYTES: usize = 8 * 1024;
const MAX_OPS: usize = 128;
const MAX_INSERT_BYTES: usize = 256;

#[derive(Debug, Clone)]
pub struct EditOp {
  pub anchor: u16,
  pub delete: u16,
  pub insert: Vec<u8>,
}

struct Scenario {
  seed:          u64,
  language_hint: u8,
  initial:       Vec<u8>,
  ops:           Vec<EditOp>,
}

pub struct FuzzSession {
  pub seed:   u64,
  pub loader: Arc<Loader>,
  pub text:   Rope,
  pub syntax: Syntax,
  pub ops:    Vec<EditOp>,
}

pub fn session_from_bytes(data: &[u8]) -> Option<FuzzSession> {
  let mut scenario = decode_scenario(data);
  if scenario.ops.is_empty() {
    scenario.ops.push(EditOp {
      anchor: 0,
      delete: 0,
      insert: vec![b'a'],
    });
  }

  let loader = fuzz_loader()?;
  let language = {
    const CANDIDATES: &[&str] = &["rust", "toml", "markdown", "nix", "json", "yaml"];
    let mut selected = None;
    for offset in 0..CANDIDATES.len() {
      let candidate = CANDIDATES[(scenario.language_hint as usize + offset) % CANDIDATES.len()];
      if let Some(language) = loader.language_for_name(candidate) {
        selected = Some(language);
        break;
      }
    }
    if selected.is_none() {
      selected = loader.languages().next().map(|(language, _)| language);
    }
    selected?
  };
  let initial = lossy_text(&scenario.initial);
  let text = Rope::from_str(&initial);
  let syntax = Syntax::new(text.slice(..), language, loader.as_ref()).ok()?;

  Some(FuzzSession {
    seed: scenario.seed,
    loader,
    text,
    syntax,
    ops: scenario.ops,
  })
}

pub fn apply_edit(text: &mut Rope, op: &EditOp) -> Option<(Rope, ChangeSet)> {
  let old_text = text.clone();
  let len_chars = old_text.len_chars();
  let from = if len_chars == 0 {
    0
  } else {
    (op.anchor as usize) % (len_chars + 1)
  };
  let max_delete = len_chars.saturating_sub(from);
  let delete = if max_delete == 0 {
    0
  } else {
    (op.delete as usize) % (max_delete + 1)
  };
  let to = from + delete;
  let replacement = lossy_text(&op.insert);
  let replacement = if replacement.is_empty() {
    None
  } else {
    Some(replacement.into())
  };

  let transaction = Transaction::change(text, std::iter::once((from, to, replacement))).ok()?;
  let changes = transaction.changes().clone();
  transaction.apply(text).ok()?;
  Some((old_text, changes))
}

pub fn warm_highlights(syntax: &Syntax, text: &Rope, loader: &Loader) {
  let len_bytes = text.len_bytes().min(16 * 1024);
  let _ = syntax.collect_highlights(text.slice(..), loader, 0..len_bytes);
  let root = syntax.tree().root_node().byte_range();
  let _ = syntax.named_descendant_for_byte_range(root.start, root.end);
}

pub fn short_timeout(seed: u64, index: usize) -> Duration {
  let jitter = ((seed.wrapping_add(index as u64)) % 7) + 1;
  Duration::from_millis(jitter)
}

fn lossy_text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

fn fuzz_loader() -> Option<Arc<Loader>> {
  static LOADER: OnceLock<Option<Arc<Loader>>> = OnceLock::new();
  LOADER
    .get_or_init(|| {
      let config_value = user_lang_config().ok()?;
      let config: Configuration = config_value.try_into().ok()?;
      let loader = Loader::new(config, RuntimeLoader::new()).ok()?;
      Some(Arc::new(loader))
    })
    .clone()
}

fn decode_scenario(data: &[u8]) -> Scenario {
  let mut cursor = ByteCursor::new(data);
  let seed = cursor.next_u64();
  let language_hint = cursor.next_u8();
  let initial_len = cursor.next_usize(MAX_INITIAL_BYTES);
  let initial = cursor.next_bytes(initial_len).to_vec();
  let op_count = cursor.next_usize(MAX_OPS);
  let mut ops = Vec::with_capacity(op_count);
  for _ in 0..op_count {
    let anchor = cursor.next_u16();
    let delete = cursor.next_u16();
    let insert_len = cursor.next_usize(MAX_INSERT_BYTES);
    let insert = cursor.next_bytes(insert_len).to_vec();
    ops.push(EditOp {
      anchor,
      delete,
      insert,
    });
  }

  Scenario {
    seed,
    language_hint,
    initial,
    ops,
  }
}

struct ByteCursor<'a> {
  data: &'a [u8],
  pos:  usize,
}

impl<'a> ByteCursor<'a> {
  fn new(data: &'a [u8]) -> Self {
    Self { data, pos: 0 }
  }

  fn next_u8(&mut self) -> u8 {
    let value = self.data.get(self.pos).copied().unwrap_or(0);
    self.pos = self.pos.saturating_add(1);
    value
  }

  fn next_u16(&mut self) -> u16 {
    let lo = self.next_u8() as u16;
    let hi = self.next_u8() as u16;
    lo | (hi << 8)
  }

  fn next_u64(&mut self) -> u64 {
    let mut value = 0u64;
    for shift in (0..64).step_by(8) {
      value |= (self.next_u8() as u64) << shift;
    }
    value
  }

  fn next_usize(&mut self, max: usize) -> usize {
    if max == 0 {
      return 0;
    }
    (self.next_u16() as usize) % (max + 1)
  }

  fn next_bytes(&mut self, len: usize) -> &'a [u8] {
    let start = self.pos.min(self.data.len());
    let end = start.saturating_add(len).min(self.data.len());
    self.pos = end;
    &self.data[start..end]
  }
}
