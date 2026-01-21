//! Benchmarks for character classification operations in the-core.
//!
//! Run with: `cargo bench -p the-core --bench chars`

use divan::{
  Bencher,
  black_box,
};
use the_core::chars::{
  CharCategory,
  WhitespaceProperties,
  categorize_char,
  char_is_line_ending,
  char_is_punctuation,
  char_is_word,
};

fn main() {
  divan::main();
}

// `categorize_char` benchmarks.

mod categorize {
  use super::*;

  #[divan::bench]
  fn whitespace(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box(' ')), CharCategory::Whitespace);
    });
  }

  #[divan::bench]
  fn eol_lf(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('\n')), CharCategory::Eol);
    });
  }

  #[divan::bench]
  fn eol_cr(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('\r')), CharCategory::Eol);
    });
  }

  #[divan::bench]
  fn word_ascii(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('a')), CharCategory::Word);
    });
  }

  #[divan::bench]
  fn word_digit(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('5')), CharCategory::Word);
    });
  }

  #[divan::bench]
  fn word_cjk(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('漢')), CharCategory::Word);
    });
  }

  #[divan::bench]
  fn punctuation_period(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('.')), CharCategory::Punctuation);
    });
  }

  #[divan::bench]
  fn punctuation_math(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('+')), CharCategory::Punctuation);
    });
  }

  #[divan::bench]
  fn unknown_emoji(bencher: Bencher) {
    bencher.bench(|| {
      assert_eq!(categorize_char(black_box('😀')), CharCategory::Unknown);
    });
  }
}

// `WhitespaceProperties::of` benchmarks.

mod whitespace_props {
  use super::*;

  #[divan::bench]
  fn space(bencher: Bencher) {
    bencher.bench(|| {
      black_box(WhitespaceProperties::of(black_box(' ')).unwrap());
    });
  }

  #[divan::bench]
  fn tab(bencher: Bencher) {
    bencher.bench(|| {
      black_box(WhitespaceProperties::of(black_box('\t')).unwrap());
    });
  }

  #[divan::bench]
  fn nbsp(bencher: Bencher) {
    // Non-breaking space
    bencher.bench(|| {
      black_box(WhitespaceProperties::of(black_box('\u{00A0}')).unwrap());
    });
  }

  #[divan::bench]
  fn zwsp(bencher: Bencher) {
    // Zero-width space
    bencher.bench(|| {
      black_box(WhitespaceProperties::of(black_box('\u{200B}')).unwrap());
    });
  }

  #[divan::bench]
  fn ideographic_space(bencher: Bencher) {
    // CJK full-width space
    bencher.bench(|| {
      black_box(WhitespaceProperties::of(black_box('\u{3000}')).unwrap());
    });
  }

  #[divan::bench]
  fn not_whitespace_ascii(bencher: Bencher) {
    bencher.bench(|| {
      assert!(WhitespaceProperties::of(black_box('a')).is_none());
    });
  }

  #[divan::bench]
  fn not_whitespace_newline(bencher: Bencher) {
    // Newline is NOT whitespace in this classification (it's EOL)
    bencher.bench(|| {
      assert!(WhitespaceProperties::of(black_box('\n')).is_none());
    });
  }
}

// `char_is_word` benchmarks.

mod is_word {
  use super::*;

  #[divan::bench]
  fn ascii_lower(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_word(black_box('a'))));
  }

  #[divan::bench]
  fn ascii_upper(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_word(black_box('Z'))));
  }

  #[divan::bench]
  fn digit(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_word(black_box('5'))));
  }

  #[divan::bench]
  fn underscore(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_word(black_box('_'))));
  }

  #[divan::bench]
  fn cjk(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_word(black_box('漢'))));
  }

  #[divan::bench]
  fn cyrillic(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_word(black_box('д'))));
  }

  #[divan::bench]
  fn not_word_space(bencher: Bencher) {
    bencher.bench(|| assert!(!char_is_word(black_box(' '))));
  }

  #[divan::bench]
  fn not_word_punctuation(bencher: Bencher) {
    bencher.bench(|| assert!(!char_is_word(black_box('.'))));
  }
}

// `char_is_punctuation` benchmarks.

mod is_punctuation {
  use super::*;

  #[divan::bench]
  fn period(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_punctuation(black_box('.'))));
  }

  #[divan::bench]
  fn comma(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_punctuation(black_box(','))));
  }

  #[divan::bench]
  fn open_paren(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_punctuation(black_box('('))));
  }

  #[divan::bench]
  fn math_plus(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_punctuation(black_box('+'))));
  }

  #[divan::bench]
  fn currency_dollar(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_punctuation(black_box('$'))));
  }

  #[divan::bench]
  fn cjk_punctuation(bencher: Bencher) {
    // CJK comma
    bencher.bench(|| assert!(char_is_punctuation(black_box('、'))));
  }

  #[divan::bench]
  fn not_punctuation_letter(bencher: Bencher) {
    bencher.bench(|| assert!(!char_is_punctuation(black_box('a'))));
  }

  #[divan::bench]
  fn not_punctuation_digit(bencher: Bencher) {
    bencher.bench(|| assert!(!char_is_punctuation(black_box('5'))));
  }
}

// `char_is_line_ending` benchmarks.

mod is_line_ending {
  use super::*;

  #[divan::bench]
  fn lf(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_line_ending(black_box('\n'))));
  }

  #[divan::bench]
  fn cr(bencher: Bencher) {
    bencher.bench(|| assert!(char_is_line_ending(black_box('\r'))));
  }

  #[divan::bench]
  fn nel(bencher: Bencher) {
    // Next Line (NEL)
    bencher.bench(|| assert!(char_is_line_ending(black_box('\u{0085}'))));
  }

  #[divan::bench]
  fn line_separator(bencher: Bencher) {
    // Unicode Line Separator
    bencher.bench(|| assert!(char_is_line_ending(black_box('\u{2028}'))));
  }

  #[divan::bench]
  fn not_line_ending_space(bencher: Bencher) {
    bencher.bench(|| assert!(!char_is_line_ending(black_box(' '))));
  }

  #[divan::bench]
  fn not_line_ending_letter(bencher: Bencher) {
    bencher.bench(|| assert!(!char_is_line_ending(black_box('a'))));
  }
}
