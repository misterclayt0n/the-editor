use std::{cmp::Ordering, collections::BTreeMap, fmt::Debug, ops::Range};

use unicode_segmentation::UnicodeSegmentation;

use crate::{
  Tendril,
  position::Position,
  render::FormattedGrapheme,
  syntax::{Highlight, OverlayHighlights},
};

const NO_ANCHOR: usize = usize::MAX;

/// An inline annotation is continuous text shown
/// on the screen before the grapheme that starts at
/// `char_idx`.
#[derive(Debug, Clone)]
pub struct InlineAnnotation {
  pub text: Tendril,
  pub char_idx: usize,
}

impl InlineAnnotation {
  pub fn new(char_idx: usize, text: impl Into<Tendril>) -> Self {
    let text = text.into();
    debug_assert!(
      !text.contains('\n') && !text.contains('\r'),
      "inline annotations must not contain line breaks"
    );
    Self { char_idx, text }
  }
}

/// Represents a **single grapheme** that is part of the document
/// that starts at `char_idx` and will be replaced with a different
/// `grapheme`.
///
/// If `grapheme` contains multiple graphemes the text
/// will render incorrectly.
/// If you want to overlay multiple graphemes simply
/// use multiple `Overlay`s.
///
/// # Examples
///
/// The following examples are valid overlays for the following text:
///
/// `aX͎̊͢͜͝͡bc`
///
/// ```
/// use the_lib::render::text_annotations::Overlay;
///
/// // replaces a
/// Overlay::new(0, "X");
///
/// // replaces X͎̊͢͜͝͡
/// Overlay::new(1, "\t");
///
/// // replaces b
/// Overlay::new(6, "X̢̢̟͖̲͌̋̇͑͝");
/// ```
///
/// The following examples are invalid uses
///
/// ```
/// use the_lib::render::text_annotations::Overlay;
///
/// // overlay is not aligned at grapheme boundary
/// Overlay::new(3, "x");
///
/// // overlay contains multiple graphemes
/// Overlay::new(0, "xy");
/// ```
#[derive(Debug, Clone)]
pub struct Overlay {
  pub char_idx: usize,
  pub grapheme: Tendril,
}

impl Overlay {
  pub fn new(char_idx: usize, grapheme: impl Into<Tendril>) -> Self {
    let grapheme = grapheme.into();
    debug_assert!(is_single_grapheme(&grapheme));
    debug_assert!(
      !grapheme.contains('\n') && !grapheme.contains('\r'),
      "overlay graphemes must not contain line breaks"
    );
    Self { char_idx, grapheme }
  }
}

/// Line annotations allow inserting virtual text lines between normal text
/// lines. These lines can be filled with text in the rendering code as their
/// contents have no effect beyond visual appearance.
///
/// The height of virtual text is usually not known ahead of time as virtual
/// text often requires soft wrapping. Furthermore the height of some virtual
/// text like side-by-side diffs depends on the height of the text (again
/// influenced by soft wrapping) and other virtual text. Therefore line
/// annotations are computed on the fly instead of ahead of time like other
/// annotations.
///
/// The core of this trait is the `insert_virtual_lines` function. It is called
/// at the end of every visual line and allows the `LineAnnotation` to insert
/// empty virtual lines. Apart from that, the `LineAnnotation` trait has multiple
/// methods that allow it to track anchors in the document.
///
/// When a new traversal of a document starts `reset_pos` is called. Afterwards
/// the other functions are called with indices that are larger than the one
/// passed to `reset_pos`. This allows performing a binary search (use
/// `partition_point`) in `reset_pos` once and then to only look at the next
/// anchor during each method call.
///
/// The `reset_pos`, `skip_concealed_anchors` and `process_anchor` functions all
/// return a `char_idx` anchor. This anchor is stored when traversing the
/// document and when the grapheme at the anchor is traversed the
/// `process_anchor` function is called.
///
/// # Note
///
/// All functions receive a mutable reference to `self`. This lets line
/// annotations keep internal traversal state without resorting to interior
/// mutability. If you need to share a line annotation across contexts, use
/// interior mutability explicitly.
pub trait LineAnnotation {
  /// Resets the internal position to `char_idx`. This function is called
  /// when a new traversal of a document starts.
  ///
  /// All `char_idx` passed to `insert_virtual_lines` are strictly monotonically
  /// increasing with the first `char_idx` greater or equal to the `char_idx`
  /// passed to this function.
  ///
  /// # Returns
  ///
  /// The `char_idx` of the next anchor this `LineAnnotation` is interested in,
  /// replaces the currently registered anchor. Return `usize::MAX` to ignore.
  fn reset_pos(&mut self, _char_idx: usize) -> usize {
    NO_ANCHOR
  }

  /// Called when a text is concealed that contains an anchor registered by this
  /// `LineAnnotation`. In this case the line decorations **must** ensure that
  /// virtual text anchored within that char range is skipped.
  ///
  /// # Returns
  ///
  /// The `char_idx` of the next anchor this `LineAnnotation` is interested in,
  /// **after the end of conceal_end_char_idx**, replaces the currently
  /// registered anchor. Return `usize::MAX` to ignore.
  fn skip_concealed_anchors(&mut self, conceal_end_char_idx: usize) -> usize {
    self.reset_pos(conceal_end_char_idx)
  }

  /// Process an anchor (horizontal position is provided) and return the next
  /// anchor.
  ///
  /// # Returns
  ///
  /// The `char_idx` of the next anchor this `LineAnnotation` is interested in,
  /// replaces the currently registered anchor. Return `usize::MAX` to ignore.
  fn process_anchor(&mut self, _grapheme: &FormattedGrapheme) -> usize {
    NO_ANCHOR
  }

  /// This function is called at the end of a visual line to insert virtual text.
  ///
  /// # Returns
  ///
  /// The added virtual position. Only the row offset is used by the formatter.
  ///
  /// # Note
  ///
  /// The `line_end_visual_pos` parameter indicates the visual vertical distance
  /// from the start of the block where the traversal starts. This includes the
  /// offset from other `LineAnnotation`s. This allows inline annotations to
  /// consider the height of the text and "align" two different documents (like
  /// for side by side diffs). These annotations that want to "align" two
  /// documents should therefore be added last so that other virtual text is also
  /// considered while aligning.
  fn insert_virtual_lines(
    &mut self,
    line_end_char_idx: usize,
    line_end_visual_pos: Position,
    doc_line: usize,
  ) -> Position;
}

#[derive(Debug)]
struct Layer<'a, A, M> {
  annotations: &'a [A],
  metadata: M,
}

impl<A, M: Clone> Clone for Layer<'_, A, M> {
  fn clone(&self) -> Self {
    Layer {
      annotations: self.annotations,
      metadata: self.metadata.clone(),
    }
  }
}

impl<'a, A, M> From<(&'a [A], M)> for Layer<'a, A, M> {
  fn from((annotations, metadata): (&'a [A], M)) -> Layer<'a, A, M> {
    Layer {
      annotations,
      metadata,
    }
  }
}

#[derive(Debug)]
struct LayerCursor<'a, A, M> {
  annotations: &'a [A],
  index: usize,
  metadata: M,
}

impl<'a, A, M: Clone> From<&Layer<'a, A, M>> for LayerCursor<'a, A, M> {
  fn from(layer: &Layer<'a, A, M>) -> Self {
    LayerCursor {
      annotations: layer.annotations,
      index: 0,
      metadata: layer.metadata.clone(),
    }
  }
}

impl<'a, A, M> LayerCursor<'a, A, M> {
  fn reset_pos(&mut self, char_idx: usize, get_char_idx: impl Fn(&A) -> usize) {
    self.index = self
      .annotations
      .partition_point(|annot| get_char_idx(annot) < char_idx);
  }

  fn consume(&mut self, char_idx: usize, get_char_idx: impl Fn(&A) -> usize) -> Option<&'a A> {
    let annot = self.annotations.get(self.index)?;
    debug_assert!(get_char_idx(annot) >= char_idx);
    if get_char_idx(annot) == char_idx {
      self.index += 1;
      Some(annot)
    } else {
      None
    }
  }
}

/// Annotations that change what is displayed when the document is rendered.
/// Also commonly called virtual text.
#[derive(Default)]
pub struct TextAnnotations<'a> {
  inline_annotations: Vec<Layer<'a, InlineAnnotation, Option<Highlight>>>,
  overlays: Vec<Layer<'a, Overlay, Option<Highlight>>>,
  line_annotations: Vec<Box<dyn LineAnnotation + 'a>>,
}

impl Debug for TextAnnotations<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TextAnnotations")
      .field("inline_annotations", &self.inline_annotations)
      .field("overlays", &self.overlays)
      .finish_non_exhaustive()
  }
}

impl<'a> TextAnnotations<'a> {
  /// Create a traversal cursor starting at `char_idx`.
  pub fn cursor<'t>(&'t mut self, char_idx: usize) -> TextAnnotationsCursor<'a, 't> {
    let mut inline = Vec::with_capacity(self.inline_annotations.len());
    for layer in &self.inline_annotations {
      let mut cursor = LayerCursor::from(layer);
      cursor.reset_pos(char_idx, |annot| annot.char_idx);
      inline.push(cursor);
    }

    let mut overlays = Vec::with_capacity(self.overlays.len());
    for layer in &self.overlays {
      let mut cursor = LayerCursor::from(layer);
      cursor.reset_pos(char_idx, |annot| annot.char_idx);
      overlays.push(cursor);
    }

    let line_annotations = &mut self.line_annotations;
    let mut next_anchors = Vec::with_capacity(line_annotations.len());
    for layer in line_annotations.iter_mut() {
      next_anchors.push(layer.reset_pos(char_idx));
    }

    TextAnnotationsCursor {
      inline,
      overlays,
      line_annotations,
      next_anchors,
    }
  }

  pub fn collect_overlay_highlights(&self, char_range: Range<usize>) -> OverlayHighlights {
    let mut highlights_by_char = BTreeMap::new();

    for layer in &self.overlays {
      let Some(highlight) = layer.metadata.clone() else { continue };
      for overlay in layer.annotations.iter() {
        if overlay.char_idx < char_range.start {
          continue;
        }
        if overlay.char_idx >= char_range.end {
          break;
        }
        highlights_by_char.insert(overlay.char_idx, highlight);
      }
    }

    let mut highlights: Vec<(Highlight, Range<usize>)> = highlights_by_char
      .into_iter()
      .map(|(char_idx, highlight)| (highlight, char_idx..char_idx + 1))
      .collect();

    if highlights.is_empty() {
      return OverlayHighlights::Heterogenous { highlights };
    }

    let first = highlights[0].0;
    if highlights.iter().all(|(highlight, _)| *highlight == first) {
      let ranges = highlights.drain(..).map(|(_, range)| range).collect();
      OverlayHighlights::Homogeneous {
        highlight: first,
        ranges,
      }
    } else {
      OverlayHighlights::Heterogenous { highlights }
    }
  }

  /// Add new inline annotations.
  ///
  /// The annotation grapheme will be rendered with `highlight`
  /// patched on top of `ui.text`.
  ///
  /// The annotations **must be sorted** by their `char_idx`.
  /// Multiple annotations with the same `char_idx` are allowed;
  /// they will be displayed in the order present in the layer.
  ///
  /// If multiple layers contain annotations at the same position,
  /// the annotations that belong to the layers added first are shown first.
  #[must_use]
  pub fn add_inline_annotations(
    &mut self,
    layer: &'a [InlineAnnotation],
    highlight: Option<Highlight>,
  ) -> &mut Self {
    debug_assert!(is_sorted_by_char_idx(layer, |annot| annot.char_idx));
    if !layer.is_empty() {
      self.inline_annotations.push((layer, highlight).into());
    }
    self
  }

  /// Add new grapheme overlays.
  ///
  /// The overlaid grapheme will be rendered with `highlight`
  /// patched on top of `ui.text`.
  ///
  /// The overlays **must be sorted** by their `char_idx`.
  /// Multiple overlays with the same `char_idx` are allowed.
  ///
  /// If multiple layers contain overlays at the same position,
  /// the overlay from the layer added last will be shown.
  #[must_use]
  pub fn add_overlay(&mut self, layer: &'a [Overlay], highlight: Option<Highlight>) -> &mut Self {
    debug_assert!(is_sorted_by_char_idx(layer, |annot| annot.char_idx));
    if !layer.is_empty() {
      self.overlays.push((layer, highlight).into());
    }
    self
  }

  /// Add new line annotations.
  #[must_use]
  pub fn add_line_annotation(&mut self, layer: Box<dyn LineAnnotation + 'a>) -> &mut Self {
    self.line_annotations.push(layer);
    self
  }

  /// Remove all line annotations, useful for vertical motions
  /// so that virtual text lines are automatically skipped.
  pub fn clear_line_annotations(&mut self) {
    self.line_annotations.clear();
  }
}

/// Cursor state for traversing a set of text annotations.
pub struct TextAnnotationsCursor<'a, 't> {
  inline: Vec<LayerCursor<'a, InlineAnnotation, Option<Highlight>>>,
  overlays: Vec<LayerCursor<'a, Overlay, Option<Highlight>>>,
  line_annotations: &'t mut [Box<dyn LineAnnotation + 'a>],
  next_anchors: Vec<usize>,
}

impl Debug for TextAnnotationsCursor<'_, '_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TextAnnotationsCursor")
      .field("inline_layers", &self.inline.len())
      .field("overlay_layers", &self.overlays.len())
      .field("line_layers", &self.line_annotations.len())
      .finish_non_exhaustive()
  }
}

impl<'a, 't> TextAnnotationsCursor<'a, 't> {
  /// Reset the cursor to a new starting char index.
  pub fn reset_pos(&mut self, char_idx: usize) {
    for layer in &mut self.inline {
      layer.reset_pos(char_idx, |annot| annot.char_idx);
    }
    for layer in &mut self.overlays {
      layer.reset_pos(char_idx, |annot| annot.char_idx);
    }
    for (layer, anchor) in self.line_annotations.iter_mut().zip(&mut self.next_anchors) {
      *anchor = layer.reset_pos(char_idx);
    }
  }

  pub(crate) fn next_inline_annotation_at(
    &mut self,
    char_idx: usize,
  ) -> Option<(&'a InlineAnnotation, Option<Highlight>)> {
    self.inline.iter_mut().find_map(|layer| {
      let annotation = layer.consume(char_idx, |annot| annot.char_idx)?;
      Some((annotation, layer.metadata.clone()))
    })
  }

  pub(crate) fn overlay_at(
    &mut self,
    char_idx: usize,
  ) -> Option<(&'a Overlay, Option<Highlight>)> {
    let mut overlay = None;
    for layer in &mut self.overlays {
      while let Some(new_overlay) = layer.consume(char_idx, |annot| annot.char_idx) {
        overlay = Some((new_overlay, layer.metadata.clone()));
      }
    }
    overlay
  }

  pub(crate) fn process_virtual_text_anchors(&mut self, grapheme: &FormattedGrapheme) {
    for (idx, layer) in self.line_annotations.iter_mut().enumerate() {
      loop {
        match self.next_anchors[idx].cmp(&grapheme.char_idx) {
          Ordering::Less => {
            self.next_anchors[idx] = layer.skip_concealed_anchors(grapheme.char_idx)
          },
          Ordering::Equal => {
            self.next_anchors[idx] = layer.process_anchor(grapheme);
          },
          Ordering::Greater => break,
        }
      }
    }
  }

  pub(crate) fn virtual_lines_at(
    &mut self,
    char_idx: usize,
    line_end_visual_pos: Position,
    doc_line: usize,
  ) -> usize {
    let mut virt_off = Position::new(0, 0);
    for layer in self.line_annotations.iter_mut() {
      virt_off += layer.insert_virtual_lines(char_idx, line_end_visual_pos + virt_off, doc_line);
    }
    virt_off.row
  }
}

fn is_sorted_by_char_idx<T>(items: &[T], get_char_idx: impl Fn(&T) -> usize) -> bool {
  items
    .windows(2)
    .all(|pair| get_char_idx(&pair[0]) <= get_char_idx(&pair[1]))
}

fn is_single_grapheme(text: &str) -> bool {
  let mut iter = UnicodeSegmentation::graphemes(text, true);
  iter.next().is_some() && iter.next().is_none()
}
