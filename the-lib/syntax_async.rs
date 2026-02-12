/// Select the most recent item that matches the active parse request id.
///
/// Async parse workers can complete out-of-order, and poll loops often drain
/// multiple results at once. This helper ensures callers can consistently pick
/// the latest relevant result while ignoring stale completions.
pub fn select_latest_matching_request<T, I, F>(
  latest_request: u64,
  drained: I,
  mut request_id: F,
) -> Option<T>
where
  I: IntoIterator<Item = T>,
  F: FnMut(&T) -> u64,
{
  let mut selected = None;
  for item in drained {
    if request_id(&item) == latest_request {
      selected = Some(item);
    }
  }
  selected
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseRequestMeta {
  pub request_id:  u64,
  pub doc_version: u64,
}

#[derive(Debug)]
pub struct ParseRequest<T> {
  pub meta:    ParseRequestMeta,
  pub payload: T,
}

#[derive(Debug)]
pub enum QueueParseDecision<T> {
  Start(ParseRequest<T>),
  Queued(ParseRequestMeta),
}

#[derive(Debug)]
pub struct ParseResultDecision<T> {
  pub apply:      bool,
  pub start_next: Option<ParseRequest<T>>,
}

/// Tracks whether rendering is currently based on interpolated (potentially
/// stale) syntax.
///
/// When interpolation has occurred and a full parse is pending, callers should
/// avoid re-querying highlight spans from tree-sitter until a parsed result is
/// applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParseHighlightState {
  interpolated: bool,
}

impl ParseHighlightState {
  /// Mark the syntax as fully parsed (synchronous parse success or async swap).
  pub fn mark_parsed(&mut self) {
    self.interpolated = false;
  }

  /// Mark the syntax as interpolation-only until async parsing catches up.
  pub fn mark_interpolated(&mut self) {
    self.interpolated = true;
  }

  /// Reset state when syntax is unavailable/cleared.
  pub fn mark_cleared(&mut self) {
    self.interpolated = false;
  }

  /// Returns true when it's safe to refresh highlight caches from syntax.
  pub fn allow_cache_refresh<T>(&self, lifecycle: &ParseLifecycle<T>) -> bool {
    if !self.interpolated {
      return true;
    }

    lifecycle.in_flight().is_none() && lifecycle.queued().is_none()
  }

  /// Returns true if current syntax state came from interpolation.
  pub fn is_interpolated(&self) -> bool {
    self.interpolated
  }
}

/// Coordinates a single in-flight async parse with one queued replacement.
///
/// This mirrors Zed's "one background parse + parse again if stale" policy:
/// - only one parse job runs at a time;
/// - newer queued jobs replace older queued jobs;
/// - when a parse completes, the latest queued job starts automatically;
/// - stale results are dropped by doc version.
pub struct ParseLifecycle<T> {
  next_request_id: u64,
  in_flight:       Option<ParseRequestMeta>,
  queued:          Option<ParseRequest<T>>,
}

impl<T> Default for ParseLifecycle<T> {
  fn default() -> Self {
    Self {
      next_request_id: 0,
      in_flight:       None,
      queued:          None,
    }
  }
}

impl<T> ParseLifecycle<T> {
  pub fn queue(&mut self, doc_version: u64, payload: T) -> QueueParseDecision<T> {
    self.next_request_id = self.next_request_id.saturating_add(1);
    let request = ParseRequest {
      meta: ParseRequestMeta {
        request_id: self.next_request_id,
        doc_version,
      },
      payload,
    };

    if self.in_flight.is_none() {
      self.in_flight = Some(request.meta);
      QueueParseDecision::Start(request)
    } else {
      let meta = request.meta;
      self.queued = Some(request);
      QueueParseDecision::Queued(meta)
    }
  }

  pub fn on_result(
    &mut self,
    request_id: u64,
    doc_version: u64,
    current_doc_version: u64,
  ) -> ParseResultDecision<T> {
    let Some(in_flight) = self.in_flight else {
      return ParseResultDecision {
        apply:      false,
        start_next: None,
      };
    };

    if in_flight.request_id != request_id {
      return ParseResultDecision {
        apply:      false,
        start_next: None,
      };
    }

    self.in_flight = None;
    let apply = in_flight.doc_version == doc_version && doc_version == current_doc_version;
    let start_next = self.queued.take().map(|request| {
      self.in_flight = Some(request.meta);
      request
    });

    ParseResultDecision { apply, start_next }
  }

  pub fn cancel_pending(&mut self) {
    self.in_flight = None;
    self.queued = None;
  }

  pub fn in_flight(&self) -> Option<ParseRequestMeta> {
    self.in_flight
  }

  pub fn queued(&self) -> Option<ParseRequestMeta> {
    self.queued.as_ref().map(|request| request.meta)
  }
}

/// Select the most recent item that matches both active parse request id and
/// document version.
///
/// This guards against applying an async parse result produced for stale text.
/// A newer document revision can exist even when `latest_request` is unchanged
/// (for example: a timed-out parse is queued, then subsequent foreground
/// updates complete within timeout and don't queue a new async parse).
pub fn select_latest_matching_request_for_doc_version<T, I, FR, FV>(
  latest_request: u64,
  current_doc_version: u64,
  drained: I,
  mut request_id: FR,
  mut doc_version: FV,
) -> Option<T>
where
  I: IntoIterator<Item = T>,
  FR: FnMut(&T) -> u64,
  FV: FnMut(&T) -> u64,
{
  let mut selected = None;
  for item in drained {
    if request_id(&item) == latest_request && doc_version(&item) == current_doc_version {
      selected = Some(item);
    }
  }
  selected
}

#[cfg(test)]
mod tests {
  use super::{
    ParseHighlightState,
    ParseLifecycle,
    QueueParseDecision,
    select_latest_matching_request,
    select_latest_matching_request_for_doc_version,
  };

  #[derive(Debug, Clone, Copy)]
  struct SimRng {
    state: u64,
  }

  impl SimRng {
    fn new(seed: u64) -> Self {
      Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
      let mut x = self.state;
      x ^= x << 13;
      x ^= x >> 7;
      x ^= x << 17;
      self.state = x;
      x
    }

    fn next_usize(&mut self, upper: usize) -> usize {
      if upper == 0 {
        0
      } else {
        (self.next_u64() as usize) % upper
      }
    }
  }

  #[derive(Debug, Default)]
  struct AsyncApplyModel {
    latest_request:        u64,
    applied_request:       Option<u64>,
    syntax_version:        u64,
    highlight_cache_epoch: u64,
  }

  impl AsyncApplyModel {
    fn queue_request(&mut self) -> u64 {
      self.latest_request = self.latest_request.saturating_add(1);
      self.latest_request
    }

    fn apply_from_drained(&mut self, drained: Vec<u64>) -> bool {
      let selected =
        select_latest_matching_request(self.latest_request, drained, |request_id| *request_id);
      let Some(request_id) = selected else {
        return false;
      };

      if let Some(previous) = self.applied_request {
        assert!(
          request_id >= previous,
          "applied request ids must be monotonic (request_id={request_id}, previous={previous})"
        );
      }
      self.applied_request = Some(request_id);
      self.syntax_version = self.syntax_version.saturating_add(1);
      self.highlight_cache_epoch = self.highlight_cache_epoch.saturating_add(1);
      true
    }
  }

  #[test]
  fn select_latest_matching_request_ignores_stale_tail() {
    let drained = vec![1u64, 3, 2, 3, 1];
    let selected = select_latest_matching_request(3, drained, |request_id| *request_id);
    assert_eq!(selected, Some(3));
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  struct ParseResult {
    request_id:  u64,
    doc_version: u64,
  }

  #[test]
  fn select_latest_matching_request_for_doc_version_ignores_stale_doc_version() {
    let drained = vec![
      ParseResult {
        request_id:  7,
        doc_version: 11,
      },
      ParseResult {
        request_id:  6,
        doc_version: 12,
      },
      ParseResult {
        request_id:  7,
        doc_version: 12,
      },
      ParseResult {
        request_id:  7,
        doc_version: 11,
      },
    ];
    let selected = select_latest_matching_request_for_doc_version(
      7,
      12,
      drained,
      |result| result.request_id,
      |result| result.doc_version,
    );
    assert_eq!(
      selected,
      Some(ParseResult {
        request_id:  7,
        doc_version: 12,
      })
    );
  }

  #[test]
  fn parse_lifecycle_starts_once_and_replaces_queued_job() {
    let mut lifecycle = ParseLifecycle::default();

    let first = lifecycle.queue(1, "first");
    let second = lifecycle.queue(2, "second");
    let third = lifecycle.queue(3, "third");

    let QueueParseDecision::Start(first) = first else {
      panic!("first request should start immediately");
    };
    assert_eq!(first.meta.doc_version, 1);
    assert_eq!(lifecycle.in_flight(), Some(first.meta));

    let QueueParseDecision::Queued(second_meta) = second else {
      panic!("second request should be queued");
    };
    assert_eq!(second_meta.doc_version, 2);

    let QueueParseDecision::Queued(third_meta) = third else {
      panic!("third request should replace queued request");
    };
    assert_eq!(third_meta.doc_version, 3);
    assert_eq!(lifecycle.queued(), Some(third_meta));

    let finished = lifecycle.on_result(first.meta.request_id, 1, 3);
    assert!(!finished.apply);
    let Some(start_next) = finished.start_next else {
      panic!("queued request should start when in-flight completes");
    };
    assert_eq!(start_next.meta.doc_version, 3);
    assert_eq!(lifecycle.in_flight(), Some(start_next.meta));
  }

  #[test]
  fn parse_lifecycle_applies_only_when_doc_version_matches() {
    let mut lifecycle = ParseLifecycle::default();
    let QueueParseDecision::Start(started) = lifecycle.queue(4, ()) else {
      panic!("expected immediate start");
    };

    let stale = lifecycle.on_result(started.meta.request_id, started.meta.doc_version, 5);
    assert!(!stale.apply);
    assert!(stale.start_next.is_none());

    let QueueParseDecision::Start(next) = lifecycle.queue(5, ()) else {
      panic!("expected immediate start");
    };
    let fresh = lifecycle.on_result(next.meta.request_id, next.meta.doc_version, 5);
    assert!(fresh.apply);
    assert!(fresh.start_next.is_none());
  }

  #[test]
  fn deterministic_async_interleaving_simulation() {
    let mut rng = SimRng::new(0xA11C_E5EED);
    let mut model = AsyncApplyModel::default();
    let mut pending: Vec<(u64, usize)> = Vec::new();
    let mut apply_count = 0u64;

    for tick in 0..640usize {
      let to_queue = rng.next_usize(3);
      for _ in 0..to_queue {
        let request_id = model.queue_request();
        let delay_ticks = rng.next_usize(12);
        pending.push((request_id, tick + delay_ticks));
      }

      if !pending.is_empty() && tick % 29 == 0 {
        let drop_index = rng.next_usize(pending.len());
        pending.swap_remove(drop_index);
      }

      let mut drained = Vec::new();
      let mut idx = 0;
      while idx < pending.len() {
        if pending[idx].1 <= tick {
          drained.push(pending.swap_remove(idx).0);
        } else {
          idx += 1;
        }
      }

      for i in 0..drained.len() {
        let j = rng.next_usize(drained.len());
        drained.swap(i, j);
      }

      if !drained.is_empty() && rng.next_usize(5) == 0 {
        let duplicate = drained[rng.next_usize(drained.len())];
        drained.push(duplicate);
      }
      if let Some(previous) = model.applied_request
        && rng.next_usize(7) == 0
      {
        drained.push(previous);
      }

      let latest_request = model.latest_request;
      let expected_apply = latest_request != 0 && drained.contains(&latest_request);
      let applied = model.apply_from_drained(drained);
      assert_eq!(
        applied, expected_apply,
        "latest-result apply mismatch at tick={tick}, latest_request={latest_request}"
      );

      if applied {
        apply_count = apply_count.saturating_add(1);
        assert_eq!(model.applied_request, Some(latest_request));
      }
      assert_eq!(
        model.syntax_version, apply_count,
        "syntax version should only advance on successful apply"
      );
      assert_eq!(
        model.highlight_cache_epoch, apply_count,
        "highlight cache epoch should only advance on successful apply"
      );
    }
  }

  #[test]
  fn parse_highlight_state_blocks_refresh_while_async_parse_is_running() {
    let mut lifecycle = ParseLifecycle::default();
    let mut state = ParseHighlightState::default();
    assert!(state.allow_cache_refresh(&lifecycle));

    state.mark_interpolated();
    assert!(state.allow_cache_refresh(&lifecycle));

    let QueueParseDecision::Start(started) = lifecycle.queue(10, ()) else {
      panic!("expected immediate start");
    };
    assert!(!state.allow_cache_refresh(&lifecycle));

    let finished = lifecycle.on_result(started.meta.request_id, 10, 10);
    assert!(finished.apply);
    assert!(finished.start_next.is_none());
    state.mark_parsed();
    assert!(state.allow_cache_refresh(&lifecycle));
  }

  #[test]
  fn parse_highlight_state_blocks_refresh_until_interpolated_queue_drains() {
    let mut lifecycle = ParseLifecycle::default();
    let mut state = ParseHighlightState::default();

    let QueueParseDecision::Start(started) = lifecycle.queue(1, ()) else {
      panic!("expected immediate start");
    };
    let QueueParseDecision::Queued(_) = lifecycle.queue(2, ()) else {
      panic!("expected queued request");
    };

    state.mark_interpolated();
    assert!(!state.allow_cache_refresh(&lifecycle));

    let first = lifecycle.on_result(started.meta.request_id, 1, 2);
    assert!(!first.apply);
    let Some(next) = first.start_next else {
      panic!("queued request should start");
    };
    assert!(!state.allow_cache_refresh(&lifecycle));

    let second = lifecycle.on_result(next.meta.request_id, 2, 2);
    assert!(second.apply);
    assert!(second.start_next.is_none());
    assert!(state.allow_cache_refresh(&lifecycle));
  }
}
