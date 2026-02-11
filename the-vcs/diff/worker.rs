use std::sync::Arc;

use imara_diff::{IndentHeuristic, IndentLevel, InternedInput};
use parking_lot::RwLock;
use ropey::{Rope, RopeSlice};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Notify;
use tokio::time::{timeout, Duration};

use crate::diff::{
    DiffInner, Event, ALGORITHM, DIFF_DEBOUNCE_TIME_ASYNC, DIFF_DEBOUNCE_TIME_SYNC,
};

use super::line_cache::InternedRopeLines;

#[cfg(test)]
mod test;

pub(super) struct DiffWorker {
    pub channel: UnboundedReceiver<Event>,
    pub diff: Arc<RwLock<DiffInner>>,
    pub diff_finished_notify: Arc<Notify>,
    pub diff_alloc: imara_diff::Diff,
}

impl DiffWorker {
    async fn accumulate_events(&mut self, event: Event) -> (Option<Rope>, Option<Rope>) {
        let mut accumulator = EventAccumulator::new();
        accumulator.handle_event(event).await;
        accumulator
            .accumulate_debounced_events(&mut self.channel, self.diff_finished_notify.clone())
            .await;
        (accumulator.doc, accumulator.diff_base)
    }

    pub async fn run(mut self, diff_base: Rope, doc: Rope) {
        let mut interner = InternedRopeLines::new(diff_base, doc);
        if let Some(lines) = interner.interned_lines() {
            self.perform_diff(lines);
        }
        self.apply_hunks(interner.diff_base(), interner.doc());
        while let Some(event) = self.channel.recv().await {
            let (doc, diff_base) = self.accumulate_events(event).await;

            let process_accumulated_events = || {
                if let Some(new_base) = diff_base {
                    interner.update_diff_base(new_base, doc)
                } else {
                    interner.update_doc(doc.unwrap())
                }

                if let Some(lines) = interner.interned_lines() {
                    self.perform_diff(lines)
                }
            };

            // Calculating diffs is computationally expensive and should
            // not run inside an async function to avoid blocking other futures.
            // Note: tokio::task::block_in_place does not work during tests
            #[cfg(test)]
            process_accumulated_events();
            #[cfg(not(test))]
            tokio::task::block_in_place(process_accumulated_events);

            self.apply_hunks(interner.diff_base(), interner.doc());
        }
    }

    /// update the hunks (used by the gutter) by replacing it with `self.new_hunks`.
    /// `self.new_hunks` is always empty after this function runs.
    /// To improve performance this function tries to reuse the allocation of the old diff previously stored in `self.line_diffs`
    fn apply_hunks(&mut self, diff_base: Rope, doc: Rope) {
        let mut diff = self.diff.write();
        diff.diff_base = diff_base;
        diff.doc = doc;
        diff.hunks.clear();
        diff.hunks.extend(self.diff_alloc.hunks());
        drop(diff);
        self.diff_finished_notify.notify_waiters();
    }

    fn perform_diff(&mut self, input: &InternedInput<RopeSlice>) {
        self.diff_alloc.compute_with(
            ALGORITHM,
            &input.before,
            &input.after,
            input.interner.num_tokens(),
        );
        self.diff_alloc.postprocess_with(
            &input.before,
            &input.after,
            IndentHeuristic::new(|token| {
                IndentLevel::for_ascii_line(input.interner[token].bytes(), 4)
            }),
        );
    }
}

struct EventAccumulator {
    diff_base: Option<Rope>,
    doc: Option<Rope>,
}

impl EventAccumulator {
    fn new() -> EventAccumulator {
        EventAccumulator {
            diff_base: None,
            doc: None,
        }
    }

    async fn handle_event(&mut self, event: Event) {
        let dst = if event.is_base {
            &mut self.diff_base
        } else {
            &mut self.doc
        };

        *dst = Some(event.text);
    }

    async fn accumulate_debounced_events(
        &mut self,
        channel: &mut UnboundedReceiver<Event>,
        _diff_finished_notify: Arc<Notify>,
    ) {
        let async_debounce = Duration::from_millis(DIFF_DEBOUNCE_TIME_ASYNC);
        let sync_debounce = Duration::from_millis(DIFF_DEBOUNCE_TIME_SYNC);
        let mut saw_input = false;
        loop {
            let debounce = if saw_input { sync_debounce } else { async_debounce };

            if let Ok(Some(event)) = timeout(debounce, channel.recv()).await {
                saw_input = true;
                self.handle_event(event).await;
            } else {
                break;
            }
        }
    }
}
