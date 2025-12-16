use std::{
  collections::HashMap,
  time::Duration,
};

use slotmap::HopSlotMap;

use crate::core::{
  ViewId,
  animation::{
    AnimationHandle,
    Easing,
    presets,
  },
  graphics::Rect,
  layout::{
    Constraint as LayoutConstraint,
    Layout as UiLayout,
  },
  view::View,
};

// Helper struct to track area animations for views
#[derive(Debug)]
struct AreaAnimation {
  x:      AnimationHandle<f32>,
  y:      AnimationHandle<f32>,
  width:  AnimationHandle<f32>,
  height: AnimationHandle<f32>,
}

impl AreaAnimation {
  fn new(from: Rect, to: Rect, duration: Duration, easing: Easing) -> Self {
    Self {
      x:      AnimationHandle::new(from.x as f32, to.x as f32, duration, easing),
      y:      AnimationHandle::new(from.y as f32, to.y as f32, duration, easing),
      width:  AnimationHandle::new(from.width as f32, to.width as f32, duration, easing),
      height: AnimationHandle::new(from.height as f32, to.height as f32, duration, easing),
    }
  }

  fn retarget(&mut self, to: Rect) {
    self.x.retarget(to.x as f32);
    self.y.retarget(to.y as f32);
    self.width.retarget(to.width as f32);
    self.height.retarget(to.height as f32);
  }

  fn update(&mut self, dt: f32) -> bool {
    let x_done = self.x.update(dt);
    let y_done = self.y.update(dt);
    let width_done = self.width.update(dt);
    let height_done = self.height.update(dt);
    x_done && y_done && width_done && height_done
  }

  fn current(&self) -> Rect {
    Rect {
      x:      *self.x.current() as u16,
      y:      *self.y.current() as u16,
      width:  (*self.width.current() as u16).max(1),
      height: (*self.height.current() as u16).max(1),
    }
  }
}

/// Info about a view that is in the process of closing (animating out).
#[derive(Debug)]
struct ClosingViewInfo {
  /// The layout direction this view was part of (determines slide direction)
  layout: Layout,
}

// the dimensions are recomputed on window resize/tree change.
//
#[derive(Debug)]
pub struct Tree {
  root:      ViewId,
  // (container, index inside the container)
  pub focus: ViewId,
  // fullscreen: bool,
  area:      Rect,

  nodes: HopSlotMap<ViewId, Node>,

  // used for traversals
  stack: Vec<(ViewId, Rect)>,

  // split animations: maps view IDs to their area animations
  area_animations: HashMap<ViewId, AreaAnimation>,

  // views that are in the process of closing (animating out)
  closing_views: HashMap<ViewId, ClosingViewInfo>,
}

#[derive(Debug)]
pub struct Node {
  parent:  ViewId,
  content: Content,
}

#[derive(Debug)]
pub enum Content {
  View(Box<View>),
  Container(Box<Container>),
}

impl Node {
  pub fn container(layout: Layout) -> Self {
    Self {
      parent:  ViewId::default(),
      content: Content::Container(Box::new(Container::new(layout))),
    }
  }

  pub fn view(view: View) -> Self {
    Self {
      parent:  ViewId::default(),
      content: Content::View(Box::new(view)),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
  Horizontal,
  Vertical,
  // could explore stacked/tabbed
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
  Up,
  Down,
  Left,
  Right,
}

#[derive(Debug)]
pub struct Container {
  layout:      Layout,
  children:    Vec<ViewId>,
  area:        Rect,
  /// Custom sizes for children (in cells). None means use Fill behavior.
  child_sizes: Vec<Option<u16>>,
}

impl Container {
  pub fn new(layout: Layout) -> Self {
    Self {
      layout,
      children: Vec::new(),
      area: Rect::default(),
      child_sizes: Vec::new(),
    }
  }
}

impl Default for Container {
  fn default() -> Self {
    Self::new(Layout::Vertical)
  }
}

impl Tree {
  pub fn new(area: Rect) -> Self {
    let root = Node::container(Layout::Vertical);

    let mut nodes = HopSlotMap::with_key();
    let root = nodes.insert(root);

    // root is it's own parent
    nodes[root].parent = root;

    Self {
      root,
      focus: root,
      // fullscreen: false,
      area,
      nodes,
      stack: Vec::new(),
      area_animations: HashMap::new(),
      closing_views: HashMap::new(),
    }
  }

  pub fn insert(&mut self, view: View) -> ViewId {
    let focus = self.focus;
    let parent = self.nodes[focus].parent;
    let mut node = Node::view(view);
    node.parent = parent;
    let node = self.nodes.insert(node);
    self.get_mut(node).id = node;

    let container = match &mut self.nodes[parent] {
      Node {
        content: Content::Container(container),
        ..
      } => container,
      _ => unreachable!(),
    };

    // insert node after the current item if there is children already
    let pos = if container.children.is_empty() {
      0
    } else {
      let pos = container
        .children
        .iter()
        .position(|&child| child == focus)
        .unwrap();
      pos + 1
    };

    container.children.insert(pos, node);
    container.child_sizes.insert(pos, None);
    // focus the new node
    self.focus = node;

    // recalculate all the sizes
    self.recalculate();

    node
  }

  pub fn split(&mut self, view: View, layout: Layout) -> ViewId {
    let focus = self.focus;
    let parent = self.nodes[focus].parent;

    // Store old areas before split for animation
    let old_areas: HashMap<ViewId, Rect> = self
      .nodes
      .iter()
      .filter_map(|(id, node)| {
        if let Content::View(view) = &node.content {
          Some((id, view.area))
        } else {
          None
        }
      })
      .collect();

    let node = Node::view(view);
    let node = self.nodes.insert(node);
    self.get_mut(node).id = node;

    let container = match &mut self.nodes[parent] {
      Node {
        content: Content::Container(container),
        ..
      } => container,
      _ => unreachable!(),
    };
    if container.layout == layout {
      // insert node after the current item if there is children already
      let pos = if container.children.is_empty() {
        0
      } else {
        let pos = container
          .children
          .iter()
          .position(|&child| child == focus)
          .unwrap();
        pos + 1
      };
      container.children.insert(pos, node);
      container.child_sizes.insert(pos, None);
      self.nodes[node].parent = parent;
    } else {
      let mut split = Node::container(layout);
      split.parent = parent;
      let split = self.nodes.insert(split);

      let container = match &mut self.nodes[split] {
        Node {
          content: Content::Container(container),
          ..
        } => container,
        _ => unreachable!(),
      };
      container.children.push(focus);
      container.child_sizes.push(None);
      container.children.push(node);
      container.child_sizes.push(None);
      self.nodes[focus].parent = split;
      self.nodes[node].parent = split;

      let container = match &mut self.nodes[parent] {
        Node {
          content: Content::Container(container),
          ..
        } => container,
        _ => unreachable!(),
      };

      let pos = container
        .children
        .iter()
        .position(|&child| child == focus)
        .unwrap();

      // replace focus on parent with split
      container.children[pos] = split;
    }

    // focus the new node
    self.focus = node;

    // recalculate all the sizes
    self.recalculate();

    // Set up animations for all views that changed size/position
    let (duration, easing) = presets::FAST;
    for (id, node) in &self.nodes {
      if let Content::View(view) = &node.content {
        if let Some(&old_area) = old_areas.get(&id) {
          // Existing view that changed area - animate from old to new
          if old_area != view.area {
            self.area_animations.insert(
              id,
              AreaAnimation::new(old_area, view.area, duration, easing),
            );
          }
        } else {
          // New view - animate sliding in from the edge
          let start_area = match layout {
            Layout::Horizontal => {
              // For horizontal splits, slide in from bottom edge
              Rect {
                x:      view.area.x,
                y:      view.area.y + view.area.height,
                width:  view.area.width,
                height: 0,
              }
            },
            Layout::Vertical => {
              // For vertical splits, slide in from right edge
              Rect {
                x:      view.area.x + view.area.width,
                y:      view.area.y,
                width:  0,
                height: view.area.height,
              }
            },
          };
          self.area_animations.insert(
            id,
            AreaAnimation::new(start_area, view.area, duration, easing),
          );
        }
      }
    }

    node
  }

  /// Get a mutable reference to a [Container] by index.
  /// # Panics
  /// Panics if `index` is not in self.nodes, or if the node's content is not a
  /// [Content::Container].
  fn container_mut(&mut self, index: ViewId) -> &mut Container {
    match &mut self.nodes[index] {
      Node {
        content: Content::Container(container),
        ..
      } => container,
      _ => unreachable!(),
    }
  }

  fn remove_or_replace(&mut self, child: ViewId, replacement: Option<ViewId>) -> Option<Node> {
    let parent = self.nodes[child].parent;

    let removed = self.nodes.remove(child)?;

    let mut replacement_id = None;
    {
      let container = self.container_mut(parent);
      let pos = container
        .children
        .iter()
        .position(|&item| item == child)
        .unwrap();

      if let Some(new) = replacement {
        container.children[pos] = new;
        replacement_id = Some(new);
      } else {
        container.children.remove(pos);
        container.child_sizes.remove(pos);
      }

      if container.children.len() == 1 {
        if let Some(size) = container.child_sizes.get_mut(0) {
          *size = None;
        }
      }
    }

    if let Some(new) = replacement_id {
      self.nodes[new].parent = parent;
    }

    Some(removed)
  }

  pub fn remove(&mut self, index: ViewId) {
    if self.focus == index {
      // focus on something else
      self.focus = self.prev();
    }

    let parent = self.nodes[index].parent;
    let parent_is_root = parent == self.root;

    let _ = self.remove_or_replace(index, None);

    let parent_container = self.container_mut(parent);
    if parent_container.children.len() == 1 && !parent_is_root {
      // Lets merge the only child back to its grandparent so that Views
      // are equally spaced.
      let sibling = parent_container.children.pop().unwrap();
      let _ = self.remove_or_replace(parent, Some(sibling));
    }

    self.recalculate()
  }

  /// Start the close animation for a view. The view will be removed
  /// after the animation completes.
  pub fn start_close(&mut self, index: ViewId) {
    // If this is the last view, skip animation and proceed with immediate close
    if self.views().count() == 1 {
      self.remove(index);
      return;
    }

    // If this is the focused view, move focus first
    if self.focus == index {
      self.focus = self.prev();
    }

    // Get the current view area
    let current_area = match self.try_get(index) {
      Some(view) => view.area,
      None => return,
    };

    // Get parent's layout direction and position of this view
    let parent = self.nodes[index].parent;
    let (layout, is_first) = match &self.nodes[parent].content {
      Content::Container(container) => {
        let pos = container.children.iter().position(|&id| id == index).unwrap_or(0);
        (container.layout, pos == 0)
      },
      _ => (Layout::Vertical, false),
    };

    // Calculate the collapse target based on position:
    // Raddebugger-style: slide towards the SCREEN EDGE (off-screen direction)
    // - First child (left/top): slide towards left/top edge
    // - Not first (right/bottom): slide towards right/bottom edge
    let target_area = match layout {
      Layout::Horizontal => {
        if is_first {
          // Top view: slide upward off screen (y stays at top, height = 0)
          Rect {
            x:      current_area.x,
            y:      current_area.y,
            width:  current_area.width,
            height: 0,
          }
        } else {
          // Bottom view: slide downward off screen (y moves to bottom, height = 0)
          Rect {
            x:      current_area.x,
            y:      current_area.y + current_area.height,
            width:  current_area.width,
            height: 0,
          }
        }
      },
      Layout::Vertical => {
        if is_first {
          // Left view: slide leftward off screen (x stays at left, width = 0)
          Rect {
            x:      current_area.x,
            y:      current_area.y,
            width:  0,
            height: current_area.height,
          }
        } else {
          // Right view: slide rightward off screen (x moves to right, width = 0)
          Rect {
            x:      current_area.x + current_area.width,
            y:      current_area.y,
            width:  0,
            height: current_area.height,
          }
        }
      },
    };

    // Retarget existing animation OR create new one
    let (duration, easing) = presets::FAST;
    if let Some(anim) = self.area_animations.get_mut(&index) {
      anim.retarget(target_area);
    } else {
      self.area_animations.insert(
        index,
        AreaAnimation::new(current_area, target_area, duration, easing),
      );
    }

    // Mark view as closing
    self.closing_views.insert(index, ClosingViewInfo { layout });

    // Animate sibling expansion
    self.animate_sibling_expansion(index, parent);
  }

  /// Animate the sibling view expanding to fill the closing view's space.
  fn animate_sibling_expansion(&mut self, closing_id: ViewId, parent: ViewId) {
    // Find the sibling that will expand
    let container = match &self.nodes[parent].content {
      Content::Container(c) => c,
      _ => return,
    };

    let pos = match container.children.iter().position(|&id| id == closing_id) {
      Some(p) => p,
      None => return,
    };

    // Prefer the sibling after, then the one before
    let sibling_id = if pos + 1 < container.children.len() {
      container.children[pos + 1]
    } else if pos > 0 {
      container.children[pos - 1]
    } else {
      return;
    };

    // Get the closing view's area
    let closing_area = match self.try_get(closing_id) {
      Some(view) => view.area,
      None => return,
    };

    // Get the sibling's current area
    let sibling_area = match self.try_get(sibling_id) {
      Some(view) => view.area,
      None => return,
    };

    let layout = match &self.nodes[parent].content {
      Content::Container(c) => c.layout,
      _ => return,
    };

    // Calculate the sibling's expanded area (includes closing view's space)
    let expanded_area = match layout {
      Layout::Horizontal => {
        if sibling_area.y > closing_area.y {
          // Sibling is below - expand upward
          Rect {
            x:      sibling_area.x,
            y:      closing_area.y,
            width:  sibling_area.width,
            height: sibling_area.height + closing_area.height,
          }
        } else {
          // Sibling is above - expand downward
          Rect {
            x:      sibling_area.x,
            y:      sibling_area.y,
            width:  sibling_area.width,
            height: sibling_area.height + closing_area.height,
          }
        }
      },
      Layout::Vertical => {
        if sibling_area.x > closing_area.x {
          // Sibling is to the right - expand leftward
          Rect {
            x:      closing_area.x,
            y:      sibling_area.y,
            width:  sibling_area.width + closing_area.width,
            height: sibling_area.height,
          }
        } else {
          // Sibling is to the left - expand rightward
          Rect {
            x:      sibling_area.x,
            y:      sibling_area.y,
            width:  sibling_area.width + closing_area.width,
            height: sibling_area.height,
          }
        }
      },
    };

    // Create or retarget sibling expansion animation
    let (duration, easing) = presets::FAST;
    if let Some(anim) = self.area_animations.get_mut(&sibling_id) {
      anim.retarget(expanded_area);
    } else {
      self.area_animations.insert(
        sibling_id,
        AreaAnimation::new(sibling_area, expanded_area, duration, easing),
      );
    }

    // Update the sibling's actual area to the target (for recalculate consistency)
    if let Content::View(view) = &mut self.nodes[sibling_id].content {
      view.area = expanded_area;
    }
  }

  /// Actually remove a view after its close animation completes.
  fn finish_close(&mut self, index: ViewId) {
    self.closing_views.remove(&index);
    self.area_animations.remove(&index);

    let parent = self.nodes[index].parent;
    let parent_is_root = parent == self.root;

    let _ = self.remove_or_replace(index, None);

    let parent_container = self.container_mut(parent);
    if parent_container.children.len() == 1 && !parent_is_root {
      let sibling = parent_container.children.pop().unwrap();
      let _ = self.remove_or_replace(parent, Some(sibling));
    }

    self.recalculate();
  }

  /// Check if a view is currently closing (animating out).
  pub fn is_closing(&self, view_id: ViewId) -> bool {
    self.closing_views.contains_key(&view_id)
  }

  pub fn views(&self) -> impl Iterator<Item = (&View, bool)> {
    let focus = self.focus;
    self.nodes.iter().filter_map(move |(key, node)| {
      match node {
        Node {
          content: Content::View(view),
          ..
        } => Some((view.as_ref(), focus == key)),
        _ => None,
      }
    })
  }

  pub fn views_mut(&mut self) -> impl Iterator<Item = (&mut View, bool)> {
    let focus = self.focus;
    self.nodes.iter_mut().filter_map(move |(key, node)| {
      match node {
        Node {
          content: Content::View(view),
          ..
        } => Some((view.as_mut(), focus == key)),
        _ => None,
      }
    })
  }

  /// Get reference to a [View] by index.
  /// # Panics
  ///
  /// Panics if `index` is not in self.nodes, or if the node's content is not
  /// [Content::View]. This can be checked with [Self::contains].
  pub fn get(&self, index: ViewId) -> &View {
    self.try_get(index).unwrap()
  }

  /// Try to get reference to a [View] by index. Returns `None` if node content
  /// is not a [`Content::View`].
  ///
  /// Does not panic if the view does not exists anymore.
  pub fn try_get(&self, index: ViewId) -> Option<&View> {
    match self.nodes.get(index) {
      Some(Node {
        content: Content::View(view),
        ..
      }) => Some(view),
      _ => None,
    }
  }

  /// Check if the currently focused view is a terminal.
  pub fn focused_is_terminal(&self) -> bool {
    self.try_get(self.focus).is_some_and(|v| v.is_terminal())
  }

  /// Get a mutable reference to a [View] by index.
  /// # Panics
  ///
  /// Panics if `index` is not in self.nodes, or if the node's content is not
  /// [Content::View]. This can be checked with [Self::contains].
  pub fn get_mut(&mut self, index: ViewId) -> &mut View {
    match &mut self.nodes[index] {
      Node {
        content: Content::View(view),
        ..
      } => view,
      _ => unreachable!(),
    }
  }

  /// Check if tree contains a [Node] with a given index.
  pub fn contains(&self, index: ViewId) -> bool {
    self.nodes.contains_key(index)
  }

  pub fn is_empty(&self) -> bool {
    match &self.nodes[self.root] {
      Node {
        content: Content::Container(container),
        ..
      } => container.children.is_empty(),
      _ => unreachable!(),
    }
  }

  pub fn resize(&mut self, area: Rect) -> bool {
    if self.area != area {
      self.area = area;
      self.recalculate();
      return true;
    }
    false
  }

  pub fn recalculate(&mut self) {
    if self.is_empty() {
      // There are no more views, so the tree should focus itself again.
      self.focus = self.root;

      return;
    }

    self.stack.push((self.root, self.area));

    // take the area
    // fetch the node
    // a) node is view, give it whole area
    // b) node is container, calculate areas for each child and push them on the
    // stack

    while let Some((key, area)) = self.stack.pop() {
      let node = &mut self.nodes[key];

      match &mut node.content {
        Content::View(view) => {
          // debug!!("setting view area {:?}", area);
          view.area = area;
        }, // TODO: call f()
        Content::Container(container) => {
          // debug!!("setting container area {:?}", area);
          container.area = area;

          match container.layout {
            Layout::Horizontal => {
              if container.children.is_empty() {
                continue;
              }

              // Build constraints based on child_sizes
              let constraints: Vec<LayoutConstraint> = container
                .child_sizes
                .iter()
                .map(|size| {
                  size
                    .map(|s| LayoutConstraint::Length(s))
                    .unwrap_or(LayoutConstraint::Fill(1))
                })
                .collect();

              let layout = UiLayout::vertical().constraints(constraints);
              let areas = layout.split(container.area);

              for (child, area) in container.children.iter().zip(areas) {
                self.stack.push((*child, area));
              }
            },
            Layout::Vertical => {
              if container.children.is_empty() {
                continue;
              }

              // Build constraints based on child_sizes
              let constraints: Vec<LayoutConstraint> = container
                .child_sizes
                .iter()
                .map(|size| {
                  size
                    .map(|s| LayoutConstraint::Length(s))
                    .unwrap_or(LayoutConstraint::Fill(1))
                })
                .collect();

              let layout = UiLayout::horizontal().constraints(constraints).spacing(1);
              let areas = layout.split(container.area);

              for (child, area) in container.children.iter().zip(areas) {
                self.stack.push((*child, area));
              }
            },
          }
        },
      }
    }
  }

  pub fn traverse(&self) -> Traverse<'_> {
    Traverse::new(self)
  }

  // Finds the split in the given direction if it exists
  pub fn find_split_in_direction(&self, id: ViewId, direction: Direction) -> Option<ViewId> {
    let parent = self.nodes[id].parent;
    // Base case, we found the root of the tree
    if parent == id {
      return None;
    }
    // Parent must always be a container
    let parent_container = match &self.nodes[parent].content {
      Content::Container(container) => container,
      Content::View(_) => unreachable!(),
    };

    match (direction, parent_container.layout) {
      (Direction::Up, Layout::Vertical)
      | (Direction::Left, Layout::Horizontal)
      | (Direction::Right, Layout::Horizontal)
      | (Direction::Down, Layout::Vertical) => {
        // The desired direction of movement is not possible within
        // the parent container so the search must continue closer to
        // the root of the split tree.
        self.find_split_in_direction(parent, direction)
      },
      (Direction::Up, Layout::Horizontal)
      | (Direction::Down, Layout::Horizontal)
      | (Direction::Left, Layout::Vertical)
      | (Direction::Right, Layout::Vertical) => {
        // It's possible to move in the desired direction within
        // the parent container so an attempt is made to find the
        // correct child.
        match self.find_child(id, &parent_container.children, direction) {
          // Child is found, search is ended
          Some(id) => Some(id),
          // A child is not found. This could be because of either two scenarios
          // 1. Its not possible to move in the desired direction, and search should end
          // 2. A layout like the following with focus at X and desired direction Right
          // | _ | x |   |
          // | _ _ _ |   |
          // | _ _ _ |   |
          // The container containing X ends at X so no rightward movement is possible
          // however there still exists another view/container to the right that hasn't
          // been explored. Thus another search is done here in the parent container
          // before concluding it's not possible to move in the desired direction.
          None => self.find_split_in_direction(parent, direction),
        }
      },
    }
  }

  fn find_child(&self, id: ViewId, children: &[ViewId], direction: Direction) -> Option<ViewId> {
    let mut child_id = match direction {
      // index wise in the child list the Up and Left represents a -1
      // thus reversed iterator.
      Direction::Up | Direction::Left => {
        children
          .iter()
          .rev()
          .skip_while(|i| **i != id)
          .copied()
          .nth(1)?
      },
      // Down and Right => +1 index wise in the child list
      Direction::Down | Direction::Right => {
        children.iter().skip_while(|i| **i != id).copied().nth(1)?
      },
    };
    let (current_x, current_y) = match &self.nodes[self.focus].content {
      Content::View(current_view) => (current_view.area.left(), current_view.area.top()),
      Content::Container(_) => unreachable!(),
    };

    // If the child is a container the search finds the closest container child
    // visually based on screen location.
    while let Content::Container(container) = &self.nodes[child_id].content {
      match (direction, container.layout) {
        (_, Layout::Vertical) => {
          // find closest split based on x because y is irrelevant
          // in a vertical container (and already correct based on previous search)
          child_id = *container.children.iter().min_by_key(|id| {
            let x = match &self.nodes[**id].content {
              Content::View(view) => view.area.left(),
              Content::Container(container) => container.area.left(),
            };
            (current_x as i16 - x as i16).abs()
          })?;
        },
        (_, Layout::Horizontal) => {
          // find closest split based on y because x is irrelevant
          // in a horizontal container (and already correct based on previous search)
          child_id = *container.children.iter().min_by_key(|id| {
            let y = match &self.nodes[**id].content {
              Content::View(view) => view.area.top(),
              Content::Container(container) => container.area.top(),
            };
            (current_y as i16 - y as i16).abs()
          })?;
        },
      }
    }
    Some(child_id)
  }

  pub fn prev(&self) -> ViewId {
    // This function is very dumb, but that's because we don't store any parent
    // links. (we'd be able to go parent.prev_sibling() recursively until we
    // find something) For now that's okay though, since it's unlikely you'll be
    // able to open a large enough number of splits to notice.

    let mut views = self
      .traverse()
      .rev()
      .skip_while(|&(id, _view)| id != self.focus)
      .skip(1); // Skip focused value
    if let Some((id, _)) = views.next() {
      id
    } else {
      // extremely crude, take the last item
      let (key, _) = self.traverse().next_back().unwrap();
      key
    }
  }

  pub fn next(&self) -> ViewId {
    // This function is very dumb, but that's because we don't store any parent
    // links. (we'd be able to go parent.next_sibling() recursively until we
    // find something) For now that's okay though, since it's unlikely you'll be
    // able to open a large enough number of splits to notice.

    let mut views = self
      .traverse()
      .skip_while(|&(id, _view)| id != self.focus)
      .skip(1); // Skip focused value
    if let Some((id, _)) = views.next() {
      id
    } else {
      // extremely crude, take the first item again
      let (key, _) = self.traverse().next().unwrap();
      key
    }
  }

  pub fn transpose(&mut self) {
    let focus = self.focus;
    let parent = self.nodes[focus].parent;
    if let Content::Container(container) = &mut self.nodes[parent].content {
      container.layout = match container.layout {
        Layout::Vertical => Layout::Horizontal,
        Layout::Horizontal => Layout::Vertical,
      };
      self.recalculate();
    }
  }

  pub fn swap_split_in_direction(&mut self, direction: Direction) -> Option<()> {
    let focus = self.focus;
    let target = self.find_split_in_direction(focus, direction)?;
    let focus_parent = self.nodes[focus].parent;
    let target_parent = self.nodes[target].parent;

    if focus_parent == target_parent {
      let parent = focus_parent;
      let [parent, focus_node, target_node] =
        self.nodes.get_disjoint_mut([parent, focus, target])?;

      let parent_container = match &mut parent.content {
        Content::Container(c) => c,
        _ => unreachable!(),
      };

      // Get positions using ViewId directly (works for both views)
      let focus_pos = parent_container
        .children
        .iter()
        .position(|&id| id == focus)?;
      let target_pos = parent_container
        .children
        .iter()
        .position(|&id| id == target)?;

      // Swap node positions in parent's children list
      parent_container.children.swap(focus_pos, target_pos);

      // Swap areas between the two nodes
      match (&mut focus_node.content, &mut target_node.content) {
        (Content::View(focus_view), Content::View(target_view)) => {
          std::mem::swap(&mut focus_view.area, &mut target_view.area);
        },
        _ => return None, // Can't swap containers
      }

      Some(())
    } else {
      let [focus_parent, target_parent, focus_node, target_node] = self
        .nodes
        .get_disjoint_mut([focus_parent, target_parent, focus, target])?;

      let focus_parent_container = match &mut focus_parent.content {
        Content::Container(c) => c,
        _ => unreachable!(),
      };
      let target_parent_container = match &mut target_parent.content {
        Content::Container(c) => c,
        _ => unreachable!(),
      };

      // Find positions
      let focus_pos = focus_parent_container
        .children
        .iter()
        .position(|&id| id == focus)?;
      let target_pos = target_parent_container
        .children
        .iter()
        .position(|&id| id == target)?;

      // Swap children in their respective parents
      std::mem::swap(
        &mut focus_parent_container.children[focus_pos],
        &mut target_parent_container.children[target_pos],
      );
      std::mem::swap(&mut focus_node.parent, &mut target_node.parent);

      // Swap areas
      match (&mut focus_node.content, &mut target_node.content) {
        (Content::View(focus_view), Content::View(target_view)) => {
          std::mem::swap(&mut focus_view.area, &mut target_view.area);
        },
        _ => return None,
      }

      Some(())
    }
  }

  pub fn area(&self) -> Rect {
    self.area
  }

  /// Update all active area animations with the given delta time.
  /// Returns the list of view IDs that were fully closed this frame (for cleanup).
  pub fn update_animations(&mut self, dt: f32) -> Vec<ViewId> {
    // Track which closing views have completed their animations
    let mut completed_closes = Vec::new();

    self.area_animations.retain(|id, anim| {
      let done = anim.update(dt);
      if done && self.closing_views.contains_key(id) {
        completed_closes.push(*id);
        return false; // Remove animation
      }
      !done
    });

    // Actually remove views whose close animations have completed
    for &id in &completed_closes {
      self.finish_close(id);
    }

    completed_closes
  }

  /// Check if there are any active area animations.
  pub fn has_active_animations(&self) -> bool {
    !self.area_animations.is_empty() || !self.closing_views.is_empty()
  }

  /// Get the current animated area for a view, or its actual area if no
  /// animation is active.
  pub fn get_animated_area(&self, view_id: ViewId) -> Option<Rect> {
    if let Some(anim) = self.area_animations.get(&view_id) {
      Some(anim.current())
    } else {
      self.try_get(view_id).map(|view| view.area)
    }
  }

  /// Resize a split separator by adjusting the view's size
  /// vertical: true for vertical separators (adjust width), false for
  /// horizontal (adjust height) delta_cells: positive to grow, negative to
  /// shrink (in cell units)
  pub fn resize_split(&mut self, view_id: ViewId, vertical: bool, delta_cells: i32) {
    if delta_cells == 0 {
      return;
    }

    // Find the appropriate container and child to resize
    // We need to traverse up the tree to find a container that matches our resize
    // direction
    let mut current_id = view_id;
    let target_layout = if vertical {
      Layout::Vertical
    } else {
      Layout::Horizontal
    };

    loop {
      // Get the node's parent
      let parent = match self.nodes.get(current_id) {
        Some(node) => node.parent,
        None => return,
      };

      // Can't resize root
      if parent == self.root && parent == current_id {
        return;
      }

      // Get parent container
      let container = match &self.nodes[parent].content {
        Content::Container(c) => c,
        _ => return,
      };

      // Check if this container's layout matches our resize direction
      if container.layout == target_layout {
        // Found the right container! Resize current_id within this container
        let pos = match container.children.iter().position(|&id| id == current_id) {
          Some(p) => p,
          None => return,
        };

        // Get current size from the node's area
        let current_size = match &self.nodes[current_id].content {
          Content::View(v) => {
            if vertical {
              v.area.width
            } else {
              v.area.height
            }
          },
          Content::Container(c) => {
            if vertical {
              c.area.width
            } else {
              c.area.height
            }
          },
        };

        // Calculate new size
        let new_size = (current_size as i32 + delta_cells).max(5) as u16;

        // Update child_sizes in the parent container
        let container_mut = match &mut self.nodes[parent].content {
          Content::Container(c) => c,
          _ => return,
        };

        // Ensure child_sizes vec is properly sized
        while container_mut.child_sizes.len() < container_mut.children.len() {
          container_mut.child_sizes.push(None);
        }

        container_mut.child_sizes[pos] = Some(new_size);

        // Recalculate layout
        self.recalculate();
        return;
      }

      // Move up to parent and try again
      current_id = parent;

      // Prevent infinite loop at root
      if current_id == self.root {
        return;
      }
    }
  }
}

#[derive(Debug)]
pub struct Traverse<'a> {
  tree:  &'a Tree,
  stack: Vec<ViewId>, // TODO: reuse the one we use on update
}

impl<'a> Traverse<'a> {
  fn new(tree: &'a Tree) -> Self {
    Self {
      tree,
      stack: vec![tree.root],
    }
  }
}

impl<'a> Iterator for Traverse<'a> {
  type Item = (ViewId, &'a View);

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let key = self.stack.pop()?;

      let node = &self.tree.nodes[key];

      match &node.content {
        Content::View(view) => return Some((key, view)),
        Content::Container(container) => {
          self.stack.extend(container.children.iter().rev());
        },
      }
    }
  }
}

impl DoubleEndedIterator for Traverse<'_> {
  fn next_back(&mut self) -> Option<Self::Item> {
    loop {
      let key = self.stack.pop()?;

      let node = &self.tree.nodes[key];

      match &node.content {
        Content::View(view) => return Some((key, view)),
        Content::Container(container) => {
          self.stack.extend(container.children.iter());
        },
      }
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::{
    core::DocumentId,
    editor::GutterConfig,
  };

  #[test]
  fn find_split_in_direction() {
    let mut tree = Tree::new(Rect {
      x:      0,
      y:      0,
      width:  180,
      height: 80,
    });
    let mut view = View::new(DocumentId::default(), GutterConfig::default());
    view.area = Rect::new(0, 0, 180, 80);
    tree.insert(view);

    let l0 = tree.focus;
    let view = View::new(DocumentId::default(), GutterConfig::default());
    tree.split(view, Layout::Vertical);
    let r0 = tree.focus;

    tree.focus = l0;
    let view = View::new(DocumentId::default(), GutterConfig::default());
    tree.split(view, Layout::Horizontal);
    let l1 = tree.focus;

    tree.focus = l0;
    let view = View::new(DocumentId::default(), GutterConfig::default());
    tree.split(view, Layout::Vertical);

    // Tree in test
    // | L0  | L2 |    |
    // |    L1    | R0 |
    let l2 = tree.focus;
    assert_eq!(Some(l0), tree.find_split_in_direction(l2, Direction::Left));
    assert_eq!(Some(l1), tree.find_split_in_direction(l2, Direction::Down));
    assert_eq!(Some(r0), tree.find_split_in_direction(l2, Direction::Right));
    assert_eq!(None, tree.find_split_in_direction(l2, Direction::Up));

    tree.focus = l1;
    assert_eq!(None, tree.find_split_in_direction(l1, Direction::Left));
    assert_eq!(None, tree.find_split_in_direction(l1, Direction::Down));
    assert_eq!(Some(r0), tree.find_split_in_direction(l1, Direction::Right));
    assert_eq!(Some(l0), tree.find_split_in_direction(l1, Direction::Up));

    tree.focus = l0;
    assert_eq!(None, tree.find_split_in_direction(l0, Direction::Left));
    assert_eq!(Some(l1), tree.find_split_in_direction(l0, Direction::Down));
    assert_eq!(Some(l2), tree.find_split_in_direction(l0, Direction::Right));
    assert_eq!(None, tree.find_split_in_direction(l0, Direction::Up));

    tree.focus = r0;
    assert_eq!(Some(l2), tree.find_split_in_direction(r0, Direction::Left));
    assert_eq!(None, tree.find_split_in_direction(r0, Direction::Down));
    assert_eq!(None, tree.find_split_in_direction(r0, Direction::Right));
    assert_eq!(None, tree.find_split_in_direction(r0, Direction::Up));
  }

  #[test]
  fn swap_split_in_direction() {
    let mut tree = Tree::new(Rect {
      x:      0,
      y:      0,
      width:  180,
      height: 80,
    });

    let doc_l0 = DocumentId::default();
    let mut view = View::new(doc_l0, GutterConfig::default());
    view.area = Rect::new(0, 0, 180, 80);
    tree.insert(view);

    let l0 = tree.focus;

    let doc_r0 = DocumentId::default();
    let view = View::new(doc_r0, GutterConfig::default());
    tree.split(view, Layout::Vertical);
    let r0 = tree.focus;

    tree.focus = l0;

    let doc_l1 = DocumentId::default();
    let view = View::new(doc_l1, GutterConfig::default());
    tree.split(view, Layout::Horizontal);
    let l1 = tree.focus;

    tree.focus = l0;

    let doc_l2 = DocumentId::default();
    let view = View::new(doc_l2, GutterConfig::default());
    tree.split(view, Layout::Vertical);
    let l2 = tree.focus;

    // Views in test
    // | L0  | L2 |    |
    // |    L1    | R0 |

    // Document IDs in test
    // | l0  | l2 |    |
    // |    l1    | r0 |

    fn doc_id(tree: &Tree, view_id: ViewId) -> Option<DocumentId> {
      if let Content::View(view) = &tree.nodes[view_id].content {
        Some(view.doc)
      } else {
        None
      }
    }

    tree.focus = l0;
    // `*` marks the view in focus from view table (here L0)
    // | l0*  | l2 |    |
    // |    l1     | r0 |
    tree.swap_split_in_direction(Direction::Down);
    // | l1   | l2 |    |
    // |    l0*    | r0 |
    assert_eq!(tree.focus, l0);
    assert_eq!(doc_id(&tree, l0), Some(doc_l1));
    assert_eq!(doc_id(&tree, l1), Some(doc_l0));
    assert_eq!(doc_id(&tree, l2), Some(doc_l2));
    assert_eq!(doc_id(&tree, r0), Some(doc_r0));

    tree.swap_split_in_direction(Direction::Right);

    // | l1  | l2 |     |
    // |    r0    | l0* |
    assert_eq!(tree.focus, l0);
    assert_eq!(doc_id(&tree, l0), Some(doc_l1));
    assert_eq!(doc_id(&tree, l1), Some(doc_r0));
    assert_eq!(doc_id(&tree, l2), Some(doc_l2));
    assert_eq!(doc_id(&tree, r0), Some(doc_l0));

    // cannot swap, nothing changes
    tree.swap_split_in_direction(Direction::Up);
    // | l1  | l2 |     |
    // |    r0    | l0* |
    assert_eq!(tree.focus, l0);
    assert_eq!(doc_id(&tree, l0), Some(doc_l1));
    assert_eq!(doc_id(&tree, l1), Some(doc_r0));
    assert_eq!(doc_id(&tree, l2), Some(doc_l2));
    assert_eq!(doc_id(&tree, r0), Some(doc_l0));

    // cannot swap, nothing changes
    tree.swap_split_in_direction(Direction::Down);
    // | l1  | l2 |     |
    // |    r0    | l0* |
    assert_eq!(tree.focus, l0);
    assert_eq!(doc_id(&tree, l0), Some(doc_l1));
    assert_eq!(doc_id(&tree, l1), Some(doc_r0));
    assert_eq!(doc_id(&tree, l2), Some(doc_l2));
    assert_eq!(doc_id(&tree, r0), Some(doc_l0));

    tree.focus = l2;
    // | l1  | l2* |    |
    // |    r0     | l0 |

    tree.swap_split_in_direction(Direction::Down);
    // | l1  | r0  |    |
    // |    l2*    | l0 |
    assert_eq!(tree.focus, l2);
    assert_eq!(doc_id(&tree, l0), Some(doc_l1));
    assert_eq!(doc_id(&tree, l1), Some(doc_l2));
    assert_eq!(doc_id(&tree, l2), Some(doc_r0));
    assert_eq!(doc_id(&tree, r0), Some(doc_l0));

    tree.swap_split_in_direction(Direction::Up);
    // | l2* | r0 |    |
    // |    l1    | l0 |
    assert_eq!(tree.focus, l2);
    assert_eq!(doc_id(&tree, l0), Some(doc_l2));
    assert_eq!(doc_id(&tree, l1), Some(doc_l1));
    assert_eq!(doc_id(&tree, l2), Some(doc_r0));
    assert_eq!(doc_id(&tree, r0), Some(doc_l0));
  }

  #[test]
  fn all_vertical_views_have_same_width() {
    let tree_area_width = 180;
    let mut tree = Tree::new(Rect {
      x:      0,
      y:      0,
      width:  tree_area_width,
      height: 80,
    });
    let mut view = View::new(DocumentId::default(), GutterConfig::default());
    view.area = Rect::new(0, 0, 180, 80);
    tree.insert(view);

    let view = View::new(DocumentId::default(), GutterConfig::default());
    tree.split(view, Layout::Vertical);

    let view = View::new(DocumentId::default(), GutterConfig::default());
    tree.split(view, Layout::Horizontal);

    tree.remove(tree.focus);

    let view = View::new(DocumentId::default(), GutterConfig::default());
    tree.split(view, Layout::Vertical);

    // Make sure that we only have one level in the tree.
    assert_eq!(3, tree.views().count());
    assert_eq!(
      vec![
        tree_area_width / 3 - 1, // gap here
        tree_area_width / 3 - 1, // gap here
        tree_area_width / 3
      ],
      tree
        .views()
        .map(|(view, _)| view.area.width)
        .collect::<Vec<_>>()
    );
  }

  #[test]
  fn vsplit_gap_rounding() {
    let (tree_area_width, tree_area_height) = (80, 24);
    let mut tree = Tree::new(Rect {
      x:      0,
      y:      0,
      width:  tree_area_width,
      height: tree_area_height,
    });
    let mut view = View::new(DocumentId::default(), GutterConfig::default());
    view.area = Rect::new(0, 0, tree_area_width, tree_area_height);
    tree.insert(view);

    for _ in 0..9 {
      let view = View::new(DocumentId::default(), GutterConfig::default());
      tree.split(view, Layout::Vertical);
    }

    assert_eq!(10, tree.views().count());
    assert_eq!(
      std::iter::repeat_n(7, 9)
        .chain(Some(8)) // Rounding in `recalculate`.
        .collect::<Vec<_>>(),
      tree
        .views()
        .map(|(view, _)| view.area.width)
        .collect::<Vec<_>>()
    );
  }
}
