#[allow(dead_code)]
#[path = "../tests/support/sim_editor.rs"]
mod sim_editor;

use ropey::Rope;
use sim_editor::{Op, SimEditor};
use the_lib::{
  render::{
    BlockWidget, CellGridMetrics, CellGridRenderSurface, CursorSemantics, FrameRequest,
    RenderManager, SourceId, SurfaceId, TextAnnotations, TextFormat, Viewport, WidgetId,
  },
  selection::{Range, Selection},
};

fn main() {
  divan::main();
}

fn shakespeare_rope() -> Rope {
  Rope::from_str(include_str!("../examples/t8.shakespeare.txt"))
}

fn long_wrapped_rope() -> Rope {
  let mut text = String::new();
  for line in 0..2_000 {
    text.push_str(&format!(
      "{line:04}     this is a deliberately long render benchmark line with indentation, words, tabs\tand unicode λλ that should wrap across several cell-grid rows.\n"
    ));
  }
  Rope::from_str(&text)
}

fn render_request(first_row: usize, row_count: usize, wrap_width: usize, since: Option<u64>) -> FrameRequest {
  FrameRequest {
    viewport: Viewport::new(first_row, row_count, 0, wrap_width),
    since_generation: since,
  }
}

fn indexed_surface(format: TextFormat) -> (RenderManager, the_lib::render::SurfaceId) {
  let mut manager = RenderManager::new();
  let surface_id = manager.create_surface(
    SourceId::new(1).unwrap(),
    format,
    TextAnnotations::default(),
  );
  manager
    .surface_mut(surface_id)
    .unwrap()
    .set_measurements(CellGridMetrics::new(8, 16));
  (manager, surface_id)
}

#[divan::bench]
fn repeated_transactions() {
  let mut ops = Vec::new();
  for _ in 0..64 {
    ops.push(Op::InsertChar('x'));
  }
  for _ in 0..16 {
    ops.push(Op::Backspace);
  }
  for _ in 0..8 {
    ops.push(Op::Undo);
  }
  for _ in 0..8 {
    ops.push(Op::Redo);
  }
  ops.push(Op::MoveLineStart);
  ops.push(Op::DeleteForward);

  SimEditor::run_script("", &ops);
}

#[divan::bench]
fn multiline_editing() {
  SimEditor::run_script("", &[
    Op::InsertText("hello"),
    Op::InsertNewline,
    Op::InsertText("world"),
    Op::MoveUp,
    Op::MoveLineEnd,
    Op::InsertChar('!'),
    Op::Undo,
    Op::Redo,
  ]);
}

#[divan::bench]
fn auto_pairs_backspace_roundtrip() {
  SimEditor::run_script("", &[Op::InsertChar('('), Op::Backspace]);
}

#[divan::bench]
fn search_and_repeat() {
  SimEditor::run_script("one two two three", &[
    Op::Search("two"),
    Op::RepeatSearchForward,
  ]);
}

#[divan::bench]
fn wrapped_visual_motion_roundtrip() {
  SimEditor::run_script("0123456789abcdef", &[
    Op::MoveRight,
    Op::MoveLeft,
    Op::MoveRight,
    Op::ToggleWrap,
    Op::MoveDown,
    Op::MoveUp,
  ]);
}

#[divan::bench]
fn medium_document_search() {
  let text = include_str!("../examples/t8.shakespeare.txt");
  SimEditor::run_script(text, &[
    Op::Search("KING"),
    Op::RepeatSearchForward,
    Op::RepeatSearchForward,
    Op::RepeatSearchForward,
  ]);
}

#[divan::bench]
fn medium_document_typing_and_rendering() {
  let text = include_str!("../examples/t8.shakespeare.txt");
  SimEditor::run_script(text, &[
    Op::MoveDocumentEnd,
    Op::InsertNewline,
    Op::InsertText("benchmark"),
    Op::MoveLineStart,
    Op::DeleteForward,
    Op::Undo,
    Op::Redo,
  ]);
}

#[divan::bench]
fn render_full_no_wrap_large_viewport() {
  let rope = shakespeare_rope();
  let (mut manager, surface_id) = indexed_surface(TextFormat::default());

  let delta = manager
    .render_surface(surface_id, rope.slice(..), 1, render_request(0, 80, 120, None))
    .unwrap();
  divan::black_box(delta);
}

#[divan::bench]
fn render_cold_then_noop_no_wrap_large_viewport() {
  let rope = shakespeare_rope();
  let (mut manager, surface_id) = indexed_surface(TextFormat::default());
  let _ = manager.render_surface(surface_id, rope.slice(..), 1, render_request(0, 80, 120, None));
  let generation = manager.surface(surface_id).unwrap().generation();

  let delta = manager
    .render_surface(surface_id, rope.slice(..), 1, render_request(0, 80, 120, Some(generation)))
    .unwrap();
  divan::black_box(delta);
}

#[divan::bench]
fn render_scroll_no_wrap_large_viewport() {
  let rope = shakespeare_rope();
  let (mut manager, surface_id) = indexed_surface(TextFormat::default());
  let mut generation = 0;

  for first_row in 0..64 {
    let since = (generation != 0).then_some(generation);
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, render_request(first_row, 80, 120, since))
      .unwrap();
    generation = delta.generation;
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_full_soft_wrap_large_viewport() {
  let rope = long_wrapped_rope();
  let mut format = TextFormat::default();
  format.soft_wrap = true;
  let (mut manager, surface_id) = indexed_surface(format);

  let delta = manager
    .render_surface(surface_id, rope.slice(..), 1, render_request(0, 80, 40, None))
    .unwrap();
  divan::black_box(delta);
}

#[divan::bench]
fn render_scroll_soft_wrap_large_viewport() {
  let rope = long_wrapped_rope();
  let mut format = TextFormat::default();
  format.soft_wrap = true;
  let (mut manager, surface_id) = indexed_surface(format);
  let mut generation = 0;

  for first_row in (0..320).step_by(5) {
    let since = (generation != 0).then_some(generation);
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, render_request(first_row, 80, 40, since))
      .unwrap();
    generation = delta.generation;
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_stateless_soft_wrap_deep_viewport() {
  let rope = long_wrapped_rope();
  let mut format = TextFormat::default();
  format.soft_wrap = true;
  let mut surface = CellGridRenderSurface::new(
    SurfaceId::new(1).unwrap(),
    format,
    TextAnnotations::default(),
  );
  surface.set_measurements(CellGridMetrics::new(8, 16));

  let delta = surface.render_frame(
    rope.slice(..),
    1,
    render_request(2_000, 80, 40, None),
  );
  divan::black_box(delta);
}

#[divan::bench]
fn render_indexed_soft_wrap_deep_viewport() {
  let rope = long_wrapped_rope();
  let mut format = TextFormat::default();
  format.soft_wrap = true;
  let (mut manager, surface_id) = indexed_surface(format);

  let delta = manager
    .render_surface(surface_id, rope.slice(..), 1, render_request(2_000, 80, 40, None))
    .unwrap();
  divan::black_box(delta);
}

#[divan::bench]
fn render_many_cursors_projection() {
  let rope = shakespeare_rope();
  let (mut manager, surface_id) = indexed_surface(TextFormat::default());
  let ranges = (0..80)
    .map(|idx| Range::point(idx * 8))
    .collect::<smallvec::SmallVec<[Range; 1]>>();
  manager
    .surface_mut(surface_id)
    .unwrap()
    .set_selection(Some(Selection::new(ranges).unwrap()));

  let delta = manager
    .render_surface(surface_id, rope.slice(..), 1, render_request(0, 80, 120, None))
    .unwrap();
  divan::black_box(delta);
}

#[divan::bench]
fn render_widgets_projection() {
  let rope = shakespeare_rope();
  let (mut manager, surface_id) = indexed_surface(TextFormat::default());
  let widgets = (1..=100)
    .map(|id| BlockWidget::after_line(WidgetId::new(id).unwrap(), id as usize * 3, 400, 32))
    .collect::<Vec<_>>();
  manager.surface_mut(surface_id).unwrap().set_widgets(widgets);

  let delta = manager
    .render_surface(surface_id, rope.slice(..), 1, render_request(0, 80, 120, None))
    .unwrap();
  divan::black_box(delta);
}

fn warm_indexed_render(
  rope: &Rope,
  format: TextFormat,
  viewport: Viewport,
) -> (RenderManager, the_lib::render::SurfaceId, u64) {
  let (mut manager, surface_id) = indexed_surface(format);
  let delta = manager
    .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
      viewport,
      since_generation: None,
    })
    .unwrap();
  (manager, surface_id, delta.generation)
}

#[divan::bench]
fn render_noop_retained_no_wrap_1_row_x100() {
  let rope = shakespeare_rope();
  let viewport = Viewport::new(0, 1, 0, 120);
  let (mut manager, surface_id, generation) =
    warm_indexed_render(&rope, TextFormat::default(), viewport);

  for _ in 0..100 {
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(generation),
      })
      .unwrap();
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_noop_retained_no_wrap_20_rows_x100() {
  let rope = shakespeare_rope();
  let viewport = Viewport::new(0, 20, 0, 120);
  let (mut manager, surface_id, generation) =
    warm_indexed_render(&rope, TextFormat::default(), viewport);

  for _ in 0..100 {
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(generation),
      })
      .unwrap();
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_noop_retained_no_wrap_80_rows_x100() {
  let rope = shakespeare_rope();
  let viewport = Viewport::new(0, 80, 0, 120);
  let (mut manager, surface_id, generation) =
    warm_indexed_render(&rope, TextFormat::default(), viewport);

  for _ in 0..100 {
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(generation),
      })
      .unwrap();
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_noop_retained_soft_wrap_80_rows_x100() {
  let rope = long_wrapped_rope();
  let mut format = TextFormat::default();
  format.soft_wrap = true;
  let viewport = Viewport::new(0, 80, 0, 40);
  let (mut manager, surface_id, generation) = warm_indexed_render(&rope, format, viewport);

  for _ in 0..100 {
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(generation),
      })
      .unwrap();
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_stale_generation_same_viewport_80_rows_x100() {
  let rope = shakespeare_rope();
  let viewport = Viewport::new(0, 80, 0, 120);
  let (mut manager, surface_id, _) = warm_indexed_render(&rope, TextFormat::default(), viewport);

  for _ in 0..100 {
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(999_999),
      })
      .unwrap();
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_horizontal_scroll_crop_only_80_rows_x100() {
  let rope = shakespeare_rope();
  let base = Viewport::new(0, 80, 0, 120);
  let (mut manager, surface_id, mut generation) =
    warm_indexed_render(&rope, TextFormat::default(), base);

  for col in 0..100 {
    let viewport = Viewport::new(0, 80, col % 8, 120);
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(generation),
      })
      .unwrap();
    generation = delta.generation;
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_selection_only_projection_80_rows_x100() {
  let rope = shakespeare_rope();
  let viewport = Viewport::new(0, 80, 0, 120);
  let (mut manager, surface_id, mut generation) =
    warm_indexed_render(&rope, TextFormat::default(), viewport);

  for idx in 0..100 {
    manager
      .surface_mut(surface_id)
      .unwrap()
      .set_selection(Some(Selection::single(idx, idx + 10)));
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(generation),
      })
      .unwrap();
    generation = delta.generation;
    divan::black_box(delta);
  }
}

#[divan::bench]
fn render_cursor_semantics_only_80_rows_x100() {
  let rope = shakespeare_rope();
  let viewport = Viewport::new(0, 80, 0, 120);
  let (mut manager, surface_id, mut generation) =
    warm_indexed_render(&rope, TextFormat::default(), viewport);
  manager
    .surface_mut(surface_id)
    .unwrap()
    .set_selection(Some(Selection::single(10, 10)));

  for idx in 0..100 {
    let semantics = if idx % 2 == 0 {
      CursorSemantics::Cell
    } else {
      CursorSemantics::InsertionPoint
    };
    manager
      .surface_mut(surface_id)
      .unwrap()
      .set_cursor_semantics(semantics);
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, FrameRequest {
        viewport,
        since_generation: Some(generation),
      })
      .unwrap();
    generation = delta.generation;
    divan::black_box(delta);
  }
}
