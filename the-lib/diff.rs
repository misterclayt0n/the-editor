use std::{
  ops::Range,
  sync::Arc,
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
  pub max_char_diff_total_chars: usize,
  pub min_large_delete_lines:    u32,
  pub min_large_delete_chars:    usize,
  pub large_hunk_lines_for_copy: u32,
  pub word_diff_min_chars:       usize,
}

impl Default for DiffOptions {
  fn default() -> Self {
    const DEFAULT_CHARS_PER_LINE: usize = 200;
    Self {
      indent_width:              4,
      max_char_diff_ratio:       5,
      max_char_diff_total_lines: 200,
      max_char_diff_total_chars: 200 * DEFAULT_CHARS_PER_LINE,
      min_large_delete_lines:    10,
      min_large_delete_chars:    10 * DEFAULT_CHARS_PER_LINE,
      large_hunk_lines_for_copy: 500,
      word_diff_min_chars:       1024,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct WordToken {
  text:      Arc<str>,
  len_chars: usize,
}

impl WordToken {
  fn new(text: String, len_chars: usize) -> Self {
    Self {
      text: Arc::from(text),
      len_chars,
    }
  }
}

impl Default for WordToken {
  fn default() -> Self {
    Self {
      text:      Arc::from(""),
      len_chars: 0,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenClass {
  Whitespace,
  Word,
  Other,
}

fn token_class(ch: char) -> TokenClass {
  if ch.is_whitespace() {
    TokenClass::Whitespace
  } else if ch.is_alphanumeric() || ch == '_' {
    TokenClass::Word
  } else {
    TokenClass::Other
  }
}

struct TokenizedSeq {
  tokens:       Vec<WordToken>,
  prefix_chars: Vec<usize>,
}

impl TokenizedSeq {
  fn new(tokens: Vec<WordToken>) -> Self {
    let mut prefix_chars = Vec::with_capacity(tokens.len() + 1);
    prefix_chars.push(0);
    for token in &tokens {
      let next = prefix_chars.last().copied().unwrap_or(0) + token.len_chars;
      prefix_chars.push(next);
    }
    Self {
      tokens,
      prefix_chars,
    }
  }

  fn char_len(&self, range: Range<u32>) -> usize {
    let start = range.start as usize;
    let end = range.end as usize;
    debug_assert!(end < self.prefix_chars.len());
    self.prefix_chars[end] - self.prefix_chars[start]
  }
}

fn tokenize_words<I: Iterator<Item = char>>(iter: I) -> TokenizedSeq {
  let mut tokens = Vec::new();
  let mut buf = String::new();
  let mut buf_len = 0usize;
  let mut class = None;

  for ch in iter {
    let next_class = token_class(ch);
    if class == Some(next_class) {
      buf.push(ch);
      buf_len += 1;
      continue;
    }

    if buf_len > 0 {
      tokens.push(WordToken::new(std::mem::take(&mut buf), buf_len));
    }
    buf.push(ch);
    buf_len = 1;
    class = Some(next_class);
  }

  if buf_len > 0 {
    tokens.push(WordToken::new(std::mem::take(&mut buf), buf_len));
  }

  TokenizedSeq::new(tokens)
}

struct ChangeSetBuilder<'a> {
  res:        ChangeSet,
  after:      RopeSlice<'a>,
  file:       &'a InternedInput<RopeSlice<'a>>,
  options:    &'a DiffOptions,
  char_hunk:  InternedInput<char>,
  word_hunk:  InternedInput<WordToken>,
  token_diff: Diff,
  pos:        u32,
}

impl ChangeSetBuilder<'_> {
  fn before_len_chars(&self, range: Range<u32>) -> usize {
    self.file.before[range.start as usize..range.end as usize]
      .iter()
      .map(|&it| self.file.interner[it].len_chars())
      .sum()
  }

  fn after_len_chars(&self, range: Range<u32>) -> usize {
    self.file.after[range.start as usize..range.end as usize]
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

  fn should_char_diff(
    &self,
    len_before_lines: u32,
    len_after_lines: u32,
    len_before_chars: usize,
    len_after_chars: usize,
  ) -> bool {
    if len_before_lines == 0 || len_after_lines == 0 {
      return false;
    }

    let total_lines = len_before_lines as u64 + len_after_lines as u64;
    if total_lines > self.options.max_char_diff_total_lines as u64 {
      return false;
    }

    let total_chars = len_before_chars.saturating_add(len_after_chars) as u64;
    if total_chars > self.options.max_char_diff_total_chars as u64 {
      return false;
    }

    let ratio = self.options.max_char_diff_ratio as u64;
    let len_before_u64 = len_before_chars as u64;
    let len_after_u64 = len_after_chars as u64;

    if len_after_u64 > ratio.saturating_mul(len_before_u64) {
      return false;
    }

    if len_before_u64 > ratio.saturating_mul(len_after_u64)
      && (len_before_lines > self.options.min_large_delete_lines
        || len_before_chars > self.options.min_large_delete_chars)
    {
      return false;
    }

    true
  }

  fn should_word_diff(&self, len_before_chars: usize, len_after_chars: usize) -> bool {
    len_before_chars.saturating_add(len_after_chars) >= self.options.word_diff_min_chars
  }

  fn process_char_diff(&mut self, before: Range<u32>, after: Range<u32>) {
    let hunk_before = self.file.before[before.start as usize..before.end as usize]
      .iter()
      .flat_map(|&it| self.file.interner[it].chars());
    let hunk_after = self.file.after[after.start as usize..after.end as usize]
      .iter()
      .flat_map(|&it| self.file.interner[it].chars());
    self.char_hunk.update_before(hunk_before);
    self.char_hunk.update_after(hunk_after);
    // the histogram heuristic does not work as well
    // for characters because the same characters often reoccur
    // use myer diff instead
    self.token_diff.compute_with(
      Algorithm::Myers,
      &self.char_hunk.before,
      &self.char_hunk.after,
      self.char_hunk.interner.num_tokens(),
    );
    let mut pos = 0;
    for Hunk { before, after } in self.token_diff.hunks() {
      self.res.retain((before.start - pos) as usize);
      self.res.delete(before.len());
      pos = before.end;

      let res = self.char_hunk.after[after.start as usize..after.end as usize]
        .iter()
        .map(|&token| self.char_hunk.interner[token])
        .collect();

      self.res.insert(res);
    }
    self.res.retain(self.char_hunk.before.len() - pos as usize);
    self.char_hunk.clear();
  }

  fn process_word_diff(&mut self, before: Range<u32>, after: Range<u32>) {
    let hunk_before = self.file.before[before.start as usize..before.end as usize]
      .iter()
      .flat_map(|&it| self.file.interner[it].chars());
    let hunk_after = self.file.after[after.start as usize..after.end as usize]
      .iter()
      .flat_map(|&it| self.file.interner[it].chars());

    let mut before_tokens = tokenize_words(hunk_before);
    let before_token_list = std::mem::take(&mut before_tokens.tokens);
    let after_tokens = tokenize_words(hunk_after);
    self.word_hunk.update_before(before_token_list.into_iter());
    self.word_hunk.update_after(after_tokens.tokens.into_iter());

    self.token_diff.compute_with(
      Algorithm::Myers,
      &self.word_hunk.before,
      &self.word_hunk.after,
      self.word_hunk.interner.num_tokens(),
    );

    let mut pos = 0;
    for Hunk { before, after } in self.token_diff.hunks() {
      let retain = before_tokens.char_len(pos..before.start);
      let delete = before_tokens.char_len(before.start..before.end);
      self.res.retain(retain);
      self.res.delete(delete);
      pos = before.end;

      let mut res = Tendril::new();
      for &token in &self.word_hunk.after[after.start as usize..after.end as usize] {
        res.push_str(self.word_hunk.interner[token].text.as_ref());
      }
      self.res.insert(res);
    }

    let tail = before_tokens.char_len(pos..self.word_hunk.before.len() as u32);
    self.res.retain(tail);
    self.word_hunk.clear();
  }

  fn process_hunk(&mut self, before: Range<u32>, after: Range<u32>) {
    let len = self.before_len_chars(self.pos..before.start);
    self.res.retain(len);
    self.pos = before.end;

    // do not perform diffs on large hunks
    let len_before_lines = before.end - before.start;
    let len_after_lines = after.end - after.start;
    let len_before_chars = self.before_len_chars(before.clone());
    let len_after_chars = self.after_len_chars(after.clone());

    // Pure insertions/removals do not require a character diff.
    // Very large changes are ignored because their character diff is expensive to
    // compute.
    if !self.should_char_diff(
      len_before_lines,
      len_after_lines,
      len_before_chars,
      len_after_chars,
    ) {
      let remove = len_before_chars;
      self.res.delete(remove);
      self.res.insert(self.build_fragment(after));
    } else if self.should_word_diff(len_before_chars, len_after_chars) {
      self.process_word_diff(before, after);
    } else {
      self.process_char_diff(before, after);
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
    char_hunk: InternedInput::default(),
    word_hunk: InternedInput::default(),
    token_diff: Diff::default(),
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
