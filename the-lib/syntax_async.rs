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

#[cfg(test)]
mod tests {
  use super::select_latest_matching_request;

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
}
