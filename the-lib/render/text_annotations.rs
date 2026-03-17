use std::{
  cmp::Ordering,
  collections::BTreeMap,
  fmt::Debug,
  ops::Range,
  rc::Rc,
};

use ropey::{
  Rope,
  RopeSlice,
};
use the_core::grapheme::Grapheme;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  Tendril,
  position::Position,
  render::{
    FormattedGrapheme,
    RenderLine,
    RenderPlan,
    RenderRowInsertion,
    RenderSpan,
    apply_row_insertions,
    doc_formatter::DocumentFormatter,
    text_format::TextFormat,
  },
  syntax::{
    Highlight,
    OverlayHighlights,
  },
};

const NO_ANCHOR: usize = usize::MAX;

/// An inline annotation is continuous text shown
/// on the screen before the grapheme that starts at
/// `char_idx`.
#[derive(Debug, Clone)]
pub struct InlineAnnotation {
  pub text:     Tendril,
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
/// ```ignore
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLineSpec {
  pub doc_line:       usize,
  pub col:            usize,
  pub text:           Tendril,
  pub highlight:      Option<Highlight>,
  pub wrap_to_view:   bool,
  pub max_wrap_width: Option<u16>,
}

impl VirtualLineSpec {
  pub fn after(doc_line: usize) -> Self {
    Self {
      doc_line,
      col: 0,
      text: Tendril::new(),
      highlight: None,
      wrap_to_view: true,
      max_wrap_width: None,
    }
  }

  pub fn col(mut self, col: usize) -> Self {
    self.col = col;
    self
  }

  pub fn text(mut self, text: impl Into<Tendril>) -> Self {
    self.text = text.into();
    self
  }

  pub fn highlight(mut self, highlight: Option<Highlight>) -> Self {
    self.highlight = highlight;
    self
  }

  pub fn wrap_to_viewport(mut self) -> Self {
    self.wrap_to_view = true;
    self
  }

  pub fn single_line(mut self) -> Self {
    self.wrap_to_view = false;
    self
  }

  pub fn max_wrap_width(mut self, width: u16) -> Self {
    self.max_wrap_width = Some(width.max(1));
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualRenderLine {
  pub row:       usize,
  pub col:       usize,
  pub text:      Tendril,
  pub highlight: Option<Highlight>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VirtualLinesLayout {
  pub lines:          Vec<VirtualRenderLine>,
  pub row_insertions: Vec<RenderRowInsertion>,
}

#[derive(Debug, Clone)]
struct ResolvedVirtualLineSpec {
  spec:            VirtualLineSpec,
  anchor_char_idx: usize,
}

#[derive(Debug, Clone)]
pub struct VirtualLineAnnotation {
  specs:             Rc<[ResolvedVirtualLineSpec]>,
  viewport_width:    u16,
  horizontal_offset: usize,
  next_spec:         usize,
}

impl VirtualLineAnnotation {
  pub fn new(
    text: RopeSlice<'_>,
    mut specs: Vec<VirtualLineSpec>,
    viewport_width: u16,
    horizontal_offset: usize,
  ) -> Self {
    specs.sort_by_key(|spec| spec.doc_line);
    let line_count = text.len_lines().max(1);
    let resolved = specs
      .into_iter()
      .map(|spec| {
        let anchor_char_idx = if spec.doc_line + 1 < line_count {
          text.line_to_char(spec.doc_line + 1)
        } else {
          text.len_chars()
        };
        ResolvedVirtualLineSpec {
          spec,
          anchor_char_idx,
        }
      })
      .collect::<Vec<_>>();
    Self {
      specs: Rc::from(resolved.into_boxed_slice()),
      viewport_width: viewport_width.max(1),
      horizontal_offset,
      next_spec: 0,
    }
  }

  pub fn from_shared(
    text: RopeSlice<'_>,
    specs: Rc<[VirtualLineSpec]>,
    viewport_width: u16,
    horizontal_offset: usize,
  ) -> Self {
    Self::new(
      text,
      specs.iter().cloned().collect(),
      viewport_width,
      horizontal_offset,
    )
  }

  fn row_count_for_spec(&self, spec: &ResolvedVirtualLineSpec) -> usize {
    wrap_virtual_line_rows(&spec.spec, self.viewport_width, self.horizontal_offset).len()
  }
}

impl LineAnnotation for VirtualLineAnnotation {
  fn reset_pos(&mut self, char_idx: usize) -> usize {
    self.next_spec = self
      .specs
      .partition_point(|spec| spec.anchor_char_idx < char_idx);
    NO_ANCHOR
  }

  fn insert_virtual_lines(
    &mut self,
    line_end_char_idx: usize,
    _line_end_visual_pos: Position,
    doc_line: usize,
  ) -> Position {
    while self.next_spec < self.specs.len() && self.specs[self.next_spec].spec.doc_line < doc_line {
      self.next_spec += 1;
    }

    let mut inserted_rows = 0usize;
    while self.next_spec < self.specs.len()
      && self.specs[self.next_spec].spec.doc_line == doc_line
      && self.specs[self.next_spec].anchor_char_idx <= line_end_char_idx
    {
      inserted_rows += self.row_count_for_spec(&self.specs[self.next_spec]);
      self.next_spec += 1;
    }

    Position::new(inserted_rows, 0)
  }
}

pub fn render_virtual_lines_for_viewport(
  plan: &RenderPlan,
  viewport_width: u16,
  horizontal_offset: usize,
  specs: &[VirtualLineSpec],
) -> VirtualLinesLayout {
  if specs.is_empty() || plan.viewport.height == 0 || viewport_width == 0 {
    return VirtualLinesLayout::default();
  }

  let mut specs = specs.to_vec();
  specs.sort_by_key(|spec| spec.doc_line);

  let mut layout = VirtualLinesLayout::default();
  let mut inserted_before = 0usize;

  for spec in specs {
    let Some(base_row) = plan
      .visible_rows
      .iter()
      .find(|row| row.doc_line == spec.doc_line && row.first_visual_line)
      .map(|row| plan.scroll.row.saturating_add(row.row as usize))
    else {
      continue;
    };

    let wrapped = wrap_virtual_line_rows(&spec, viewport_width, horizontal_offset);
    if wrapped.is_empty() {
      continue;
    }

    let row_start = base_row.saturating_add(inserted_before).saturating_add(1);
    for (offset, text) in wrapped.iter().enumerate() {
      layout.lines.push(VirtualRenderLine {
        row:       row_start.saturating_add(offset),
        col:       spec.col.saturating_sub(horizontal_offset),
        text:      text.clone(),
        highlight: spec.highlight,
      });
    }
    layout.row_insertions.push(RenderRowInsertion {
      base_row,
      inserted_rows: wrapped.len(),
    });
    inserted_before = inserted_before.saturating_add(wrapped.len());
  }

  layout
}

pub fn apply_virtual_lines_layout(plan: &mut RenderPlan, layout: &VirtualLinesLayout) {
  if layout.lines.is_empty() {
    return;
  }

  apply_row_insertions(plan, &layout.row_insertions);

  for line in &layout.lines {
    if line.row < plan.scroll.row {
      continue;
    }
    let relative_row = line.row - plan.scroll.row;
    if relative_row >= plan.viewport.height as usize {
      continue;
    }

    let Some(existing) = plan
      .lines
      .iter_mut()
      .find(|existing| existing.row as usize == relative_row)
    else {
      plan.lines.push(RenderLine {
        row:   relative_row as u16,
        spans: vec![RenderSpan {
          col:        line.col as u16,
          cols:       virtual_text_display_width(&line.text),
          text:       line.text.clone(),
          highlight:  line.highlight,
          is_virtual: true,
        }],
      });
      continue;
    };

    existing.spans.push(RenderSpan {
      col:        line.col as u16,
      cols:       virtual_text_display_width(&line.text),
      text:       line.text.clone(),
      highlight:  line.highlight,
      is_virtual: true,
    });
    existing.spans.sort_by_key(|span| span.col);
  }

  plan.lines.sort_by_key(|line| line.row);
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
/// empty virtual lines. Apart from that, the `LineAnnotation` trait has
/// multiple methods that allow it to track anchors in the document.
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

  /// This function is called at the end of a visual line to insert virtual
  /// text.
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
  /// documents should therefore be added last so that other virtual text is
  /// also considered while aligning.
  fn insert_virtual_lines(
    &mut self,
    line_end_char_idx: usize,
    line_end_visual_pos: Position,
    doc_line: usize,
  ) -> Position;
}

#[derive(Debug)]
enum AnnotationStorage<'a, A> {
  Borrowed(&'a [A]),
  Owned(Box<[A]>),
}

impl<A> AnnotationStorage<'_, A> {
  fn as_slice(&self) -> &[A] {
    match self {
      Self::Borrowed(items) => items,
      Self::Owned(items) => items,
    }
  }

  fn is_empty(&self) -> bool {
    self.as_slice().is_empty()
  }
}

impl<A: Clone> Clone for AnnotationStorage<'_, A> {
  fn clone(&self) -> Self {
    match self {
      Self::Borrowed(items) => Self::Borrowed(items),
      Self::Owned(items) => Self::Owned(items.clone()),
    }
  }
}

#[derive(Debug)]
struct Layer<'a, A, M> {
  annotations: AnnotationStorage<'a, A>,
  metadata:    M,
}

impl<A: Clone, M: Clone> Clone for Layer<'_, A, M> {
  fn clone(&self) -> Self {
    Layer {
      annotations: self.annotations.clone(),
      metadata:    self.metadata.clone(),
    }
  }
}

impl<'a, A, M> From<(&'a [A], M)> for Layer<'a, A, M> {
  fn from((annotations, metadata): (&'a [A], M)) -> Layer<'a, A, M> {
    Layer {
      annotations: AnnotationStorage::Borrowed(annotations),
      metadata,
    }
  }
}

#[derive(Debug)]
struct LayerCursor<'t, A, M> {
  annotations: &'t [A],
  index:       usize,
  metadata:    M,
}

impl<'t, 'a, A, M: Clone> From<&'t Layer<'a, A, M>> for LayerCursor<'t, A, M> {
  fn from(layer: &'t Layer<'a, A, M>) -> Self {
    LayerCursor {
      annotations: layer.annotations.as_slice(),
      index:       0,
      metadata:    layer.metadata.clone(),
    }
  }
}

impl<'t, A, M> LayerCursor<'t, A, M> {
  fn reset_pos(&mut self, char_idx: usize, get_char_idx: impl Fn(&A) -> usize) {
    self.index = self
      .annotations
      .partition_point(|annot| get_char_idx(annot) < char_idx);
  }

  fn consume(&mut self, char_idx: usize, get_char_idx: impl Fn(&A) -> usize) -> Option<&'t A> {
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
  generation:         u64,
  inline_annotations: Vec<Layer<'a, InlineAnnotation, Option<Highlight>>>,
  overlays:           Vec<Layer<'a, Overlay, Option<Highlight>>>,
  line_annotations:   Vec<Box<dyn LineAnnotation + 'a>>,
}

impl Debug for TextAnnotations<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TextAnnotations")
      .field("inline_annotations", &self.inline_annotations)
      .field("overlays", &self.overlays)
      .finish_non_exhaustive()
  }
}

#[derive(Debug, Default, Clone)]
pub struct OwnedTextAnnotations {
  inline_annotations: Vec<(Vec<InlineAnnotation>, Option<Highlight>)>,
  overlays:           Vec<(Vec<Overlay>, Option<Highlight>)>,
  virtual_lines:      Vec<VirtualLineSpec>,
}

impl OwnedTextAnnotations {
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.inline_annotations.is_empty() && self.overlays.is_empty() && self.virtual_lines.is_empty()
  }

  #[must_use]
  pub fn add_inline_annotations_owned(
    &mut self,
    mut layer: Vec<InlineAnnotation>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    layer.sort_by_key(|annot| annot.char_idx);
    if !layer.is_empty() {
      self.inline_annotations.push((layer, highlight));
    }
    self
  }

  #[must_use]
  pub fn add_inline_text(
    &mut self,
    char_idx: usize,
    text: impl Into<Tendril>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    self.add_inline_annotations_owned(vec![InlineAnnotation::new(char_idx, text)], highlight)
  }

  #[must_use]
  pub fn add_overlays_owned(
    &mut self,
    mut layer: Vec<Overlay>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    layer.sort_by_key(|overlay| overlay.char_idx);
    if !layer.is_empty() {
      self.overlays.push((layer, highlight));
    }
    self
  }

  #[must_use]
  pub fn add_overlay_grapheme(
    &mut self,
    char_idx: usize,
    grapheme: impl Into<Tendril>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    self.add_overlays_owned(vec![Overlay::new(char_idx, grapheme)], highlight)
  }

  #[must_use]
  pub fn add_virtual_line(&mut self, spec: VirtualLineSpec) -> &mut Self {
    if !spec.text.is_empty() {
      self.virtual_lines.push(spec);
    }
    self
  }

  #[must_use]
  pub fn extend_into<'a>(
    self,
    annotations: &mut TextAnnotations<'a>,
    text: RopeSlice<'_>,
    viewport_width: u16,
    horizontal_offset: usize,
  ) {
    for (layer, highlight) in self.inline_annotations {
      let _ = annotations.add_inline_annotations_owned(layer, highlight);
    }
    for (layer, highlight) in self.overlays {
      let _ = annotations.add_overlays_owned(layer, highlight);
    }
    if !self.virtual_lines.is_empty() {
      let _ = annotations.add_line_annotation(Box::new(VirtualLineAnnotation::new(
        text,
        self.virtual_lines,
        viewport_width.max(1),
        horizontal_offset,
      )));
    }
  }
}

impl<'a> TextAnnotations<'a> {
  /// Create a traversal cursor starting at `char_idx`.
  pub fn cursor<'t>(&'t mut self, char_idx: usize) -> TextAnnotationsCursor<'t, 'a> {
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
      let Some(highlight) = layer.metadata.clone() else {
        continue;
      };
      for overlay in layer.annotations.as_slice().iter() {
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

  pub fn has_line_annotations(&self) -> bool {
    !self.line_annotations.is_empty()
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
    self.add_inline_annotations_borrowed(layer, highlight)
  }

  /// Add new inline annotations from borrowed storage.
  #[must_use]
  pub fn add_inline_annotations_borrowed(
    &mut self,
    layer: &'a [InlineAnnotation],
    highlight: Option<Highlight>,
  ) -> &mut Self {
    debug_assert!(is_sorted_by_char_idx(layer, |annot| annot.char_idx));
    if !layer.is_empty() {
      self.inline_annotations.push((layer, highlight).into());
      self.generation = self.generation.wrapping_add(1);
    }
    self
  }

  /// Add new inline annotations from owned storage.
  #[must_use]
  pub fn add_inline_annotations_owned(
    &mut self,
    mut layer: Vec<InlineAnnotation>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    layer.sort_by_key(|annot| annot.char_idx);
    if !layer.is_empty() {
      self.inline_annotations.push(Layer {
        annotations: AnnotationStorage::Owned(layer.into_boxed_slice()),
        metadata:    highlight,
      });
      self.generation = self.generation.wrapping_add(1);
    }
    self
  }

  /// Add a single inline annotation from owned data.
  #[must_use]
  pub fn add_inline_annotation(
    &mut self,
    annotation: InlineAnnotation,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    self.add_inline_annotations_owned(vec![annotation], highlight)
  }

  /// Add a single inline text annotation from owned data.
  #[must_use]
  pub fn add_inline_text(
    &mut self,
    char_idx: usize,
    text: impl Into<Tendril>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    self.add_inline_annotation(InlineAnnotation::new(char_idx, text), highlight)
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
    self.add_overlays_borrowed(layer, highlight)
  }

  /// Add new overlays from borrowed storage.
  #[must_use]
  pub fn add_overlays_borrowed(
    &mut self,
    layer: &'a [Overlay],
    highlight: Option<Highlight>,
  ) -> &mut Self {
    debug_assert!(is_sorted_by_char_idx(layer, |annot| annot.char_idx));
    if !layer.is_empty() {
      self.overlays.push((layer, highlight).into());
      self.generation = self.generation.wrapping_add(1);
    }
    self
  }

  /// Add new overlays from owned storage.
  #[must_use]
  pub fn add_overlays_owned(
    &mut self,
    mut layer: Vec<Overlay>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    layer.sort_by_key(|overlay| overlay.char_idx);
    if !layer.is_empty() {
      self.overlays.push(Layer {
        annotations: AnnotationStorage::Owned(layer.into_boxed_slice()),
        metadata:    highlight,
      });
      self.generation = self.generation.wrapping_add(1);
    }
    self
  }

  /// Add a single grapheme overlay from owned data.
  #[must_use]
  pub fn add_overlay_grapheme(
    &mut self,
    char_idx: usize,
    grapheme: impl Into<Tendril>,
    highlight: Option<Highlight>,
  ) -> &mut Self {
    self.add_overlays_owned(vec![Overlay::new(char_idx, grapheme)], highlight)
  }

  /// Add new line annotations.
  #[must_use]
  pub fn add_line_annotation(&mut self, layer: Box<dyn LineAnnotation + 'a>) -> &mut Self {
    self.line_annotations.push(layer);
    self.generation = self.generation.wrapping_add(1);
    self
  }

  /// Remove all line annotations, useful for vertical motions
  /// so that virtual text lines are automatically skipped.
  pub fn clear_line_annotations(&mut self) {
    self.line_annotations.clear();
    self.generation = self.generation.wrapping_add(1);
  }

  pub fn generation(&self) -> u64 {
    self.generation
  }
}

/// Cursor state for traversing a set of text annotations.
pub struct TextAnnotationsCursor<'t, 'a> {
  inline:           Vec<LayerCursor<'t, InlineAnnotation, Option<Highlight>>>,
  overlays:         Vec<LayerCursor<'t, Overlay, Option<Highlight>>>,
  line_annotations: &'t mut [Box<dyn LineAnnotation + 'a>],
  next_anchors:     Vec<usize>,
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

impl<'t, 'a> TextAnnotationsCursor<'t, 'a> {
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
  ) -> Option<(&'t InlineAnnotation, Option<Highlight>)> {
    self.inline.iter_mut().find_map(|layer| {
      let annotation = layer.consume(char_idx, |annot| annot.char_idx)?;
      Some((annotation, layer.metadata.clone()))
    })
  }

  pub(crate) fn overlay_at(&mut self, char_idx: usize) -> Option<(&'t Overlay, Option<Highlight>)> {
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

fn wrap_virtual_line_rows(
  spec: &VirtualLineSpec,
  viewport_width: u16,
  horizontal_offset: usize,
) -> Vec<Tendril> {
  let text = spec.text.trim();
  if text.is_empty() {
    return Vec::new();
  }

  if !spec.wrap_to_view {
    return text.lines().map(|line| line.to_string().into()).collect();
  }

  let available_width = spec
    .max_wrap_width
    .unwrap_or_else(|| {
      viewport_width
        .saturating_sub(spec.col.saturating_sub(horizontal_offset) as u16)
        .max(1)
    })
    .max(1);

  let rope = Rope::from(text);
  let mut text_fmt = TextFormat::default();
  text_fmt.soft_wrap = true;
  text_fmt.tab_width = 4;
  text_fmt.max_wrap = available_width;
  text_fmt.max_indent_retain = 0;
  text_fmt.wrap_indicator = "".into();
  text_fmt.rebuild_wrap_indicator();
  text_fmt.wrap_indicator_highlight = None;
  text_fmt.viewport_width = available_width;
  text_fmt.soft_wrap_at_text_width = true;
  let mut annotations = TextAnnotations::default();

  let mut formatter =
    DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);
  let mut rows: Vec<String> = Vec::new();
  for grapheme in &mut formatter {
    if grapheme.source.is_eof() {
      break;
    }

    match grapheme.raw {
      Grapheme::Newline => {},
      Grapheme::Tab { width } => {
        while rows.len() <= grapheme.visual_pos.row {
          rows.push(String::new());
        }
        rows[grapheme.visual_pos.row].push_str(&" ".repeat(width));
      },
      Grapheme::Other { ref g } => {
        while rows.len() <= grapheme.visual_pos.row {
          rows.push(String::new());
        }
        rows[grapheme.visual_pos.row].push_str(g.as_ref());
      },
    }
  }

  if rows.is_empty() {
    return vec![text.to_string().into()];
  }

  rows.into_iter().map(Into::into).collect()
}

fn virtual_text_display_width(text: &str) -> u16 {
  UnicodeSegmentation::graphemes(text, true)
    .map(the_core::grapheme::grapheme_width)
    .sum::<usize>() as u16
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::{
    LineAnnotation,
    TextAnnotations,
    VirtualLineAnnotation,
    VirtualLineSpec,
    apply_virtual_lines_layout,
    render_virtual_lines_for_viewport,
  };
  use crate::{
    document::{
      Document,
      DocumentId,
    },
    position::Position,
    render::{
      GutterConfig,
      NoHighlights,
      RenderCache,
      RenderStyles,
      build_plan,
      graphics::Rect,
      text_format::TextFormat,
    },
    view::ViewState,
  };

  fn no_gutter() -> GutterConfig {
    GutterConfig {
      layout: Vec::new(),
      ..GutterConfig::default()
    }
  }

  fn line_text(plan: &crate::render::RenderPlan, row: u16) -> Option<String> {
    plan.lines.iter().find(|line| line.row == row).map(|line| {
      let mut out = String::new();
      for span in &line.spans {
        out.push_str(&span.text);
      }
      out
    })
  }

  #[test]
  fn virtual_line_annotation_reserves_rows_for_wrapped_text() {
    let specs = vec![
      VirtualLineSpec::after(0)
        .text("alpha beta gamma")
        .max_wrap_width(5),
    ];
    let text = Rope::from("source line\n");
    let mut annotation = VirtualLineAnnotation::new(text.slice(..), specs, 20, 0);

    assert_eq!(annotation.reset_pos(0), usize::MAX);
    let inserted = annotation.insert_virtual_lines(text.len_chars(), Position::new(0, 0), 0);
    assert!(inserted.row >= 3);
  }

  #[test]
  fn virtual_line_layout_applies_rows_into_render_plan() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("one\ntwo"));
    let view = ViewState::new(Rect::new(0, 0, 20, 4), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let gutter = no_gutter();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let mut plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      RenderStyles::default(),
    );

    let specs = vec![VirtualLineSpec::after(0).text("ghost line").single_line()];
    let layout =
      render_virtual_lines_for_viewport(&plan, text_fmt.viewport_width.max(1) as u16, 0, &specs);
    apply_virtual_lines_layout(&mut plan, &layout);

    assert_eq!(line_text(&plan, 0).as_deref(), Some("one "));
    assert_eq!(line_text(&plan, 1).as_deref(), Some("ghost line"));
    assert_eq!(line_text(&plan, 2).as_deref(), Some("two"));
  }
}
