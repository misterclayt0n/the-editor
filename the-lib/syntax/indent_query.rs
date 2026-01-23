//! Indentation query parsing for tree-sitter.
//!
//! This is a small, query-only parser that extracts indent-related captures
//! and predicates. It does not compute indentation by itself; it only parses
//! and stores the query metadata so other modules can evaluate it.
//!
//! # Example
//!
//! ```ignore
//! use the_lib::syntax::IndentQuery;
//! # use tree_house::tree_sitter::Grammar;
//!
//! # let grammar: Grammar = /* tree-sitter grammar */ unimplemented!();
//! let query = r#"
//!   (block) @indent
//!   (block) @outdent
//! "#;
//! let indent_query = IndentQuery::new(grammar, query).unwrap();
//! ```
use std::collections::HashMap;

use tree_house::tree_sitter::{
  Capture,
  Grammar,
  Pattern,
  Query,
  query::{
    self,
    InvalidPredicateError,
    UserPredicate,
  },
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum IndentScope {
  /// The indent applies to the whole node.
  All,
  /// The indent applies to everything except for the first line of the node.
  Tail,
}

#[derive(Debug, Default)]
pub(crate) struct IndentQueryPredicates {
  pub(crate) not_kind_eq: Vec<(Capture, Box<str>)>,
  pub(crate) same_line:   Option<(Capture, Capture, bool)>,
  pub(crate) one_line:    Option<(Capture, bool)>,
}

#[derive(Debug)]
pub struct IndentQuery {
  pub(crate) query:                       Query,
  pub(crate) properties:                  HashMap<Pattern, IndentScope>,
  pub(crate) predicates:                  HashMap<Pattern, IndentQueryPredicates>,
  pub(crate) indent_capture:              Option<Capture>,
  pub(crate) indent_always_capture:       Option<Capture>,
  pub(crate) outdent_capture:             Option<Capture>,
  pub(crate) outdent_always_capture:      Option<Capture>,
  pub(crate) align_capture:               Option<Capture>,
  pub(crate) anchor_capture:              Option<Capture>,
  pub(crate) extend_capture:              Option<Capture>,
  pub(crate) extend_prevent_once_capture: Option<Capture>,
}

impl IndentQuery {
  pub fn new(grammar: Grammar, source: &str) -> Result<Self, query::ParseError> {
    let mut properties = HashMap::new();
    let mut predicates: HashMap<Pattern, IndentQueryPredicates> = HashMap::new();
    let query = Query::new(grammar, source, |pattern, predicate| {
      match predicate {
        UserPredicate::SetProperty { key: "scope", val } => {
          let scope = match val {
            Some("all") => IndentScope::All,
            Some("tail") => IndentScope::Tail,
            Some(other) => return Err(format!("unknown scope (#set! scope \"{other}\")").into()),
            None => return Err("missing scope value (#set! scope ...)".into()),
          };

          properties.insert(pattern, scope);

          Ok(())
        },
        UserPredicate::Other(predicate) => {
          let name = predicate.name();
          match name {
            "not-kind-eq?" => {
              predicate.check_arg_count(2)?;
              let capture = predicate.capture_arg(0)?;
              let not_expected_kind = predicate.str_arg(1)?;

              predicates
                .entry(pattern)
                .or_default()
                .not_kind_eq
                .push((capture, not_expected_kind.into()));
              Ok(())
            },
            "same-line?" | "not-same-line?" => {
              predicate.check_arg_count(2)?;
              let capture1 = predicate.capture_arg(0)?;
              let capture2 = predicate.capture_arg(1)?;
              let negated = name == "not-same-line?";

              predicates.entry(pattern).or_default().same_line =
                Some((capture1, capture2, negated));
              Ok(())
            },
            "one-line?" | "not-one-line?" => {
              predicate.check_arg_count(1)?;
              let capture = predicate.capture_arg(0)?;
              let negated = name == "not-one-line?";

              predicates.entry(pattern).or_default().one_line = Some((capture, negated));
              Ok(())
            },
            _ => {
              Err(InvalidPredicateError::unknown(UserPredicate::Other(
                predicate,
              )))
            },
          }
        },
        _ => Err(InvalidPredicateError::unknown(predicate)),
      }
    })?;

    Ok(Self {
      properties,
      predicates,
      indent_capture: query.get_capture("indent"),
      indent_always_capture: query.get_capture("indent.always"),
      outdent_capture: query.get_capture("outdent"),
      outdent_always_capture: query.get_capture("outdent.always"),
      align_capture: query.get_capture("align"),
      anchor_capture: query.get_capture("anchor"),
      extend_capture: query.get_capture("extend"),
      extend_prevent_once_capture: query.get_capture("extend.prevent-once"),
      query,
    })
  }
}
