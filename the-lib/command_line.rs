//! Types and parsing code for command mode (`:`) input.
//!
//! This module implements command-line parsing for the editor's command mode,
//! supporting Unix-style flags, quoting, variable expansion, and shell
//! integration.
//!
//! # Parsing Pipeline
//!
//! Command line parsing proceeds in stages:
//!
//! 1. **Tokenization** ([`Tokenizer`]): Splits input into [`Token`]s based on
//!    whitespace, quotes, and expansion syntax. This stage is syntax-aware but
//!    command-agnostic.
//!
//! 2. **Expansion**: Tokens marked as expandable (double-quoted strings,
//!    `%{...}` expressions) are processed by the caller to substitute
//!    variables, evaluate shell commands, etc.
//!
//! 3. **Argument parsing** ([`Args`]): Expanded tokens are classified as
//!    positional arguments or flags according to a command's [`Signature`].
//!    Validation is performed (argument count, unknown flags, etc.).
//!
//! # Quoting Rules
//!
//! | Syntax | Behavior |
//! |--------|----------|
//! | `foo` | Unquoted, split on whitespace |
//! | `'foo bar'` | Single-quoted, literal (no expansion) |
//! | `` `foo bar` `` | Backtick-quoted, literal (no expansion) |
//! | `"foo bar"` | Double-quoted, supports `%{...}` expansion |
//! | `%{expr}` | Variable expansion |
//! | `%sh{cmd}` | Shell expansion |
//! | `%u{XXXX}` | Unicode codepoint (hex) |
//!
//! Quotes are escaped by doubling: `'it''s'` becomes `it's`.
//!
//! On Unix, backslash escapes space in unquoted context: `foo\ bar` is one
//! token.
//!
//! # Flags
//!
//! Commands declare accepted flags in their [`Signature`]. Flags support:
//! - Long form: `--reverse`, `--output file.txt`
//! - Short form: `-r`, `-o file.txt`
//! - Boolean flags (present/absent) or value-accepting flags
//! - `--` to mark end of flags (everything after is positional)
//!
//! # Examples
//!
//! ```ignore
//! use the_lib::command_line::{Args, Signature, Flag};
//!
//! let signature = Signature {
//!     positionals: (1, Some(2)),  // 1-2 positional args
//!     flags: &[Flag {
//!         name: "reverse",
//!         alias: Some('r'),
//!         doc: "Reverse the order",
//!         takes_value: false,
//!         completions: None,
//!     }],
//!     ..Signature::DEFAULT
//! };
//!
//! // Parse without expansion (pass tokens through unchanged)
//! let args = Args::parse("hello --reverse world", signature, true, |t| Ok(t.content))?;
//!
//! assert_eq!(args.len(), 2);           // 2 positionals
//! assert_eq!(&args[0], "hello");
//! assert_eq!(&args[1], "world");
//! assert!(args.has_flag("reverse"));
//! ```
//!
//! # Raw Mode
//!
//! Some commands need custom parsing for part of their input (e.g., JSON values
//! for `:set-option`). The [`Signature::raw_after`] field specifies how many
//! positionals to parse normally before returning the rest as a single raw
//! token.
//!
//! ```ignore
//! let signature = Signature {
//!     positionals: (1, Some(2)),
//!     raw_after: Some(1),  // After first positional, return rest raw
//!     ..Signature::DEFAULT
//! };
//!
//! let args = Args::parse("option-name [1, 2, 3]", signature, true, |t| Ok(t.content))?;
//! assert_eq!(&args[0], "option-name");
//! assert_eq!(&args[1], "[1, 2, 3]");  // Raw, not split
//! ```
//!
//! # Validation
//!
//! When `validate: true`:
//! - Unterminated quotes/expansions are errors
//! - Unknown flags are rejected
//! - Duplicate flags are rejected
//! - Positional count is checked against signature
//!
//! When `validate: false` (e.g., during completion), parsing is lenient and
//! always produces a result.
//!
//! # Error Handling
//!
//! Errors are categorized in [`ParseArgsError`] for argument-level issues and
//! [`ParseError`] which additionally wraps expansion errors from the caller.

use std::{
  borrow::Cow,
  collections::HashMap,
  fmt,
  ops,
  slice,
  vec,
};

use thiserror::Error;

/// Splits a command line into the command and arguments parts.
///
/// The third tuple member describes whether the command part is finished. When
/// this boolean is true the completion code for the command line should
/// complete command names, otherwise command arguments.
pub fn split(line: &str) -> (&str, &str, bool) {
  const SEPARATOR_PATTERN: [char; 2] = [' ', '\t'];

  let (command, rest) = line.split_once(SEPARATOR_PATTERN).unwrap_or((line, ""));

  let complete_command =
    command.is_empty() || (rest.trim().is_empty() && !line.ends_with(SEPARATOR_PATTERN));

  (command, rest, complete_command)
}

/// The value associated with a flag in parsed arguments.
///
/// This distinguishes between boolean flags (which are either present or not)
/// and flags that accept a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagValue<'a> {
  /// A boolean flag that was present (e.g., `--verbose`).
  Bool,
  /// A flag with an associated value (e.g., `--output foo.txt`).
  Value(Cow<'a, str>),
}

/// A Unix-like flag that a command may accept.
///
/// For example the `:sort` command accepts a `--reverse` (or `-r` for
/// shorthand) boolean flag which controls the direction of sorting. Flags may
/// accept an argument by setting `takes_value` to `true`.
#[derive(Debug, Clone, Copy)]
pub struct Flag {
  /// The name of the flag.
  ///
  /// This value is also used to construct the "longhand" version of the flag.
  /// For example a flag with a name "reverse" has a longhand `--reverse`.
  ///
  /// This value should be supplied when reading a flag out of the [Args] with
  /// [Args::get_flag] and [Args::has_flag]. The `:sort` command
  /// implementation for example should ask for `args.has_flag("reverse")`.
  pub name:        &'static str,
  /// The character that can be used as a shorthand for the flag, optionally.
  ///
  /// For example a flag like "reverse" mentioned above might take an alias
  /// `Some('r')` to allow specifying the flag as `-r`.
  pub alias:       Option<char>,
  pub doc:         &'static str,
  /// Whether the flag accepts a value argument.
  ///
  /// When `true`, the next token after the flag is consumed as the flag's
  /// value. When `false`, the flag is treated as a boolean (present or not).
  ///
  /// This is independent of `completions` - a flag can take a value without
  /// having predefined completions, or have completions for documentation
  /// purposes without taking a value.
  pub takes_value: bool,
  /// The completion values to use when specifying an argument for a flag.
  ///
  /// This should be set to `None` for flags without completions and
  /// `Some(&["foo", "bar", "baz"])` for flags with predefined options.
  /// Note: This is independent of `takes_value` - completions are for UI/docs.
  pub completions: Option<&'static [&'static str]>,
}

impl Flag {
  // This allows defining flags with the `..Flag::DEFAULT` shorthand. The `name`
  // and `doc` fields should always be overwritten.
  pub const DEFAULT: Self = Self {
    name:        "",
    doc:         "",
    alias:       None,
    takes_value: false,
    completions: None,
  };
}

/// A description of how a command's input should be handled.
///
/// Each typable command defines a signature (with the help of
/// `Signature::DEFAULT`) at least to declare how many positional arguments it
/// accepts. Command flags are also declared in this struct. The `raw_after`
/// option may be set optionally to avoid evaluating quotes in parts of
/// the command line (useful for shell commands for example).
#[derive(Debug, Clone, Copy)]
#[allow(clippy::manual_non_exhaustive)]
pub struct Signature {
  /// The minimum and (optionally) maximum number of positional arguments a
  /// command may take.
  ///
  /// For example accepting exactly one positional can be specified with `(1,
  /// Some(1))` while accepting zero-or-more positionals can be specified as
  /// `(0, None)`.
  ///
  /// The number of positionals is checked when hitting `<ret>` in command mode.
  /// If the actual number of positionals is outside the declared range then
  /// the command is not executed and an error is shown instead. For example
  /// `:write` accepts zero or one positional arguments (`(0, Some(1))`). A
  /// command line like `:write a.txt b.txt` is outside the declared range and
  /// is not accepted.
  pub positionals: (usize, Option<usize>),
  /// The number of **positional** arguments for the parser to read with normal
  /// quoting rules.
  ///
  /// Once the number has been exceeded then the tokenizer returns the rest of
  /// the input as a `TokenKind::Raw` token (see `Tokenizer::rest`),
  /// meaning that quoting rules do not apply and none of the remaining text
  /// may be treated as a flag.
  ///
  /// If this is set to `None` then the entire command line is parsed with
  /// normal quoting and flag rules.
  ///
  /// A good example use-case for this option is `:toggle-option` which sets
  /// `Some(1)`. Everything up to the first positional argument is interpreted
  /// according to normal rules and the rest of the input is parsed "raw".
  /// This allows `:toggle-option` to perform custom parsing on the rest of
  /// the input - namely parsing complicated values as a JSON stream.
  /// `:toggle-option` could accept a flag in the future. If so, the flag would
  /// need to come before the first positional argument.
  ///
  /// Consider these lines for `:toggle-option` which sets `Some(1)`:
  ///
  /// * `:toggle foo` has one positional "foo" and no flags.
  /// * `:toggle foo bar` has two positionals. Expansions for `bar` are
  ///   evaluated but quotes and anything that looks like a flag are treated
  ///   literally.
  /// * `:toggle foo --bar` has two positionals: `["foo", "--bar"]`. `--bar` is
  ///   not considered to be a flag because it comes after the first positional.
  /// * `:toggle --bar foo` has one positional "foo" and one flag "--bar".
  /// * `:toggle --bar foo --baz` has two positionals `["foo", "--baz"]` and one
  ///   flag "--bar".
  ///
  /// **Note on validation**: When `raw_after` triggers, the remaining input is
  /// returned as a single `TokenKind::Raw` token. Validation errors
  /// (unterminated quotes, unknown expansions) that occur in the raw portion
  /// are NOT checked, even when `validate = true`. This is intentional: the raw
  /// portion is meant for custom parsing by the command implementation.
  /// Validation still applies to tokens *before* the raw cutoff.
  pub raw_after:   Option<u8>,
  /// A set of flags that a command may accept.
  ///
  /// See the `Flag` struct for more info.
  pub flags:       &'static [Flag],
  /// Do not set this field. Use `..Signature::DEFAULT` to construct a
  /// `Signature` instead.
  // This field allows adding new fields later with minimal code changes. This works like a
  // `#[non_exhaustive]` annotation except that it supports the `..Signature::DEFAULT`
  // shorthand.
  pub _dummy: (),
}

impl Signature {
  // This allows defining signatures with the `..Signature::DEFAULT` shorthand.
  // The `positionals` field should always be overwritten.
  pub const DEFAULT: Self = Self {
    positionals: (0, None),
    raw_after:   None,
    flags:       &[],
    _dummy:      (),
  };

  fn check_positional_count(&self, actual: usize) -> Result<(), ParseArgsError<'static>> {
    let (min, max) = self.positionals;
    if min <= actual && max.unwrap_or(usize::MAX) >= actual {
      Ok(())
    } else {
      Err(ParseArgsError::WrongPositionalCount { min, max, actual })
    }
  }
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum ParseArgsError<'a> {
  #[error("{}", format_positional_count_error(*.min, *.max, *.actual))]
  WrongPositionalCount {
    min:    usize,
    max:    Option<usize>,
    actual: usize,
  },
  #[error("unterminated token {}", token.content)]
  UnterminatedToken { token: Token<'a> },
  #[error("flag '--{flag}' specified more than once")]
  DuplicatedFlag { flag: &'static str },
  #[error("unknown flag '{text}'")]
  UnknownFlag { text: Cow<'a, str> },
  #[error("flag '--{flag}' missing an argument")]
  FlagMissingArgument { flag: &'static str },
  #[error("{}", format_expansion_delimiter_error(expansion))]
  MissingExpansionDelimiter { expansion: &'a str },
  #[error("unknown expansion '{kind}'")]
  UnknownExpansion { kind: &'a str },
}

fn format_positional_count_error(min: usize, max: Option<usize>, actual: usize) -> String {
  let plural = |n| if n == 1 { "" } else { "s" };
  let expected = match (min, max) {
    (0, Some(0)) => "no arguments".to_string(),
    (min, Some(max)) if min == max => format!("exactly {min} argument{}", plural(min)),
    (min, _) if actual < min => format!("at least {min} argument{}", plural(min)),
    (_, Some(max)) if actual > max => format!("at most {max} argument{}", plural(max)),
    _ => unreachable!(),
  };
  format!("expected {expected}, got {actual}")
}

fn format_expansion_delimiter_error(expansion: &str) -> String {
  if expansion.is_empty() {
    "'%' was not properly escaped. Please use '%%'".to_string()
  } else {
    format!("missing a string delimiter after '%{expansion}'")
  }
}

/// Error type for `Args::parse` that preserves structured error information.
///
/// This enum distinguishes between errors that occur during argument parsing
/// (tokenization, flag handling, positional count validation) and errors that
/// occur during token expansion (variable lookup, shell execution, etc.).
#[derive(Debug)]
pub enum ParseError<'a, E> {
  /// An error during argument parsing (tokenization, flags, positionals).
  Args(ParseArgsError<'a>),
  /// An error during token expansion.
  Expand(E),
}

impl<E: fmt::Display> fmt::Display for ParseError<'_, E> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Args(e) => write!(f, "{e}"),
      Self::Expand(e) => write!(f, "{e}"),
    }
  }
}

impl<E: std::error::Error + 'static> std::error::Error for ParseError<'_, E> {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Args(_) => None, // Can't return non-'static reference
      Self::Expand(e) => Some(e),
    }
  }
}

impl<'a, E> From<ParseArgsError<'a>> for ParseError<'a, E> {
  fn from(err: ParseArgsError<'a>) -> Self {
    Self::Args(err)
  }
}

/// The kind of expansion to use on the token's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionKind {
  /// Expand variables from the editor's state.
  ///
  /// For example `%{cursor_line}`.
  Variable,
  /// Treat the token contents as hexadecimal corresponding to a Unicode
  /// codepoint value.
  ///
  /// For example `%u{25CF}`.
  Unicode,
  /// Run the token's contents via the configured shell program.
  ///
  /// For example `%sh{echo hello}`.
  Shell,
}

impl ExpansionKind {
  pub const VARIANTS: &'static [Self] = &[Self::Variable, Self::Unicode, Self::Shell];

  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Variable => "",
      Self::Unicode => "u",
      Self::Shell => "sh",
    }
  }

  pub fn from_kind(name: &str) -> Option<Self> {
    match name {
      "" => Some(Self::Variable),
      "u" => Some(Self::Unicode),
      "sh" => Some(Self::Shell),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
  Single,
  Backtick,
}

impl Quote {
  pub const fn char(&self) -> char {
    match self {
      Self::Single => '\'',
      Self::Backtick => '`',
    }
  }

  // Quotes can be escaped by doubling them: `'hello '' world'` becomes `hello '
  // world`.
  pub const fn escape(&self) -> &'static str {
    match self {
      Self::Single => "''",
      Self::Backtick => "``",
    }
  }
}

/// The type of argument being written.
///
/// The token kind decides how an argument in the command line will be expanded
/// upon hitting `<ret>` in command mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
  /// Unquoted text.
  ///
  /// For example in `:echo hello world`, "hello" and "world" are raw tokens.
  Unquoted,
  /// Quoted text which is interpreted literally.
  ///
  /// The purpose of this kind is to avoid splitting arguments on whitespace.
  /// For example `:open 'a b.txt'` will result in opening a file with a
  /// single argument `"a b.txt"`.
  ///
  /// Using expansions within single quotes or backticks will result in the
  /// expansion text being shown literally. For example `:echo '%u{0020}'`
  /// will print `"%u{0020}"` to the statusline.
  Quoted(Quote),
  /// Text within double quote delimiters (`"`).
  ///
  /// The inner text of a double quoted argument can be further expanded. For
  /// example `:echo "line: #%{cursor_line}"` could print `"line: #1"` to the
  /// statusline.
  Expand,
  /// An expansion / "percent token".
  ///
  /// These take the form `%[<kind>]<open><contents><close>`. See
  /// `ExpansionKind`.
  Expansion(ExpansionKind),
  /// A token kind that exists for the sake of completion.
  ///
  /// In input like `%foo` this token contains the text `"%foo"`. The content
  /// start is the byte after the percent token.
  ///
  /// When `Tokenizer` is passed `true` for its `validate` parameter this token
  /// cannot be returned: inputs that would return this token get a validation
  /// error instead.
  ExpansionKind,
  /// An expansion with an unrecognized kind specifier.
  ///
  /// For example `%xyz{content}` where "xyz" is not a known expansion kind.
  /// This is distinct from `Expand` to allow completion flows to identify
  /// that this was an attempted expansion with an unknown kind, rather than
  /// treating it as expandable text.
  ///
  /// When `Tokenizer` is passed `true` for its `validate` parameter this token
  /// cannot be returned: inputs that would return this token get a validation
  /// error (`UnknownExpansion`) instead.
  UnknownExpansion,
  /// Raw remainder of the input produced by `Tokenizer::rest`.
  ///
  /// This is emitted when `raw_after` triggers. The content is returned
  /// verbatim (no quote or expansion processing) and is not validated.
  Raw,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
  pub kind:          TokenKind,
  /// The byte index into the input where the token's content starts.
  ///
  /// For quoted text this means the byte after the quote. For expansions this
  /// means the byte after the opening delimiter.
  pub content_start: usize,
  /// The inner content of the token.
  ///
  /// Usually this content borrows from the input but an owned value may be used
  /// in cases of escaping. On Unix systems a raw token like `a\ b` has the
  /// contents `"a b"`.
  pub content:       Cow<'a, str>,
  /// Whether the token's opening delimiter is closed.
  ///
  /// For example a quote `"foo"` is closed but not `"foo` or an expansion
  /// `%sh{..}` is closed but not `%sh{echo {}`.
  pub is_terminated: bool,
}

impl<'a> Token<'a> {
  pub fn empty_at(content_start: usize) -> Self {
    Self {
      kind: TokenKind::Unquoted,
      content_start,
      content: Cow::Borrowed(""),
      is_terminated: false,
    }
  }

  pub fn expand(content: impl Into<Cow<'a, str>>) -> Self {
    Self {
      kind:          TokenKind::Expand,
      content_start: 0,
      content:       content.into(),
      is_terminated: true,
    }
  }

  pub fn is_expandable(&self) -> bool {
    matches!(self.kind, TokenKind::Expand | TokenKind::Expansion(_))
  }
}

#[derive(Debug)]
pub struct Tokenizer<'a> {
  input:    &'a str,
  /// Whether to return errors in the iterator for failed validations like
  /// unterminated strings or expansions. When this is set to `false` the
  /// iterator will never return `Err`.
  validate: bool,
  /// The current byte index of the input being considered.
  pos:      usize,
}

impl<'a> Tokenizer<'a> {
  pub fn new(input: &'a str, validate: bool) -> Self {
    Self {
      input,
      validate,
      pos: 0,
    }
  }

  /// Returns the current byte index position of the parser in the input.
  pub fn pos(&self) -> usize {
    self.pos
  }

  /// Returns the rest of the input as a single `TokenKind::Raw` token
  /// literally.
  ///
  /// Returns `None` if the tokenizer is already at the end of the input or
  /// advances the tokenizer to the end of the input otherwise. Leading
  /// whitespace characters are skipped. Quoting is not interpreted.
  pub fn rest(&mut self) -> Option<Token<'a>> {
    self.skip_blanks();

    if self.pos == self.input.len() {
      return None;
    }

    let content_start = self.pos;
    self.pos = self.input.len();
    Some(Token {
      kind: TokenKind::Raw,
      content_start,
      content: Cow::Borrowed(&self.input[content_start..]),
      is_terminated: false,
    })
  }

  fn byte(&self) -> Option<u8> {
    self.input.as_bytes().get(self.pos).copied()
  }

  fn peek_byte(&self) -> Option<u8> {
    self.input.as_bytes().get(self.pos + 1).copied()
  }

  fn prev_byte(&self) -> Option<u8> {
    self
      .pos
      .checked_sub(1)
      .map(|idx| self.input.as_bytes()[idx])
  }

  fn skip_blanks(&mut self) {
    while let Some(b' ' | b'\t') = self.byte() {
      self.pos += 1;
    }
  }

  fn parse_unquoted(&mut self) -> Cow<'a, str> {
    if cfg!(unix) {
      self.parse_unquoted_unix()
    } else {
      self.parse_unquoted_simple()
    }
  }

  /// Simple unquoted parsing for non-Unix systems (no backslash escaping).
  fn parse_unquoted_simple(&mut self) -> Cow<'a, str> {
    let start = self.pos;
    while let Some(byte) = self.byte() {
      if matches!(byte, b' ' | b'\t') {
        return Cow::Borrowed(&self.input[start..self.pos]);
      }
      self.pos += 1;
    }
    Cow::Borrowed(&self.input[start..self.pos])
  }

  /// Unix unquoted parsing with backslash escape handling.
  ///
  /// Backslash semantics:
  /// - `\ ` (backslash + space) → escaped space (space becomes part of token)
  /// - `\\` before whitespace/end → collapsed to single literal backslash
  /// - `\x` (backslash + other) → both chars passed through literally
  /// - Trailing odd backslash → stripped (improves completion behavior)
  #[cfg(unix)]
  fn parse_unquoted_unix(&mut self) -> Cow<'a, str> {
    let bytes = self.input.as_bytes();
    let mut result: Option<String> = None;
    let mut segment_start = self.pos;

    while self.pos < bytes.len() {
      let byte = bytes[self.pos];

      if matches!(byte, b' ' | b'\t') {
        break;
      }

      if byte != b'\\' {
        self.pos += 1;
        continue;
      }

      // Count consecutive backslashes starting here.
      let backslash_start = self.pos;
      while self.pos < bytes.len() && bytes[self.pos] == b'\\' {
        self.pos += 1;
      }
      let backslash_count = self.pos - backslash_start;

      let at_end = self.pos >= bytes.len();
      let next_is_whitespace = !at_end && matches!(bytes[self.pos], b' ' | b'\t');

      if next_is_whitespace || at_end {
        let collapsed_backslashes = backslash_count / 2;
        let has_escaping_backslash = backslash_count % 2 == 1;

        if result.is_none() {
          if at_end && has_escaping_backslash && collapsed_backslashes == 0 {
            return Cow::Borrowed(&self.input[segment_start..backslash_start]);
          }
          result = Some(String::with_capacity(self.pos - segment_start));
        }

        let result = result.as_mut().unwrap();
        result.push_str(&self.input[segment_start..backslash_start]);
        for _ in 0..collapsed_backslashes {
          result.push('\\');
        }

        if at_end {
          segment_start = self.pos;
          break;
        }

        if has_escaping_backslash {
          // Space IS escaped - consume it as part of this token.
          result.push(bytes[self.pos] as char);
          self.pos += 1;
          segment_start = self.pos;
        } else {
          // Space is NOT escaped - token ends here.
          segment_start = self.pos;
          break;
        }
      } else if let Some(result) = result.as_mut() {
        result.push_str(&self.input[segment_start..self.pos]);
        segment_start = self.pos;
      }
    }

    if let Some(mut result) = result {
      if segment_start < self.pos {
        result.push_str(&self.input[segment_start..self.pos]);
      }
      Cow::Owned(result)
    } else {
      Cow::Borrowed(&self.input[segment_start..self.pos])
    }
  }

  /// Parses a string quoted by the given grapheme cluster.
  ///
  /// The position of the tokenizer is asserted to be immediately after the
  /// quote grapheme cluster.
  fn parse_quoted(&mut self, quote: u8) -> (Cow<'a, str>, bool) {
    assert_eq!(self.byte(), Some(quote));
    self.pos += 1;

    let mut escaped = String::new();
    while let Some(offset) = self.input[self.pos..].find(quote as char) {
      let idx = self.pos + offset;
      if self.input.as_bytes().get(idx + 1) == Some(&quote) {
        // Treat two quotes in a row as an escape.
        escaped.push_str(&self.input[self.pos..idx + 1]);
        // Advance past the escaped quote.
        self.pos = idx + 2;
      } else {
        // Otherwise this quote string is finished.
        let quoted = if escaped.is_empty() {
          Cow::Borrowed(&self.input[self.pos..idx])
        } else {
          escaped.push_str(&self.input[self.pos..idx]);
          Cow::Owned(escaped)
        };
        // Advance past the closing quote.
        self.pos = idx + 1;
        return (quoted, true);
      }
    }

    let quoted = if escaped.is_empty() {
      Cow::Borrowed(&self.input[self.pos..])
    } else {
      escaped.push_str(&self.input[self.pos..]);
      Cow::Owned(escaped)
    };
    self.pos = self.input.len();

    (quoted, false)
  }

  /// Parses the percent token expansion under the tokenizer's cursor.
  ///
  /// This function should only be called when the tokenizer's cursor is on a
  /// non-escaped percent token.
  pub fn parse_percent_token(&mut self) -> Option<Result<Token<'a>, ParseArgsError<'a>>> {
    assert_eq!(self.byte(), Some(b'%'));

    self.pos += 1;
    let kind_start = self.pos;
    self.pos += self.input[self.pos..]
      .bytes()
      .take_while(|b| b.is_ascii_lowercase())
      .count();
    let kind = &self.input[kind_start..self.pos];

    let (open, close) = match self.byte() {
      // We support a couple of hard-coded chars only to make sure we can provide more
      // useful errors and avoid weird behavior in case of typos. These should cover
      // practical cases.
      Some(b'(') => (b'(', b')'),
      Some(b'[') => (b'[', b']'),
      Some(b'{') => (b'{', b'}'),
      Some(b'<') => (b'<', b'>'),
      Some(b'\'') => (b'\'', b'\''),
      Some(b'\"') => (b'\"', b'\"'),
      Some(b'|') => (b'|', b'|'),
      Some(_) | None => {
        return Some(if self.validate {
          Err(ParseArgsError::MissingExpansionDelimiter { expansion: kind })
        } else {
          Ok(Token {
            kind:          TokenKind::ExpansionKind,
            content_start: kind_start,
            content:       Cow::Borrowed(kind),
            is_terminated: false,
          })
        });
      },
    };
    // The content start for expansions is the start of the content - after the
    // opening delimiter grapheme.
    let content_start = self.pos + 1;
    let kind = match ExpansionKind::from_kind(kind) {
      Some(kind) => TokenKind::Expansion(kind),
      None if self.validate => {
        return Some(Err(ParseArgsError::UnknownExpansion { kind }));
      },
      None => TokenKind::UnknownExpansion,
    };

    let (content, is_terminated) = if open == close {
      self.parse_quoted(open)
    } else {
      self.parse_quoted_balanced(open, close)
    };

    let token = Token {
      kind,
      content_start,
      content,
      is_terminated,
    };

    if self.validate && !is_terminated {
      return Some(Err(ParseArgsError::UnterminatedToken { token }));
    }

    Some(Ok(token))
  }

  /// Parse the next string under the cursor given an open and closing pair.
  ///
  /// The open and closing pair are different ASCII characters. The cursor is
  /// asserted to be immediately after the opening delimiter.
  ///
  /// This function parses with nesting support. `%sh{echo {hello}}` for example
  /// should consume the entire input and not quit after the first '}'
  /// character is found.
  fn parse_quoted_balanced(&mut self, open: u8, close: u8) -> (Cow<'a, str>, bool) {
    assert_eq!(self.byte(), Some(open));
    self.pos += 1;
    let start = self.pos;
    let mut level = 1;

    while let Some(offset) = self.input[self.pos..].find([open as char, close as char]) {
      let idx = self.pos + offset;
      // Move past the delimiter.
      self.pos = idx + 1;

      let byte = self.input.as_bytes()[idx];
      if byte == open {
        level += 1;
      } else if byte == close {
        level -= 1;
        if level == 0 {
          break;
        }
      } else {
        unreachable!()
      }
    }

    let is_terminated = level == 0;
    let end = if is_terminated {
      // Exclude the closing delimiter from the token's content.
      self.pos - 1
    } else {
      // When the token is not closed, advance to the end of the input.
      self.pos = self.input.len();
      self.pos
    };

    (Cow::Borrowed(&self.input[start..end]), is_terminated)
  }
}

impl<'a> Iterator for Tokenizer<'a> {
  type Item = Result<Token<'a>, ParseArgsError<'a>>;

  fn next(&mut self) -> Option<Self::Item> {
    self.skip_blanks();

    let byte = self.byte()?;
    match byte {
      b'"' | b'\'' | b'`' => {
        let content_start = self.pos + 1;
        let (content, is_terminated) = self.parse_quoted(byte);
        let token = Token {
          kind: match byte {
            b'"' => TokenKind::Expand,
            b'\'' => TokenKind::Quoted(Quote::Single),
            b'`' => TokenKind::Quoted(Quote::Backtick),
            _ => unreachable!(),
          },
          content_start,
          content,
          is_terminated,
        };

        Some(if self.validate && !is_terminated {
          Err(ParseArgsError::UnterminatedToken { token })
        } else {
          Ok(token)
        })
      },
      b'%' => self.parse_percent_token(),
      _ => {
        let content_start = self.pos;

        Some(Ok(Token {
          kind: TokenKind::Unquoted,
          content_start,
          content: self.parse_unquoted(),
          is_terminated: false,
        }))
      },
    }
  }
}

pub fn tokenize<'a>(input: &'a str, validate: bool) -> Result<Vec<Token<'a>>, ParseArgsError<'a>> {
  Tokenizer::new(input, validate).collect()
}

pub fn expand_tokens<'a, E>(
  tokens: impl IntoIterator<Item = Token<'a>>,
  mut expand: impl FnMut(Token<'a>) -> Result<Cow<'a, str>, E>,
) -> Result<Vec<Cow<'a, str>>, E> {
  tokens
    .into_iter()
    .map(|token| {
      if token.is_expandable() {
        expand(token)
      } else {
        Ok(token.content)
      }
    })
    .collect()
}

#[derive(Debug, Default, Clone, Copy)]
pub enum CompletionState {
  #[default]
  Positional,
  Flag(Option<Flag>),
  FlagArgument(Flag),
}

/// A set of arguments provided to a command on the command line.
///
/// Regular arguments are called "positional" arguments (or "positionals" for
/// short). Command line input might also specify "flags" which can modify a
/// command's behavior.
///
/// ```rust,ignore
/// // Say that the command accepts a "bar" flag which doesn't accept an argument itself.
/// // This input has two positionals, "foo" and "baz" and one flag "--bar".
/// let args = Args::parse("foo --bar baz", /* .. */);
/// // `Args` may be treated like a slice to access positionals.
/// assert_eq!(args.len(), 2);
/// assert_eq!(&args[0], "foo");
/// assert_eq!(&args[1], "baz");
/// // Use `has_flag` or `get_flag` to access flags.
/// assert!(args.has_flag("bar"));
/// ```
///
/// The `Args` type can be treated mostly the same as a slice when accessing
/// positional arguments. Common slice methods like `len`, `get`, `first` and
/// `join` only expose positional arguments. Additionally, common syntax like
/// `for arg in args` or `&args[idx]` is supported for accessing positional
/// arguments.
///
/// To look up flags, use `Args::get_flag` for flags which should accept an
/// argument or `Args::has_flag` for boolean flags.
///
/// The way that `Args` is parsed from the input depends on a command's
/// `Signature`. See the `Signature` type for more details.
#[derive(Debug)]
pub struct Args<'a> {
  signature:        Signature,
  /// Whether to validate the arguments.
  /// See the `ParseArgsError` type for the validations.
  validate:         bool,
  /// Whether args pushed with `Self::push` should be treated as positionals
  /// even if they start with '-'.
  only_positionals: bool,
  state:            CompletionState,
  positionals:      Vec<Cow<'a, str>>,
  flags:            HashMap<&'static str, FlagValue<'a>>,
}

impl Default for Args<'_> {
  fn default() -> Self {
    Self {
      signature:        Signature::DEFAULT,
      validate:         Default::default(),
      only_positionals: Default::default(),
      state:            CompletionState::default(),
      positionals:      Default::default(),
      flags:            Default::default(),
    }
  }
}

#[derive(Debug)]
pub struct ArgsBuilder<'a> {
  args: Args<'a>,
}

impl<'a> ArgsBuilder<'a> {
  pub fn new(signature: Signature, validate: bool) -> Self {
    Self {
      args: Args::new(signature, validate),
    }
  }

  pub fn read_token<'p>(
    &mut self,
    parser: &mut Tokenizer<'p>,
  ) -> Result<Option<Token<'p>>, ParseArgsError<'p>> {
    self.args.read_token(parser)
  }

  pub fn step(mut self, arg: Cow<'a, str>) -> Result<Self, ParseArgsError<'a>> {
    self.args.push(arg)?;
    Ok(self)
  }

  pub fn finish(self) -> Result<Args<'a>, ParseArgsError<'a>> {
    self.args.finish()?;
    Ok(self.args)
  }
}

pub fn parse_args<'a>(
  args: impl IntoIterator<Item = Cow<'a, str>>,
  signature: Signature,
  validate: bool,
) -> Result<Args<'a>, ParseArgsError<'a>> {
  args
    .into_iter()
    .try_fold(ArgsBuilder::new(signature, validate), |builder, arg| {
      builder.step(arg)
    })
    .and_then(ArgsBuilder::finish)
}

impl<'a> Args<'a> {
  pub fn new(signature: Signature, validate: bool) -> Self {
    Self {
      signature,
      validate,
      only_positionals: false,
      positionals: Vec::new(),
      flags: HashMap::new(),
      state: CompletionState::default(),
    }
  }

  /// Reads the next token out of the given parser.
  ///
  /// If the command's signature sets a maximum number of positionals (via
  /// `raw_after`) then the token may contain the rest of the parser's input.
  pub fn read_token<'p>(
    &mut self,
    parser: &mut Tokenizer<'p>,
  ) -> Result<Option<Token<'p>>, ParseArgsError<'p>> {
    if self
      .signature
      .raw_after
      .is_some_and(|max| self.len() >= max as usize)
    {
      self.only_positionals = true;
      Ok(parser.rest())
    } else {
      parser.next().transpose()
    }
  }

  /// Parses the given command line according to a command's signature.
  ///
  /// The `try_map_fn` function can be used to try changing each token before it
  /// is considered as an argument - this is used for variable expansion.
  ///
  /// The generic parameter `E` is the error type returned by the expansion
  /// function. This allows callers to use their own error types for expansion
  /// failures while still getting structured error information.
  pub fn parse<E, M>(
    line: &'a str,
    signature: Signature,
    validate: bool,
    mut try_map_fn: M,
  ) -> Result<Self, ParseError<'a, E>>
  where
    // Note: this is a `FnMut` in case we decide to allow caching expansions in the future.
    // The `mut` is not currently used.
    M: FnMut(Token<'a>) -> Result<Cow<'a, str>, E>,
  {
    if signature.raw_after.is_none() {
      let tokens = tokenize(line, validate)?;
      let expanded = expand_tokens(tokens, |token| {
        if token.is_expandable() {
          try_map_fn(token)
        } else {
          Ok(token.content)
        }
      })
      .map_err(ParseError::Expand)?;
      let args = parse_args(expanded, signature, validate)?;
      Ok(args)
    } else {
      let mut tokenizer = Tokenizer::new(line, validate);
      let mut builder = ArgsBuilder::new(signature, validate);

      while let Some(token) = builder.read_token(&mut tokenizer)? {
        let arg = if token.is_expandable() {
          try_map_fn(token).map_err(ParseError::Expand)?
        } else {
          token.content
        };
        builder = builder.step(arg)?;
      }

      Ok(builder.finish()?)
    }
  }

  /// Adds the given argument token.
  ///
  /// Once all arguments have been added, `Self::finish` should be called to
  /// perform any closing validations.
  pub fn push(&mut self, arg: Cow<'a, str>) -> Result<(), ParseArgsError<'a>> {
    if !self.only_positionals && arg == "--" {
      // "--" marks the end of flags, everything after is a positional even if it
      // starts with '-'.
      self.only_positionals = true;
      self.state = CompletionState::Flag(None);
    } else if let Some(flag) = self.flag_awaiting_argument() {
      // If the last token was a flag which accepts an argument, treat this token as a
      // flag argument.
      self.flags.insert(flag.name, FlagValue::Value(arg));
      self.state = CompletionState::FlagArgument(flag);
    } else if !self.only_positionals && arg.starts_with('-') && arg != "-" {
      // If the token starts with '-' (but is not a lone '-', which is a common stdin
      // sentinel) and we are not only accepting positional arguments, treat this
      // token as a flag.
      let flag = if let Some(longhand) = arg.strip_prefix("--") {
        self
          .signature
          .flags
          .iter()
          .find(|flag| flag.name == longhand)
      } else {
        let shorthand = arg.strip_prefix('-').unwrap();
        self.signature.flags.iter().find(|flag| {
          flag
            .alias
            .is_some_and(|ch| shorthand == ch.encode_utf8(&mut [0; 4]))
        })
      };

      let Some(flag) = flag else {
        if self.validate {
          return Err(ParseArgsError::UnknownFlag { text: arg });
        }

        self.positionals.push(arg);
        self.state = CompletionState::Flag(None);
        return Ok(());
      };

      if self.validate && self.flags.contains_key(flag.name) {
        return Err(ParseArgsError::DuplicatedFlag { flag: flag.name });
      }

      // Insert Bool for now; if the flag takes a value, flag_awaiting_argument will
      // return it and the next push will upgrade this to FlagValue::Value.
      self.flags.insert(flag.name, FlagValue::Bool);
      self.state = CompletionState::Flag(Some(*flag));
    } else {
      // Otherwise this token is a positional argument (including lone "-").
      self.positionals.push(arg);
      self.state = CompletionState::Positional;
    }

    Ok(())
  }

  /// Performs any validations that must be done after the input args are
  /// finished being pushed with `Self::push`.
  fn finish(&self) -> Result<(), ParseArgsError<'a>> {
    if !self.validate {
      return Ok(());
    };

    if let Some(flag) = self.flag_awaiting_argument() {
      return Err(ParseArgsError::FlagMissingArgument { flag: flag.name });
    }
    self
      .signature
      .check_positional_count(self.positionals.len())?;

    Ok(())
  }

  fn flag_awaiting_argument(&self) -> Option<Flag> {
    match self.state {
      CompletionState::Flag(flag) => flag.filter(|f| f.takes_value),
      _ => None,
    }
  }

  /// Returns the kind of argument the last token is considered to be.
  ///
  /// For example if the last argument in the command line is `--foo` then the
  /// argument may be considered to be a flag.
  pub fn completion_state(&self) -> CompletionState {
    self.state
  }

  /// Returns the number of positionals supplied in the input.
  ///
  /// This number does not account for any flags passed in the input.
  pub fn len(&self) -> usize {
    self.positionals.len()
  }

  /// Checks whether the arguments contain no positionals.
  ///
  /// Note that this function returns `true` if there are no positional
  /// arguments even if the input contained flags.
  pub fn is_empty(&self) -> bool {
    self.positionals.is_empty()
  }

  /// Gets the first positional argument, if one exists.
  pub fn first(&'a self) -> Option<&'a str> {
    self.positionals.first().map(AsRef::as_ref)
  }

  /// Gets the positional argument at the given index, if one exists.
  pub fn get(&'a self, index: usize) -> Option<&'a str> {
    self.positionals.get(index).map(AsRef::as_ref)
  }

  /// Flattens all positional arguments together with the given separator
  /// between each positional.
  pub fn join(&self, sep: &str) -> String {
    self.positionals.join(sep)
  }

  /// Returns an iterator over all positional arguments.
  pub fn iter(&self) -> slice::Iter<'_, Cow<'_, str>> {
    self.positionals.iter()
  }

  /// Gets the value associated with a flag's long name if the flag was
  /// provided.
  ///
  /// This function should be preferred over [Self::has_flag] when the flag
  /// accepts an argument (i.e., `takes_value: true`).
  pub fn get_flag(&'a self, name: &'static str) -> Option<&'a str> {
    debug_assert!(
      self.signature.flags.iter().any(|flag| flag.name == name),
      "flag '--{name}' does not belong to the command's signature"
    );
    debug_assert!(
      self
        .signature
        .flags
        .iter()
        .any(|flag| flag.name == name && flag.takes_value),
      "Args::get_flag was used for '--{name}' but should only be used for flags with takes_value: \
       true, use Args::has_flag instead"
    );

    match self.flags.get(name) {
      Some(FlagValue::Value(v)) => Some(v.as_ref()),
      _ => None,
    }
  }

  /// Checks if a flag was provided in the arguments.
  ///
  /// This function should be preferred over [Self::get_flag] for boolean flags
  /// - flags that either are present or not (i.e., `takes_value: false`).
  pub fn has_flag(&self, name: &'static str) -> bool {
    debug_assert!(
      self.signature.flags.iter().any(|flag| flag.name == name),
      "flag '--{name}' does not belong to the command's signature"
    );
    debug_assert!(
      self
        .signature
        .flags
        .iter()
        .any(|flag| flag.name == name && !flag.takes_value),
      "Args::has_flag was used for '--{name}' but should only be used for flags with takes_value: \
       false, use Args::get_flag instead"
    );

    self.flags.contains_key(name)
  }
}

// `arg[n]`
impl ops::Index<usize> for Args<'_> {
  type Output = str;

  fn index(&self, index: usize) -> &Self::Output {
    self.positionals[index].as_ref()
  }
}

// `for arg in args { .. }`
impl<'a> IntoIterator for Args<'a> {
  type Item = Cow<'a, str>;
  type IntoIter = vec::IntoIter<Cow<'a, str>>;

  fn into_iter(self) -> Self::IntoIter {
    self.positionals.into_iter()
  }
}

// `for arg in &args { .. }`
impl<'i, 'a> IntoIterator for &'i Args<'a> {
  type Item = &'i Cow<'a, str>;
  type IntoIter = slice::Iter<'i, Cow<'a, str>>;

  fn into_iter(self) -> Self::IntoIter {
    self.positionals.iter()
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[track_caller]
  fn assert_tokens(input: &str, expected: &[&str]) {
    let actual: Vec<_> = Tokenizer::new(input, true)
      .map(|arg| arg.unwrap().content)
      .collect();
    let actual: Vec<_> = actual.iter().map(|c| c.as_ref()).collect();

    assert_eq!(actual.as_slice(), expected);
  }

  #[track_caller]
  fn assert_incomplete_tokens(input: &str, expected: &[&str]) {
    assert!(
      Tokenizer::new(input, true)
        .collect::<Result<Vec<_>, _>>()
        .is_err(),
      "`assert_incomplete_tokens` only accepts input that fails validation, consider using \
       `assert_tokens` instead"
    );

    let actual: Vec<_> = Tokenizer::new(input, false)
      .map(|arg| arg.unwrap().content)
      .collect();
    let actual: Vec<_> = actual.iter().map(|c| c.as_ref()).collect();

    assert_eq!(actual.as_slice(), expected);
  }

  #[test]
  fn tokenize_unquoted() {
    assert_tokens("", &[]);
    assert_tokens("hello", &["hello"]);
    assert_tokens("hello world", &["hello", "world"]);
    // Any amount of whitespace is considered a separator.
    assert_tokens("hello\t \tworld", &["hello", "world"]);
  }

  // This escaping behavior is specific to Unix systems.
  #[cfg(unix)]
  #[test]
  fn tokenize_backslash_unix() {
    assert_tokens(r#"hello\ world"#, &["hello world"]);
    assert_tokens(r#"one\ two three"#, &["one two", "three"]);
    assert_tokens(r#"one two\ three"#, &["one", "two three"]);
    // Trailing backslash is ignored - this improves completions.
    assert_tokens(r#"hello\"#, &["hello"]);
    // The backslash at the start of the double quote makes the quote be treated as
    // raw. For the backslash before the ending quote the token is already
    // considered raw so the backslash and quote are treated literally.
    assert_tokens(r#"echo \"hello        world\""#, &[
      "echo",
      r#"\"hello"#,
      r#"world\""#,
    ]);
  }

  #[test]
  fn tokenize_backslash() {
    assert_tokens(r#"\n"#, &["\\n"]);
    assert_tokens(r#"'\'"#, &["\\"]);
  }

  #[test]
  fn tokenize_quoting() {
    // Using a quote character twice escapes it.
    assert_tokens(r#"''"#, &[""]);
    assert_tokens(r#""""#, &[""]);
    assert_tokens(r#"``"#, &[""]);
    assert_tokens(r#"echo """#, &["echo", ""]);

    assert_tokens(r#"'hello'"#, &["hello"]);
    assert_tokens(r#"'hello world'"#, &["hello world"]);

    assert_tokens(r#""hello "" world""#, &["hello \" world"]);
  }

  #[test]
  fn tokenize_percent() {
    // Pair delimiters:
    assert_tokens(r#"echo %{hello world}"#, &["echo", "hello world"]);
    assert_tokens(r#"echo %[hello world]"#, &["echo", "hello world"]);
    assert_tokens(r#"echo %(hello world)"#, &["echo", "hello world"]);
    assert_tokens(r#"echo %<hello world>"#, &["echo", "hello world"]);
    assert_tokens(r#"echo %|hello world|"#, &["echo", "hello world"]);
    assert_tokens(r#"echo %'hello world'"#, &["echo", "hello world"]);
    assert_tokens(r#"echo %"hello world""#, &["echo", "hello world"]);
    // When invoking a command, double percents can be used within a string as an
    // escape for the percent. This is done in the expansion code though, not in
    // the parser here.
    assert_tokens(r#"echo "%%hello world""#, &["echo", "%%hello world"]);
    // Different kinds of quotes nested:
    assert_tokens(r#"echo "%sh{echo 'hello world'}""#, &[
      "echo",
      r#"%sh{echo 'hello world'}"#,
    ]);
    // Nesting of the expansion delimiter:
    assert_tokens(r#"echo %{hello {x} world}"#, &["echo", "hello {x} world"]);
    assert_tokens(r#"echo %{hello {{😎}} world}"#, &[
      "echo",
      "hello {{😎}} world",
    ]);

    // Balanced nesting:
    assert_tokens(r#"echo %{hello {}} world}"#, &[
      "echo", "hello {}", "world}",
    ]);

    // Recursive expansions:
    assert_tokens(r#"echo %sh{echo "%{cursor_line}"}"#, &[
      "echo",
      r#"echo "%{cursor_line}""#,
    ]);
    // Completion should provide variable names here. (Unbalanced nesting)
    assert_incomplete_tokens(r#"echo %sh{echo "%{c"#, &["echo", r#"echo "%{c"#]);
    assert_incomplete_tokens(r#"echo %{hello {{} world}"#, &["echo", "hello {{} world}"]);
  }

  fn parse_signature<'a>(
    input: &'a str,
    signature: Signature,
  ) -> Result<Args<'a>, ParseError<'a, std::convert::Infallible>> {
    Args::parse(input, signature, true, |token| Ok(token.content))
  }

  #[test]
  fn signature_validation_positionals() {
    let signature = Signature {
      positionals: (2, Some(3)),
      ..Signature::DEFAULT
    };

    assert!(parse_signature("hello world", signature).is_ok());
    assert!(parse_signature("foo bar baz", signature).is_ok());
    assert!(parse_signature(r#"a "b c" d"#, signature).is_ok());

    assert!(parse_signature("hello", signature).is_err());
    assert!(parse_signature("foo bar baz quiz", signature).is_err());

    let signature = Signature {
      positionals: (1, None),
      ..Signature::DEFAULT
    };

    assert!(parse_signature("a", signature).is_ok());
    assert!(parse_signature("a b", signature).is_ok());
    assert!(parse_signature(r#"a "b c" d"#, signature).is_ok());

    assert!(parse_signature("", signature).is_err());
  }

  #[test]
  fn flags() {
    let signature = Signature {
      positionals: (1, Some(2)),
      flags: &[
        Flag {
          name:        "foo",
          alias:       Some('f'),
          doc:         "",
          takes_value: false,
          completions: None,
        },
        Flag {
          name:        "bar",
          alias:       Some('b'),
          doc:         "",
          takes_value: true,
          completions: Some(&[]),
        },
      ],
      ..Signature::DEFAULT
    };

    let args = parse_signature("hello", signature).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "hello");
    assert!(!args.has_flag("foo"));
    assert!(args.get_flag("bar").is_none());

    let args = parse_signature("--bar abcd hello world --foo", signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "hello");
    assert_eq!(&args[1], "world");
    assert!(args.has_flag("foo"));
    assert_eq!(args.get_flag("bar"), Some("abcd"));

    let args = parse_signature("hello -f -b abcd world", signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "hello");
    assert_eq!(&args[1], "world");
    assert!(args.has_flag("foo"));
    assert_eq!(args.get_flag("bar"), Some("abcd"));

    // The signature requires at least one positional.
    assert!(parse_signature("--foo", signature).is_err());
    // And at most two.
    assert!(parse_signature("abc --bar baz def efg", signature).is_err());

    let args = parse_signature(r#"abc -b "xyz 123" def"#, signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "abc");
    assert_eq!(&args[1], "def");
    assert_eq!(args.get_flag("bar"), Some("xyz 123"));

    // Unknown flags are validation errors.
    assert!(parse_signature(r#"foo --quiz"#, signature).is_err());
    // Duplicated flags are parsing errors.
    assert!(parse_signature(r#"--foo bar --foo"#, signature).is_err());
    assert!(parse_signature(r#"-f bar --foo"#, signature).is_err());

    // "--" can be used to mark the end of flags. Everything after is considered a
    // positional.
    let args = parse_signature(r#"hello --bar baz -- --foo"#, signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "hello");
    assert_eq!(&args[1], "--foo");
    assert_eq!(args.get_flag("bar"), Some("baz"));
    assert!(!args.has_flag("foo"));
  }

  #[test]
  fn raw_after() {
    let signature = Signature {
      positionals: (1, Some(1)),
      raw_after: Some(0),
      ..Signature::DEFAULT
    };

    // All quoting and escaping is treated literally in raw mode.
    let args = parse_signature(r#"'\'"#, signature).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "'\\'");
    let args = parse_signature(r#"\''"#, signature).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "\\''");

    // Leading space is trimmed.
    let args = parse_signature(r#"   %sh{foo}"#, signature).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "%sh{foo}");

    let signature = Signature {
      positionals: (1, Some(2)),
      raw_after: Some(1),
      ..Signature::DEFAULT
    };

    let args = parse_signature("foo", signature).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "foo");

    // "--bar" is treated as a positional.
    let args = parse_signature("foo --bar", signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "foo");
    assert_eq!(&args[1], "--bar");

    let args = parse_signature("abc def ghi", signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "abc");
    assert_eq!(&args[1], "def ghi");

    let args = parse_signature("rulers [20, 30]", signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "rulers");
    assert_eq!(&args[1], "[20, 30]");

    let args = parse_signature(r#"gutters ["diff"] ["diff", "diagnostics"]"#, signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "gutters");
    assert_eq!(&args[1], r#"["diff"] ["diff", "diagnostics"]"#);
  }

  #[cfg(unix)]
  #[test]
  fn backslash_parity() {
    // Single backslash escapes the space
    assert_tokens(r#"hello\ world"#, &["hello world"]);

    // Double backslash: first escapes second, space is NOT escaped (two tokens)
    assert_tokens(r#"hello\\ world"#, &["hello\\", "world"]);

    // Triple backslash: first two form escaped backslash, third escapes space
    assert_tokens(r#"hello\\\ world"#, &["hello\\ world"]);

    // Four backslashes: two pairs, space is NOT escaped
    assert_tokens(r#"hello\\\\ world"#, &["hello\\\\", "world"]);

    // Mixed: backslash at end of first word, space starts new word
    assert_tokens(r#"a\\ b"#, &["a\\", "b"]);

    // Trailing backslash (odd) is stripped
    assert_tokens(r#"hello\"#, &["hello"]);

    // Trailing double backslash (even) is kept
    assert_tokens(r#"hello\\"#, &["hello\\"]);
  }

  #[test]
  fn lone_dash_as_positional() {
    let signature = Signature {
      positionals: (1, Some(2)),
      flags: &[Flag {
        name:        "foo",
        alias:       Some('f'),
        doc:         "",
        takes_value: false,
        completions: None,
      }],
      ..Signature::DEFAULT
    };

    // Lone "-" should be treated as a positional (stdin sentinel)
    let args = parse_signature("-", signature).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "-");

    // "-" mixed with flags
    let args = parse_signature("--foo -", signature).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(&args[0], "-");
    assert!(args.has_flag("foo"));

    // Multiple lone dashes
    let args = parse_signature("- -", signature).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(&args[0], "-");
    assert_eq!(&args[1], "-");
  }

  #[test]
  fn unknown_expansion_token_kind() {
    // In non-validate mode, unknown expansion kinds should return UnknownExpansion
    let tokens: Vec<_> = Tokenizer::new("%xyz{content}", false)
      .map(|t| t.unwrap())
      .collect();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::UnknownExpansion);
    assert_eq!(tokens[0].content.as_ref(), "content");

    // In validate mode, it should error
    let result: Result<Vec<_>, _> = Tokenizer::new("%xyz{content}", true).collect();
    assert!(result.is_err());
    match result.unwrap_err() {
      ParseArgsError::UnknownExpansion { kind } => assert_eq!(kind, "xyz"),
      _ => panic!("expected UnknownExpansion error"),
    }
  }

  #[test]
  fn flag_value_semantics() {
    let signature = Signature {
      positionals: (0, Some(1)),
      flags: &[
        Flag {
          name:        "bool_flag",
          alias:       Some('b'),
          doc:         "",
          takes_value: false,
          completions: None,
        },
        Flag {
          name:        "value_flag",
          alias:       Some('v'),
          doc:         "",
          takes_value: true,
          // Note: takes_value is independent of completions
          completions: None,
        },
      ],
      ..Signature::DEFAULT
    };

    // Boolean flag present
    let args = parse_signature("--bool_flag", signature).unwrap();
    assert!(args.has_flag("bool_flag"));

    // Value flag with argument
    let args = parse_signature("--value_flag myvalue", signature).unwrap();
    assert_eq!(args.get_flag("value_flag"), Some("myvalue"));

    // Value flag without completions still takes a value
    let args = parse_signature("-v test", signature).unwrap();
    assert_eq!(args.get_flag("value_flag"), Some("test"));

    // Missing value for value_flag is an error
    assert!(parse_signature("--value_flag", signature).is_err());
  }

  #[test]
  fn parse_error_variants() {
    let signature = Signature {
      positionals: (1, Some(1)),
      flags: &[Flag {
        name:        "flag",
        alias:       None,
        doc:         "",
        takes_value: true,
        completions: None,
      }],
      ..Signature::DEFAULT
    };

    // Test that ParseError preserves the error kind
    let result = parse_signature("", signature);
    assert!(matches!(
      result,
      Err(ParseError::Args(
        ParseArgsError::WrongPositionalCount { .. }
      ))
    ));

    let result = parse_signature("--flag", signature);
    assert!(matches!(
      result,
      Err(ParseError::Args(ParseArgsError::FlagMissingArgument { .. }))
    ));

    let result = parse_signature("arg --unknown", signature);
    assert!(matches!(
      result,
      Err(ParseError::Args(ParseArgsError::UnknownFlag { .. }))
    ));
  }
}
