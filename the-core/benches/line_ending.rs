//! Benchmarks for line ending operations in the-core.
//!
//! Run with: `cargo bench -p the-core --bench line_ending`

use divan::{
  Bencher,
  black_box,
};
use ropey::Rope;
use the_core::line_ending::{
  LineEnding,
  auto_detect_line_ending,
  can_break_between,
  char_can_break_after,
  get_line_ending,
  get_line_ending_of_str,
  line_end_char_index,
  rope_end_without_line_ending,
};

fn main() {
  divan::main();
}

// ─────────────────────────────────────────────────────────────────────────────
// LineEnding::from_char benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod from_char {
  use super::*;

  #[divan::bench]
  fn lf(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(LineEnding::from_char(black_box('\n')), Some(LineEnding::LF));
    });
  }

  #[divan::bench]
  fn cr(bencher: Bencher) {
    bencher.bench(|| {
      // CR alone is only a line ending with unicode-lines feature
      let _ = LineEnding::from_char(black_box('\r'));
    });
  }

  #[divan::bench]
  fn nel(bencher: Bencher) {
    // Next Line character
    bencher.bench(|| {
      let _ = LineEnding::from_char(black_box('\u{0085}'));
    });
  }

  #[divan::bench]
  fn not_line_ending(bencher: Bencher) {
    bencher.bench(|| {
      assert!(LineEnding::from_char(black_box('a')).is_none());
    });
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// LineEnding::from_str benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod from_str {
  use super::*;

  #[divan::bench]
  fn lf(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(LineEnding::from_str(black_box("\n")), Some(LineEnding::LF));
    });
  }

  #[divan::bench]
  fn crlf(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(
        LineEnding::from_str(black_box("\r\n")),
        Some(LineEnding::Crlf)
      );
    });
  }

  #[divan::bench]
  fn not_line_ending_short(bencher: Bencher) {
    bencher.bench(|| {
      assert!(LineEnding::from_str(black_box("a")).is_none());
    });
  }

  #[divan::bench]
  fn not_line_ending_long(bencher: Bencher) {
    bencher.bench(|| {
      assert!(LineEnding::from_str(black_box("hello world")).is_none());
    });
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_line_ending (RopeSlice) benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod get_line_ending_rope {
  use super::*;

  #[divan::bench]
  fn lf(bencher: Bencher) {
    let rope = Rope::from_str("hello world\n");
    let slice = rope.slice(..);
    bencher.bench(|| {
      assert_eq!(get_line_ending(black_box(&slice)), Some(LineEnding::LF));
    });
  }

  #[divan::bench]
  fn crlf(bencher: Bencher) {
    let rope = Rope::from_str("hello world\r\n");
    let slice = rope.slice(..);
    bencher.bench(|| {
      assert_eq!(get_line_ending(black_box(&slice)), Some(LineEnding::Crlf));
    });
  }

  #[divan::bench]
  fn none(bencher: Bencher) {
    let rope = Rope::from_str("hello world");
    let slice = rope.slice(..);
    bencher.bench(|| {
      assert!(get_line_ending(black_box(&slice)).is_none());
    });
  }

  #[divan::bench]
  fn long_line_lf(bencher: Bencher) {
    let text = "a".repeat(10000) + "\n";
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    bencher.bench(|| {
      assert_eq!(get_line_ending(black_box(&slice)), Some(LineEnding::LF));
    });
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_line_ending_of_str benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod get_line_ending_str {
  use super::*;

  #[divan::bench]
  fn lf(bencher: Bencher) {
    let text = "hello world\n";
    bencher.bench(|| {
      assert_eq!(
        get_line_ending_of_str(black_box(text)),
        Some(LineEnding::LF)
      );
    });
  }

  #[divan::bench]
  fn crlf(bencher: Bencher) {
    let text = "hello world\r\n";
    bencher.bench(|| {
      assert_eq!(
        get_line_ending_of_str(black_box(text)),
        Some(LineEnding::Crlf)
      );
    });
  }

  #[divan::bench]
  fn none(bencher: Bencher) {
    let text = "hello world";
    bencher.bench(|| {
      assert!(get_line_ending_of_str(black_box(text)).is_none());
    });
  }

  #[divan::bench]
  fn long_line_lf(bencher: Bencher) {
    let text = "a".repeat(10000) + "\n";
    bencher.bench(|| {
      assert_eq!(
        get_line_ending_of_str(black_box(&text)),
        Some(LineEnding::LF)
      );
    });
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// char_can_break_after benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod break_after {
  use super::*;

  #[divan::bench]
  fn space(bencher: Bencher) {
    bencher.bench(|| assert!(char_can_break_after(black_box(' '))));
  }

  #[divan::bench]
  fn hyphen(bencher: Bencher) {
    bencher.bench(|| assert!(char_can_break_after(black_box('-'))));
  }

  #[divan::bench]
  fn newline(bencher: Bencher) {
    bencher.bench(|| assert!(char_can_break_after(black_box('\n'))));
  }

  #[divan::bench]
  fn letter_ascii(bencher: Bencher) {
    bencher.bench(|| assert!(!char_can_break_after(black_box('a'))));
  }

  #[divan::bench]
  fn cjk(bencher: Bencher) {
    bencher.bench(|| assert!(char_can_break_after(black_box('漢'))));
  }

  #[divan::bench]
  fn nbsp(bencher: Bencher) {
    // Non-breaking space should NOT allow break
    bencher.bench(|| assert!(!char_can_break_after(black_box('\u{00A0}'))));
  }

  #[divan::bench]
  fn zwsp(bencher: Bencher) {
    // Zero-width space SHOULD allow break
    bencher.bench(|| assert!(char_can_break_after(black_box('\u{200B}'))));
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// can_break_between benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod break_between {
  use super::*;

  #[divan::bench]
  fn space_letter(bencher: Bencher) {
    bencher.bench(|| assert!(can_break_between(black_box(' '), black_box('a'))));
  }

  #[divan::bench]
  fn letter_letter(bencher: Bencher) {
    bencher.bench(|| assert!(!can_break_between(black_box('a'), black_box('b'))));
  }

  #[divan::bench]
  fn cjk_cjk(bencher: Bencher) {
    bencher.bench(|| assert!(can_break_between(black_box('漢'), black_box('字'))));
  }

  #[divan::bench]
  fn hyphen_letter(bencher: Bencher) {
    bencher.bench(|| assert!(can_break_between(black_box('-'), black_box('a'))));
  }

  #[divan::bench]
  fn letter_close_paren(bencher: Bencher) {
    // Should not break before close paren
    bencher.bench(|| assert!(!can_break_between(black_box('a'), black_box(')'))));
  }

  #[divan::bench]
  fn open_paren_letter(bencher: Bencher) {
    // Should not break after open paren
    bencher.bench(|| assert!(!can_break_between(black_box('('), black_box('a'))));
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// auto_detect_line_ending benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod auto_detect {
  use super::*;

  #[divan::bench]
  fn first_line_lf(bencher: Bencher) {
    // Line ending on first line - best case
    let rope = Rope::from_str("hello\nworld\ntest\n");
    bencher.bench(|| {
      assert_eq!(
        auto_detect_line_ending(black_box(&rope)),
        Some(LineEnding::LF)
      );
    });
  }

  #[divan::bench]
  fn first_line_crlf(bencher: Bencher) {
    let rope = Rope::from_str("hello\r\nworld\r\ntest\r\n");
    bencher.bench(|| {
      assert_eq!(
        auto_detect_line_ending(black_box(&rope)),
        Some(LineEnding::Crlf)
      );
    });
  }

  #[divan::bench]
  fn line_50_lf(bencher: Bencher) {
    // Line ending at line 50 - mid case
    let mut text = String::new();
    for i in 0..50 {
      text.push_str(&format!("line {} without ending", i));
      // Use form feed which is ignored by auto_detect
      text.push('\u{000C}');
    }
    text.push_str("line with LF\n");
    let rope = Rope::from_str(&text);
    bencher.bench(|| {
      assert_eq!(
        auto_detect_line_ending(black_box(&rope)),
        Some(LineEnding::LF)
      );
    });
  }

  #[divan::bench]
  fn no_line_ending_small(bencher: Bencher) {
    // No line ending - must check all lines (but document is small)
    let rope = Rope::from_str("hello world no line ending here");
    bencher.bench(|| {
      assert!(auto_detect_line_ending(black_box(&rope)).is_none());
    });
  }

  #[divan::bench]
  fn no_line_ending_100_lines(bencher: Bencher) {
    // Worst case: 100 lines checked, no valid line ending found
    // Using form feeds which are ignored by auto_detect
    let text = (0..100)
      .map(|i| format!("line {} content\u{000C}", i))
      .collect::<String>();
    let rope = Rope::from_str(&text);
    bencher.bench(|| {
      assert!(auto_detect_line_ending(black_box(&rope)).is_none());
    });
  }

  #[divan::bench]
  fn large_doc_first_line_lf(bencher: Bencher) {
    // Large document but line ending on first line
    let mut text = "first line\n".to_string();
    for i in 0..10000 {
      text.push_str(&format!("line {}\n", i));
    }
    let rope = Rope::from_str(&text);
    bencher.bench(|| {
      assert_eq!(
        auto_detect_line_ending(black_box(&rope)),
        Some(LineEnding::LF)
      );
    });
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// line_end_char_index benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod line_end_index {
  use super::*;

  #[divan::bench]
  fn first_line_lf(bencher: Bencher) {
    let rope = Rope::from_str("hello\nworld\ntest\n");
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(line_end_char_index(black_box(&slice), black_box(0)));
    });
  }

  #[divan::bench]
  fn middle_line_lf(bencher: Bencher) {
    let rope = Rope::from_str("hello\nworld\ntest\n");
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(line_end_char_index(black_box(&slice), black_box(1)));
    });
  }

  #[divan::bench]
  fn last_line_no_ending(bencher: Bencher) {
    let rope = Rope::from_str("hello\nworld\ntest");
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(line_end_char_index(black_box(&slice), black_box(2)));
    });
  }

  #[divan::bench]
  fn large_doc_middle_line(bencher: Bencher) {
    let text = (0..1000)
      .map(|i| format!("line {}\n", i))
      .collect::<String>();
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(line_end_char_index(black_box(&slice), black_box(500)));
    });
  }

  #[divan::bench]
  fn crlf_line(bencher: Bencher) {
    let rope = Rope::from_str("hello\r\nworld\r\ntest\r\n");
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(line_end_char_index(black_box(&slice), black_box(0)));
    });
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// rope_end_without_line_ending benchmarks
// ─────────────────────────────────────────────────────────────────────────────

mod rope_end {
  use super::*;

  #[divan::bench]
  fn with_lf(bencher: Bencher) {
    let rope = Rope::from_str("hello world\n");
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(rope_end_without_line_ending(black_box(&slice)));
    });
  }

  #[divan::bench]
  fn with_crlf(bencher: Bencher) {
    let rope = Rope::from_str("hello world\r\n");
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(rope_end_without_line_ending(black_box(&slice)));
    });
  }

  #[divan::bench]
  fn without_line_ending(bencher: Bencher) {
    let rope = Rope::from_str("hello world");
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(rope_end_without_line_ending(black_box(&slice)));
    });
  }

  #[divan::bench]
  fn large_with_lf(bencher: Bencher) {
    let text = "a".repeat(100000) + "\n";
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);
    bencher.bench(|| {
      black_box(rope_end_without_line_ending(black_box(&slice)));
    });
  }
}
