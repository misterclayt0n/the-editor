//! Benchmarks for transaction operations in the-lib.
//!
//! Run with: `cargo bench -p the-lib --bench transaction`

use divan::{
  Bencher,
  black_box,
};
use ropey::Rope;
use smallvec::SmallVec;
use the_lib::{
  Tendril,
  selection::{
    Range,
    Selection,
  },
  transaction::{
    Assoc,
    Change,
    Transaction,
  },
};

fn main() {
  divan::main();
}

fn make_ascii_text(size: usize) -> String {
  let line = "The quick brown fox jumps over the lazy dog. ";
  let mut s = String::with_capacity(size);
  while s.len() < size {
    s.push_str(line);
  }
  s.truncate(size);
  s
}

fn make_rope(size: usize) -> Rope {
  let text = make_ascii_text(size);
  let rope = Rope::from_str(&text);
  drop(text);
  rope
}

fn clamp_count(len: usize, count: usize, span: usize) -> usize {
  let max = if span == 0 { len } else { len / (span + 1) };
  count.min(max.max(1))
}

fn make_changes(len: usize, count: usize, span: usize, insert: &str) -> Vec<Change> {
  let count = clamp_count(len, count, span);
  let step = len / (count + 1);
  let mut changes = Vec::with_capacity(count);
  let insert = Tendril::from(insert);

  for i in 0..count {
    let start = (i + 1) * step;
    let end = (start + span).min(len);
    changes.push((start, end, Some(insert.clone())));
  }

  changes
}

fn make_point_selection(doc: &Rope, count: usize) -> Selection {
  let len = doc.len_chars();
  let count = clamp_count(len, count, 0);
  let step = len / (count + 1);
  let mut ranges = SmallVec::with_capacity(count);

  for i in 0..count {
    let pos = ((i + 1) * step).min(len);
    ranges.push(Range::point(pos));
  }

  Selection::new(ranges).unwrap()
}

fn make_range_selection(doc: &Rope, count: usize, span: usize) -> Selection {
  let len = doc.len_chars();
  let count = clamp_count(len, count, span);
  let step = len / (count + 1);
  let mut ranges = SmallVec::with_capacity(count);

  for i in 0..count {
    let start = (i + 1) * step;
    let end = (start + span).min(len);
    ranges.push(Range::new(start, end));
  }

  Selection::new(ranges).unwrap()
}

// `Transaction::change benchmarks`.

mod change {
  use super::*;

  const SIZE: usize = 100 * 1024;
  const SPAN: usize = 3;

  #[divan::bench(args = [1, 8, 64])]
  fn replace_ranges(bencher: Bencher, count: usize) {
    let doc = make_rope(SIZE);
    let changes = make_changes(doc.len_chars(), count, SPAN, "xyz");

    bencher.bench(|| {
      let transaction =
        Transaction::change(black_box(&doc), black_box(changes.iter().cloned())).unwrap();
      black_box(transaction);
    });
  }
}

// `Transaction::change_by_selection benchmarks`.

mod change_by_selection {
  use super::*;

  const SIZE: usize = 100 * 1024;
  const SPAN: usize = 3;

  #[divan::bench(args = [1, 8, 64])]
  fn replace_selection(bencher: Bencher, count: usize) {
    let doc = make_rope(SIZE);
    let selection = make_range_selection(&doc, count, SPAN);
    let insert = Tendril::from("x");

    bencher.bench(|| {
      let transaction =
        Transaction::change_by_selection(black_box(&doc), black_box(&selection), |range| {
          (range.from(), range.to(), Some(insert.clone()))
        })
        .unwrap();
      black_box(transaction);
    });
  }
}

// `Transaction::insert` benchmarks.

mod insert {
  use super::*;

  const SIZE: usize = 100 * 1024;

  #[divan::bench(args = [1, 8, 64])]
  fn multi_cursor(bencher: Bencher, count: usize) {
    let doc = make_rope(SIZE);
    let selection = make_point_selection(&doc, count);
    let text = Tendril::from("x");

    bencher.bench(|| {
      let transaction = Transaction::insert(
        black_box(&doc),
        black_box(&selection),
        black_box(text.clone()),
      )
      .unwrap();
      black_box(transaction);
    });
  }
}

// `Transaction::apply` benchmarks.

mod apply {
  use super::*;

  const SPAN: usize = 3;

  #[divan::bench]
  fn small(bencher: Bencher) {
    let doc = make_rope(4 * 1024);
    let changes = make_changes(doc.len_chars(), 8, SPAN, "x");
    let transaction = Transaction::change(&doc, changes).unwrap();

    bencher.bench(|| {
      let mut next = doc.clone();
      transaction.apply(&mut next).unwrap();
      black_box(next);
    });
  }

  #[divan::bench]
  fn medium(bencher: Bencher) {
    let doc = make_rope(100 * 1024);
    let changes = make_changes(doc.len_chars(), 32, SPAN, "x");
    let transaction = Transaction::change(&doc, changes).unwrap();

    bencher.bench(|| {
      let mut next = doc.clone();
      transaction.apply(&mut next).unwrap();
      black_box(next);
    });
  }

  #[divan::bench]
  fn large(bencher: Bencher) {
    let doc = make_rope(1024 * 1024);
    let changes = make_changes(doc.len_chars(), 64, SPAN, "x");
    let transaction = Transaction::change(&doc, changes).unwrap();

    bencher.bench(|| {
      let mut next = doc.clone();
      transaction.apply(&mut next).unwrap();
      black_box(next);
    });
  }
}

// `ChangeSet::map_pos` benchmarks.

mod map_pos {
  use super::*;

  const SIZE: usize = 100 * 1024;
  const SPAN: usize = 3;

  #[divan::bench]
  fn before(bencher: Bencher) {
    let doc = make_rope(SIZE);
    let changes = make_changes(doc.len_chars(), 32, SPAN, "x");
    let transaction = Transaction::change(&doc, changes).unwrap();
    let pos = doc.len_chars() / 2;

    bencher.bench(|| {
      let mapped = transaction
        .changes()
        .map_pos(black_box(pos), Assoc::Before)
        .unwrap();
      black_box(mapped);
    });
  }

  #[divan::bench]
  fn after(bencher: Bencher) {
    let doc = make_rope(SIZE);
    let changes = make_changes(doc.len_chars(), 32, SPAN, "x");
    let transaction = Transaction::change(&doc, changes).unwrap();
    let pos = doc.len_chars() / 2;

    bencher.bench(|| {
      let mapped = transaction
        .changes()
        .map_pos(black_box(pos), Assoc::After)
        .unwrap();
      black_box(mapped);
    });
  }
}
