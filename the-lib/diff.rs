use std::{
  ops::Range,
  time::Instant,
};

use imara_diff::{
  Algorithm,
  Diff,
  Hunk,
  IndentHeuristic,
  IndentLevel,
  InternedInput,
};
use ropey::{
  Rope,
  RopeSlice,
};

use crate::{
  Tendril,
  transaction::{
    ChangeSet,
    Transaction,
  },
};

#[derive(Debug, Clone)]
pub struct DiffOptions {
  pub indent_width:              u8,
  pub max_char_diff_ratio:       u32,
  pub max_char_diff_total_lines: u32,
  pub min_large_delete_lines:    u32,
  pub large_hunk_lines_for_copy: u32,
}

impl Default for DiffOptions {
  fn default() -> Self {
    Self {
      indent_width:              4,
      max_char_diff_ratio:       5,
      max_char_diff_total_lines: 200,
      min_large_delete_lines:    10,
      large_hunk_lines_for_copy: 500,
    }
  }
}

struct ChangeSetBuilder<'a> {
  res:          ChangeSet,
  after:        RopeSlice<'a>,
  file:         &'a InternedInput<RopeSlice<'a>>,
  options:      &'a DiffOptions,
  current_hunk: InternedInput<char>,
  char_diff:    Diff,
  pos:          u32,
}

impl ChangeSetBuilder<'_> {
  fn before_len_chars(&self, range: Range<u32>) -> usize {
    self.file.before[range.start as usize..range.end as usize]
      .iter()
      .map(|&it| self.file.interner[it].len_chars())
      .sum()
  }

  fn build_fragment(&self, range: Range<u32>) -> Tendril {
    let start = range.start as usize;
    let end = range.end as usize;
    let len_lines = end.saturating_sub(start) as u32;

    if len_lines >= self.options.large_hunk_lines_for_copy {
      let start_char = self.after.line_to_char(start);
      let end_char = self.after.line_to_char(end);
      return self.after.slice(start_char..end_char).to_string().into();
    }

    let mut fragment = Tendril::new();
    for &line in &self.file.after[start..end] {
      for chunk in self.file.interner[line].chunks() {
        fragment.push_str(chunk);
      }
    }
    fragment
  }

  fn should_char_diff(&self, len_before: u32, len_after: u32) -> bool {
    if len_before == 0 || len_after == 0 {
      return false;
    }

    let total = len_before as u64 + len_after as u64;
    if total > self.options.max_char_diff_total_lines as u64 {
      return false;
    }

    let ratio = self.options.max_char_diff_ratio as u64;
    let len_before_u64 = len_before as u64;
    let len_after_u64 = len_after as u64;

    if len_after_u64 > ratio.saturating_mul(len_before_u64) {
      return false;
    }

    if len_before_u64 > ratio.saturating_mul(len_after_u64)
      && len_before > self.options.min_large_delete_lines
    {
      return false;
    }

    true
  }

  fn process_hunk(&mut self, before: Range<u32>, after: Range<u32>) {
    let len = self.before_len_chars(self.pos..before.start);
    self.res.retain(len);
    self.pos = before.end;

    // do not perform diffs on large hunks
    let len_before = before.end - before.start;
    let len_after = after.end - after.start;

    // Pure insertions/removals do not require a character diff.
    // Very large changes are ignored because their character diff is expensive to
    // compute TODO adjust heuristic to detect large changes?
    if !self.should_char_diff(len_before, len_after) {
      let remove = self.before_len_chars(before);
      self.res.delete(remove);
      self.res.insert(self.build_fragment(after));
    } else {
      // for reasonably small hunks, generating a ChangeSet from char diff can save
      // memory TODO use a tokenizer (word diff?) for improved performance
      let hunk_before = self.file.before[before.start as usize..before.end as usize]
        .iter()
        .flat_map(|&it| self.file.interner[it].chars());
      let hunk_after = self.file.after[after.start as usize..after.end as usize]
        .iter()
        .flat_map(|&it| self.file.interner[it].chars());
      self.current_hunk.update_before(hunk_before);
      self.current_hunk.update_after(hunk_after);
      // the histogram heuristic does not work as well
      // for characters because the same characters often reoccur
      // use myer diff instead
      self.char_diff.compute_with(
        Algorithm::Myers,
        &self.current_hunk.before,
        &self.current_hunk.after,
        self.current_hunk.interner.num_tokens(),
      );
      let mut pos = 0;
      for Hunk { before, after } in self.char_diff.hunks() {
        self.res.retain((before.start - pos) as usize);
        self.res.delete(before.len());
        pos = before.end;

        let res = self.current_hunk.after[after.start as usize..after.end as usize]
          .iter()
          .map(|&token| self.current_hunk.interner[token])
          .collect();

        self.res.insert(res);
      }
      self
        .res
        .retain(self.current_hunk.before.len() - pos as usize);
      // reuse allocations
      self.current_hunk.clear();
    }
  }

  fn finish(mut self) -> ChangeSet {
    let end = u32::try_from(self.file.before.len()).unwrap_or(u32::MAX);
    let len = self.before_len_chars(self.pos..end);

    self.res.retain(len);
    self.res
  }
}

struct RopeLines<'a>(RopeSlice<'a>);

impl<'a> imara_diff::TokenSource for RopeLines<'a> {
  type Token = RopeSlice<'a>;
  type Tokenizer = ropey::iter::Lines<'a>;

  fn tokenize(&self) -> Self::Tokenizer {
    self.0.lines()
  }

  fn estimate_tokens(&self) -> u32 {
    // we can provide a perfect estimate which is very nice for performance
    u32::try_from(self.0.len_lines()).unwrap_or(u32::MAX)
  }
}

/// Compares `old` and `new` to generate a [`Transaction`] describing
/// the steps required to get from `old` to `new`.
pub fn compare_ropes(before: &Rope, after: &Rope) -> Transaction {
  compare_ropes_with_options(before, after, &DiffOptions::default())
}

pub fn compare_ropes_with_options(
  before: &Rope,
  after: &Rope,
  options: &DiffOptions,
) -> Transaction {
  let start = tracing::enabled!(tracing::Level::DEBUG).then(Instant::now);
  let after = after.slice(..);
  let file = InternedInput::new(RopeLines(before.slice(..)), RopeLines(after));
  let mut diff = Diff::compute(Algorithm::Histogram, &file);
  diff.postprocess_with_heuristic(
    &file,
    IndentHeuristic::new(|token| {
      IndentLevel::for_ascii_line(file.interner[token].bytes(), options.indent_width)
    }),
  );
  let hunk_count = diff.hunks().count();
  let mut builder = ChangeSetBuilder {
    res: ChangeSet::with_capacity(hunk_count * 3),
    file: &file,
    after,
    options,
    pos: 0,
    current_hunk: InternedInput::default(),
    char_diff: Diff::default(),
  };
  for hunk in diff.hunks() {
    builder.process_hunk(hunk.before, hunk.after)
  }
  let res = builder.finish().into();

  if let Some(start) = start {
    tracing::debug!(
      "rope diff took {}s",
      Instant::now().duration_since(start).as_secs_f64()
    );
  }
  res
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_identity(a: &str, b: &str) {
    let mut old = Rope::from(a);
    let new = Rope::from(b);
    compare_ropes(&old, &new).apply(&mut old).unwrap();
    assert_eq!(old, new);
  }

  quickcheck::quickcheck! {
      fn test_compare_ropes(a: String, b: String) -> bool {
          let mut old = Rope::from(a);
          let new = Rope::from(b);
          compare_ropes(&old, &new).apply(&mut old).unwrap();
          old == new
      }
  }

  #[test]
  fn equal_files() {
    test_identity("foo", "foo");
  }

  #[test]
  fn trailing_newline() {
    test_identity("foo\n", "foo");
    test_identity("foo", "foo\n");
  }

  #[test]
  fn new_file() {
    test_identity("", "foo");
  }

  #[test]
  fn deleted_file() {
    test_identity("foo", "");
  }
}
