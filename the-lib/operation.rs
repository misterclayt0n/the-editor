use crate::{
  document::Document,
  grapheme::{
    next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  indent::indent_for_newline,
  movement::{
    Direction,
    MotionMode,
    MovementContext,
    WordKind,
    WordTarget,
    transform_selection,
  },
  selection::{
    Range as SelectionRange,
    Selection,
  },
  surface::Surface,
  syntax::IndentationHeuristic,
  transaction::{
    DeletionOverlapPolicy,
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
  },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
  pub transaction: Transaction,
  pub selection:   Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SelectionTransform {
  Horizontal {
    direction: Direction,
    count:     usize,
    mode:      MotionMode,
  },
  Vertical {
    direction: Direction,
    count:     usize,
    mode:      MotionMode,
  },
  VisualVertical {
    direction: Direction,
    count:     usize,
    mode:      MotionMode,
  },
  Word {
    direction: Direction,
    kind:      WordKind,
    target:    WordTarget,
    count:     usize,
    mode:      MotionMode,
  },
  Paragraph {
    direction: Direction,
    count:     usize,
    mode:      MotionMode,
  },
  LineStart {
    mode: MotionMode,
  },
  LineEnd {
    mode: MotionMode,
  },
  FirstNonWhitespace {
    mode: MotionMode,
  },
  DocumentStart {
    mode: MotionMode,
  },
  DocumentEnd {
    mode: MotionMode,
  },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SurfaceOperation {
  TransformSelection {
    transform: SelectionTransform,
  },
  InsertText {
    text: String,
  },
  InsertNewline,
  DeleteBackward,
  DeleteForward,
}

pub fn apply_surface_operation(
  surface: &mut Surface,
  document: &mut Document,
  operation: SurfaceOperation,
) -> crate::surface::Result<()> {
  match operation {
    SurfaceOperation::TransformSelection { transform } => {
      let viewport = surface.viewport();
      let result = transform_selection(
        document.text().slice(..),
        surface.selection().clone(),
        surface.visual_goals(),
        transform,
        &MovementContext {
          text_format: surface.text_format(),
          annotations: surface.text_annotations(),
          wrap_width:  viewport.wrap_width,
        },
      );
      surface.apply_motion_result(document, result)
    },
    SurfaceOperation::InsertText { text } => insert_text(surface, document, &text),
    SurfaceOperation::InsertNewline => insert_newline(surface, document),
    SurfaceOperation::DeleteBackward => delete_backward(surface, document),
    SurfaceOperation::DeleteForward => delete_forward(surface, document),
  }
}

fn insert_text(surface: &mut Surface, document: &mut Document, text: &str) -> crate::surface::Result<()> {
  let range = surface.primary_range();
  let span = Span {
    start: range.from(),
    end:   range.to(),
  };
  let tx = Transaction::from_replacements(
    document.text().len_chars(),
    [Replacement::replace(span, text)],
    ReplacementOverlapPolicy::Reject,
  )
  .expect("insert text transaction should be valid");
  let selection = surface
    .selection()
    .clone()
    .transform(|_| SelectionRange::point(span.start + text.chars().count()));
  surface.apply_edit(document, Edit {
    transaction: tx,
    selection,
  })
}

fn insert_newline(surface: &mut Surface, document: &mut Document) -> crate::surface::Result<()> {
  let text = document.text().slice(..);
  let cursor = surface.primary_range().cursor(text);
  let line = text.char_to_line(cursor);
  let indent = indent_for_newline(
    None,
    None,
    &IndentationHeuristic::Simple,
    &document.indent_style(),
    surface.text_format().tab_width as usize,
    text,
    line,
    cursor,
    line,
  );
  let newline = format!("{}{}", document.line_ending().as_str(), indent);
  insert_text(surface, document, &newline)
}

fn delete_backward(surface: &mut Surface, document: &mut Document) -> crate::surface::Result<()> {
  let range = surface.primary_range();
  if !range.is_empty() {
    return delete_span(surface, document, range.from(), range.to());
  }

  if range.head == 0 {
    return Ok(());
  }

  let text = document.text().slice(..);
  let start = prev_grapheme_boundary(text, range.head);
  delete_span(surface, document, start, range.head)
}

fn delete_forward(surface: &mut Surface, document: &mut Document) -> crate::surface::Result<()> {
  let range = surface.primary_range();
  if !range.is_empty() {
    return delete_span(surface, document, range.from(), range.to());
  }

  let text = document.text().slice(..);
  if range.head >= text.len_chars() {
    return Ok(());
  }

  let end = next_grapheme_boundary(text, range.head);
  delete_span(surface, document, range.head, end)
}

fn delete_span(
  surface: &mut Surface,
  document: &mut Document,
  start: usize,
  end: usize,
) -> crate::surface::Result<()> {
  let tx = Transaction::delete_spans(
    document.text().len_chars(),
    [Span { start, end }],
    DeletionOverlapPolicy::Reject,
  )
  .expect("delete span transaction should be valid");
  let selection = surface
    .selection()
    .clone()
    .transform(|_| SelectionRange::point(start));
  surface.apply_edit(document, Edit {
    transaction: tx,
    selection,
  })
}
