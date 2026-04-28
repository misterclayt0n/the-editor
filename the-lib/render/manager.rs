use std::{
  collections::{
    BTreeSet,
    HashMap,
  },
  num::NonZeroU64,
};

use ropey::RopeSlice;

use super::{
  FrameRequest,
  RenderFrameDelta,
  RenderSurface,
  SurfaceId,
  TextAnnotations,
  TextFormat,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(NonZeroU64);

impl SourceId {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderManager {
  next_surface_id:    u64,
  surfaces:           HashMap<SurfaceId, ManagedSurface>,
  surfaces_by_source: HashMap<SourceId, BTreeSet<SurfaceId>>,
  dirty_surfaces:     BTreeSet<SurfaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManagedSurface {
  source_id: SourceId,
  surface:   RenderSurface,
}

impl Default for RenderManager {
  fn default() -> Self {
    Self {
      next_surface_id:    1,
      surfaces:           HashMap::new(),
      surfaces_by_source: HashMap::new(),
      dirty_surfaces:     BTreeSet::new(),
    }
  }
}

impl RenderManager {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn create_surface(
    &mut self,
    source_id: SourceId,
    format: TextFormat,
    annotations: TextAnnotations,
  ) -> SurfaceId {
    let surface_id = next_surface_id(&mut self.next_surface_id);
    let surface = RenderSurface::new(surface_id, format, annotations);

    self
      .surfaces
      .insert(surface_id, ManagedSurface { source_id, surface });
    self
      .surfaces_by_source
      .entry(source_id)
      .or_default()
      .insert(surface_id);
    self.dirty_surfaces.insert(surface_id);

    surface_id
  }

  pub fn destroy_surface(&mut self, surface_id: SurfaceId) -> Option<RenderSurface> {
    let managed = self.surfaces.remove(&surface_id)?;
    self.dirty_surfaces.remove(&surface_id);

    if let Some(surface_ids) = self.surfaces_by_source.get_mut(&managed.source_id) {
      surface_ids.remove(&surface_id);
      if surface_ids.is_empty() {
        self.surfaces_by_source.remove(&managed.source_id);
      }
    }

    Some(managed.surface)
  }

  pub fn source_id(&self, surface_id: SurfaceId) -> Option<SourceId> {
    self
      .surfaces
      .get(&surface_id)
      .map(|managed| managed.source_id)
  }

  pub fn surfaces_for_source(&self, source_id: SourceId) -> Vec<SurfaceId> {
    self
      .surfaces_by_source
      .get(&source_id)
      .map(|surface_ids| surface_ids.iter().copied().collect())
      .unwrap_or_default()
  }

  pub fn surface(&self, surface_id: SurfaceId) -> Option<&RenderSurface> {
    self
      .surfaces
      .get(&surface_id)
      .map(|managed| &managed.surface)
  }

  pub fn surface_mut(&mut self, surface_id: SurfaceId) -> Option<&mut RenderSurface> {
    self.dirty_surfaces.insert(surface_id);
    self
      .surfaces
      .get_mut(&surface_id)
      .map(|managed| &mut managed.surface)
  }

  pub fn is_surface_dirty(&self, surface_id: SurfaceId) -> bool {
    self.dirty_surfaces.contains(&surface_id)
  }

  pub fn dirty_surfaces(&self) -> Vec<SurfaceId> {
    self.dirty_surfaces.iter().copied().collect()
  }

  pub fn dirty_surfaces_for_source(&self, source_id: SourceId) -> Vec<SurfaceId> {
    self
      .surfaces_for_source(source_id)
      .into_iter()
      .filter(|surface_id| self.dirty_surfaces.contains(surface_id))
      .collect()
  }

  pub fn mark_surface_dirty(&mut self, surface_id: SurfaceId) -> bool {
    if !self.surfaces.contains_key(&surface_id) {
      return false;
    }

    self.dirty_surfaces.insert(surface_id);
    true
  }

  pub fn mark_source_dirty(&mut self, source_id: SourceId) {
    if let Some(surface_ids) = self.surfaces_by_source.get(&source_id) {
      self.dirty_surfaces.extend(surface_ids.iter().copied());
    }
  }

  pub fn render_surface(
    &mut self,
    surface_id: SurfaceId,
    text: RopeSlice<'_>,
    text_revision: u64,
    request: FrameRequest,
  ) -> Option<RenderFrameDelta> {
    let delta =
      self
        .surfaces
        .get_mut(&surface_id)?
        .surface
        .render_frame(text, text_revision, request);

    self.dirty_surfaces.remove(&surface_id);
    Some(delta)
  }
}

fn next_surface_id(next_surface_id: &mut u64) -> SurfaceId {
  let id = NonZeroU64::new(*next_surface_id).expect("surface ids start at one");
  *next_surface_id = next_surface_id.wrapping_add(1).max(1);
  SurfaceId::new(id.get()).expect("surface ids stay non-zero")
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::{
    RenderManager,
    SourceId,
  };
  use crate::render::{
    FrameRequest,
    SurfaceId,
    TextAnnotations,
    TextFormat,
    Viewport,
  };

  fn request(
    first_row: usize,
    row_count: usize,
    wrap_width: usize,
    since: Option<u64>,
  ) -> FrameRequest {
    FrameRequest {
      viewport:         Viewport::new(first_row, row_count, 0, wrap_width),
      since_generation: since,
    }
  }

  #[test]
  fn create_surface_registers_source_and_marks_surface_dirty() {
    let mut manager = RenderManager::new();
    let source_id = SourceId::new(1).unwrap();

    let surface_id =
      manager.create_surface(source_id, TextFormat::default(), TextAnnotations::default());

    assert_eq!(manager.source_id(surface_id), Some(source_id));
    assert_eq!(manager.surfaces_for_source(source_id), vec![surface_id]);
    assert!(manager.is_surface_dirty(surface_id));
    assert_eq!(manager.dirty_surfaces(), vec![surface_id]);
  }

  #[test]
  fn mark_source_dirty_fans_out_to_all_attached_surfaces() {
    let mut manager = RenderManager::new();
    let source_id = SourceId::new(1).unwrap();
    let other_source_id = SourceId::new(2).unwrap();

    let first =
      manager.create_surface(source_id, TextFormat::default(), TextAnnotations::default());
    let second =
      manager.create_surface(source_id, TextFormat::default(), TextAnnotations::default());
    let third = manager.create_surface(
      other_source_id,
      TextFormat::default(),
      TextAnnotations::default(),
    );

    let rope = Rope::from("a\nb\nc");
    let first_delta = manager
      .render_surface(first, rope.slice(..), 1, request(0, 1, 20, None))
      .unwrap();
    let second_delta = manager
      .render_surface(second, rope.slice(..), 1, request(0, 1, 20, None))
      .unwrap();
    let _third_delta = manager
      .render_surface(third, rope.slice(..), 1, request(0, 1, 20, None))
      .unwrap();

    manager.mark_source_dirty(source_id);

    assert_eq!(manager.dirty_surfaces_for_source(source_id), vec![
      first, second
    ]);
    assert!(!manager.is_surface_dirty(third));

    let _ = manager.render_surface(
      first,
      rope.slice(..),
      2,
      request(0, 1, 20, Some(first_delta.generation)),
    );

    assert!(!manager.is_surface_dirty(first));
    assert!(manager.is_surface_dirty(second));
    assert!(!manager.is_surface_dirty(third));

    let _ = manager.render_surface(
      second,
      rope.slice(..),
      2,
      request(0, 1, 20, Some(second_delta.generation)),
    );
    assert!(manager.dirty_surfaces_for_source(source_id).is_empty());
  }

  #[test]
  fn surface_mut_marks_surface_dirty() {
    let mut manager = RenderManager::new();
    let source_id = SourceId::new(1).unwrap();
    let surface_id =
      manager.create_surface(source_id, TextFormat::default(), TextAnnotations::default());

    let rope = Rope::from("abcd");
    let delta = manager
      .render_surface(surface_id, rope.slice(..), 1, request(0, 1, 20, None))
      .unwrap();
    assert!(!manager.is_surface_dirty(surface_id));

    manager.surface_mut(surface_id).unwrap().set_selection(None);
    assert!(manager.is_surface_dirty(surface_id));

    let _ = manager
      .render_surface(
        surface_id,
        rope.slice(..),
        1,
        request(0, 1, 20, Some(delta.generation)),
      )
      .unwrap();
    assert!(!manager.is_surface_dirty(surface_id));
  }

  #[test]
  fn destroy_surface_removes_all_manager_state() {
    let mut manager = RenderManager::new();
    let source_id = SourceId::new(1).unwrap();
    let surface_id =
      manager.create_surface(source_id, TextFormat::default(), TextAnnotations::default());

    let destroyed = manager.destroy_surface(surface_id).unwrap();

    assert_eq!(destroyed.id(), surface_id);
    assert!(manager.surface(surface_id).is_none());
    assert!(manager.surfaces_for_source(source_id).is_empty());
    assert!(manager.dirty_surfaces().is_empty());
    assert!(
      manager
        .render_surface(
          surface_id,
          Rope::from("x").slice(..),
          1,
          request(0, 1, 20, None),
        )
        .is_none()
    );
  }

  #[test]
  fn surface_ids_are_manager_owned() {
    let mut manager = RenderManager::new();
    let source_id = SourceId::new(1).unwrap();

    let first =
      manager.create_surface(source_id, TextFormat::default(), TextAnnotations::default());
    let second =
      manager.create_surface(source_id, TextFormat::default(), TextAnnotations::default());

    assert_ne!(first, second);
    assert_eq!(first, SurfaceId::new(1).unwrap());
    assert_eq!(second, SurfaceId::new(2).unwrap());
  }
}
