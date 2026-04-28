use std::iter;

use ropey::RopeSlice;
use tree_house::tree_sitter::Node;

use crate::{
  movement::Direction::{
    self,
    Backward,
    Forward,
  },
  syntax::Syntax,
};

const MAX_PLAINTEXT_SCAN: usize = 10_000;
const MATCH_LIMIT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchMode {
  Strict,
  Fuzzy,
}

#[must_use]
pub fn find_matching_bracket(syntax: &Syntax, text: RopeSlice, pos: usize) -> Option<usize> {
  if pos >= text.len_chars() || !is_bracket(text.char(pos)) {
    return None;
  }

  find_match_in_syntax(syntax, text, pos, MatchMode::Strict)
    .or_else(|| find_match_in_plaintext_window(text, pos, MatchMode::Strict))
}

#[must_use]
pub fn find_matching_bracket_fuzzy(syntax: &Syntax, text: RopeSlice, pos: usize) -> Option<usize> {
  if pos >= text.len_chars() {
    return None;
  }

  find_match_in_syntax(syntax, text, pos, MatchMode::Fuzzy)
    .or_else(|| find_match_in_plaintext_window(text, pos, MatchMode::Fuzzy))
}

fn find_match_in_syntax(
  syntax: &Syntax,
  text: RopeSlice,
  pos: usize,
  mode: MatchMode,
) -> Option<usize> {
  let byte_pos = text.char_to_byte(pos) as u32;
  let root = syntax.tree_for_byte_range(byte_pos, byte_pos).root_node();
  let mut node = root.descendant_for_byte_range(byte_pos, byte_pos)?;

  loop {
    if let Some(matched) = match_wrapping_node(text, &node, pos, mode) {
      return Some(matched);
    }

    if let Some(matched) = match_bracket_node(text, &node, pos) {
      return Some(matched);
    }

    if !matches!(mode, MatchMode::Fuzzy) && node.is_named() {
      break;
    }

    let Some(parent) = node.parent() else {
      break;
    };
    node = parent;
  }

  None
}

fn match_wrapping_node(
  text: RopeSlice,
  node: &Node<'_>,
  pos: usize,
  mode: MatchMode,
) -> Option<usize> {
  if !node.is_named() || node.child_count() < 2 {
    return None;
  }

  let open = node.child(0)?;
  let close = node.child(node.child_count() - 1)?;
  let (open_pos, open_ch) = single_char(text, &open)?;
  let (close_pos, close_ch) = single_char(text, &close)?;

  if bracket_pair(open_ch) != Some((open_ch, close_ch)) {
    return None;
  }

  if pos == close_pos {
    return Some(open_pos);
  }

  if matches!(mode, MatchMode::Fuzzy) || pos == open_pos {
    return Some(close_pos);
  }

  None
}

fn match_bracket_node(text: RopeSlice, node: &Node<'_>, pos: usize) -> Option<usize> {
  let (node_pos, ch) = single_char(text, node)?;
  if node_pos != pos {
    return None;
  }

  let (open, close) = bracket_pair(ch)?;
  if ch == open {
    return find_sibling_match(text, node.next_sibling(), open, close, Forward);
  }

  find_sibling_match(text, node.prev_sibling(), open, close, Backward)
}

fn find_sibling_match(
  text: RopeSlice,
  node: Option<Node<'_>>,
  open: char,
  close: char,
  direction: Direction,
) -> Option<usize> {
  let advance = match direction {
    Forward => Node::next_sibling,
    Backward => Node::prev_sibling,
  };

  let mut depth = 0usize;
  iter::successors(node, advance)
    .take(MATCH_LIMIT)
    .find_map(|node| {
      let (pos, ch) = single_char(text, &node)?;
      if ch == close {
        if depth == 0 {
          return Some(pos);
        }
        depth -= 1;
      } else if ch == open {
        depth += 1;
      }
      None
    })
}

fn find_match_in_plaintext_window(
  text: RopeSlice,
  pos: usize,
  mode: MatchMode,
) -> Option<usize> {
  let window_start = pos.saturating_sub(MAX_PLAINTEXT_SCAN);
  let window_end = (pos + MAX_PLAINTEXT_SCAN).min(text.len_chars());
  let window = text.slice(window_start..window_end);
  let local_pos = pos - window_start;

  match mode {
    MatchMode::Strict => find_matching_bracket_plaintext(window, local_pos).map(|idx| idx + window_start),
    MatchMode::Fuzzy => find_enclosing_bracket_plaintext(window, local_pos).map(|idx| idx + window_start),
  }
}

fn find_enclosing_bracket_plaintext(text: RopeSlice, pos: usize) -> Option<usize> {
  if pos >= text.len_chars() {
    return None;
  }

  if let Some(matched) = find_matching_bracket_plaintext(text, pos) {
    return Some(matched);
  }

  for idx in (0..=pos).rev() {
    let Some(ch) = text.get_char(idx) else {
      continue;
    };
    if !is_open_bracket(ch) {
      continue;
    }

    let Some(close) = find_matching_bracket_plaintext(text, idx) else {
      continue;
    };

    if close >= pos {
      return Some(close);
    }
  }

  None
}

fn find_matching_bracket_plaintext(text: RopeSlice, pos: usize) -> Option<usize> {
  if pos >= text.len_chars() {
    return None;
  }

  let bracket = text.get_char(pos)?;
  let (open, close) = bracket_pair(bracket)?;
  let is_forward = bracket == open;
  let needle = if is_forward { close } else { open };
  let iter = if is_forward {
    text.chars_at(pos + 1)
  } else {
    text.chars_at(pos).reversed()
  };

  let mut depth = 1usize;
  for (offset, candidate) in iter.take(MAX_PLAINTEXT_SCAN).enumerate() {
    if candidate == bracket {
      depth += 1;
      continue;
    }
    if candidate != needle {
      continue;
    }
    if depth == 1 {
      return Some(if is_forward {
        pos + offset + 1
      } else {
        pos - offset - 1
      });
    }
    depth -= 1;
  }

  None
}

fn single_char(text: RopeSlice, node: &Node<'_>) -> Option<(usize, char)> {
  let start = text.try_byte_to_char(node.start_byte() as usize).ok()?;
  let end = text.try_byte_to_char(node.end_byte() as usize).ok()?;
  if end != start + 1 {
    return None;
  }
  Some((start, text.char(start)))
}

fn bracket_pair(ch: char) -> Option<(char, char)> {
  match ch {
    '(' | ')' => Some(('(', ')')),
    '{' | '}' => Some(('{', '}')),
    '[' | ']' => Some(('[', ']')),
    '<' | '>' => Some(('<', '>')),
    '‘' | '’' => Some(('‘', '’')),
    '“' | '”' => Some(('“', '”')),
    '«' | '»' => Some(('«', '»')),
    '「' | '」' => Some(('「', '」')),
    '（' | '）' => Some(('（', '）')),
    _ => None,
  }
}

fn is_open_bracket(ch: char) -> bool {
  matches!(ch, '(' | '{' | '[' | '<' | '‘' | '“' | '«' | '「' | '（')
}

fn is_bracket(ch: char) -> bool {
  bracket_pair(ch).is_some()
}

#[cfg(test)]
mod tests {
  use std::borrow::Cow;

  use ropey::Rope;
  use tree_house::tree_sitter::Grammar;

  use super::*;
  use crate::syntax::{
    FileType,
    LanguageId,
    LanguageRegistry,
    LanguageSyntaxConfig,
    QueryKind,
    SyntaxConfiguration,
    SyntaxResources,
  };

  #[derive(Debug, Clone, Copy)]
  struct JsonResources {
    grammar: Grammar,
  }

  impl SyntaxResources for JsonResources {
    fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
      (grammar_name == "json").then_some(self.grammar)
    }

    fn query(&self, _language_id: &str, _kind: QueryKind) -> Option<Cow<'_, str>> {
      None
    }
  }

  fn json_syntax(text: &str) -> (Rope, Syntax) {
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
      JsonResources { grammar },
    )
    .unwrap();

    let rope = Rope::from(text);
    let syntax = Syntax::new(rope.slice(..), LanguageId::new(0), &registry).unwrap();
    (rope, syntax)
  }

  #[test]
  fn strict_matching_is_symmetric_for_nested_plaintext_brackets() {
    let (text, syntax) = json_syntax("(a[b]c)");

    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 0), Some(6));
    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 6), Some(0));
    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 2), Some(4));
    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 4), Some(2));
  }

  #[test]
  fn strict_matching_returns_none_for_non_brackets_quotes_and_unmatched_input() {
    let (text, syntax) = json_syntax("\"hi\"");
    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 0), None);
    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 1), None);

    let (text, syntax) = json_syntax("[");
    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 0), None);
    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 1), None);
  }

  #[test]
  fn fuzzy_matching_returns_the_closing_bracket_of_the_nearest_enclosing_scope() {
    let (text, syntax) = json_syntax("[1,[2,3]]");

    assert_eq!(find_matching_bracket_fuzzy(&syntax, text.slice(..), 4), Some(7));
    assert_eq!(find_matching_bracket_fuzzy(&syntax, text.slice(..), 2), Some(8));
  }

  #[test]
  fn fuzzy_matching_behaves_like_strict_when_cursor_is_on_a_bracket() {
    let (text, syntax) = json_syntax("[1,[2,3]]");

    assert_eq!(find_matching_bracket_fuzzy(&syntax, text.slice(..), 0), Some(8));
    assert_eq!(find_matching_bracket_fuzzy(&syntax, text.slice(..), 7), Some(3));
  }

  #[test]
  fn plaintext_fallback_handles_brackets_that_json_syntax_does_not_structure() {
    let (text, syntax) = json_syntax("prefix (middle [inner]) suffix");

    assert_eq!(find_matching_bracket(&syntax, text.slice(..), 7), Some(22));
    assert_eq!(find_matching_bracket_fuzzy(&syntax, text.slice(..), 16), Some(21));
  }
}
