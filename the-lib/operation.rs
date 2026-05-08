use std::num::NonZeroU64;

use ropey::{
  Rope,
  RopeSlice,
};
use smallvec::SmallVec;
use thiserror::Error;

pub use crate::editing::{
  Edit,
  InsertNewlineOptions,
  OpenLine,
};
use crate::{
  Tendril,
  editing::{
    self,
    EditingError,
  },
  indent::IndentStyle,
  match_brackets,
  movement::{
    self,
    MotionMode,
    MotionResult,
    VisualGoals,
  },
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
    SelectionError,
  },
  syntax::{
    IndentQuery,
    LanguageRegistry,
    Syntax,
    SyntaxError,
  },
  text_object::{
    self,
    TextObjectKind,
  },
  transaction::{
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
    TransactionError,
  },
};

#[derive(Debug, Error)]
pub enum OperationError {
  #[error(transparent)]
  Editing(#[from] EditingError),

  #[error(transparent)]
  Selection(#[from] SelectionError),

  #[error(transparent)]
  Transaction(#[from] TransactionError),

  #[error(transparent)]
  Syntax(#[from] SyntaxError),

  #[error("unknown operation {0}")]
  UnknownOperation(String),
}

pub type Result<T> = std::result::Result<T, OperationError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterWrite {
  pub name:   char,
  pub values: Vec<Tendril>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationAtom {
  Selection(Selection),
  Motion(MotionResult),
  Edit(Edit),
  RegisterWrite(RegisterWrite),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OperationOutput {
  pub atoms: Vec<OperationAtom>,
}

impl OperationOutput {
  pub fn none() -> Self {
    Self::default()
  }

  pub fn atom(atom: OperationAtom) -> Self {
    Self { atoms: vec![atom] }
  }

  pub fn atoms(atoms: impl Into<Vec<OperationAtom>>) -> Self {
    Self {
      atoms: atoms.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OperationSpec {
  pub name:         String,
  pub count:        Option<usize>,
  pub text:         Option<String>,
  pub values:       Option<Vec<String>>,
  pub after:        Option<bool>,
  pub line_ending:  Option<String>,
  pub indent:       Option<String>,
  pub indent_unit:  Option<String>,
  pub indent_width: Option<usize>,
  pub tab_width:    Option<usize>,
}

pub fn execute_operation(
  doc: &Rope,
  selection: &Selection,
  visual_goals: &VisualGoals,
  spec: OperationSpec,
  line_ending: &str,
  indent_style: &IndentStyle,
  indent_query: Option<&IndentQuery>,
  syntax: Option<&Syntax>,
  registry: Option<&LanguageRegistry>,
) -> Result<OperationOutput> {
  let text = doc.slice(..);
  let count = spec.count.unwrap_or(1).max(1);

  match spec.name.as_str() {
    "move_char_left" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_char_left(text, selection, count),
      )))
    },
    "move_char_right" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_char_right(text, selection, count),
      )))
    },
    "extend_char_left" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_char_left(text, selection, count),
      )))
    },
    "extend_char_right" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_char_right(text, selection, count),
      )))
    },
    "move_visual_line_up" => {
      Ok(OperationOutput::atom(OperationAtom::Motion(
        movement::move_visual_line_up(text, selection, visual_goals, count),
      )))
    },
    "move_visual_line_down" => {
      Ok(OperationOutput::atom(OperationAtom::Motion(
        movement::move_visual_line_down(text, selection, visual_goals, count),
      )))
    },
    "extend_visual_line_up" => {
      Ok(OperationOutput::atom(OperationAtom::Motion(
        movement::extend_visual_line_up(text, selection, visual_goals, count),
      )))
    },
    "extend_visual_line_down" => {
      Ok(OperationOutput::atom(OperationAtom::Motion(
        movement::extend_visual_line_down(text, selection, visual_goals, count),
      )))
    },

    "move_next_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_next_word_start_selection(text, selection, count),
      )))
    },
    "move_prev_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_prev_word_start_selection(text, selection, count),
      )))
    },
    "move_next_word_end" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_next_word_end_selection(text, selection, count),
      )))
    },
    "move_next_long_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_next_long_word_start_selection(text, selection, count),
      )))
    },
    "move_prev_long_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_prev_long_word_start_selection(text, selection, count),
      )))
    },
    "move_next_long_word_end" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::move_next_long_word_end_selection(text, selection, count),
      )))
    },
    "extend_next_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_next_word_start(text, selection, count),
      )))
    },
    "extend_prev_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_prev_word_start(text, selection, count),
      )))
    },
    "extend_next_word_end" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_next_word_end(text, selection, count),
      )))
    },
    "extend_next_long_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_next_long_word_start(text, selection, count),
      )))
    },
    "extend_prev_long_word_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_prev_long_word_start(text, selection, count),
      )))
    },
    "extend_next_long_word_end" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_next_long_word_end(text, selection, count),
      )))
    },

    "goto_line_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_line_start(text, selection, MotionMode::Move),
      )))
    },
    "goto_line_end" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_line_end(text, selection, MotionMode::Move),
      )))
    },
    "goto_line_end_newline" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_line_end_newline(text, selection, MotionMode::Move),
      )))
    },
    "goto_first_nonwhitespace" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_first_non_whitespace(text, selection, MotionMode::Move),
      )))
    },
    "extend_to_first_nonwhitespace" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_first_non_whitespace(text, selection, MotionMode::Extend),
      )))
    },
    "goto_document_start" | "goto_file_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_document_start(text, selection, MotionMode::Move),
      )))
    },
    "goto_document_end" | "goto_last_line" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_document_end(text, selection, MotionMode::Move),
      )))
    },
    "extend_to_line_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_line_start(text, selection, MotionMode::Extend),
      )))
    },
    "extend_to_line_end" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_line_end(text, selection, MotionMode::Extend),
      )))
    },
    "extend_to_file_start" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_document_start(text, selection, MotionMode::Extend),
      )))
    },
    "extend_to_file_end" | "extend_to_last_line" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::goto_document_end(text, selection, MotionMode::Extend),
      )))
    },

    "select_all" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(select_all(
        text,
      ))))
    },
    "collapse_selection" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        collapse_selection(text, selection),
      )))
    },
    "flip_selections" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        flip_selections(selection),
      )))
    },
    "merge_selections" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        merge_selections(selection),
      )))
    },
    "merge_consecutive_selections" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        merge_consecutive_selections(selection),
      )))
    },
    "extend_line_below" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_line_below(text, selection, count),
      )))
    },
    "extend_line_above" | "extend_line_up" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        movement::extend_line_above(text, selection, count),
      )))
    },
    "extend_to_line_bounds" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        extend_to_line_bounds(text, selection),
      )))
    },
    "shrink_to_line_bounds" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        shrink_to_line_bounds(text, selection),
      )))
    },
    "match_brackets" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        match_brackets_selection(text, selection, syntax, false),
      )))
    },
    "extend_match_brackets" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        match_brackets_selection(text, selection, syntax, true),
      )))
    },
    "select_word_around" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::word(
          text,
          selection.clone(),
          TextObjectKind::Around,
          count,
          false,
        ),
      )))
    },
    "select_word_inner" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::word(
          text,
          selection.clone(),
          TextObjectKind::Inside,
          count,
          false,
        ),
      )))
    },
    "select_long_word_around" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::word(text, selection.clone(), TextObjectKind::Around, count, true),
      )))
    },
    "select_long_word_inner" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::word(text, selection.clone(), TextObjectKind::Inside, count, true),
      )))
    },
    "select_paragraph_around" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::paragraph(text, selection.clone(), TextObjectKind::Around, count),
      )))
    },
    "select_paragraph_inner" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::paragraph(text, selection.clone(), TextObjectKind::Inside, count),
      )))
    },
    "select_closest_pair_around" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::closest_pair(
          syntax,
          text,
          selection.clone(),
          TextObjectKind::Around,
          count,
        ),
      )))
    },
    "select_closest_pair_inner" => {
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::closest_pair(
          syntax,
          text,
          selection.clone(),
          TextObjectKind::Inside,
          count,
        ),
      )))
    },
    "select_pair_around" => {
      let Some(ch) = spec.text.as_deref().and_then(|text| text.chars().next()) else {
        return Ok(OperationOutput::none());
      };
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::pair(
          syntax,
          text,
          selection.clone(),
          TextObjectKind::Around,
          ch,
          count,
        ),
      )))
    },
    "select_pair_inner" => {
      let Some(ch) = spec.text.as_deref().and_then(|text| text.chars().next()) else {
        return Ok(OperationOutput::none());
      };
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::pair(
          syntax,
          text,
          selection.clone(),
          TextObjectKind::Inside,
          ch,
          count,
        ),
      )))
    },
    "select_syntax_object_around" => {
      let Some(object_name) = spec.text.as_deref() else {
        return Ok(OperationOutput::none());
      };
      let (Some(syntax), Some(registry)) = (syntax, registry) else {
        return Ok(OperationOutput::atom(OperationAtom::Selection(
          selection.clone(),
        )));
      };
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::syntax_named(
          syntax,
          registry,
          text,
          selection.clone(),
          TextObjectKind::Around,
          object_name,
        )?,
      )))
    },
    "select_syntax_object_inner" => {
      let Some(object_name) = spec.text.as_deref() else {
        return Ok(OperationOutput::none());
      };
      let (Some(syntax), Some(registry)) = (syntax, registry) else {
        return Ok(OperationOutput::atom(OperationAtom::Selection(
          selection.clone(),
        )));
      };
      Ok(OperationOutput::atom(OperationAtom::Selection(
        text_object::syntax_named(
          syntax,
          registry,
          text,
          selection.clone(),
          TextObjectKind::Inside,
          object_name,
        )?,
      )))
    },

    "insert_text" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::insert_text(doc, selection, spec.text.unwrap_or_default())?,
      )))
    },
    "insert_tab" | "smart_tab" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::insert_tab(doc, selection, spec.text.as_deref().unwrap_or("\t"))?,
      )))
    },
    "insert_newline" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::insert_newline(
          doc,
          selection,
          InsertNewlineOptions::new(
            spec.line_ending.as_deref().unwrap_or(line_ending),
            spec.indent.as_deref().unwrap_or(""),
            spec.indent_unit.as_deref().unwrap_or(indent_style.as_str()),
          ),
        )?,
      )))
    },
    "open_above" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::open_line(
          doc,
          selection,
          OpenLine::Above,
          spec.line_ending.as_deref().unwrap_or(line_ending),
          count,
          indent_query,
          syntax,
          indent_style,
          spec.tab_width.unwrap_or(4),
        )?,
      )))
    },
    "open_below" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::open_line(
          doc,
          selection,
          OpenLine::Below,
          spec.line_ending.as_deref().unwrap_or(line_ending),
          count,
          indent_query,
          syntax,
          indent_style,
          spec.tab_width.unwrap_or(4),
        )?,
      )))
    },
    "surround_add" => {
      let Some(values) = spec.values else {
        return Ok(OperationOutput::none());
      };
      let Some(open) = values.first() else {
        return Ok(OperationOutput::none());
      };
      let Some(close) = values.get(1) else {
        return Ok(OperationOutput::none());
      };
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::surround_add(doc, selection, open.clone(), close.clone())?,
      )))
    },
    "surround_delete" => {
      let surround_ch = spec.text.as_deref().and_then(|text| text.chars().next());
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::surround_delete(doc, selection, syntax, surround_ch, count)?,
      )))
    },
    "surround_replace" => {
      let Some(target) = spec.text.as_deref().and_then(|text| text.chars().next()) else {
        return Ok(OperationOutput::none());
      };
      let Some(values) = spec.values else {
        return Ok(OperationOutput::none());
      };
      let Some(open) = values.first() else {
        return Ok(OperationOutput::none());
      };
      let Some(close) = values.get(1) else {
        return Ok(OperationOutput::none());
      };
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::surround_replace(
          doc,
          selection,
          syntax,
          target,
          open.clone(),
          close.clone(),
          count,
        )?,
      )))
    },

    "delete_char_backward" | "delete_backward" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::delete_backward(doc, selection, count)?,
      )))
    },
    "delete_char_forward" | "delete_forward" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::delete_forward(doc, selection, count)?,
      )))
    },
    "delete_selection" => {
      delete_with_yank(
        editing::delete_selection(doc, selection)?,
        '"',
        editing::yank_deleted_text(doc, selection),
      )
    },
    "delete_selection_noyank" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::delete_selection(doc, selection)?,
      )))
    },
    "delete_word_backward" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::delete_word_backward(doc, selection, count)?,
      )))
    },
    "delete_word_forward" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::delete_word_forward(doc, selection, count)?,
      )))
    },
    "kill_to_line_start" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::kill_to_line_start(doc, selection)?,
      )))
    },
    "kill_to_line_end" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::kill_to_line_end(doc, selection)?,
      )))
    },

    "yank" => {
      Ok(OperationOutput::atom(OperationAtom::RegisterWrite(
        RegisterWrite {
          name:   '"',
          values: editing::yank_selection(doc, selection),
        },
      )))
    },
    "yank_main_selection" => {
      Ok(OperationOutput::atom(OperationAtom::RegisterWrite(
        RegisterWrite {
          name:   '"',
          values: editing::yank_main_selection(doc, selection),
        },
      )))
    },
    "paste" => {
      let Some(values) = spec.values else {
        return Ok(OperationOutput::none());
      };
      let Some(edit) = editing::paste(
        doc,
        selection,
        &values,
        spec.after.unwrap_or(true),
        true,
        spec.line_ending.as_deref().unwrap_or(line_ending),
      )?
      else {
        return Ok(OperationOutput::none());
      };
      Ok(OperationOutput::atom(OperationAtom::Edit(edit)))
    },

    "switch_case" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::switch_case(doc, selection)?,
      )))
    },
    "switch_to_uppercase" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::switch_to_uppercase(doc, selection)?,
      )))
    },
    "switch_to_lowercase" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::switch_to_lowercase(doc, selection)?,
      )))
    },
    "indent" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(editing::indent(
        doc,
        selection,
        spec.text.as_deref().unwrap_or(indent_style.as_str()),
        count,
      )?)))
    },
    "unindent" => {
      Ok(OperationOutput::atom(OperationAtom::Edit(
        editing::unindent(
          doc,
          selection,
          spec.indent_width.unwrap_or(2) * count,
          spec.tab_width.unwrap_or(4),
        )?,
      )))
    },

    other => Err(OperationError::UnknownOperation(other.to_string())),
  }
}

fn delete_with_yank(edit: Edit, name: char, values: Vec<Tendril>) -> Result<OperationOutput> {
  Ok(OperationOutput::atoms(vec![
    OperationAtom::RegisterWrite(RegisterWrite { name, values }),
    OperationAtom::Edit(edit),
  ]))
}

fn match_brackets_selection(
  text: RopeSlice,
  selection: &Selection,
  syntax: Option<&Syntax>,
  extend: bool,
) -> Selection {
  selection.clone().transform(|range| {
    let pos = range.cursor(text);
    let matched = syntax
      .and_then(|syntax| match_brackets::find_matching_bracket_fuzzy(syntax, text, pos))
      .or_else(|| match_brackets::find_matching_bracket_plaintext(text, pos));
    matched
      .map(|matched| range.put_cursor(text, matched, extend))
      .unwrap_or(range)
  })
}

pub fn select_all(text: RopeSlice) -> Selection {
  Selection::single(0, text.len_chars())
}

pub fn collapse_selection(text: RopeSlice, selection: &Selection) -> Selection {
  selection
    .clone()
    .transform(|range| Range::point(range.cursor(text)))
}

pub fn flip_selections(selection: &Selection) -> Selection {
  selection.clone().transform(|range| range.flip())
}

pub fn merge_selections(selection: &Selection) -> Selection {
  selection.clone().merge_ranges()
}

pub fn merge_consecutive_selections(selection: &Selection) -> Selection {
  selection.clone().merge_consecutive_ranges()
}

pub fn extend_to_line_bounds(text: RopeSlice, selection: &Selection) -> Selection {
  selection.clone().transform(|range| {
    let (start_line, end_line) = range.line_range(text);
    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));
    Range::new(start, end).with_direction(range.direction())
  })
}

pub fn shrink_to_line_bounds(text: RopeSlice, selection: &Selection) -> Selection {
  selection.clone().transform(|range| {
    let (start_line, end_line) = range.line_range(text);
    if start_line == end_line {
      return range;
    }

    let mut start = text.line_to_char(start_line);
    let mut end = text.line_to_char((end_line + 1).min(text.len_lines()));

    if start != range.from() {
      start = text.line_to_char((start_line + 1).min(text.len_lines()));
    }
    if end != range.to() {
      end = text.line_to_char(end_line);
    }

    Range::new(start, end).with_direction(range.direction())
  })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectionSpec {
  pub ranges: Vec<CursorRangeSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CursorRangeSpec {
  pub id:     Option<u64>,
  pub anchor: usize,
  pub head:   usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TransactionSpec {
  pub replacements: Vec<ReplacementSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplacementSpec {
  pub start: usize,
  pub end:   usize,
  pub text:  Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SurfaceSnapshot {
  pub text:             String,
  pub selection:        SelectionSpec,
  pub active_cursor_id: Option<u64>,
}

pub fn selection_to_spec(selection: &Selection) -> SelectionSpec {
  SelectionSpec {
    ranges: selection
      .iter()
      .map(|cursor| {
        CursorRangeSpec {
          id:     Some(cursor.id.get()),
          anchor: cursor.range.anchor,
          head:   cursor.range.head,
        }
      })
      .collect(),
  }
}

pub fn selection_from_spec(spec: SelectionSpec) -> crate::selection::Result<Selection> {
  let cursors: SmallVec<[CursorRange; 1]> = spec
    .ranges
    .into_iter()
    .map(|range| {
      CursorRange {
        id:    range
          .id
          .and_then(NonZeroU64::new)
          .map(CursorId::new)
          .unwrap_or_else(CursorId::fresh),
        range: Range::new(range.anchor, range.head),
      }
    })
    .collect();
  Selection::from_cursors(cursors)
}

pub fn transaction_from_spec(
  len: usize,
  spec: TransactionSpec,
) -> crate::transaction::Result<Transaction> {
  Transaction::from_replacements(
    len,
    spec.replacements.into_iter().map(|replacement| {
      Replacement {
        span: Span {
          start: replacement.start,
          end:   replacement.end,
        },
        text: replacement.text.map(Tendril::from),
      }
    }),
    ReplacementOverlapPolicy::Reject,
  )
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn move_and_extend_char_are_selection_only() {
    let doc = Rope::from("abc");
    let selection = Selection::point(1);

    assert_eq!(
      movement::move_char_right(doc.slice(..), &selection, 1).ranges(),
      &[Range::point(2)]
    );
    assert_eq!(
      movement::extend_char_right(doc.slice(..), &selection, 1).ranges(),
      &[Range::new(1, 3)]
    );
  }

  #[test]
  fn word_motions_use_helix_block_selection_semantics() {
    let doc = Rope::from("hello world");

    let selection = Selection::point(0);
    assert_eq!(
      movement::move_next_word_start_selection(doc.slice(..), &selection, 1).ranges(),
      &[Range::new(0, 6)]
    );
    assert_eq!(
      movement::move_next_word_end_selection(doc.slice(..), &selection, 1).ranges(),
      &[Range::new(0, 5)]
    );

    let selection = Selection::point(6);
    assert_eq!(
      movement::move_prev_word_start_selection(doc.slice(..), &selection, 1).ranges(),
      &[Range::new(6, 0)]
    );
  }

  #[test]
  fn extend_word_motions_extend_from_original_anchor() {
    let doc = Rope::from("hello world");
    let selection = Selection::point(0);

    assert_eq!(
      movement::extend_next_word_start(doc.slice(..), &selection, 1).ranges(),
      &[Range::new(0, 6)]
    );
  }

  #[test]
  fn delete_selection_outputs_register_write_and_edit() {
    let doc = Rope::from("hello");
    let selection = Selection::single(0, 5);

    let output = execute_operation(
      &doc,
      &selection,
      &VisualGoals::new(),
      OperationSpec {
        name:         "delete_selection".into(),
        count:        None,
        text:         None,
        values:       None,
        after:        None,
        line_ending:  None,
        indent:       None,
        indent_unit:  None,
        indent_width: None,
        tab_width:    None,
      },
      "\n",
      &IndentStyle::Spaces(2),
      None,
      None,
      None,
    )
    .unwrap();

    assert!(matches!(
      output.atoms.first(),
      Some(OperationAtom::RegisterWrite(RegisterWrite { name: '"', values })) if values == &vec![Tendril::from("hello")]
    ));
    assert!(matches!(output.atoms.get(1), Some(OperationAtom::Edit(_))));
  }

  #[test]
  fn yank_outputs_default_register_write() {
    let doc = Rope::from("hello");
    let selection = Selection::single(0, 5);

    let output = execute_operation(
      &doc,
      &selection,
      &VisualGoals::new(),
      OperationSpec {
        name:         "yank".into(),
        count:        None,
        text:         None,
        values:       None,
        after:        None,
        line_ending:  None,
        indent:       None,
        indent_unit:  None,
        indent_width: None,
        tab_width:    None,
      },
      "\n",
      &IndentStyle::Spaces(2),
      None,
      None,
      None,
    )
    .unwrap();

    assert_eq!(
      output,
      OperationOutput::atom(OperationAtom::RegisterWrite(RegisterWrite {
        name:   '"',
        values: vec![Tendril::from("hello")],
      }))
    );
  }
}
