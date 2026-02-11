use std::iter::Peekable;
use std::sync::{
    Arc,
    Mutex,
    RwLock,
    RwLockReadGuard,
};
use std::collections::BTreeMap;

use imara_diff::Algorithm;
use ropey::Rope;

pub use imara_diff::Hunk;

mod line_cache;
use line_cache::InternedRopeLines;

struct PendingEvent {
    text: Rope,
    is_base: bool,
}

#[derive(Clone, Debug, Default)]
struct DiffInner {
    diff_base: Rope,
    doc: Rope,
    hunks: Vec<Hunk>,
}

struct DiffState {
    interner: InternedRopeLines,
    diff_alloc: imara_diff::Diff,
    pending_doc: Option<Rope>,
    pending_diff_base: Option<Rope>,
}

impl DiffState {
    fn new(diff_base: Rope, doc: Rope) -> Self {
        let mut state = Self {
            interner: InternedRopeLines::new(diff_base, doc),
            diff_alloc: imara_diff::Diff::default(),
            pending_doc: None,
            pending_diff_base: None,
        };
        state.recompute_current();
        state
    }

    fn enqueue(&mut self, event: PendingEvent) {
        if event.is_base {
            self.pending_diff_base = Some(event.text);
        } else {
            self.pending_doc = Some(event.text);
        }
    }

    fn recompute_current(&mut self) {
        if let Some(lines) = self.interner.interned_lines() {
            self.diff_alloc.compute_with(
                ALGORITHM,
                &lines.before,
                &lines.after,
                lines.interner.num_tokens(),
            );
            self.diff_alloc.postprocess_with_heuristic(
                lines,
                imara_diff::IndentHeuristic::new(|token| {
                    imara_diff::IndentLevel::for_ascii_line(lines.interner[token].bytes(), 4)
                }),
            );
        } else {
            self.diff_alloc = imara_diff::Diff::default();
        }
    }

    fn flush_pending(&mut self) -> bool {
        if self.pending_doc.is_none() && self.pending_diff_base.is_none() {
            return false;
        }

        if let Some(base) = self.pending_diff_base.take() {
            let doc = self.pending_doc.take();
            self.interner.update_diff_base(base, doc);
        } else if let Some(doc) = self.pending_doc.take() {
            self.interner.update_doc(doc);
        }

        self.recompute_current();
        true
    }

    fn snapshot(&self) -> DiffInner {
        DiffInner {
            diff_base: self.interner.diff_base(),
            doc: self.interner.doc(),
            hunks: self.diff_alloc.hunks().collect(),
        }
    }
}

/// Representation of a diff that can be updated.
#[derive(Clone)]
pub struct DiffHandle {
    state: Arc<Mutex<DiffState>>,
    diff: Arc<RwLock<DiffInner>>,
    inverted: bool,
}

impl DiffHandle {
    pub fn new(diff_base: Rope, doc: Rope) -> DiffHandle {
        let state = DiffState::new(diff_base, doc);
        let snapshot = state.snapshot();
        let diff = Arc::new(RwLock::new(snapshot));
        DiffHandle {
            state: Arc::new(Mutex::new(state)),
            diff,
            inverted: false,
        }
    }

    /// Switch base and modified texts' roles
    pub fn invert(&mut self) {
        self.inverted = !self.inverted;
    }

    /// Load the actual diff
    pub fn load(&self) -> Diff<'_> {
        let _ = self.poll();
        Diff {
            diff: self.diff.read().expect("diff read lock poisoned"),
            inverted: self.inverted,
        }
    }

    /// Updates the document associated with this redraw handle
    /// `block` is currently ignored; callers should decide redraw scheduling.
    pub fn update_document(&self, doc: Rope, block: bool) -> bool {
        let _queued = self.update_document_impl(doc, self.inverted);
        if block {
            let _ = self.poll();
        }
        _queued
    }

    /// Updates the base text of the diff. Returns if the update was successful.
    pub fn update_diff_base(&self, diff_base: Rope) -> bool {
        self.update_document_impl(diff_base, !self.inverted)
    }

    fn update_document_impl(&self, text: Rope, is_base: bool) -> bool {
        let mut state = self.state.lock().expect("diff state lock poisoned");
        state.enqueue(PendingEvent { text, is_base });
        true
    }

    /// Recompute hunks for all queued updates.
    ///
    /// Returns `true` if a new snapshot was produced.
    pub fn poll(&self) -> bool {
        let mut state = self.state.lock().expect("diff state lock poisoned");
        if !state.flush_pending() {
            return false;
        }
        let snapshot = state.snapshot();
        drop(state);
        let mut diff = self.diff.write().expect("diff write lock poisoned");
        *diff = snapshot;
        true
    }
}

const ALGORITHM: Algorithm = Algorithm::Histogram;
const MAX_DIFF_LINES: usize = 64 * u16::MAX as usize;
// cap average line length to 128 for files with MAX_DIFF_LINES
const MAX_DIFF_BYTES: usize = MAX_DIFF_LINES * 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffSignKind {
    Added,
    Modified,
    Removed,
}

/// A list of changes in a file sorted in ascending
/// non-overlapping order
#[derive(Debug)]
pub struct Diff<'a> {
    diff: RwLockReadGuard<'a, DiffInner>,
    inverted: bool,
}

impl Diff<'_> {
    /// Returns the base [Rope] of the [Diff]
    pub fn diff_base(&self) -> &Rope {
        if self.inverted {
            &self.diff.doc
        } else {
            &self.diff.diff_base
        }
    }

    /// Returns the [Rope] being compared against
    pub fn doc(&self) -> &Rope {
        if self.inverted {
            &self.diff.diff_base
        } else {
            &self.diff.doc
        }
    }

    pub fn is_inverted(&self) -> bool {
        self.inverted
    }

    /// Returns the `Hunk` for the `n`th change in this file.
    /// if there is no `n`th change  `Hunk::NONE` is returned instead.
    pub fn nth_hunk(&self, n: u32) -> Hunk {
        match self.diff.hunks.get(n as usize) {
            Some(hunk) if self.inverted => hunk.invert(),
            Some(hunk) => hunk.clone(),
            None => Hunk::NONE,
        }
    }

    pub fn len(&self) -> u32 {
        self.diff.hunks.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Gives the index of the first hunk after the given line, if one exists.
    pub fn next_hunk(&self, line: u32) -> Option<u32> {
        let hunk_range = if self.inverted {
            |hunk: &Hunk| hunk.before.clone()
        } else {
            |hunk: &Hunk| hunk.after.clone()
        };

        let res = self
            .diff
            .hunks
            .binary_search_by_key(&line, |hunk| hunk_range(hunk).start);

        match res {
            // Search found a hunk that starts exactly at this line, return the next hunk if it exists.
            Ok(pos) if pos + 1 == self.diff.hunks.len() => None,
            Ok(pos) => Some(pos as u32 + 1),

            // No hunk starts exactly at this line, so the search returns
            // the position where a hunk starting at this line should be inserted.
            // That position is exactly the position of the next hunk or the end
            // of the list if no such hunk exists
            Err(pos) if pos == self.diff.hunks.len() => None,
            Err(pos) => Some(pos as u32),
        }
    }

    /// Gives the index of the first hunk before the given line, if one exists.
    pub fn prev_hunk(&self, line: u32) -> Option<u32> {
        let hunk_range = if self.inverted {
            |hunk: &Hunk| hunk.before.clone()
        } else {
            |hunk: &Hunk| hunk.after.clone()
        };
        let res = self
            .diff
            .hunks
            .binary_search_by_key(&line, |hunk| hunk_range(hunk).end);

        match res {
            // Search found a hunk that ends exactly at this line (so it does not include the current line).
            // We can usually just return that hunk, however a special case for empty hunk is necessary
            // which represents a pure removal.
            // Removals are technically empty but are still shown as single line hunks
            // and as such we must jump to the previous hunk (if it exists) if we are already inside the removal
            Ok(pos) if !hunk_range(&self.diff.hunks[pos]).is_empty() => Some(pos as u32),

            // No hunk ends exactly at this line, so the search returns
            // the position where a hunk ending at this line should be inserted.
            // That position before this one is exactly the position of the previous hunk
            Err(0) | Ok(0) => None,
            Err(pos) | Ok(pos) => Some(pos as u32 - 1),
        }
    }

    /// Iterates over all hunks that intersect with the given line ranges.
    ///
    /// Hunks are returned at most once even when intersecting with multiple of the line
    /// ranges.
    pub fn hunks_intersecting_line_ranges<I>(&self, line_ranges: I) -> impl Iterator<Item = &Hunk>
    where
        I: Iterator<Item = (usize, usize)>,
    {
        HunksInLineRangesIter {
            hunks: &self.diff.hunks,
            line_ranges: line_ranges.peekable(),
            inverted: self.inverted,
            cursor: 0,
        }
    }

    /// Returns the index of the hunk containing the given line if it exists.
    pub fn hunk_at(&self, line: u32, include_removal: bool) -> Option<u32> {
        let hunk_range = if self.inverted {
            |hunk: &Hunk| hunk.before.clone()
        } else {
            |hunk: &Hunk| hunk.after.clone()
        };

        let res = self
            .diff
            .hunks
            .binary_search_by_key(&line, |hunk| hunk_range(hunk).start);

        match res {
            // Search found a hunk that starts exactly at this line, return it
            Ok(pos) => Some(pos as u32),

            // No hunk starts exactly at this line, so the search returns
            // the position where a hunk starting at this line should be inserted.
            // The previous hunk contains this hunk if it exists and doesn't end before this line
            Err(0) => None,
            Err(pos) => {
                let hunk = hunk_range(&self.diff.hunks[pos - 1]);
                if hunk.end > line || include_removal && hunk.start == line && hunk.is_empty() {
                    Some(pos as u32 - 1)
                } else {
                    None
                }
            }
        }
    }

    pub fn line_signs(&self) -> BTreeMap<usize, DiffSignKind> {
        self.line_signs_in_range(0, usize::MAX)
    }

    pub fn line_signs_in_range(
        &self,
        start_line: usize,
        end_line: usize,
    ) -> BTreeMap<usize, DiffSignKind> {
        let mut out = BTreeMap::new();
        if start_line >= end_line {
            return out;
        }

        for hunk in &self.diff.hunks {
            let range = if self.inverted {
                hunk.before.clone()
            } else {
                hunk.after.clone()
            };

            let kind = if hunk.is_pure_insertion() {
                DiffSignKind::Added
            } else if hunk.is_pure_removal() {
                DiffSignKind::Removed
            } else {
                DiffSignKind::Modified
            };

            if hunk.is_pure_removal() {
                let line = range.start as usize;
                if (start_line..end_line).contains(&line) {
                    out.insert(line, kind);
                }
                continue;
            }

            let from = (range.start as usize).max(start_line);
            let to = (range.end as usize).min(end_line);
            for line in from..to {
                out.insert(line, kind);
            }
        }
        out
    }
}

pub struct HunksInLineRangesIter<'a, I: Iterator<Item = (usize, usize)>> {
    hunks: &'a [Hunk],
    line_ranges: Peekable<I>,
    inverted: bool,
    cursor: usize,
}

#[cfg(test)]
mod tests {
    use super::{
        DiffHandle,
        DiffSignKind,
    };
    use ropey::Rope;

    #[test]
    fn line_signs_report_added_lines() {
        let handle = DiffHandle::new(Rope::from_str("a\n"), Rope::from_str("a\nb\n"));
        let diff = handle.load();
        let signs = diff.line_signs();
        assert_eq!(signs.get(&1).copied(), Some(DiffSignKind::Added));
    }

    #[test]
    fn line_signs_report_removed_lines() {
        let handle = DiffHandle::new(Rope::from_str("a\nb\n"), Rope::from_str("a\n"));
        let diff = handle.load();
        let signs = diff.line_signs();
        assert_eq!(signs.get(&1).copied(), Some(DiffSignKind::Removed));
    }
}

impl<'a, I: Iterator<Item = (usize, usize)>> Iterator for HunksInLineRangesIter<'a, I> {
    type Item = &'a Hunk;

    fn next(&mut self) -> Option<Self::Item> {
        let hunk_range = if self.inverted {
            |hunk: &Hunk| hunk.before.clone()
        } else {
            |hunk: &Hunk| hunk.after.clone()
        };

        loop {
            let (start_line, end_line) = self.line_ranges.peek()?;
            let hunk = self.hunks.get(self.cursor)?;

            if (hunk_range(hunk).end as usize) < *start_line {
                // If the hunk under the cursor comes before this range, jump the cursor
                // ahead to the next hunk that overlaps with the line range.
                self.cursor += self.hunks[self.cursor..]
                    .partition_point(|hunk| (hunk_range(hunk).end as usize) < *start_line);
            } else if (hunk_range(hunk).start as usize) <= *end_line {
                // If the hunk under the cursor overlaps with this line range, emit it
                // and move the cursor up so that the hunk cannot be emitted twice.
                self.cursor += 1;
                return Some(hunk);
            } else {
                // Otherwise, go to the next line range.
                self.line_ranges.next();
            }
        }
    }
}
