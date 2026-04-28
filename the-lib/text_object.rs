use std::fmt::Display;

use ropey::RopeSlice;

use crate::{
  chars::{
    CharCategory,
    WhitespaceProperties,
    categorize_char,
  },
  grapheme::next_grapheme_boundary,
  line_ending::rope_is_line_ending,
  movement::Direction,
  selection::{
    Range,
    Selection,
  },
  surround,
  syntax::{
    LanguageRegistry,
    Syntax,
    SyntaxError,
  },
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TextObjectKind {
  Around,
  Inside,
}

impl Display for TextObjectKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      Self::Around => "around",
      Self::Inside => "inside",
    })
  }
}

pub fn word(
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  count: usize,
  long: bool,
) -> Selection {
  selection.transform(|range| word_range(text, range, kind, count, long))
}

pub fn paragraph(
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  count: usize,
) -> Selection {
  selection.transform(|range| paragraph_range(text, range, kind, count))
}

pub fn pair(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  ch: char,
  count: usize,
) -> Selection {
  selection.transform(|range| pair_range(syntax, text, range, kind, Some(ch), count))
}

pub fn closest_pair(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  count: usize,
) -> Selection {
  selection.transform(|range| pair_range(syntax, text, range, kind, None, count))
}

pub fn syntax_named(
  syntax: &Syntax,
  registry: &LanguageRegistry,
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  object_name: &str,
) -> std::result::Result<Selection, SyntaxError> {
  let Some(query) = registry.textobject_query(syntax.root_language())? else {
    return Ok(selection);
  };

  let root = syntax.tree().root_node();
  let capture_name = format!("{object_name}.{kind}");
  Ok(selection.transform(|range| {
    syntax_named_range(text, range, &capture_name, query, &root).unwrap_or(range)
  }))
}

pub fn move_syntax_named(
  syntax: &Syntax,
  registry: &LanguageRegistry,
  text: RopeSlice,
  selection: Selection,
  object_name: &str,
  direction: Direction,
  count: usize,
) -> std::result::Result<Selection, SyntaxError> {
  let Some(query) = registry.textobject_query(syntax.root_language())? else {
    return Ok(selection);
  };

  let root = syntax.tree().root_node();
  let around_capture = format!("{object_name}.{}", TextObjectKind::Around);
  let inside_capture = format!("{object_name}.{}", TextObjectKind::Inside);

  Ok(selection.transform(|range| {
    let mut current = range;
    for _ in 0..count.max(1) {
      let Some(next) = next_syntax_named_range(
        text,
        current,
        direction,
        query,
        &root,
        &around_capture,
        &inside_capture,
      ) else {
        break;
      };
      if next == current {
        break;
      }
      current = next;
    }
    current
  }))
}

fn word_range(
  text: RopeSlice,
  range: Range,
  kind: TextObjectKind,
  count: usize,
  long: bool,
) -> Range {
  let count = count.max(1);
  let pos = range.cursor(text);

  let word_start = find_word_boundary(text, pos, Direction::Backward, long);
  let Some(mut word_end) = current_word_end(text, pos, long) else {
    return Range::point(pos).with_direction(range.direction());
  };

  for _ in 1..count {
    let Some(next_start) = next_word_start(text, word_end) else {
      break;
    };
    let Some(next_end) = current_word_end(text, next_start, long) else {
      break;
    };
    word_end = next_end;
  }

  match kind {
    TextObjectKind::Inside => Range::new(word_start, word_end).with_direction(range.direction()),
    TextObjectKind::Around => around_word_range(text, range, word_start, word_end),
  }
}

fn current_word_end(text: RopeSlice, pos: usize, long: bool) -> Option<usize> {
  match text.get_char(pos).map(categorize_char) {
    None | Some(CharCategory::Whitespace | CharCategory::Eol) => None,
    _ => Some(find_word_boundary(text, pos + 1, Direction::Forward, long)),
  }
}

fn next_word_start(text: RopeSlice, mut pos: usize) -> Option<usize> {
  while let Some(ch) = text.get_char(pos) {
    match categorize_char(ch) {
      CharCategory::Whitespace | CharCategory::Eol => pos += 1,
      _ => return Some(pos),
    }
  }
  None
}

fn around_word_range(text: RopeSlice, range: Range, word_start: usize, word_end: usize) -> Range {
  let whitespace_count_right = text
    .chars_at(word_end)
    .take_while(|c| is_whitespace(*c))
    .count();

  let result = if whitespace_count_right > 0 {
    Range::new(word_start, word_end + whitespace_count_right)
  } else {
    let whitespace_count_left = {
      let mut iter = text.chars_at(word_start);
      iter.reverse();
      iter.take_while(|c| is_whitespace(*c)).count()
    };
    Range::new(word_start - whitespace_count_left, word_end)
  };

  result.with_direction(range.direction())
}

fn find_word_boundary(slice: RopeSlice, mut pos: usize, direction: Direction, long: bool) -> usize {
  use CharCategory::{
    Eol,
    Whitespace,
  };

  let iter = match direction {
    Direction::Forward => slice.chars_at(pos),
    Direction::Backward => {
      let mut iter = slice.chars_at(pos);
      iter.reverse();
      iter
    },
  };

  let mut prev_category = match direction {
    Direction::Forward if pos == 0 => Whitespace,
    Direction::Forward => categorize_char(slice.char(pos - 1)),
    Direction::Backward if pos == slice.len_chars() => Whitespace,
    Direction::Backward => categorize_char(slice.char(pos)),
  };

  for ch in iter {
    match categorize_char(ch) {
      Eol | Whitespace => return pos,
      category => {
        if !long && category != prev_category && pos != 0 && pos != slice.len_chars() {
          return pos;
        }

        match direction {
          Direction::Forward => pos += 1,
          Direction::Backward => pos = pos.saturating_sub(1),
        }
        prev_category = category;
      },
    }
  }

  pos
}

fn paragraph_range(text: RopeSlice, range: Range, kind: TextObjectKind, count: usize) -> Range {
  let cursor = range.cursor(text);
  let mut line = range.cursor_line(text);

  if is_empty_line(text, line) {
    if let Some(next) = next_non_empty_line(text, line) {
      line = next;
    } else if let Some(prev) = prev_non_empty_line(text, line.saturating_sub(1)) {
      line = prev;
    } else {
      return Range::point(cursor).with_direction(range.direction());
    }
  }

  let start_line = paragraph_start(text, line);
  let (end_inside, end_around) = paragraph_end_lines(text, start_line, count.max(1));
  let end_line = match kind {
    TextObjectKind::Inside => end_inside,
    TextObjectKind::Around => end_around,
  };

  Range::new(text.line_to_char(start_line), text.line_to_char(end_line))
    .with_direction(range.direction())
}

#[inline]
fn is_whitespace(ch: char) -> bool {
  WhitespaceProperties::of(ch).is_some()
}

#[inline]
fn is_empty_line(text: RopeSlice, line: usize) -> bool {
  if line >= text.len_lines() {
    return true;
  }
  rope_is_line_ending(text.line(line))
}

fn next_non_empty_line(text: RopeSlice, mut line: usize) -> Option<usize> {
  while line < text.len_lines() && is_empty_line(text, line) {
    line += 1;
  }
  (line < text.len_lines()).then_some(line)
}

fn prev_non_empty_line(text: RopeSlice, mut line: usize) -> Option<usize> {
  while line > 0 && is_empty_line(text, line) {
    line -= 1;
  }
  (!is_empty_line(text, line)).then_some(line)
}

fn paragraph_start(text: RopeSlice, mut line: usize) -> usize {
  while line > 0 && !is_empty_line(text, line - 1) {
    line -= 1;
  }
  line
}

fn paragraph_end_lines(text: RopeSlice, start_line: usize, count: usize) -> (usize, usize) {
  let mut line = start_line;
  let mut end_inside = start_line;
  let mut end_around = start_line;
  let mut remaining = count;

  while remaining > 0 {
    while line < text.len_lines() && !is_empty_line(text, line) {
      line += 1;
    }
    end_inside = line;

    while line < text.len_lines() && is_empty_line(text, line) {
      line += 1;
    }
    end_around = line;

    remaining -= 1;
    if line >= text.len_lines() {
      break;
    }
  }

  (end_inside, end_around)
}

fn pair_range(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  range: Range,
  kind: TextObjectKind,
  ch: Option<char>,
  count: usize,
) -> Range {
  let pair_pos = match ch {
    Some(ch) => surround::find_nth_pairs_pos(text, ch, range, count.max(1)),
    None => surround::find_nth_closest_pairs_pos(syntax, text, range, count.max(1)),
  };

  pair_pos
    .map(|(open, close)| {
      match kind {
        TextObjectKind::Inside => Range::new(next_grapheme_boundary(text, open), close),
        TextObjectKind::Around => Range::new(open, next_grapheme_boundary(text, close)),
      }
      .with_direction(range.direction())
    })
    .unwrap_or(range)
}

fn syntax_named_range(
  text: RopeSlice,
  range: Range,
  capture_name: &str,
  query: &crate::syntax::TextObjectQuery,
  root: &crate::syntax::tree_sitter::Node<'_>,
) -> Option<Range> {
  let byte_pos = text.char_to_byte(range.cursor(text));
  let node = query
    .capture_nodes(capture_name, root, text)?
    .filter(|node| node.byte_range().contains(&byte_pos))
    .min_by_key(|node| node.byte_range().len())?;

  let len = text.len_bytes();
  let start_byte = node.start_byte();
  let end_byte = node.end_byte();
  if start_byte > len || end_byte > len {
    return None;
  }

  Some(
    Range::new(text.byte_to_char(start_byte), text.byte_to_char(end_byte))
      .with_direction(range.direction()),
  )
}

fn next_syntax_named_range(
  text: RopeSlice,
  range: Range,
  direction: Direction,
  query: &crate::syntax::TextObjectQuery,
  root: &crate::syntax::tree_sitter::Node<'_>,
  around_capture: &str,
  inside_capture: &str,
) -> Option<Range> {
  let byte_pos = text.char_to_byte(range.cursor(text));
  let nodes = query.capture_nodes_any([around_capture, inside_capture], root, text)?;

  let node = match direction {
    Direction::Forward => {
      nodes
        .filter(|node| node.start_byte() > byte_pos)
        .min_by_key(|node| (node.start_byte(), std::cmp::Reverse(node.end_byte())))?
    },
    Direction::Backward => {
      nodes
        .filter(|node| node.end_byte() < byte_pos)
        .max_by_key(|node| (node.end_byte(), std::cmp::Reverse(node.start_byte())))?
    },
  };

  let len = text.len_bytes();
  let start_byte = node.start_byte();
  let end_byte = node.end_byte();
  if start_byte > len || end_byte > len {
    return None;
  }

  Some(
    Range::new(text.byte_to_char(start_byte), text.byte_to_char(end_byte))
      .with_direction(range.direction()),
  )
}

#[cfg(test)]
mod test {
  use std::{
    borrow::Cow,
    num::NonZeroU64,
  };

  use ropey::Rope;
  use smallvec::smallvec;
  use tree_house::tree_sitter::Grammar;

  use super::*;
  use crate::{
    selection::{
      CursorId,
      CursorRange,
    },
    syntax::{
      FileType,
      LanguageRegistry,
      LanguageSyntaxConfig,
      QueryKind,
      SyntaxConfiguration,
      SyntaxResources,
    },
  };

  #[derive(Debug, Clone, Copy)]
  struct JsonResources {
    grammar: Grammar,
    query:   Option<&'static str>,
  }

  impl SyntaxResources for JsonResources {
    fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
      (grammar_name == "json").then_some(self.grammar)
    }

    fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
      if language_id == "json" && kind == QueryKind::TextObjects {
        return self.query.map(Cow::Borrowed);
      }
      None
    }
  }

  fn cursor(id: u64, anchor: usize, head: usize) -> CursorRange {
    CursorRange {
      id:    CursorId::new(NonZeroU64::new(id).unwrap()),
      range: Range::new(anchor, head),
    }
  }

  fn json_syntax(text: &str, query: Option<&'static str>) -> (Rope, Syntax, LanguageRegistry) {
    let grammar = Grammar::try_from(tree_sitter_json::LANGUAGE).unwrap();
    let registry = LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![LanguageSyntaxConfig {
          language_id:          "json".into(),
          scope:                "source.json".into(),
          file_types:           vec![FileType::Extension("json".into())],
          shebangs:             Vec::new(),
          comment_tokens:       None,
          block_comment_tokens: None,
          text_width:           None,
          soft_wrap:            None,
          grammar:              Some("json".into()),
          injection_regex:      None,
          indent:               None,
          auto_pairs:           None,
          rulers:               None,
          rainbow_brackets:     None,
        }],
      },
      JsonResources { grammar, query },
    )
    .unwrap();

    let rope = Rope::from(text);
    let syntax = Syntax::new(rope.slice(..), crate::syntax::LanguageId::new(0), &registry).unwrap();
    (rope, syntax, registry)
  }

  #[test]
  fn word_objects_select_inside_or_around_and_preserve_cursor_ids() {
    let text = Rope::from("hello world again");
    let selection = Selection::from_cursors(smallvec![cursor(7, 1, 1), cursor(8, 7, 7)]).unwrap();

    let inside = word(
      text.slice(..),
      selection.clone(),
      TextObjectKind::Inside,
      1,
      false,
    );
    assert_eq!(
      inside.range_by_id(cursor(7, 1, 1).id),
      Some(&Range::new(0, 5))
    );
    assert_eq!(
      inside.range_by_id(cursor(8, 7, 7).id),
      Some(&Range::new(6, 11))
    );

    let around = word(text.slice(..), selection, TextObjectKind::Around, 1, false);
    assert_eq!(
      around.range_by_id(cursor(7, 1, 1).id),
      Some(&Range::new(0, 6))
    );
    assert_eq!(
      around.range_by_id(cursor(8, 7, 7).id),
      Some(&Range::new(6, 12))
    );
  }

  #[test]
  fn paragraph_objects_handle_blank_lines_and_count() {
    let text = Rope::from("one\n\ntwo\n\nthree");
    let selection = Selection::point(1);

    let inside = paragraph(text.slice(..), selection.clone(), TextObjectKind::Inside, 2);
    assert_eq!(inside.ranges(), &[Range::new(0, 9)]);

    let around = paragraph(
      text.slice(..),
      Selection::point(5),
      TextObjectKind::Around,
      1,
    );
    assert_eq!(around.ranges(), &[Range::new(5, 10)]);
  }

  #[test]
  fn pair_objects_select_inside_or_around_and_no_op_when_not_found() {
    let text = Rope::from("(alpha) beta");
    let selection = Selection::point(2);

    let inside = pair(
      None,
      text.slice(..),
      selection.clone(),
      TextObjectKind::Inside,
      '(',
      1,
    );
    assert_eq!(inside.ranges(), &[Range::new(1, 6)]);

    let around = pair(
      None,
      text.slice(..),
      selection.clone(),
      TextObjectKind::Around,
      '(',
      1,
    );
    assert_eq!(around.ranges(), &[Range::new(0, 7)]);

    let no_pair = pair(
      None,
      text.slice(..),
      Selection::point(9),
      TextObjectKind::Around,
      '[',
      1,
    );
    assert_eq!(no_pair.ranges(), &[Range::new(9, 9)]);
  }

  #[test]
  fn closest_pair_uses_surround_detection() {
    let text = Rope::from("(outer [inner])");
    let selection = Selection::point(8);

    let inside = closest_pair(
      None,
      text.slice(..),
      selection.clone(),
      TextObjectKind::Inside,
      1,
    );
    assert_eq!(inside.ranges(), &[Range::new(8, 13)]);

    let next_outer = closest_pair(None, text.slice(..), selection, TextObjectKind::Around, 2);
    assert_eq!(next_outer.ranges(), &[Range::new(0, 15)]);
  }

  #[test]
  fn syntax_named_uses_the_smallest_containing_capture_and_preserves_ids() {
    let (text, syntax, registry) = json_syntax("[1,[2,3]]", Some("(array) @array.around"));
    let selection = Selection::from_cursors(smallvec![cursor(5, 5, 5)]).unwrap();

    let next = syntax_named(
      &syntax,
      &registry,
      text.slice(..),
      selection,
      TextObjectKind::Around,
      "array",
    )
    .unwrap();

    assert_eq!(
      next.range_by_id(cursor(5, 5, 5).id),
      Some(&Range::new(3, 8))
    );
  }

  #[test]
  fn move_syntax_named_finds_the_next_matching_capture() {
    let (text, syntax, registry) = json_syntax("[1,[2,3],[4,5]]", Some("(array) @array.around"));
    let selection = Selection::from_cursors(smallvec![cursor(7, 1, 1)]).unwrap();

    let next = move_syntax_named(
      &syntax,
      &registry,
      text.slice(..),
      selection,
      "array",
      Direction::Forward,
      1,
    )
    .unwrap();

    assert_eq!(
      next.range_by_id(cursor(7, 1, 1).id),
      Some(&Range::new(3, 8))
    );
  }

  #[test]
  fn syntax_named_is_a_no_op_when_no_matching_capture_exists() {
    let (text, syntax, registry) = json_syntax("[1,[2,3]]", None);
    let selection = Selection::point(5);

    let next = syntax_named(
      &syntax,
      &registry,
      text.slice(..),
      selection.clone(),
      TextObjectKind::Around,
      "array",
    )
    .unwrap();

    assert_eq!(next, selection);
  }
}
