//! Hardcoded syntax theme using terminal colors.

use crossterm::style::Color;
use the_lib::syntax::Highlight;

/// Standard highlight scopes (indices into this array = Highlight values).
pub const SCOPES: &[&str] = &[
  "attribute",
  "type",
  "type.builtin",
  "constructor",
  "constant",
  "constant.builtin",
  "constant.character",
  "constant.character.escape",
  "string",
  "string.regexp",
  "string.special",
  "comment",
  "variable",
  "variable.builtin",
  "variable.parameter",
  "variable.other.member",
  "label",
  "punctuation",
  "punctuation.delimiter",
  "punctuation.bracket",
  "keyword",
  "keyword.control",
  "keyword.control.conditional",
  "keyword.control.repeat",
  "keyword.control.import",
  "keyword.control.return",
  "keyword.control.exception",
  "keyword.operator",
  "keyword.directive",
  "keyword.function",
  "keyword.storage",
  "keyword.storage.type",
  "keyword.storage.modifier",
  "operator",
  "function",
  "function.builtin",
  "function.method",
  "function.macro",
  "function.special",
  "tag",
  "namespace",
  "special",
  "markup.heading",
  "markup.list",
  "markup.bold",
  "markup.italic",
  "markup.link",
  "markup.quote",
  "markup.raw",
  "diff.plus",
  "diff.minus",
  "diff.delta",
];

/// Map a Highlight index to a terminal color.
pub fn highlight_to_color(highlight: Highlight) -> Color {
  let Some(scope) = SCOPES.get(highlight.idx()).copied() else {
    return Color::Reset;
  };

  // Keywords - Blue
  if scope.starts_with("keyword") {
    return Color::Blue;
  }
  // Types - Yellow
  if scope.starts_with("type") {
    return Color::Yellow;
  }
  // Functions - Cyan
  if scope.starts_with("function") {
    return Color::Cyan;
  }
  // Strings - Green
  if scope.starts_with("string") {
    return Color::Green;
  }
  // Comments - Dark Grey
  if scope.starts_with("comment") {
    return Color::DarkGrey;
  }
  // Constants - Magenta
  if scope.starts_with("constant") {
    return Color::Magenta;
  }
  // Variables - Reset (default terminal color)
  if scope.starts_with("variable") {
    return Color::Reset;
  }
  // Punctuation - Dark Grey
  if scope.starts_with("punctuation") {
    return Color::DarkGrey;
  }
  // Attributes - Yellow
  if scope.starts_with("attribute") {
    return Color::Yellow;
  }
  // Markup
  if scope.starts_with("markup.heading") {
    return Color::Blue;
  }
  if scope.starts_with("markup.bold") || scope.starts_with("markup.italic") {
    return Color::White;
  }
  if scope.starts_with("markup.link") {
    return Color::Cyan;
  }
  if scope.starts_with("markup.quote") {
    return Color::DarkGrey;
  }
  if scope.starts_with("markup.raw") {
    return Color::Green;
  }
  if scope.starts_with("markup") {
    return Color::Reset;
  }

  // Exact matches
  match scope {
    "operator" => Color::White,
    "namespace" => Color::Cyan,
    "tag" => Color::Blue,
    "constructor" => Color::Yellow,
    "label" => Color::Cyan,
    "special" => Color::Magenta,
    "diff.plus" => Color::Green,
    "diff.minus" => Color::Red,
    "diff.delta" => Color::Yellow,
    _ => Color::Reset,
  }
}
