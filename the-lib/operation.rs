use std::num::NonZeroU64;

use crate::{
  selection::{CursorId, CursorRange, Range, Selection},
  transaction::{Replacement, ReplacementOverlapPolicy, Span, Transaction},
  Tendril,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
  pub transaction: Transaction,
  pub selection: Selection,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectionSpec {
  pub ranges: Vec<CursorRangeSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CursorRangeSpec {
  pub id: Option<u64>,
  pub anchor: usize,
  pub head: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TransactionSpec {
  pub replacements: Vec<ReplacementSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplacementSpec {
  pub start: usize,
  pub end: usize,
  pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SurfaceSnapshot {
  pub text: String,
  pub selection: SelectionSpec,
  pub active_cursor_id: Option<u64>,
}

pub fn selection_to_spec(selection: &Selection) -> SelectionSpec {
  SelectionSpec {
    ranges: selection
      .iter()
      .map(|cursor| CursorRangeSpec {
        id: Some(cursor.id.get()),
        anchor: cursor.range.anchor,
        head: cursor.range.head,
      })
      .collect(),
  }
}

pub fn selection_from_spec(spec: SelectionSpec) -> crate::selection::Result<Selection> {
  let cursors = spec
    .ranges
    .into_iter()
    .map(|range| CursorRange {
      id: range
        .id
        .and_then(NonZeroU64::new)
        .map(CursorId::new)
        .unwrap_or_else(CursorId::fresh),
      range: Range::new(range.anchor, range.head),
    })
    .collect();
  Selection::from_cursors(cursors)
}

pub fn transaction_from_spec(len: usize, spec: TransactionSpec) -> crate::transaction::Result<Transaction> {
  Transaction::from_replacements(
    len,
    spec.replacements.into_iter().map(|replacement| Replacement {
      span: Span {
        start: replacement.start,
        end: replacement.end,
      },
      text: replacement.text.map(Tendril::from),
    }),
    ReplacementOverlapPolicy::Reject,
  )
}
