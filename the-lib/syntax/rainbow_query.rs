use std::collections::HashSet;

use tree_house::tree_sitter::{
  Capture,
  Grammar,
  Pattern,
  Query,
  query::{
    InvalidPredicateError,
    ParseError,
    UserPredicate,
  },
};

#[allow(dead_code)]
#[derive(Debug)]
pub struct RainbowQuery {
  pub(crate) query:                     Query,
  pub(crate) include_children_patterns: HashSet<Pattern>,
  pub(crate) scope_capture:             Option<Capture>,
  pub(crate) bracket_capture:           Option<Capture>,
}

impl RainbowQuery {
  pub fn new(grammar: Grammar, source: &str) -> Result<Self, ParseError> {
    let mut include_children_patterns = HashSet::default();

    let query = Query::new(grammar, source, |pattern, predicate| {
      match predicate {
        UserPredicate::SetProperty {
          key: "rainbow.include-children",
          val,
        } => {
          if val.is_some() {
            return Err("property 'rainbow.include-children' does not take an argument".into());
          }
          include_children_patterns.insert(pattern);
          Ok(())
        },
        _ => Err(InvalidPredicateError::unknown(predicate)),
      }
    })?;

    Ok(Self {
      include_children_patterns,
      scope_capture: query.get_capture("rainbow.scope"),
      bracket_capture: query.get_capture("rainbow.bracket"),
      query,
    })
  }
}
