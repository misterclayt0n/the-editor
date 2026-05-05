use ropey::RopeSlice;

use super::{
  doc_formatter::{
    DocumentFormatter,
    FormatterCheckpoint,
    Position,
  },
  text_format::TextFormat,
};
use crate::grapheme::Grapheme;

const WRAP_CHECKPOINT_ROW_STRIDE: usize = 8;
const WRAP_CHECKPOINT_CHAR_STRIDE: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LayoutSignature {
  wrap_width: usize,
  format:     TextFormat,
}

impl LayoutSignature {
  fn from_inputs(inputs: LayoutInputs<'_>) -> Self {
    Self {
      wrap_width: inputs.wrap_width,
      format:     inputs.format.clone(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextChangeSummary {
  pub(crate) start_line:   usize,
  pub(crate) old_end_line: usize,
  pub(crate) new_end_line: usize,
}

impl TextChangeSummary {
  fn merge(self, other: Self) -> Self {
    Self {
      start_line:   self.start_line.min(other.start_line),
      old_end_line: self.old_end_line.max(other.old_end_line),
      new_end_line: self.new_end_line.max(other.new_end_line),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineWrapCheckpoint {
  char_offset:  usize,
  row:          usize,
  col:          usize,
  indent_level: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LineLayoutMetrics {
  pub(crate) visual_row_count: usize,
  checkpoints:                 Vec<LineWrapCheckpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayoutStart {
  pub(crate) line_idx:            usize,
  pub(crate) line_doc_visual_row: usize,
  pub(crate) surface_row:         usize,
  pub(crate) checkpoint:          FormatterCheckpoint,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FenwickTree {
  tree: Vec<usize>,
}

impl FenwickTree {
  fn clear(&mut self) {
    self.tree.clear();
  }

  fn rebuild_from(values: impl IntoIterator<Item = usize>) -> Self {
    let values = values.into_iter().collect::<Vec<_>>();
    let mut fenwick = Self {
      tree: vec![0; values.len()],
    };
    for (index, value) in values.into_iter().enumerate() {
      fenwick.add(index, value as isize);
    }
    fenwick
  }

  fn prefix_sum(&self, end_exclusive: usize) -> usize {
    let mut sum = 0usize;
    let mut index = end_exclusive;
    while index > 0 {
      sum += self.tree[index - 1];
      index &= index - 1;
    }
    sum
  }

  fn add(&mut self, index: usize, delta: isize) {
    let mut index = index + 1;
    while index <= self.tree.len() {
      let slot = &mut self.tree[index - 1];
      if delta >= 0 {
        *slot = slot.saturating_add(delta as usize);
      } else {
        *slot = slot.saturating_sub((-delta) as usize);
      }
      index += index & (!index + 1);
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct LayoutIndex {
  signature:           Option<LayoutSignature>,
  text_revision:       Option<u64>,
  line_metrics:        Vec<LineLayoutMetrics>,
  visual_row_tree:     FenwickTree,
  pending_text_change: Option<TextChangeSummary>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LayoutInputs<'a> {
  pub(crate) text:          RopeSlice<'a>,
  pub(crate) text_revision: u64,
  pub(crate) format:        &'a TextFormat,
  pub(crate) wrap_width:    usize,
}

impl LayoutIndex {
  pub(crate) fn invalidate_all(&mut self) {
    self.signature = None;
    self.text_revision = None;
    self.line_metrics.clear();
    self.visual_row_tree.clear();
    self.pending_text_change = Some(TextChangeSummary {
      start_line:   0,
      old_end_line: 0,
      new_end_line: 0,
    });
  }

  pub(crate) fn note_text_change(&mut self, change: TextChangeSummary) {
    self.pending_text_change = Some(
      self
        .pending_text_change
        .map_or(change, |current| current.merge(change)),
    );
    self.text_revision = None;
  }

  pub(crate) fn ensure(&mut self, inputs: LayoutInputs<'_>) {
    let next_signature = LayoutSignature::from_inputs(inputs);
    let layout_changed = self.signature.as_ref() != Some(&next_signature);

    if layout_changed {
      self.signature = Some(next_signature);
      self.rebuild_full(inputs);
      self.text_revision = Some(inputs.text_revision);
      self.pending_text_change = None;
      return;
    }

    if self.text_revision == Some(inputs.text_revision) {
      return;
    }

    if !inputs.format.soft_wrap {
      self.line_metrics.clear();
      self.visual_row_tree.clear();
      self.text_revision = Some(inputs.text_revision);
      self.pending_text_change = None;
      return;
    }

    match self.pending_text_change.take() {
      Some(change) => self.apply_soft_wrap_change(inputs, change),
      None => self.rebuild_soft_wrap_metrics(inputs),
    }

    self.text_revision = Some(inputs.text_revision);
  }

  fn rebuild_full(&mut self, inputs: LayoutInputs<'_>) {
    if inputs.format.soft_wrap {
      self.rebuild_soft_wrap_metrics(inputs);
    } else {
      self.line_metrics.clear();
      self.visual_row_tree.clear();
    }
  }

  fn rebuild_soft_wrap_metrics(&mut self, inputs: LayoutInputs<'_>) {
    let line_count = inputs.text.len_lines().max(1);
    self.line_metrics = (0..line_count)
      .map(|line_idx| build_line_metrics(inputs, line_idx))
      .collect();
    self.rebuild_visual_row_tree();
  }

  fn apply_soft_wrap_change(&mut self, inputs: LayoutInputs<'_>, change: TextChangeSummary) {
    let line_count = inputs.text.len_lines().max(1);
    if self.line_metrics.is_empty() {
      self.rebuild_soft_wrap_metrics(inputs);
      return;
    }

    let start = change.start_line.min(line_count.saturating_sub(1));
    let remove_end = change
      .old_end_line
      .min(self.line_metrics.len().saturating_sub(1));
    let remove_len = if start <= remove_end {
      remove_end - start + 1
    } else {
      0
    };
    let insert_end = change.new_end_line.min(line_count.saturating_sub(1));
    let replacement = if start <= insert_end {
      (start..=insert_end)
        .map(|line_idx| build_line_metrics(inputs, line_idx))
        .collect::<Vec<_>>()
    } else {
      Vec::new()
    };

    self
      .line_metrics
      .splice(start..start.saturating_add(remove_len), replacement);
    self.rebuild_visual_row_tree();
  }

  fn rebuild_visual_row_tree(&mut self) {
    self.visual_row_tree = FenwickTree::rebuild_from(
      self
        .line_metrics
        .iter()
        .map(|metrics| metrics.visual_row_count),
    );
  }

  pub(crate) fn line_for_char(&self, text: RopeSlice<'_>, char_idx: usize) -> usize {
    text.char_to_line(char_idx.min(text.len_chars()))
  }

  pub(crate) fn doc_visual_row_for_line(&self, line_idx: usize) -> usize {
    if self.line_metrics.is_empty() {
      line_idx
    } else {
      self.visual_row_tree.prefix_sum(line_idx)
    }
  }

  pub(crate) fn surface_row_for_line(&self, line_idx: usize) -> usize {
    self.doc_visual_row_for_line(line_idx)
  }

  pub(crate) fn line_for_surface_row(&self, text: RopeSlice<'_>, surface_row: usize) -> usize {
    let line_count = text.len_lines().max(1);
    let mut lo = 0usize;
    let mut hi = line_count.saturating_sub(1);
    while lo < hi {
      let mid = lo + (hi - lo).div_ceil(2);
      if self.surface_row_for_line(mid) <= surface_row {
        lo = mid;
      } else {
        hi = mid - 1;
      }
    }
    lo
  }

  pub(crate) fn position_for_char(
    &mut self,
    inputs: LayoutInputs<'_>,
    char_idx: usize,
  ) -> Option<Position> {
    self.ensure(inputs);

    let text = inputs.text;
    let char_idx = char_idx.min(text.len_chars());
    let line_idx = self.line_for_char(text, char_idx);
    let line_position =
      position_in_line(self.line_metrics.get(line_idx), inputs, line_idx, char_idx)?;
    let base_row = self.surface_row_for_line(line_idx);
    Some(Position {
      row: base_row.saturating_add(line_position.row),
      col: line_position.col,
    })
  }

  pub(crate) fn visible_layout_start(
    &mut self,
    inputs: LayoutInputs<'_>,
    surface_row: usize,
  ) -> LayoutStart {
    self.ensure(inputs);
    let line_idx = self.line_for_surface_row(inputs.text, surface_row);
    let line_start_char = inputs.text.line_to_char(line_idx);
    let line_doc_visual_row = self.doc_visual_row_for_line(line_idx);

    let checkpoint = if inputs.format.soft_wrap {
      let target_doc_row = surface_row.saturating_sub(line_doc_visual_row);
      checkpoint_for_visible_start(
        self.line_metrics.get(line_idx),
        line_start_char,
        line_idx,
        target_doc_row,
      )
    } else {
      FormatterCheckpoint {
        char_idx: line_start_char,
        line_idx,
        visual_pos: Position::default(),
        indent_level: None,
      }
    };

    LayoutStart {
      line_idx,
      line_doc_visual_row,
      surface_row: line_doc_visual_row.saturating_add(checkpoint.visual_pos.row),
      checkpoint,
    }
  }
}

fn build_line_metrics(inputs: LayoutInputs<'_>, line_idx: usize) -> LineLayoutMetrics {
  let text = inputs.text;
  let line_start_char = text.line_to_char(line_idx);
  let mut formatter = DocumentFormatter::new_exact_at_char(
    text,
    inputs.format,
    inputs.wrap_width,
    line_start_char,
    line_idx,
  );

  let mut visual_row_count = 1usize;
  let mut checkpoints = Vec::new();
  let mut last_checkpoint_row = 0usize;
  let mut last_checkpoint_char_offset = 0usize;

  while let Some(grapheme) = formatter.next() {
    if grapheme.source.is_eof() || grapheme.line_idx != line_idx {
      break;
    }

    if grapheme.raw != Grapheme::Newline {
      visual_row_count = visual_row_count.max(grapheme.visual_pos.row.saturating_add(1));
    }

    if grapheme.is_virtual() || grapheme.raw == Grapheme::Newline {
      continue;
    }

    let Some(checkpoint) = formatter.checkpoint() else {
      continue;
    };
    if checkpoint.line_idx != line_idx {
      continue;
    }

    let char_offset = checkpoint.char_idx.saturating_sub(line_start_char);
    let row_advanced = checkpoint
      .visual_pos
      .row
      .saturating_sub(last_checkpoint_row);
    let chars_advanced = char_offset.saturating_sub(last_checkpoint_char_offset);
    if row_advanced < WRAP_CHECKPOINT_ROW_STRIDE && chars_advanced < WRAP_CHECKPOINT_CHAR_STRIDE {
      continue;
    }

    checkpoints.push(LineWrapCheckpoint {
      char_offset,
      row: checkpoint.visual_pos.row,
      col: checkpoint.visual_pos.col,
      indent_level: checkpoint.indent_level,
    });
    last_checkpoint_row = checkpoint.visual_pos.row;
    last_checkpoint_char_offset = char_offset;
  }

  LineLayoutMetrics {
    visual_row_count,
    checkpoints,
  }
}

fn checkpoint_for_visible_start(
  metrics: Option<&LineLayoutMetrics>,
  line_start_char: usize,
  line_idx: usize,
  target_doc_row: usize,
) -> FormatterCheckpoint {
  let checkpoint = metrics
    .and_then(|metrics| {
      metrics
        .checkpoints
        .iter()
        .rev()
        .find(|checkpoint| checkpoint.row < target_doc_row)
        .copied()
    })
    .unwrap_or(LineWrapCheckpoint {
      char_offset:  0,
      row:          0,
      col:          0,
      indent_level: None,
    });

  FormatterCheckpoint {
    char_idx: line_start_char + checkpoint.char_offset,
    line_idx,
    visual_pos: Position::new(checkpoint.row, checkpoint.col),
    indent_level: checkpoint.indent_level,
  }
}

fn checkpoint_for_char_position(
  metrics: Option<&LineLayoutMetrics>,
  line_start_char: usize,
  line_idx: usize,
  char_idx: usize,
) -> FormatterCheckpoint {
  let target_offset = char_idx.saturating_sub(line_start_char);
  let checkpoint = metrics
    .and_then(|metrics| {
      metrics
        .checkpoints
        .iter()
        .rev()
        .find(|checkpoint| checkpoint.char_offset <= target_offset)
        .copied()
    })
    .unwrap_or(LineWrapCheckpoint {
      char_offset:  0,
      row:          0,
      col:          0,
      indent_level: None,
    });

  FormatterCheckpoint {
    char_idx: line_start_char + checkpoint.char_offset,
    line_idx,
    visual_pos: Position::new(checkpoint.row, checkpoint.col),
    indent_level: checkpoint.indent_level,
  }
}

fn position_in_line(
  metrics: Option<&LineLayoutMetrics>,
  inputs: LayoutInputs<'_>,
  line_idx: usize,
  char_idx: usize,
) -> Option<Position> {
  let text = inputs.text;
  let line_start = text.line_to_char(line_idx);
  let checkpoint = checkpoint_for_char_position(metrics, line_start, line_idx, char_idx);
  let mut formatter = if checkpoint.char_idx == line_start
    && checkpoint.visual_pos == Position::default()
    && checkpoint.indent_level.is_none()
  {
    DocumentFormatter::new_exact_at_char(
      text,
      inputs.format,
      inputs.wrap_width,
      line_start,
      line_idx,
    )
  } else {
    DocumentFormatter::new_at_checkpoint(text, inputs.format, inputs.wrap_width, checkpoint)
  };

  let mut last_row = checkpoint.visual_pos.row;
  let mut last_col = checkpoint.visual_pos.col;

  while let Some(grapheme) = formatter.next() {
    if grapheme.source.is_eof() || grapheme.line_idx != line_idx {
      return Some(Position {
        row: last_row,
        col: last_col,
      });
    }

    if grapheme.char_idx == char_idx && !grapheme.is_virtual() {
      return Some(grapheme.visual_pos);
    }

    if grapheme.raw != Grapheme::Newline {
      last_row = grapheme.visual_pos.row;
      last_col = grapheme.visual_pos.col + grapheme.width();
      if !grapheme.is_virtual() && grapheme.char_idx + grapheme.doc_chars() == char_idx {
        return Some(Position {
          row: grapheme.visual_pos.row,
          col: last_col,
        });
      }
    }
  }

  Some(Position {
    row: last_row,
    col: last_col,
  })
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn visible_rows_use_prefix_sums() {
    let rope = Rope::from_str("abcdefghij\nklmnopqrst\n");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let mut index = LayoutIndex::default();
    index.ensure(LayoutInputs {
      text:          rope.slice(..),
      text_revision: 1,
      format:        &format,
      wrap_width:    8,
    });

    assert_eq!(index.doc_visual_row_for_line(0), 0);
    assert_eq!(
      index.doc_visual_row_for_line(1),
      index.line_metrics[0].visual_row_count
    );
  }

  #[test]
  fn incremental_same_line_change_updates_only_that_line_count() {
    let rope = Rope::from_str("abcdefghij\nklmnopqrst\n");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let mut index = LayoutIndex::default();
    index.ensure(LayoutInputs {
      text:          rope.slice(..),
      text_revision: 1,
      format:        &format,
      wrap_width:    8,
    });
    let original_second = index.line_metrics[1].visual_row_count;

    let edited = Rope::from_str("abc\nklmnopqrst\n");
    index.note_text_change(TextChangeSummary {
      start_line:   0,
      old_end_line: 0,
      new_end_line: 0,
    });
    index.ensure(LayoutInputs {
      text:          edited.slice(..),
      text_revision: 2,
      format:        &format,
      wrap_width:    8,
    });

    assert_eq!(index.line_metrics[1].visual_row_count, original_second);
  }
}
