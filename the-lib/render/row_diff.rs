use std::{
  collections::HashMap,
  num::NonZeroU64,
};

use super::{
  damage::Damage,
  display_list::{
    RenderRow,
    RowId,
    RowMetadata,
    RowMetadataDelta,
    RowMove,
  },
  visible_layout::{
    BuiltRow,
    RowKey,
  },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RetainedRow {
  pub(super) key:      RowKey,
  pub(super) row:      RenderRow,
  pub(super) metadata: RowMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RowDiff {
  pub(super) next_visible_rows: Vec<RetainedRow>,
  pub(super) retained_rows:     Vec<RowMove>,
  pub(super) changed_rows:      Vec<RenderRow>,
  pub(super) removed_rows:      Vec<RowId>,
  pub(super) damage:            Damage,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct RowDiffEngine;

impl RowDiffEngine {
  pub(super) fn diff(
    next_row_id_counter: &mut u64,
    previous_rows: Vec<RetainedRow>,
    next_rows: Vec<BuiltRow>,
    force_full: bool,
  ) -> RowDiff {
    let mut previous = previous_rows
      .into_iter()
      .map(|row| (row.key.clone(), row))
      .collect::<HashMap<_, _>>();

    let mut retained_rows = Vec::new();
    let mut changed_rows = Vec::new();
    let mut removed_rows = Vec::new();
    let mut next_visible_rows = Vec::with_capacity(next_rows.len());
    let mut damaged_rows = Vec::new();

    for built in next_rows {
      if let Some(previous_row) = previous.remove(&built.key) {
        let content_changed = previous_row.row.text != built.text;
        let moved = previous_row.row.placement != built.placement;
        let generation = if content_changed {
          previous_row.row.generation.wrapping_add(1)
        } else {
          previous_row.row.generation
        };

        let row = RenderRow {
          id: previous_row.row.id,
          generation,
          placement: built.placement,
          text: built.text,
        };
        let metadata = built
          .metadata
          .into_row_metadata(row.id, row.placement.surface_row);

        if !force_full {
          if content_changed {
            damaged_rows.push(previous_row.row.placement.surface_row);
            damaged_rows.push(row.placement.surface_row);
            changed_rows.push(row.clone());
          } else if moved {
            damaged_rows.push(previous_row.row.placement.surface_row);
            damaged_rows.push(row.placement.surface_row);
            retained_rows.push(RowMove {
              id:   row.id,
              from: Some(previous_row.row.placement),
              to:   row.placement,
            });
          }
        }

        next_visible_rows.push(RetainedRow {
          key: built.key,
          row,
          metadata,
        });
      } else {
        let row = RenderRow {
          id:         next_row_id(next_row_id_counter),
          generation: 1,
          placement:  built.placement,
          text:       built.text,
        };
        let metadata = built
          .metadata
          .into_row_metadata(row.id, row.placement.surface_row);

        if !force_full {
          damaged_rows.push(row.placement.surface_row);
          changed_rows.push(row.clone());
        }

        next_visible_rows.push(RetainedRow {
          key: built.key,
          row,
          metadata,
        });
      }
    }

    if !force_full {
      for removed in previous.into_values() {
        damaged_rows.push(removed.row.placement.surface_row);
        removed_rows.push(removed.row.id);
      }
    }

    let damage = if force_full {
      Damage::full(next_visible_rows.len())
    } else {
      Damage::from_rows(damaged_rows)
    };

    RowDiff {
      next_visible_rows,
      retained_rows,
      changed_rows,
      removed_rows,
      damage,
    }
  }

  pub(super) fn build_row_metadata_delta(
    old: &[RowMetadata],
    new: &[RowMetadata],
    force_full: bool,
  ) -> Option<RowMetadataDelta> {
    if force_full {
      return Some(RowMetadataDelta { rows: new.to_vec() });
    }

    let previous = old
      .iter()
      .map(|metadata| (metadata.row_id, metadata))
      .collect::<HashMap<_, _>>();

    let rows = new
      .iter()
      .filter(|metadata| {
        previous
          .get(&metadata.row_id)
          .is_none_or(|old| *old != *metadata)
      })
      .cloned()
      .collect::<Vec<_>>();

    (!rows.is_empty()).then_some(RowMetadataDelta { rows })
  }

  pub(super) fn damage_for_row_metadata_change(old: &[RowMetadata], new: &[RowMetadata]) -> Damage {
    Damage::from_rows(
      old
        .iter()
        .map(|metadata| metadata.surface_row)
        .chain(new.iter().map(|metadata| metadata.surface_row)),
    )
  }
}

fn next_row_id(next_row_id: &mut u64) -> RowId {
  let id = NonZeroU64::new(*next_row_id).expect("row ids start at one");
  *next_row_id = next_row_id.wrapping_add(1).max(1);
  RowId::from_non_zero(id)
}
