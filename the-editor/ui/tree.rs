use std::cmp::Ordering;

use anyhow::Result;

use crate::{
  core::{
    graphics::Rect,
    movement::Direction,
  },
  keymap::KeyBinding,
  ui::{
    components::Prompt,
    compositor::{
      Context,
      Event,
      EventResult,
      Surface,
    },
  },
};

pub trait TreeViewItem: Sized + Ord {
  type Params: Default;

  fn name(&self) -> String;
  fn is_parent(&self) -> bool;

  fn filter(&self, s: &str) -> bool {
    self.name().to_lowercase().contains(&s.to_lowercase())
  }

  fn get_children(&self) -> Result<Vec<Self>>;
}

fn tree_item_cmp<T: TreeViewItem>(item1: &T, item2: &T) -> Ordering {
  T::cmp(item1, item2)
}

fn vec_to_tree<T: TreeViewItem>(mut items: Vec<T>) -> Vec<Tree<T>> {
  items.sort();
  index_elems(
    0,
    items
      .into_iter()
      .map(|item| Tree::new(item, vec![]))
      .collect(),
  )
}

pub enum TreeOp {
  Noop,
  GetChildsAndInsert,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Tree<T> {
  item:         T,
  parent_index: Option<usize>,
  index:        usize,
  children:     Vec<Self>,

  /// Why do we need this property?
  /// Can't we just use `!children.is_empty()`?
  ///
  /// Because we might have for example an open folder that is empty,
  /// and user just added a new file under that folder,
  /// and the user refreshes the whole tree.
  ///
  /// Without `open`, we will not refresh any node without children,
  /// and thus the folder still appears empty after refreshing.
  is_opened: bool,
}

impl<T: Clone> Clone for Tree<T> {
  fn clone(&self) -> Self {
    Self {
      item:         self.item.clone(),
      index:        self.index,
      children:     self.children.clone(),
      is_opened:    self.is_opened,
      parent_index: self.parent_index,
    }
  }
}

#[derive(Clone)]
struct TreeIter<'a, T> {
  current_index_forward: usize,
  current_index_reverse: isize,
  tree:                  &'a Tree<T>,
}

impl<'a, T> Iterator for TreeIter<'a, T> {
  type Item = &'a Tree<T>;

  fn next(&mut self) -> Option<Self::Item> {
    let index = self.current_index_forward;
    if index > self.tree.len().saturating_sub(1) {
      None
    } else {
      self.current_index_forward = self.current_index_forward.saturating_add(1);
      self.tree.get(index)
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.tree.len(), Some(self.tree.len()))
  }
}

impl<'a, T> DoubleEndedIterator for TreeIter<'a, T> {
  fn next_back(&mut self) -> Option<Self::Item> {
    let index = self.current_index_reverse;
    if index < 0 {
      None
    } else {
      self.current_index_reverse = self.current_index_reverse.saturating_sub(1);
      self.tree.get(index as usize)
    }
  }
}

impl<'a, T> ExactSizeIterator for TreeIter<'a, T> {}

impl<T: TreeViewItem> Tree<T> {
  fn open(&mut self) -> Result<()> {
    if self.item.is_parent() {
      self.children = self.get_children()?;
      self.is_opened = true;
    }
    Ok(())
  }

  fn close(&mut self) {
    self.is_opened = false;
    self.children = vec![];
  }

  fn refresh(&mut self) -> Result<()> {
    if !self.is_opened {
      return Ok(());
    }
    let latest_children = self.get_children()?;
    let filtered = std::mem::take(&mut self.children)
            .into_iter()
            // Remove children that does not exists in latest_children
            .filter(|tree| {
                latest_children
                    .iter()
                    .any(|child| tree.item.name().eq(&child.item.name()))
            })
            .map(|mut tree| {
                tree.refresh()?;
                Ok(tree)
            })
            .collect::<Result<Vec<_>>>()?;

    // Add new children
    let new_nodes = latest_children
      .into_iter()
      .filter(|child| {
        !filtered
          .iter()
          .any(|child_| child.item.name().eq(&child_.item.name()))
      })
      .collect::<Vec<_>>();

    self.children = filtered.into_iter().chain(new_nodes).collect();

    self.sort();

    self.regenerate_index();

    Ok(())
  }

  fn get_children(&self) -> Result<Vec<Tree<T>>> {
    Ok(vec_to_tree(self.item.get_children()?))
  }

  fn sort(&mut self) {
    self
      .children
      .sort_by(|a, b| tree_item_cmp(&a.item, &b.item))
  }
}

impl<T> Tree<T> {
  pub fn new(item: T, children: Vec<Tree<T>>) -> Self {
    let is_opened = !children.is_empty();
    Self {
      item,
      index: 0,
      parent_index: None,
      children: index_elems(0, children),
      is_opened,
    }
  }

  fn iter(&self) -> TreeIter<'_, T> {
    TreeIter {
      tree:                  self,
      current_index_forward: 0,
      current_index_reverse: (self.len() - 1) as isize,
    }
  }

  /// Find an element in the tree with given `predicate`.
  /// `start_index` is inclusive if direction is `Forward`.
  /// `start_index` is exclusive if direction is `Backward`.
  fn find<F>(&self, start_index: usize, direction: Direction, predicate: F) -> Option<usize>
  where
    F: Clone + FnMut(&Tree<T>) -> bool,
  {
    match direction {
      Direction::Forward => {
        match self
          .iter()
          .skip(start_index)
          .position(predicate.clone())
          .map(|index| index + start_index)
        {
          Some(index) => Some(index),
          None => self.iter().position(predicate),
        }
      },

      Direction::Backward => {
        match self.iter().take(start_index).rposition(predicate.clone()) {
          Some(index) => Some(index),
          None => self.iter().rposition(predicate),
        }
      },
    }
  }

  pub fn item(&self) -> &T {
    &self.item
  }

  fn get(&self, index: usize) -> Option<&Tree<T>> {
    if self.index == index {
      Some(self)
    } else {
      self.children.iter().find_map(|elem| elem.get(index))
    }
  }

  fn get_mut(&mut self, index: usize) -> Option<&mut Tree<T>> {
    if self.index == index {
      Some(self)
    } else {
      self
        .children
        .iter_mut()
        .find_map(|elem| elem.get_mut(index))
    }
  }

  fn len(&self) -> usize {
    (1_usize).saturating_add(self.children.iter().map(|elem| elem.len()).sum())
  }

  fn regenerate_index(&mut self) {
    let items = std::mem::take(&mut self.children);
    self.children = index_elems(0, items);
  }
}

#[derive(Clone, Debug)]
struct SavedView {
  selected: usize,
  winline:  usize,
}

pub struct TreeView<T: TreeViewItem> {
  tree: Tree<T>,

  search_prompt: Option<(Direction, Prompt)>,

  search_str: String,

  /// Selected item idex
  selected: usize,

  backward_jumps: Vec<usize>,
  forward_jumps:  Vec<usize>,

  saved_view: Option<SavedView>,

  /// For implementing vertical scroll
  winline: usize,

  /// For implementing horizontal scoll
  column: usize,

  /// For implementing horizontal scoll
  max_len:           usize,
  count:             usize,
  tree_symbol_style: String,

  #[allow(clippy::type_complexity)]
  pre_render: Option<Box<dyn Fn(&mut Self, Rect) + 'static>>,

  #[allow(clippy::type_complexity)]
  on_opened_fn: Option<Box<dyn FnMut(&mut T, &mut Context, &mut T::Params) -> TreeOp + 'static>>,

  #[allow(clippy::type_complexity)]
  on_folded_fn: Option<Box<dyn FnMut(&mut T, &mut Context, &mut T::Params) + 'static>>,

  #[allow(clippy::type_complexity)]
  on_next_key: Option<Box<dyn FnMut(&mut Context, &mut Self, &KeyBinding) -> Result<()>>>,
  
  /// Cached tree indices for visible rows (updated during render)
  visible_tree_indices: Vec<usize>,
  
  /// Selection animation (0.0 -> 1.0 when selection changes)
  selection_anim: crate::core::animation::AnimationHandle<f32>,
  /// Previous selected index for animation
  prev_selected: usize,
  /// Hovered visual row (for hover glow effect)
  hovered_row: Option<usize>,
  /// Entrance animation progress (0.0 -> 1.0), None when complete
  entrance_anim: Option<crate::core::animation::AnimationHandle<f32>>,
  /// Global alpha multiplier (for closing animations, etc.)
  global_alpha: f32,
  /// Cached viewport height for scrolloff calculations
  viewport_height: usize,
}

impl<T: TreeViewItem> TreeView<T> {
  pub fn build_tree(root: T) -> Result<Self> {
    let children = root.get_children()?;
    let items = vec_to_tree(children);
    let (duration, easing) = crate::core::animation::presets::FAST;
    let (entrance_dur, entrance_ease) = crate::core::animation::presets::MEDIUM;
    Ok(Self {
      tree:                 Tree::new(root, items),
      selected:             0,
      backward_jumps:       vec![],
      forward_jumps:        vec![],
      saved_view:           None,
      winline:              0,
      column:               0,
      max_len:              0,
      count:                0,
      tree_symbol_style:    "ui.text".into(),
      pre_render:           None,
      on_opened_fn:         None,
      on_folded_fn:         None,
      on_next_key:          None,
      search_prompt:        None,
      search_str:           "".into(),
      visible_tree_indices: Vec::new(),
      selection_anim:       crate::core::animation::AnimationHandle::new(1.0, 1.0, duration, easing),
      prev_selected:        0,
      hovered_row:          None,
      entrance_anim:        Some(crate::core::animation::AnimationHandle::new(0.0, 1.0, entrance_dur, entrance_ease)),
      global_alpha:         1.0,
      viewport_height:      0, // Will be updated on first render
    })
  }

  pub fn with_enter_fn<F>(mut self, f: F) -> Self
  where
    F: FnMut(&mut T, &mut Context, &mut T::Params) -> TreeOp + 'static,
  {
    self.on_opened_fn = Some(Box::new(f));
    self
  }

  pub fn with_folded_fn<F>(mut self, f: F) -> Self
  where
    F: FnMut(&mut T, &mut Context, &mut T::Params) + 'static,
  {
    self.on_folded_fn = Some(Box::new(f));
    self
  }

  pub fn tree_symbol_style(mut self, style: String) -> Self {
    self.tree_symbol_style = style;
    self
  }

  /// Reveal item in the tree based on the given `segments`.
  ///
  /// The name of the root should be excluded.
  ///
  /// Example `segments`:
  ///
  ///    vec!["helix-term", "src", "ui", "tree.rs"]
  pub fn reveal_item(&mut self, segments: Vec<String>) -> Result<()> {
    // Expand the tree
    let root = self.tree.item.name();
    segments
      .iter()
      .fold(Ok(&mut self.tree), |current_tree, segment| {
        match current_tree {
          Err(err) => Err(err),
          Ok(current_tree) => {
            match current_tree
              .children
              .iter_mut()
              .find(|tree| tree.item.name().eq(segment))
            {
              Some(tree) => {
                if !tree.is_opened {
                  tree.open()?;
                }
                Ok(tree)
              },
              None => {
                Err(anyhow::anyhow!(format!(
                  "Unable to find path: '{}'. current_segment = '{segment}'. current_root = \
                   '{root}'",
                  segments.join("/"),
                )))
              },
            }
          },
        }
      })?;

    // Locate the item
    self.regenerate_index();
    self.set_selected(
      segments
        .iter()
        .fold(&self.tree, |tree, segment| {
          tree
            .children
            .iter()
            .find(|tree| tree.item.name().eq(segment))
            .expect("Should be unreachable")
        })
        .index,
    );

    self.align_view_center();
    Ok(())
  }

  fn align_view_center(&mut self) {
    self.pre_render = Some(Box::new(|tree, area| {
      tree.winline = area.height as usize / 2
    }))
  }

  fn align_view_top(&mut self) {
    self.winline = 0
  }

  fn align_view_bottom(&mut self) {
    self.pre_render = Some(Box::new(|tree, area| tree.winline = area.height as usize))
  }

  fn regenerate_index(&mut self) {
    self.tree.regenerate_index();
  }

  fn move_to_parent(&mut self) -> Result<()> {
    if let Some(parent) = self.current_parent()? {
      let index = parent.index;
      self.set_selected(index)
    }
    Ok(())
  }

  fn move_to_children(&mut self) -> Result<()> {
    let current = self.current_mut()?;
    if current.is_opened {
      self.set_selected(self.selected + 1);
      Ok(())
    } else {
      current.open()?;
      if !current.children.is_empty() {
        self.set_selected(self.selected + 1);
        self.regenerate_index();
      }
      Ok(())
    }
  }

  pub fn refresh(&mut self) -> Result<()> {
    self.tree.refresh()?;
    self.set_selected(self.selected);
    Ok(())
  }

  fn move_to_first_line(&mut self) {
    self.move_up(usize::MAX / 2)
  }

  fn move_to_last_line(&mut self) {
    self.move_down(usize::MAX / 2)
  }

  fn move_leftmost(&mut self) {
    self.move_left(usize::MAX / 2);
  }

  fn move_rightmost(&mut self) {
    self.move_right(usize::MAX / 2)
  }

  fn restore_saved_view(&mut self) -> Result<()> {
    if let Some(saved_view) = self.saved_view.take() {
      self.selected = saved_view.selected;
      self.winline = saved_view.winline;
      self.refresh()
    } else {
      Ok(())
    }
  }

  pub fn prompt(&self) -> Option<&Prompt> {
    if let Some((_, prompt)) = self.search_prompt.as_ref() {
      Some(prompt)
    } else {
      None
    }
  }
}

pub fn tree_view_help() -> Vec<(&'static str, &'static str)> {
  vec![
    ("o, Enter", "Open/Close"),
    ("j, down, C-n", "Down"),
    ("k, up, C-p", "Up"),
    ("h, left", "Go to parent"),
    ("l, right", "Expand"),
    ("J", "Go to next sibling"),
    ("K", "Go to previous sibling"),
    ("H", "Go to first child"),
    ("L", "Go to last child"),
    ("R", "Refresh"),
    ("/", "Search"),
    ("n", "Go to next search match"),
    ("N", "Go to previous search match"),
    ("gh, Home", "Scroll to the leftmost"),
    ("gl, End", "Scroll to the rightmost"),
    ("C-o", "Jump backward"),
    ("C-i, Tab", "Jump forward"),
    ("C-d", "Half page down"),
    ("C-u", "Half page up"),
    ("PageDown", "Full page down"),
    ("PageUp", "Full page up"),
    ("zt", "Align view top"),
    ("zz", "Align view center"),
    ("zb", "Align view bottom"),
    ("gg", "Go to first line"),
    ("ge", "Go to last line"),
  ]
}

impl<T: TreeViewItem> TreeView<T> {
  pub fn on_enter(
    &mut self,
    cx: &mut Context,
    params: &mut T::Params,
    selected_index: usize,
  ) -> Result<()> {
    let selected_item = self.get_mut(selected_index)?;
    if selected_item.is_opened {
      selected_item.close();
      self.regenerate_index();
      return Ok(());
    }

    if let Some(mut on_open_fn) = self.on_opened_fn.take() {
      let mut f = || -> Result<()> {
        let current = self.current_mut()?;
        match on_open_fn(&mut current.item, cx, params) {
          TreeOp::GetChildsAndInsert => {
            if let Err(err) = current.open() {
              cx.editor.set_error(format!("{err}"))
            }
          },
          TreeOp::Noop => {},
        };
        Ok(())
      };
      f()?;
      self.regenerate_index();
      self.on_opened_fn = Some(on_open_fn);
    };
    Ok(())
  }

  fn set_search_str(&mut self, s: String) {
    self.search_str = s;
    self.saved_view = None;
  }

  fn saved_view(&self) -> SavedView {
    self.saved_view.clone().unwrap_or(SavedView {
      selected: self.selected,
      winline:  self.winline,
    })
  }

  fn search_next(&mut self, s: &str) {
    let saved_view = self.saved_view();
    let skip = std::cmp::max(2, saved_view.selected + 1);
    self.set_selected(
      self
        .tree
        .find(skip, Direction::Forward, |e| e.item.filter(s))
        .unwrap_or(saved_view.selected),
    );
  }

  fn search_previous(&mut self, s: &str) {
    let saved_view = self.saved_view();
    let take = saved_view.selected;
    self.set_selected(
      self
        .tree
        .find(take, Direction::Backward, |e| e.item.filter(s))
        .unwrap_or(saved_view.selected),
    );
  }

  fn move_to_next_search_match(&mut self) {
    self.search_next(&self.search_str.clone())
  }

  fn move_to_previous_next_match(&mut self) {
    self.search_previous(&self.search_str.clone())
  }

  pub fn move_down(&mut self, rows: usize) {
    self.set_selected(self.selected.saturating_add(rows))
  }

  pub fn set_selected(&mut self, selected: usize) {
    let previous_selected = self.selected;
    self.set_selected_without_history(selected);
    if previous_selected.abs_diff(selected) > 1 {
      self.backward_jumps.push(previous_selected)
    }
  }

  fn set_selected_without_history(&mut self, selected: usize) {
    let selected = selected.clamp(0, self.tree.len().saturating_sub(1));
    if selected > self.selected {
      // Move down
      self.winline = selected.min(
        self
          .winline
          .saturating_add(selected.saturating_sub(self.selected)),
      );
    } else {
      // Move up
      self.winline = selected.min(
        self
          .winline
          .saturating_sub(self.selected.saturating_sub(selected)),
      );
    }
    // Trigger selection animation when selection changes
    if selected != self.selected {
      self.prev_selected = self.selected;
      self.selection_anim.retarget(1.0);
    }
    self.selected = selected
  }

  fn jump_backward(&mut self) {
    if let Some(index) = self.backward_jumps.pop() {
      self.forward_jumps.push(self.selected);
      self.set_selected_without_history(index);
    }
  }

  fn jump_forward(&mut self) {
    if let Some(index) = self.forward_jumps.pop() {
      self.set_selected(index)
    }
  }

  pub fn move_up(&mut self, rows: usize) {
    self.set_selected(self.selected.saturating_sub(rows))
  }

  /// Get the tree index for a given visual row (0-based from top of rendered area)
  pub fn tree_index_at_row(&self, visual_row: usize) -> Option<usize> {
    self.visible_tree_indices.get(visual_row).copied()
  }

  /// Select an item by its tree index
  pub fn select_by_tree_index(&mut self, tree_index: usize) {
    self.set_selected(tree_index);
  }

  /// Get the number of visible items
  pub fn visible_item_count(&self) -> usize {
    self.visible_tree_indices.len()
  }

  /// Set the hovered visual row (for hover effects)
  pub fn set_hovered_row(&mut self, row: Option<usize>) {
    self.hovered_row = row;
  }

  /// Get the currently hovered row
  pub fn hovered_row(&self) -> Option<usize> {
    self.hovered_row
  }

  /// Set the global alpha multiplier (for closing animations)
  pub fn set_global_alpha(&mut self, alpha: f32) {
    self.global_alpha = alpha;
  }

  fn move_to_next_sibling(&mut self) -> Result<()> {
    if let Some(parent) = self.current_parent()? {
      if let Some(local_index) = parent
        .children
        .iter()
        .position(|child| child.index == self.selected)
      {
        if let Some(next_sibling) = parent.children.get(local_index.saturating_add(1)) {
          self.set_selected(next_sibling.index)
        }
      }
    }
    Ok(())
  }

  fn move_to_previous_sibling(&mut self) -> Result<()> {
    if let Some(parent) = self.current_parent()? {
      if let Some(local_index) = parent
        .children
        .iter()
        .position(|child| child.index == self.selected)
      {
        if let Some(next_sibling) = parent.children.get(local_index.saturating_sub(1)) {
          self.set_selected(next_sibling.index)
        }
      }
    }
    Ok(())
  }

  fn move_to_last_sibling(&mut self) -> Result<()> {
    if let Some(parent) = self.current_parent()? {
      if let Some(last) = parent.children.last() {
        self.set_selected(last.index)
      }
    }
    Ok(())
  }

  fn move_to_first_sibling(&mut self) -> Result<()> {
    if let Some(parent) = self.current_parent()? {
      if let Some(last) = parent.children.first() {
        self.set_selected(last.index)
      }
    }
    Ok(())
  }

  fn move_left(&mut self, cols: usize) {
    self.column = self.column.saturating_sub(cols);
  }

  fn move_right(&mut self, cols: usize) {
    self.pre_render = Some(Box::new(move |tree, area| {
      let max_scroll = tree
        .max_len
        .saturating_sub(area.width as usize)
        .saturating_add(1);
      tree.column = max_scroll.min(tree.column + cols);
    }));
  }

  fn move_down_half_page(&mut self) {
    self.pre_render = Some(Box::new(|tree, area| {
      tree.move_down((area.height / 2) as usize);
    }));
  }

  fn move_up_half_page(&mut self) {
    self.pre_render = Some(Box::new(|tree, area| {
      tree.move_up((area.height / 2) as usize);
    }));
  }

  fn move_down_page(&mut self) {
    self.pre_render = Some(Box::new(|tree, area| {
      tree.move_down((area.height) as usize);
    }));
  }

  fn move_up_page(&mut self) {
    self.pre_render = Some(Box::new(|tree, area| {
      tree.move_up((area.height) as usize);
    }));
  }

  fn save_view(&mut self) {
    self.saved_view = Some(SavedView {
      selected: self.selected,
      winline:  self.winline,
    })
  }

  fn get(&self, index: usize) -> Result<&Tree<T>> {
    self.tree.get(index).ok_or_else(|| {
      anyhow::anyhow!("Programming error: TreeView.get: index {index} is out of bound")
    })
  }

  fn get_mut(&mut self, index: usize) -> Result<&mut Tree<T>> {
    self.tree.get_mut(index).ok_or_else(|| {
      anyhow::anyhow!("Programming error: TreeView.get_mut: index {index} is out of bound")
    })
  }

  pub fn current(&self) -> Result<&Tree<T>> {
    self.get(self.selected)
  }

  pub fn current_mut(&mut self) -> Result<&mut Tree<T>> {
    self.get_mut(self.selected)
  }

  fn current_parent(&self) -> Result<Option<&Tree<T>>> {
    if let Some(parent_index) = self.current()?.parent_index {
      Ok(Some(self.get(parent_index)?))
    } else {
      Ok(None)
    }
  }

  pub fn current_item(&self) -> Result<&T> {
    Ok(&self.current()?.item)
  }

  pub fn winline(&self) -> usize {
    self.winline
  }
}

#[derive(Clone)]
struct RenderedLine {
  indent:                      String,
  content:                     String,
  selected:                    bool,
  is_ancestor_of_current_item: bool,
  /// The actual tree index of this item
  tree_index:                  usize,
}
struct RenderTreeParams<'a, T> {
  tree:     &'a Tree<T>,
  prefix:   &'a String,
  level:    usize,
  selected: usize,
}

fn render_tree<T: TreeViewItem>(
  RenderTreeParams {
    tree,
    prefix,
    level,
    selected,
  }: RenderTreeParams<T>,
) -> Vec<RenderedLine> {
  let indent = if level > 0 {
    let indicator = if tree.item().is_parent() {
      if tree.is_opened { "⏷" } else { "⏵" }
    } else {
      " "
    };
    format!("{}{} ", prefix, indicator)
  } else {
    "".to_string()
  };
  let name = tree.item.name();
  let head = RenderedLine {
    indent,
    selected: selected == tree.index,
    is_ancestor_of_current_item: selected != tree.index && tree.get(selected).is_some(),
    content: name,
    tree_index: tree.index,
  };
  let prefix = format!("{}{}", prefix, if level == 0 { "" } else { "  " });
  vec![head]
    .into_iter()
    .chain(tree.children.iter().flat_map(|elem| {
      render_tree(RenderTreeParams {
        tree: elem,
        prefix: &prefix,
        level: level + 1,
        selected,
      })
    }))
    .collect()
}

impl<T: TreeViewItem + Clone> TreeView<T> {
  pub fn render(
    &mut self,
    area: Rect,
    _prompt_area: Rect,
    surface: &mut Surface,
    cx: &mut Context,
  ) {
    use the_editor_renderer::{
      Color,
      TextSection,
    };

    use crate::ui::UI_FONT_SIZE;

    // Update animations
    self.selection_anim.update(cx.dt);
    
    // Update entrance animation and clear it when complete
    let entrance_progress = if let Some(ref mut anim) = self.entrance_anim {
      anim.update(cx.dt);
      let progress = *anim.current();
      if anim.is_complete() {
        self.entrance_anim = None;
      }
      progress
    } else {
      1.0 // Animation complete, all items fully visible
    };
    
    let selection_anim_value = *self.selection_anim.current();

    // Configure font to UI font size (independent of editor font size)
    let ui_font_family = surface.current_font_family().to_owned();
    surface.configure_font(&ui_font_family, UI_FONT_SIZE);

    // Get cell dimensions for UI font
    let cell_width = surface.cell_width();

    // Get theme colors
    let theme = &cx.editor.theme;
    let text_style = theme.get("ui.text");
    let text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::WHITE);

    // Get picker-style selection colors
    let selection_style = theme.try_get("ui.selection");
    let selection_bg = selection_style
      .and_then(|s| s.bg)
      .map(crate::ui::theme_color_to_renderer_color);
    let selection_fg = selection_style
      .and_then(|s| s.fg)
      .map(crate::ui::theme_color_to_renderer_color);

    let picker_selected_style = theme.get("ui.picker.selected");
    let picker_selected_fill = picker_selected_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .or(selection_bg)
      .unwrap_or(Color::new(0.3, 0.3, 0.5, 1.0));
    let picker_selected_outline = picker_selected_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .or(selection_fg)
      .or(selection_bg)
      .unwrap_or(Color::new(0.5, 0.5, 0.8, 1.0));

    // Compact item styling matching picker
    let line_height = UI_FONT_SIZE;
    let item_padding_y = 2.0;
    let item_padding_x = 6.0;
    let item_height = line_height + item_padding_y * 2.0;
    let item_gap = 1.0;
    let item_radius = 4.0;

    // Calculate positions using UI font metrics
    let area_px_x = area.x as f32 * cell_width;
    let area_px_y = area.y as f32 * (UI_FONT_SIZE + 4.0);
    let area_px_width = area.width as f32 * cell_width;

    let lines = self.render_lines(area);
    let iter = lines.into_iter().enumerate();

    // Global alpha for closing animations
    let global_alpha = self.global_alpha;

    for (index, line) in iter {
      // Apply entrance animation - items slide in from left with staggered delay
      // Each item starts appearing after a small delay based on its position
      let (item_entrance, slide_offset) = if entrance_progress < 1.0 {
        // Stagger: item i starts at progress = i * 0.02, completes at progress = i * 0.02 + 0.5
        // This ensures all items animate within the full animation duration
        let item_start = (index as f32 * 0.015).min(0.5); // Max 50% delay for last items
        let item_duration = 0.5; // Each item takes 50% of total duration to animate
        let item_progress = ((entrance_progress - item_start) / item_duration).clamp(0.0, 1.0);
        let slide = (1.0 - item_progress) * 15.0; // Slide 15px from left
        (item_progress * global_alpha, slide)
      } else {
        (global_alpha, 0.0) // No entrance animation, apply global alpha
      };

      let item_y = area_px_y + index as f32 * (item_height + item_gap);
      let item_x = area_px_x + 4.0 - slide_offset;
      let item_width = area_px_width - 8.0;

      // Check if this row is hovered
      let is_hovered = self.hovered_row == Some(index);

      // Draw hover highlight (subtle glow effect)
      if is_hovered && !line.selected {
        let hover_bg = Color::new(
          picker_selected_fill.r,
          picker_selected_fill.g,
          picker_selected_fill.b,
          0.25 * item_entrance,
        );
        surface.draw_rounded_rect(
          item_x,
          item_y,
          item_width,
          item_height,
          item_radius,
          hover_bg,
        );
      }

      // Draw selection background if selected (picker style)
      if line.selected {
        // Selection fill - use picker colors with entrance animation
        let mut fill_color = picker_selected_fill;
        fill_color.a = fill_color.a.max(0.8) * item_entrance;
        surface.draw_rounded_rect(
          item_x,
          item_y,
          item_width,
          item_height,
          item_radius,
          fill_color,
        );

        // Selection glow effect when animation is active
        if selection_anim_value < 1.0 {
          let glow_intensity = (1.0 - selection_anim_value) * 0.4;
          let glow_color = Color::new(
            picker_selected_outline.r,
            picker_selected_outline.g,
            picker_selected_outline.b,
            glow_intensity * item_entrance,
          );
          let glow_radius = item_height * 1.5 * (1.0 + (1.0 - selection_anim_value) * 0.5);
          let center_x = item_x + item_width / 2.0;
          let center_y = item_y + item_height / 2.0;
          surface.draw_rounded_rect_glow(
            item_x,
            item_y,
            item_width,
            item_height,
            item_radius,
            center_x,
            center_y,
            glow_radius,
            glow_color,
          );
        }

        // Selection border with gradient thickness (picker style)
        let mut outline_color = picker_selected_outline;
        outline_color.a = outline_color.a.max(0.9) * item_entrance;
        let bottom_thickness = (item_height * 0.035).clamp(0.6, 1.2);
        let side_thickness = (bottom_thickness * 1.55).min(bottom_thickness + 1.6);
        let top_thickness = (bottom_thickness * 2.2).min(bottom_thickness + 2.4);
        surface.draw_rounded_rect_stroke_fade(
          item_x,
          item_y,
          item_width,
          item_height,
          item_radius,
          top_thickness,
          side_thickness,
          bottom_thickness,
          outline_color,
        );
      } else if line.is_ancestor_of_current_item {
        // Ancestor highlight (subtle)
        let ancestor_bg = Color::new(
          picker_selected_fill.r,
          picker_selected_fill.g,
          picker_selected_fill.b,
          0.15 * item_entrance,
        );
        surface.draw_rounded_rect(
          item_x,
          item_y,
          item_width,
          item_height,
          item_radius,
          ancestor_bg,
        );
      }

      // Draw indent + content
      let text_x = item_x + item_padding_x;
      let text_y = item_y + item_padding_y;
      let full_line = format!("{}{}", line.indent, line.content);

      // Use slightly different color for directories vs files
      let mut item_color = if line.content.ends_with('/') || line.indent.contains('⏵') || line.indent.contains('⏷') {
        // Directory - slightly brighter
        Color::new(
          (text_color.r + 0.1).min(1.0),
          (text_color.g + 0.1).min(1.0),
          (text_color.b + 0.05).min(1.0),
          text_color.a,
        )
      } else {
        text_color
      };
      // Apply entrance animation alpha
      item_color.a *= item_entrance;

      surface.draw_text(TextSection::simple(
        text_x,
        text_y,
        &full_line,
        UI_FONT_SIZE,
        item_color,
      ));
    }
  }

  #[cfg(test)]
  pub fn render_to_string(&mut self, area: Rect) -> String {
    let lines = self.render_lines(area);
    lines
      .into_iter()
      .map(|line| {
        let name = if line.selected {
          format!("({})", line.content)
        } else if line.is_ancestor_of_current_item {
          format!("[{}]", line.content)
        } else {
          line.content
        };
        format!("{}{}", line.indent, name)
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn render_lines(&mut self, area: Rect) -> Vec<RenderedLine> {
    if let Some(pre_render) = self.pre_render.take() {
      pre_render(self, area);
    }

    // Cache viewport height for scrolloff calculations
    self.viewport_height = area.height as usize;
    
    // Apply scrolloff to keep selection away from top/bottom edges
    // Only apply for viewports large enough (at least 2*scrolloff + 3 rows)
    const SCROLLOFF: usize = 3;
    let viewport_height = area.height as usize;
    let effective_scrolloff = if viewport_height >= SCROLLOFF * 2 + 3 {
      SCROLLOFF
    } else {
      0
    };
    
    // Clamp winline to keep selection within scrolloff bounds
    let max_winline = viewport_height.saturating_sub(1).saturating_sub(effective_scrolloff);
    let min_winline = effective_scrolloff.min(self.selected);
    self.winline = self.winline.clamp(min_winline, max_winline);
    
    // Also ensure winline doesn't exceed viewport
    self.winline = self.winline.min(area.height.saturating_sub(1) as usize);
    let skip = self.selected.saturating_sub(self.winline);
    let params = RenderTreeParams {
      tree:     &self.tree,
      prefix:   &"".to_string(),
      level:    0,
      selected: self.selected,
    };

    let lines = render_tree(params);

    self.max_len = lines
      .iter()
      .map(|line| {
        line
          .indent
          .chars()
          .count()
          .saturating_add(line.content.chars().count())
      })
      .max()
      .unwrap_or(0);

    let max_width = area.width as usize;

    let take = area.height as usize;

    struct RetainAncestorResult {
      skipped_ancestors: Vec<RenderedLine>,
      remaining_lines:   Vec<RenderedLine>,
    }
    fn retain_ancestors(lines: Vec<RenderedLine>, skip: usize) -> RetainAncestorResult {
      if skip == 0 {
        return RetainAncestorResult {
          skipped_ancestors: vec![],
          remaining_lines:   lines,
        };
      }
      if let Some(line) = lines.get(0) {
        if line.selected {
          return RetainAncestorResult {
            skipped_ancestors: vec![],
            remaining_lines:   lines,
          };
        }
      }

      let selected_index = lines.iter().position(|line| line.selected);
      let skip = match selected_index {
        None => skip,
        Some(selected_index) => skip.min(selected_index),
      };
      let (skipped, remaining) = lines.split_at(skip.min(lines.len().saturating_sub(1)));

      let skipped_ancestors = skipped
        .iter()
        .cloned()
        .filter(|line| line.is_ancestor_of_current_item)
        .collect::<Vec<_>>();

      let result = retain_ancestors(remaining.to_vec(), skipped_ancestors.len());
      RetainAncestorResult {
        skipped_ancestors: skipped_ancestors
          .into_iter()
          .chain(result.skipped_ancestors.into_iter())
          .collect(),
        remaining_lines:   result.remaining_lines,
      }
    }

    let RetainAncestorResult {
      skipped_ancestors,
      remaining_lines,
    } = retain_ancestors(lines, skip);

    let max_ancestors_len = take.saturating_sub(1);

    // Skip furthest ancestors
    let skipped_ancestors = skipped_ancestors
      .into_iter()
      .rev()
      .take(max_ancestors_len)
      .rev()
      .collect::<Vec<_>>();

    let skipped_ancestors_len = skipped_ancestors.len();

    let result: Vec<RenderedLine> = skipped_ancestors
            .into_iter()
            .chain(
                remaining_lines
                    .into_iter()
                    .take(take.saturating_sub(skipped_ancestors_len)),
            )
            // Horizontal scroll
            .map(|line| {
                let skip = self.column;
                let indent_len = line.indent.chars().count();
                RenderedLine {
                    indent: if line.indent.is_empty() {
                        "".to_string()
                    } else {
                        line.indent
                            .chars()
                            .skip(skip)
                            .take(max_width)
                            .collect::<String>()
                    },
                    content: line
                        .content
                        .chars()
                        .skip(skip.saturating_sub(indent_len))
                        .take((max_width.saturating_sub(indent_len)).clamp(0, line.content.len()))
                        .collect::<String>(),
                    ..line
                }
            })
            .collect();
    
    // Cache visible tree indices for mouse interaction
    self.visible_tree_indices = result.iter().map(|line| line.tree_index).collect();
    
    result
  }

  #[cfg(test)]
  pub fn handle_events(
    &mut self,
    events: &str,
    cx: &mut Context,
    params: &mut T::Params,
  ) -> Result<()> {
    use crate::keymap::parse_macro;

    for event in parse_macro(events)? {
      self.handle_event(&Event::Key(event), cx, params);
    }
    Ok(())
  }

  pub fn handle_event(
    &mut self,
    event: &Event,
    cx: &mut Context,
    params: &mut T::Params,
  ) -> EventResult {
    use the_editor_renderer::Key;

    let key_event = match event {
      Event::Key(event) => event,
      Event::Resize(..) => return EventResult::Consumed(None),
      _ => return EventResult::Ignored(None),
    };
    (|| -> Result<EventResult> {
      if let Some(mut on_next_key) = self.on_next_key.take() {
        on_next_key(cx, self, key_event)?;
        return Ok(EventResult::Consumed(None));
      }

      if let EventResult::Consumed(c) = self.handle_search_event(key_event, cx) {
        return Ok(EventResult::Consumed(c));
      }

      let count = std::mem::replace(&mut self.count, 0);

      // Handle digit keys for count
      if let Key::Char(c @ '0'..='9') = key_event.code {
        if !key_event.ctrl && !key_event.alt {
          self.count = c.to_digit(10).unwrap_or(0) as usize + count * 10;
          return Ok(EventResult::Consumed(None));
        }
      }

      // Handle shifted keys
      if key_event.shift && !key_event.ctrl && !key_event.alt {
        match key_event.code {
          Key::Char('J') => {
            self.move_to_next_sibling()?;
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('K') => {
            self.move_to_previous_sibling()?;
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('H') => {
            self.move_to_first_sibling()?;
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('L') => {
            self.move_to_last_sibling()?;
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('N') => {
            self.move_to_previous_next_match();
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('R') => {
            if let Err(error) = self.refresh() {
              cx.editor.set_error(error.to_string())
            }
            return Ok(EventResult::Consumed(None));
          },
          _ => {},
        }
      }

      // Handle ctrl keys
      if key_event.ctrl && !key_event.alt && !key_event.shift {
        match key_event.code {
          Key::Char('n') => {
            self.move_down(1.max(count));
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('p') => {
            self.move_up(1.max(count));
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('d') => {
            self.move_down_half_page();
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('u') => {
            self.move_up_half_page();
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('o') => {
            self.jump_backward();
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('i') => {
            self.jump_forward();
            return Ok(EventResult::Consumed(None));
          },
          _ => {},
        }
      }

      // Handle regular keys (no modifiers)
      if !key_event.ctrl && !key_event.alt && !key_event.shift {
        match key_event.code {
          Key::Char('j') | Key::Down => {
            self.move_down(1.max(count));
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('k') | Key::Up => {
            self.move_up(1.max(count));
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('h') | Key::Left => {
            self.move_to_parent()?;
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('l') | Key::Right => {
            self.move_to_children()?;
            return Ok(EventResult::Consumed(None));
          },
          Key::Enter | Key::Char('o') => {
            self.on_enter(cx, params, self.selected)?;
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('z') => {
            self.on_next_key = Some(Box::new(|_, tree, event| {
              if !event.ctrl && !event.alt && !event.shift {
                match event.code {
                  Key::Char('z') => tree.align_view_center(),
                  Key::Char('t') => tree.align_view_top(),
                  Key::Char('b') => tree.align_view_bottom(),
                  _ => {},
                }
              }
              Ok(())
            }));
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('g') => {
            self.on_next_key = Some(Box::new(|_, tree, event| {
              if !event.ctrl && !event.alt && !event.shift {
                match event.code {
                  Key::Char('g') => tree.move_to_first_line(),
                  Key::Char('e') => tree.move_to_last_line(),
                  Key::Char('h') => tree.move_leftmost(),
                  Key::Char('l') => tree.move_rightmost(),
                  _ => {},
                }
              }
              Ok(())
            }));
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('/') => {
            self.new_search_prompt(Direction::Forward);
            return Ok(EventResult::Consumed(None));
          },
          Key::Char('n') => {
            self.move_to_next_search_match();
            return Ok(EventResult::Consumed(None));
          },
          Key::PageDown => {
            self.move_down_page();
            return Ok(EventResult::Consumed(None));
          },
          Key::PageUp => {
            self.move_up_page();
            return Ok(EventResult::Consumed(None));
          },
          Key::Home => {
            self.move_leftmost();
            return Ok(EventResult::Consumed(None));
          },
          Key::End => {
            self.move_rightmost();
            return Ok(EventResult::Consumed(None));
          },
          Key::Tab => {
            self.jump_forward();
            return Ok(EventResult::Consumed(None));
          },
          _ => {},
        }
      }

      Ok(EventResult::Ignored(None))
    })()
    .unwrap_or_else(|err| {
      cx.editor.set_error(format!("{err}"));
      EventResult::Consumed(None)
    })
  }

  fn handle_search_event(&mut self, event: &KeyBinding, cx: &mut Context) -> EventResult {
    use the_editor_renderer::Key;

    use crate::ui::compositor::Component;

    if let Some((direction, mut prompt)) = self.search_prompt.take() {
      if !event.ctrl && !event.alt && !event.shift {
        match event.code {
          Key::Enter => {
            self.set_search_str(prompt.input().to_string());
            return EventResult::Consumed(None);
          },
          Key::Escape => {
            if let Err(err) = self.restore_saved_view() {
              cx.editor.set_error(format!("{err}"))
            }
            return EventResult::Consumed(None);
          },
          _ => {},
        }
      }

      let result = prompt.handle_event(&Event::Key(*event), cx);
      let line = prompt.input();
      match direction {
        Direction::Forward => self.search_next(line),
        Direction::Backward => self.search_previous(line),
      }
      self.search_prompt = Some((direction, prompt));
      result
    } else {
      EventResult::Ignored(None)
    }
  }

  fn new_search_prompt(&mut self, direction: Direction) {
    self.save_view();
    self.search_prompt = Some((direction, Prompt::new("search: ".into())))
  }

  pub fn prompting(&self) -> bool {
    self.search_prompt.is_some() || self.on_next_key.is_some()
  }
}

/// Recalculate the index of each item of a tree.
///
/// For example:
///
/// ```txt
/// foo (0)
///   bar (1)
/// spam (2)
///   jar (3)
///     yo (4)
/// ```
fn index_elems<T>(parent_index: usize, elems: Vec<Tree<T>>) -> Vec<Tree<T>> {
  fn index_elems<T>(
    current_index: usize,
    elems: Vec<Tree<T>>,
    parent_index: usize,
  ) -> (usize, Vec<Tree<T>>) {
    elems
      .into_iter()
      .fold((current_index, vec![]), |(current_index, trees), elem| {
        let index = current_index;
        let item = elem.item;
        let (current_index, folded) = index_elems(current_index + 1, elem.children, index);
        let tree = Tree {
          item,
          children: folded,
          index,
          is_opened: elem.is_opened,
          parent_index: Some(parent_index),
        };
        (
          current_index,
          trees.into_iter().chain(vec![tree].into_iter()).collect(),
        )
      })
  }
  index_elems(parent_index + 1, elems, parent_index).1
}

#[cfg(test)]
mod test_tree_view {

  use super::{
    TreeView,
    TreeViewItem,
  };
  use crate::{
    core::graphics::Rect,
    ui::compositor::Context,
  };

  #[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
  /// The children of DivisibleItem is the division of itself.
  /// This is used to ease the creation of a dummy tree without having to
  /// specify so many things.
  struct DivisibleItem<'a> {
    name: &'a str,
  }

  fn item(name: &str) -> DivisibleItem {
    DivisibleItem { name }
  }

  impl<'a> TreeViewItem for DivisibleItem<'a> {
    type Params = ();

    fn name(&self) -> String {
      self.name.to_string()
    }

    fn is_parent(&self) -> bool {
      self.name.len() > 2
    }

    fn get_children(&self) -> anyhow::Result<Vec<Self>> {
      if self.name.eq("who_lives_in_a_pineapple_under_the_sea") {
        Ok(vec![
          item("gary_the_snail"),
          item("krabby_patty"),
          item("larry_the_lobster"),
          item("patrick_star"),
          item("sandy_cheeks"),
          item("spongebob_squarepants"),
          item("mrs_puff"),
          item("king_neptune"),
          item("karen"),
          item("plankton"),
        ])
      } else if self.is_parent() {
        let (left, right) = self.name.split_at(self.name.len() / 2);
        Ok(vec![item(left), item(right)])
      } else {
        Ok(vec![])
      }
    }
  }

  fn dummy_tree_view<'a>() -> TreeView<DivisibleItem<'a>> {
    TreeView::build_tree(item("who_lives_in_a_pineapple_under_the_sea")).unwrap()
  }

  fn dummy_area() -> Rect {
    Rect::new(0, 0, 50, 5)
  }

  fn render(view: &mut TreeView<DivisibleItem>) -> String {
    view.render_to_string(dummy_area())
  }

  #[test]
  fn test_init() {
    let mut view = dummy_tree_view();

    // Expect the items to be sorted
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );
  }

  #[test]
  fn test_move_up_down() {
    let mut view = dummy_tree_view();
    view.move_down(1);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ (gary_the_snail)
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_down(3);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
"
      .trim()
    );

    view.move_down(1);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ karen
⏵ king_neptune
⏵ krabby_patty
⏵ (larry_the_lobster)
"
      .trim()
    );

    view.move_up(1);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
⏵ larry_the_lobster
"
      .trim()
    );

    view.move_up(3);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ (gary_the_snail)
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_up(1);
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_to_first_line();
    view.move_up(1);
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_to_last_line();
    view.move_down(1);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ patrick_star
⏵ plankton
⏵ sandy_cheeks
⏵ (spongebob_squarepants)
"
      .trim()
    );
  }

  #[test]
  fn test_move_to_first_last_sibling() {
    let mut view = dummy_tree_view();
    view.move_to_children().unwrap();
    view.move_to_children().unwrap();
    view.move_to_parent().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ (gary_the_snail)
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_to_last_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ patrick_star
⏵ plankton
⏵ sandy_cheeks
⏵ (spongebob_squarepants)
"
      .trim()
    );

    view.move_to_first_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ (gary_the_snail)
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
"
      .trim()
    );
  }

  #[test]
  fn test_move_to_previous_next_sibling() {
    let mut view = dummy_tree_view();
    view.move_to_children().unwrap();
    view.move_to_children().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ (e_snail)
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_to_next_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ e_snail
  ⏵ (gary_th)
⏵ karen
"
      .trim()
    );

    view.move_to_next_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ e_snail
  ⏵ (gary_th)
⏵ karen
"
      .trim()
    );

    view.move_to_previous_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ (e_snail)
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_to_previous_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ (e_snail)
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_to_parent().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ (gary_the_snail)
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_to_next_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ gary_the_snail
  ⏵ e_snail
  ⏵ gary_th
⏵ (karen)
"
      .trim()
    );

    view.move_to_previous_sibling().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ (gary_the_snail)
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
"
      .trim()
    );
  }

  #[test]
  fn test_align_view() {
    let mut view = dummy_tree_view();
    view.move_down(5);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ karen
⏵ king_neptune
⏵ krabby_patty
⏵ (larry_the_lobster)
"
      .trim()
    );

    view.align_view_center();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ krabby_patty
⏵ (larry_the_lobster)
⏵ mrs_puff
⏵ patrick_star
"
      .trim()
    );

    view.align_view_bottom();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ karen
⏵ king_neptune
⏵ krabby_patty
⏵ (larry_the_lobster)
"
      .trim()
    );
  }

  #[test]
  fn test_move_to_first_last() {
    let mut view = dummy_tree_view();

    view.move_to_last_line();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ patrick_star
⏵ plankton
⏵ sandy_cheeks
⏵ (spongebob_squarepants)
"
      .trim()
    );

    view.move_to_first_line();
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );
  }

  #[test]
  fn test_move_half() {
    let mut view = dummy_tree_view();
    view.move_down_half_page();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ (karen)
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_down_half_page();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
"
      .trim()
    );

    view.move_down_half_page();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ king_neptune
⏵ krabby_patty
⏵ larry_the_lobster
⏵ (mrs_puff)
"
      .trim()
    );

    view.move_up_half_page();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ king_neptune
⏵ (krabby_patty)
⏵ larry_the_lobster
⏵ mrs_puff
"
      .trim()
    );

    view.move_up_half_page();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ (karen)
⏵ king_neptune
⏵ krabby_patty
⏵ larry_the_lobster
"
      .trim()
    );

    view.move_up_half_page();
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );
  }

  #[test]
  fn move_to_children_parent() {
    let mut view = dummy_tree_view();
    view.move_down(1);
    view.move_to_children().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ (e_snail)
  ⏵ gary_th
⏵ karen
 "
      .trim()
    );

    view.move_down(1);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ e_snail
  ⏵ (gary_th)
⏵ karen
 "
      .trim()
    );

    view.move_to_parent().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ (gary_the_snail)
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
 "
      .trim()
    );

    view.move_to_last_line();
    view.move_to_parent().unwrap();
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏷ gary_the_snail
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
 "
      .trim()
    );
  }

  #[test]
  fn test_move_left_right() {
    let mut view = dummy_tree_view();

    fn render(view: &mut TreeView<DivisibleItem>) -> String {
      view.render_to_string(dummy_area().with_width(20))
    }

    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pinea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_right(1);
    assert_eq!(
      render(&mut view),
      "
(ho_lives_in_a_pineap)
 gary_the_snail
 karen
 king_neptune
 krabby_patty
"
      .trim()
    );

    view.move_right(1);
    assert_eq!(
      render(&mut view),
      "
(o_lives_in_a_pineapp)
gary_the_snail
karen
king_neptune
krabby_patty
"
      .trim()
    );

    view.move_right(1);
    assert_eq!(
      render(&mut view),
      "
(_lives_in_a_pineappl)
ary_the_snail
aren
ing_neptune
rabby_patty
"
      .trim()
    );

    view.move_left(1);
    assert_eq!(
      render(&mut view),
      "
(o_lives_in_a_pineapp)
gary_the_snail
karen
king_neptune
krabby_patty
"
      .trim()
    );

    view.move_leftmost();
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pinea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_left(1);
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pinea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_rightmost();
    assert_eq!(render(&mut view), "(apple_under_the_sea)\n\n\n\n");
  }

  #[test]
  fn test_move_to_parent_child() {
    let mut view = dummy_tree_view();

    view.move_to_children().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ (gary_the_snail)
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );

    view.move_to_children().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ (e_snail)
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_down(1);
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ [gary_the_snail]
  ⏵ e_snail
  ⏵ (gary_th)
⏵ karen
"
      .trim()
    );

    view.move_to_parent().unwrap();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏷ (gary_the_snail)
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_to_parent().unwrap();
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏷ gary_the_snail
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
"
      .trim()
    );

    view.move_to_parent().unwrap();
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏷ gary_the_snail
  ⏵ e_snail
  ⏵ gary_th
⏵ karen
"
      .trim()
    )
  }

  #[test]
  fn test_search_next() {
    let mut view = dummy_tree_view();

    view.search_next("pat");
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
"
      .trim()
    );

    view.search_next("larr");
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ karen
⏵ king_neptune
⏵ krabby_patty
⏵ (larry_the_lobster)
"
      .trim()
    );

    view.move_to_last_line();
    view.search_next("who_lives");
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
"
      .trim()
    );
  }

  #[test]
  fn test_search_previous() {
    let mut view = dummy_tree_view();

    view.search_previous("larry");
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ karen
⏵ king_neptune
⏵ krabby_patty
⏵ (larry_the_lobster)
"
      .trim()
    );

    view.move_to_last_line();
    view.search_previous("krab");
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
⏵ larry_the_lobster
"
      .trim()
    );
  }

  #[test]
  fn test_move_to_next_search_match() {
    let mut view = dummy_tree_view();
    view.set_search_str("pat".to_string());
    view.move_to_next_search_match();

    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
 "
      .trim()
    );

    view.move_to_next_search_match();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ krabby_patty
⏵ larry_the_lobster
⏵ mrs_puff
⏵ (patrick_star)
 "
      .trim()
    );

    view.move_to_next_search_match();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ (krabby_patty)
⏵ larry_the_lobster
⏵ mrs_puff
⏵ patrick_star
 "
      .trim()
    );
  }

  #[test]
  fn test_move_to_previous_search_match() {
    let mut view = dummy_tree_view();
    view.set_search_str("pat".to_string());
    view.move_to_previous_next_match();

    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ krabby_patty
⏵ larry_the_lobster
⏵ mrs_puff
⏵ (patrick_star)
 "
      .trim()
    );

    view.move_to_previous_next_match();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ (krabby_patty)
⏵ larry_the_lobster
⏵ mrs_puff
⏵ patrick_star
 "
      .trim()
    );

    view.move_to_previous_next_match();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ krabby_patty
⏵ larry_the_lobster
⏵ mrs_puff
⏵ (patrick_star)
 "
      .trim()
    );
  }

  #[test]
  fn test_jump_backward_forward() {
    let mut view = dummy_tree_view();
    view.move_down_half_page();
    render(&mut view);

    view.move_down_half_page();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
          "
      .trim()
    );

    view.jump_backward();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ (karen)
⏵ king_neptune
⏵ krabby_patty
          "
      .trim()
    );

    view.jump_backward();
    assert_eq!(
      render(&mut view),
      "
(who_lives_in_a_pineapple_under_the_sea)
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ krabby_patty
          "
      .trim()
    );

    view.jump_forward();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ (karen)
⏵ king_neptune
⏵ krabby_patty
          "
      .trim()
    );

    view.jump_forward();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ karen
⏵ king_neptune
⏵ (krabby_patty)
          "
      .trim()
    );

    view.jump_backward();
    assert_eq!(
      render(&mut view),
      "
[who_lives_in_a_pineapple_under_the_sea]
⏵ gary_the_snail
⏵ (karen)
⏵ king_neptune
⏵ krabby_patty
          "
      .trim()
    );
  }

  mod static_tree {
    use super::dummy_area;
    use crate::ui::{
      TreeView,
      TreeViewItem,
    };

    #[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
    /// This is used for test cases where the structure of the tree has to be
    /// known upfront
    pub struct StaticItem<'a> {
      pub name:     &'a str,
      pub children: Option<Vec<StaticItem<'a>>>,
    }

    pub fn parent<'a>(name: &'a str, children: Vec<StaticItem<'a>>) -> StaticItem<'a> {
      StaticItem {
        name,
        children: Some(children),
      }
    }

    pub fn child(name: &str) -> StaticItem {
      StaticItem {
        name,
        children: None,
      }
    }

    impl<'a> TreeViewItem for StaticItem<'a> {
      type Params = ();

      fn name(&self) -> String {
        self.name.to_string()
      }

      fn is_parent(&self) -> bool {
        self.children.is_some()
      }

      fn get_children(&self) -> anyhow::Result<Vec<Self>> {
        match &self.children {
          Some(children) => Ok(children.clone()),
          None => Ok(vec![]),
        }
      }
    }

    pub fn render(view: &mut TreeView<StaticItem<'_>>) -> String {
      view.render_to_string(dummy_area().with_height(3))
    }
  }

  #[test]
  fn test_sticky_ancestors() {
    // The ancestors of the current item should always be visible
    // However, if there's not enough space, the current item will take precedence,
    // and the nearest ancestor has higher precedence than further ancestors
    use static_tree::*;

    let mut view = TreeView::build_tree(parent("root", vec![
      parent("a", vec![child("aa"), child("ab")]),
      parent("b", vec![parent("ba", vec![parent("baa", vec![
        child("baaa"),
        child("baab"),
      ])])]),
    ]))
    .unwrap();

    assert_eq!(
      render(&mut view),
      "
(root)
⏵ a
⏵ b
          "
      .trim()
    );

    // 1. Move down to "a", and expand it
    view.move_down(1);
    view.move_to_children().unwrap();

    assert_eq!(
      render(&mut view),
      "
[root]
⏷ [a]
    (aa)
          "
      .trim()
    );

    // 2. Move down by 1
    view.move_down(1);

    // 2a. Expect all ancestors (i.e. "root" and "a") are visible,
    //     and the cursor is at "ab"
    assert_eq!(
      render(&mut view),
      "
[root]
⏷ [a]
    (ab)
          "
      .trim()
    );

    // 3. Move down by 1
    view.move_down(1);

    // 3a. Expect "a" is out of view, because it is no longer the ancestor of the
    // current item
    assert_eq!(
      render(&mut view),
      "
[root]
    ab
⏵ (b)
          "
      .trim()
    );

    // 4. Move to the children of "b", which is "ba"
    view.move_to_children().unwrap();
    assert_eq!(
      render(&mut view),
      "
[root]
⏷ [b]
  ⏵ (ba)
          "
      .trim()
    );

    // 5. Move to the children of "ba", which is "baa"
    view.move_to_children().unwrap();

    // 5a. Expect the furthest ancestor "root" is out of view,
    //     because when there's no enough space, the nearest ancestor takes
    // precedence
    assert_eq!(
      render(&mut view),
      "
⏷ [b]
  ⏷ [ba]
    ⏵ (baa)
          "
      .trim()
    );

    // 5.1 Move to child
    view.move_to_children().unwrap();
    assert_eq!(
      render(&mut view),
      "
  ⏷ [ba]
    ⏷ [baa]
        (baaa)
"
      .trim_matches('\n')
    );

    // 5.2 Move down
    view.move_down(1);
    assert_eq!(
      render(&mut view),
      "
  ⏷ [ba]
    ⏷ [baa]
        (baab)
"
      .trim_matches('\n')
    );

    // 5.3 Move up
    view.move_up(1);
    assert_eq!(view.current_item().unwrap().name, "baaa");
    assert_eq!(
      render(&mut view),
      "
  ⏷ [ba]
    ⏷ [baa]
        (baaa)
"
      .trim_matches('\n')
    );

    // 5.4 Move up
    view.move_up(1);
    assert_eq!(
      render(&mut view),
      "
⏷ [b]
  ⏷ [ba]
    ⏷ (baa)
          "
      .trim()
    );

    // 6. Move up by one
    view.move_up(1);

    // 6a. Expect "root" is visible again, because now there's enough space to
    // render all     ancestors
    assert_eq!(
      render(&mut view),
      "
[root]
⏷ [b]
  ⏷ (ba)
          "
      .trim()
    );

    // 7. Move up by one
    view.move_up(1);
    assert_eq!(
      render(&mut view),
      "
[root]
⏷ (b)
  ⏷ ba
          "
      .trim()
    );

    // 8. Move up by one
    view.move_up(1);
    assert_eq!(
      render(&mut view),
      "
[root]
⏷ [a]
    (ab)
          "
      .trim()
    );

    // 9. Move up by one
    view.move_up(1);
    assert_eq!(
      render(&mut view),
      "
[root]
⏷ [a]
    (aa)
          "
      .trim()
    );
  }

  // NOTE: test_search_prompt requires Context::dummy_* infrastructure
  // which is not yet implemented in this project.
  // #[tokio::test(flavor = "multi_thread")]
  // async fn test_search_prompt() { ... }
}

#[cfg(test)]
mod test_tree {
  use super::Tree;
  use crate::core::movement::Direction;

  #[test]
  fn test_get() {
    let result = Tree::new("root", vec![
      Tree::new("foo", vec![Tree::new("bar", vec![])]),
      Tree::new("spam", vec![Tree::new("jar", vec![Tree::new(
        "yo",
        vec![],
      )])]),
    ]);
    assert_eq!(result.get(0).unwrap().item, "root");
    assert_eq!(result.get(1).unwrap().item, "foo");
    assert_eq!(result.get(2).unwrap().item, "bar");
    assert_eq!(result.get(3).unwrap().item, "spam");
    assert_eq!(result.get(4).unwrap().item, "jar");
    assert_eq!(result.get(5).unwrap().item, "yo");
  }

  #[test]
  fn test_iter() {
    let tree = Tree::new("spam", vec![
      Tree::new("jar", vec![Tree::new("yo", vec![])]),
      Tree::new("foo", vec![Tree::new("bar", vec![])]),
    ]);

    let mut iter = tree.iter();
    assert_eq!(iter.next().map(|tree| tree.item), Some("spam"));
    assert_eq!(iter.next().map(|tree| tree.item), Some("jar"));
    assert_eq!(iter.next().map(|tree| tree.item), Some("yo"));
    assert_eq!(iter.next().map(|tree| tree.item), Some("foo"));
    assert_eq!(iter.next().map(|tree| tree.item), Some("bar"));

    assert_eq!(iter.next().map(|tree| tree.item), None)
  }

  #[test]
  fn test_iter_double_ended() {
    let tree = Tree::new("spam", vec![
      Tree::new("jar", vec![Tree::new("yo", vec![])]),
      Tree::new("foo", vec![Tree::new("bar", vec![])]),
    ]);

    let mut iter = tree.iter();
    assert_eq!(iter.next_back().map(|tree| tree.item), Some("bar"));
    assert_eq!(iter.next_back().map(|tree| tree.item), Some("foo"));
    assert_eq!(iter.next_back().map(|tree| tree.item), Some("yo"));
    assert_eq!(iter.next_back().map(|tree| tree.item), Some("jar"));
    assert_eq!(iter.next_back().map(|tree| tree.item), Some("spam"));
    assert_eq!(iter.next_back().map(|tree| tree.item), None)
  }

  #[test]
  fn test_len() {
    let tree = Tree::new("spam", vec![
      Tree::new("jar", vec![Tree::new("yo", vec![])]),
      Tree::new("foo", vec![Tree::new("bar", vec![])]),
    ]);

    assert_eq!(tree.len(), 5)
  }

  #[test]
  fn test_find_forward() {
    let tree = Tree::new(".cargo", vec![
      Tree::new("jar", vec![Tree::new("Cargo.toml", vec![])]),
      Tree::new("Cargo.toml", vec![Tree::new("bar", vec![])]),
    ]);
    let result = tree.find(0, Direction::Forward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(0));

    let result = tree.find(1, Direction::Forward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(2));

    let result = tree.find(2, Direction::Forward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(2));

    let result = tree.find(3, Direction::Forward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(3));

    let result = tree.find(4, Direction::Forward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(0));
  }

  #[test]
  fn test_find_backward() {
    let tree = Tree::new(".cargo", vec![
      Tree::new("jar", vec![Tree::new("Cargo.toml", vec![])]),
      Tree::new("Cargo.toml", vec![Tree::new("bar", vec![])]),
    ]);
    let result = tree.find(0, Direction::Backward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(3));

    let result = tree.find(1, Direction::Backward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(0));

    let result = tree.find(2, Direction::Backward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(0));

    let result = tree.find(3, Direction::Backward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(2));

    let result = tree.find(4, Direction::Backward, |tree| {
      tree.item.to_lowercase().contains(&"cargo".to_lowercase())
    });

    assert_eq!(result, Some(3));
  }
}
