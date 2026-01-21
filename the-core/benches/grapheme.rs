//! Benchmarks for grapheme-related operations in the-core.
//!
//! Run with: `cargo bench -p the-core`

use divan::{
  Bencher,
  black_box,
};
use ropey::Rope;
use the_core::grapheme::{
  ensure_grapheme_boundary_next,
  ensure_grapheme_boundary_prev,
  grapheme_width,
  next_grapheme_boundary,
  nth_next_grapheme_boundary,
  nth_prev_grapheme_boundary,
  prev_grapheme_boundary,
};

fn main() {
  divan::main();
}

// Test data generators.

fn make_ascii_text(size: usize) -> String {
  let line = "The quick brown fox jumps over the lazy dog. ";
  let mut s = String::with_capacity(size);
  while s.len() < size {
    s.push_str(line);
  }
  s.truncate(size);
  s
}

fn make_cjk_text(size: usize) -> String {
  // Each CJK char is 3 bytes in UTF-8
  let line = "漢字文字測試中文日本語韓國語";
  let mut s = String::with_capacity(size);
  while s.len() < size {
    s.push_str(line);
  }
  // Truncate at char boundary
  while s.len() > size {
    s.pop();
  }
  s
}

fn make_emoji_text(size: usize) -> String {
  // Emoji are typically 4 bytes each
  let line = "😀🎉🚀💻🔥✨🌟💡🎯🏆";
  let mut s = String::with_capacity(size);
  while s.len() < size {
    s.push_str(line);
  }
  while s.len() > size {
    s.pop();
  }
  s
}

fn make_combining_text(size: usize) -> String {
  // "a\u{0301}" = á (a + combining acute), 3 bytes per grapheme
  let grapheme = "a\u{0301}";
  let mut s = String::with_capacity(size);
  while s.len() < size {
    s.push_str(grapheme);
  }
  while s.len() > size {
    s.pop();
  }
  s
}

fn make_mixed_text(size: usize) -> String {
  let line = "Hello 世界! 🎉 Test テスト 😀 ";
  let mut s = String::with_capacity(size);
  while s.len() < size {
    s.push_str(line);
  }
  while s.len() > size {
    s.pop();
  }
  s
}

// `grapheme_width` benchmarks.

mod width {
  use super::*;

  #[divan::bench]
  fn ascii_single(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("a")));
  }

  #[divan::bench]
  fn ascii_control_nul(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("\u{0000}")));
  }

  #[divan::bench]
  fn ascii_control_esc(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("\u{001B}")));
  }

  #[divan::bench]
  fn ascii_control_del(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("\u{007F}")));
  }

  #[divan::bench]
  fn cjk_single(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("漢")));
  }

  #[divan::bench]
  fn emoji_simple(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("😀")));
  }

  #[divan::bench]
  fn emoji_flag(bencher: Bencher) {
    // Flag emoji (regional indicators)
    bencher.bench(|| grapheme_width(black_box("🇺🇸")));
  }

  #[divan::bench]
  fn emoji_zwj_sequence(bencher: Bencher) {
    // Family emoji with ZWJ
    bencher.bench(|| grapheme_width(black_box("👨‍👩‍👧‍👦")));
  }

  #[divan::bench]
  fn combining_acute(bencher: Bencher) {
    // a + combining acute accent
    bencher.bench(|| grapheme_width(black_box("a\u{0301}")));
  }

  #[divan::bench]
  fn combining_multiple(bencher: Bencher) {
    // Letter with multiple combining marks
    bencher.bench(|| grapheme_width(black_box("o\u{0302}\u{0323}"))); // ô with dot below
  }

  #[divan::bench]
  fn zero_width_space(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("\u{200B}")));
  }

  #[divan::bench]
  fn bom(bencher: Bencher) {
    bencher.bench(|| grapheme_width(black_box("\u{FEFF}")));
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Grapheme boundary benchmarks - small documents (~100 bytes)
// ─────────────────────────────────────────────────────────────────────────────

mod boundary_small {
  use super::*;

  const SIZE: usize = 100;

  #[divan::bench]
  fn next_ascii(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_cjk(bencher: Bencher) {
    let text = make_cjk_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_emoji(bencher: Bencher) {
    let text = make_emoji_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_combining(bencher: Bencher) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_mixed(bencher: Bencher) {
    let text = make_mixed_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn prev_ascii(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn prev_combining(bencher: Bencher) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(mid)));
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Grapheme boundary benchmarks - medium documents (~10KB)
// ─────────────────────────────────────────────────────────────────────────────

mod boundary_medium {
  use super::*;

  const SIZE: usize = 10 * 1024; // 10KB

  #[divan::bench]
  fn next_ascii(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_cjk(bencher: Bencher) {
    let text = make_cjk_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_mixed(bencher: Bencher) {
    let text = make_mixed_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn prev_ascii(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn prev_combining(bencher: Bencher) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(mid)));
  }
}

// Grapheme boundary benchmarks - large documents (~1MB).

mod boundary_large {
  use super::*;

  const SIZE: usize = 1024 * 1024; // 1MB

  #[divan::bench]
  fn next_ascii(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_cjk(bencher: Bencher) {
    let text = make_cjk_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn next_mixed(bencher: Bencher) {
    let text = make_mixed_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn prev_ascii(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn prev_combining(bencher: Bencher) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(mid)));
  }
}

//` nth_*_grapheme_boundary` benchmarks - varying `n`.

mod nth_boundary {
  use super::*;

  const SIZE: usize = 10 * 1024; // 10KB

  #[divan::bench(args = [1, 10, 100, 1000])]
  fn nth_next_ascii(bencher: Bencher, n: usize) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);

    bencher.bench(|| nth_next_grapheme_boundary(black_box(slice), black_box(0), black_box(n)));
  }

  #[divan::bench(args = [1, 10, 100, 1000])]
  fn nth_next_combining(bencher: Bencher, n: usize) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);

    bencher.bench(|| nth_next_grapheme_boundary(black_box(slice), black_box(0), black_box(n)));
  }

  #[divan::bench(args = [1, 10, 100, 1000])]
  fn nth_prev_ascii(bencher: Bencher, n: usize) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let end = slice.len_chars();

    bencher.bench(|| nth_prev_grapheme_boundary(black_box(slice), black_box(end), black_box(n)));
  }

  #[divan::bench(args = [1, 10, 100, 1000])]
  fn nth_prev_combining(bencher: Bencher, n: usize) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let end = slice.len_chars();

    bencher.bench(|| nth_prev_grapheme_boundary(black_box(slice), black_box(end), black_box(n)));
  }
}

// `ensure_grapheme_boundary` benchmarks.

mod ensure_boundary {
  use super::*;

  const SIZE: usize = 10 * 1024;

  #[divan::bench]
  fn ensure_next_ascii_at_boundary(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| ensure_grapheme_boundary_next(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn ensure_next_combining_mid_cluster(bencher: Bencher) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    // Position 1 is inside the first grapheme cluster (between 'a' and combining
    // accent).
    let pos = 1;

    bencher.bench(|| ensure_grapheme_boundary_next(black_box(slice), black_box(pos)));
  }

  #[divan::bench]
  fn ensure_prev_ascii_at_boundary(bencher: Bencher) {
    let text = make_ascii_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let mid = slice.len_chars() / 2;

    bencher.bench(|| ensure_grapheme_boundary_prev(black_box(slice), black_box(mid)));
  }

  #[divan::bench]
  fn ensure_prev_combining_mid_cluster(bencher: Bencher) {
    let text = make_combining_text(SIZE);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let pos = 1;

    bencher.bench(|| ensure_grapheme_boundary_prev(black_box(slice), black_box(pos)));
  }
}

// Edge case benchmarks.

mod edge_cases {
  use super::*;

  #[divan::bench]
  fn next_at_start(bencher: Bencher) {
    let text = make_ascii_text(10 * 1024);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(0)));
  }

  #[divan::bench]
  fn next_at_end(bencher: Bencher) {
    let text = make_ascii_text(10 * 1024);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let end = slice.len_chars();

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(end)));
  }

  #[divan::bench]
  fn prev_at_start(bencher: Bencher) {
    let text = make_ascii_text(10 * 1024);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(0)));
  }

  #[divan::bench]
  fn prev_at_end(bencher: Bencher) {
    let text = make_ascii_text(10 * 1024);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    let end = slice.len_chars();

    bencher.bench(|| prev_grapheme_boundary(black_box(slice), black_box(end)));
  }

  #[divan::bench]
  fn crlf_boundary(bencher: Bencher) {
    // CRLF should be treated as single grapheme
    let text = "line1\r\nline2\r\nline3\r\n".repeat(500);
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    // Position just before a CRLF
    let pos = 5;

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(pos)));
  }

  #[divan::bench]
  fn empty_document(bencher: Bencher) {
    let rope = Rope::from_str("");
    let slice = rope.slice(..);

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(0)));
  }

  #[divan::bench]
  fn single_char_document(bencher: Bencher) {
    let rope = Rope::from_str("x");
    let slice = rope.slice(..);

    bencher.bench(|| next_grapheme_boundary(black_box(slice), black_box(0)));
  }
}
