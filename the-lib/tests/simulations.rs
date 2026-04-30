mod support {
  pub mod sim_editor;
}

use support::sim_editor::{Op, SimEditor};
use the_lib::{Tendril, selection::Range};

#[test]
fn auto_pairs_backspace_roundtrip_script() {
  let editor = SimEditor::run_script("", &[Op::InsertChar('('), Op::Backspace]);

  assert_eq!(editor.document.text().to_string(), "");
  assert_eq!(editor.surface.primary_range(), Range::point(0));
}

#[test]
fn multiline_editing_script() {
  let editor = SimEditor::run_script("", &[
    Op::InsertText("hello"),
    Op::InsertNewline,
    Op::InsertText("world"),
    Op::MoveUp,
    Op::MoveLineEnd,
    Op::InsertChar('!'),
    Op::Undo,
    Op::Redo,
  ]);

  assert_eq!(editor.document.text().to_string(), "hello!\nworld");
  assert_eq!(editor.surface.primary_range(), Range::point(6));
}

#[test]
fn copy_and_paste_register_script() {
  let editor = SimEditor::run_script("", &[
    Op::InsertText("alpha"),
    Op::SelectAll,
    Op::Copy,
    Op::MoveDocumentEnd,
    Op::InsertNewline,
    Op::Paste,
  ]);

  assert_eq!(editor.document.text().to_string(), "alpha\nalpha");
  assert_eq!(editor.registers.read('"').unwrap(), &[Tendril::from(
    "alpha"
  )]);
}

#[test]
fn cut_updates_register_and_text() {
  let editor = SimEditor::run_script("alpha beta", &[Op::Search("beta"), Op::Cut]);

  assert_eq!(editor.document.text().to_string(), "alpha ");
  assert_eq!(editor.registers.read('"').unwrap(), &[Tendril::from(
    "beta"
  )]);
}

#[test]
fn undo_restores_exact_committed_selection_snapshot() {
  let editor = SimEditor::run_script("", &[
    Op::InsertText("alpha"),
    Op::MoveDocumentStart,
    Op::InsertChar('>'),
    Op::MoveDocumentEnd,
    Op::InsertChar('<'),
    Op::Undo,
  ]);

  assert_eq!(editor.document.text().to_string(), ">alpha");
  assert_eq!(editor.surface.primary_range(), Range::point(1));
}

#[test]
fn search_and_repeat_script() {
  let editor = SimEditor::run_script("one two two three", &[
    Op::Search("two"),
    Op::RepeatSearchForward,
  ]);

  assert_eq!(editor.surface.primary_range(), Range::new(8, 11));
}

#[test]
fn wrapped_visual_motion_roundtrip_script() {
  let editor = SimEditor::run_script("0123456789abcdef", &[
    Op::MoveRight,
    Op::MoveLeft,
    Op::MoveRight,
    Op::ToggleWrap,
    Op::MoveDown,
    Op::MoveUp,
  ]);

  assert!(editor.surface.text_format().soft_wrap);
  assert_eq!(editor.surface.primary_range(), Range::point(1));
  assert_eq!(editor.surface.viewport().first_col, 0);
}

#[test]
fn repeated_transactions_script() {
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

  let editor = SimEditor::run_script("", &ops);

  assert_eq!(editor.document.text().len_chars(), 47);
  assert_eq!(editor.document.text().to_string(), "x".repeat(47));
  assert_eq!(editor.surface.primary_range(), Range::point(0));
}
