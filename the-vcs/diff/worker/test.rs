use ropey::Rope;
use tokio::task::JoinHandle;

use crate::diff::{DiffHandle, Hunk};

impl DiffHandle {
  fn new_test(diff_base: &str, doc: &str) -> (DiffHandle, JoinHandle<()>) {
    DiffHandle::new_with_handle(Rope::from_str(diff_base), Rope::from_str(doc))
  }
  async fn into_diff(self, handle: JoinHandle<()>) -> Vec<Hunk> {
    let diff = self.diff;
    // dropping the channel terminates the task
    drop(self.channel);
    handle.await.unwrap();
    let diff = diff.read();
    Vec::clone(&diff.hunks)
  }
}

#[tokio::test(flavor = "current_thread")]
async fn append_line() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test("foo\n", "foo\nbar\n");
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[Hunk {
          before: 1..1,
          after: 1..2,
        }]
      )
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn prepend_line() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test("foo\n", "bar\nfoo\n");
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[Hunk {
          before: 0..0,
          after: 0..1,
        }]
      )
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn modify() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test("foo\nbar\n", "foo bar\nbar\n");
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[Hunk {
          before: 0..1,
          after: 0..1,
        }]
      )
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn delete_line() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test("foo\nfoo bar\nbar\n", "foo\nbar\n");
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[Hunk {
          before: 1..2,
          after: 1..1,
        }]
      )
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn delete_line_and_modify() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test("foo\nbar\ntest\nfoo", "foo\ntest\nfoo bar");
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[
          Hunk {
            before: 1..2,
            after: 1..1,
          },
          Hunk {
            before: 3..4,
            after: 2..3,
          },
        ]
      )
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn add_use() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test(
        "use ropey::Rope;\nuse tokio::task::JoinHandle;\n",
        "use ropey::Rope;\nuse ropey::RopeSlice;\nuse tokio::task::JoinHandle;\n",
      );
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[Hunk {
          before: 1..1,
          after: 1..2,
        },]
      )
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn update_document() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test("foo\nbar\ntest\nfoo", "foo\nbar\ntest\nfoo");
      differ.update_document(Rope::from_str("foo\ntest\nfoo bar"), false);
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[
          Hunk {
            before: 1..2,
            after: 1..1,
          },
          Hunk {
            before: 3..4,
            after: 2..3,
          },
        ]
      )
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn update_base() {
  let local = tokio::task::LocalSet::new();
  local
    .run_until(async {
      let (differ, handle) = DiffHandle::new_test("foo\ntest\nfoo bar", "foo\ntest\nfoo bar");
      differ.update_diff_base(Rope::from_str("foo\nbar\ntest\nfoo"));
      let line_diffs = differ.into_diff(handle).await;
      assert_eq!(
        &line_diffs,
        &[
          Hunk {
            before: 1..2,
            after: 1..1,
          },
          Hunk {
            before: 3..4,
            after: 2..3,
          },
        ]
      )
    })
    .await;
}
