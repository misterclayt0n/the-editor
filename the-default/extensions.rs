use the_lib::render::{
  RenderPlan,
  UiTree,
  text_annotations::TextAnnotations,
};

use crate::file_picker::FilePickerItem;

pub type NamedActionFn<Ctx> = fn(&mut Ctx);
pub type PickerQueryHandler<Ctx> = fn(&mut Ctx, &str);
pub type PickerSubmitHandler<Ctx> = fn(&mut Ctx, &FilePickerItem) -> PickerSubmitResult;
pub type TextAnnotationsProvider<Ctx> = for<'a> fn(&'a Ctx, &mut TextAnnotations<'a>);
pub type RenderPlanPostProcessor<Ctx> = fn(&mut Ctx, &mut RenderPlan);
pub type UiTreePostProcessor<Ctx> = fn(&mut Ctx, &mut UiTree);

pub struct NamedAction<Ctx> {
  pub name:    &'static str,
  pub doc:     &'static str,
  pub handler: NamedActionFn<Ctx>,
}

impl<Ctx> Clone for NamedAction<Ctx> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<Ctx> Copy for NamedAction<Ctx> {}

impl<Ctx> std::fmt::Debug for NamedAction<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NamedAction")
      .field("name", &self.name)
      .field("doc", &self.doc)
      .finish()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedActionInfo {
  pub name: &'static str,
  pub doc:  &'static str,
}

pub struct NamedActionRegistry<Ctx> {
  actions: Vec<NamedAction<Ctx>>,
}

impl<Ctx> Clone for NamedActionRegistry<Ctx> {
  fn clone(&self) -> Self {
    Self {
      actions: self.actions.iter().copied().collect(),
    }
  }
}

impl<Ctx> Default for NamedActionRegistry<Ctx> {
  fn default() -> Self {
    Self {
      actions: Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for NamedActionRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NamedActionRegistry")
      .field("actions", &self.actions)
      .finish()
  }
}

impl<Ctx> NamedActionRegistry<Ctx> {
  pub fn register(&mut self, action: NamedAction<Ctx>) {
    if let Some(existing) = self
      .actions
      .iter_mut()
      .find(|existing| existing.name == action.name)
    {
      *existing = action;
      return;
    }
    self.actions.push(action);
  }

  pub fn get(&self, name: &str) -> Option<&NamedAction<Ctx>> {
    self.actions.iter().find(|action| action.name == name)
  }

  pub fn handler(&self, name: &str) -> Option<NamedActionFn<Ctx>> {
    self.get(name).map(|action| action.handler)
  }

  pub fn doc(&self, name: &str) -> Option<&'static str> {
    self.get(name).map(|action| action.doc)
  }

  pub fn infos(&self) -> Vec<NamedActionInfo> {
    self
      .actions
      .iter()
      .map(|action| {
        NamedActionInfo {
          name: action.name,
          doc:  action.doc,
        }
      })
      .collect()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerSubmitResult {
  Unhandled,
  KeepOpen,
  Close,
}

pub struct PickerQueryHandlerEntry<Ctx> {
  pub name:    &'static str,
  pub handler: PickerQueryHandler<Ctx>,
}

impl<Ctx> Clone for PickerQueryHandlerEntry<Ctx> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<Ctx> Copy for PickerQueryHandlerEntry<Ctx> {}

impl<Ctx> std::fmt::Debug for PickerQueryHandlerEntry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PickerQueryHandlerEntry")
      .field("name", &self.name)
      .finish()
  }
}

pub struct PickerQueryHandlerRegistry<Ctx> {
  handlers: Vec<PickerQueryHandlerEntry<Ctx>>,
}

impl<Ctx> Clone for PickerQueryHandlerRegistry<Ctx> {
  fn clone(&self) -> Self {
    Self {
      handlers: self.handlers.iter().copied().collect(),
    }
  }
}

impl<Ctx> Default for PickerQueryHandlerRegistry<Ctx> {
  fn default() -> Self {
    Self {
      handlers: Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for PickerQueryHandlerRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PickerQueryHandlerRegistry")
      .field("handlers", &self.handlers.len())
      .finish()
  }
}

impl<Ctx> PickerQueryHandlerRegistry<Ctx> {
  pub fn register(&mut self, entry: PickerQueryHandlerEntry<Ctx>) {
    if let Some(existing) = self
      .handlers
      .iter_mut()
      .find(|existing| existing.name == entry.name)
    {
      *existing = entry;
      return;
    }
    self.handlers.push(entry);
  }

  pub fn handler(&self, name: &str) -> Option<PickerQueryHandler<Ctx>> {
    self
      .handlers
      .iter()
      .find(|entry| entry.name == name)
      .map(|entry| entry.handler)
  }
}

pub struct PickerSubmitHandlerEntry<Ctx> {
  pub name:    &'static str,
  pub handler: PickerSubmitHandler<Ctx>,
}

impl<Ctx> Clone for PickerSubmitHandlerEntry<Ctx> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<Ctx> Copy for PickerSubmitHandlerEntry<Ctx> {}

impl<Ctx> std::fmt::Debug for PickerSubmitHandlerEntry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PickerSubmitHandlerEntry")
      .field("name", &self.name)
      .finish()
  }
}

pub struct PickerSubmitHandlerRegistry<Ctx> {
  handlers: Vec<PickerSubmitHandlerEntry<Ctx>>,
}

impl<Ctx> Clone for PickerSubmitHandlerRegistry<Ctx> {
  fn clone(&self) -> Self {
    Self {
      handlers: self.handlers.iter().copied().collect(),
    }
  }
}

impl<Ctx> Default for PickerSubmitHandlerRegistry<Ctx> {
  fn default() -> Self {
    Self {
      handlers: Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for PickerSubmitHandlerRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PickerSubmitHandlerRegistry")
      .field("handlers", &self.handlers.len())
      .finish()
  }
}

impl<Ctx> PickerSubmitHandlerRegistry<Ctx> {
  pub fn register(&mut self, entry: PickerSubmitHandlerEntry<Ctx>) {
    if let Some(existing) = self
      .handlers
      .iter_mut()
      .find(|existing| existing.name == entry.name)
    {
      *existing = entry;
      return;
    }
    self.handlers.push(entry);
  }

  pub fn handler(&self, name: &str) -> Option<PickerSubmitHandler<Ctx>> {
    self
      .handlers
      .iter()
      .find(|entry| entry.name == name)
      .map(|entry| entry.handler)
  }
}

pub struct EditorExtensions<Ctx> {
  pub named_actions:           NamedActionRegistry<Ctx>,
  pub picker_query_handlers:   PickerQueryHandlerRegistry<Ctx>,
  pub picker_submit_handlers:  PickerSubmitHandlerRegistry<Ctx>,
  text_annotations_providers:  Vec<TextAnnotationsProvider<Ctx>>,
  render_plan_post_processors: Vec<RenderPlanPostProcessor<Ctx>>,
  ui_tree_post_processors:     Vec<UiTreePostProcessor<Ctx>>,
}

impl<Ctx> Clone for EditorExtensions<Ctx> {
  fn clone(&self) -> Self {
    Self {
      named_actions:               self.named_actions.clone(),
      picker_query_handlers:       self.picker_query_handlers.clone(),
      picker_submit_handlers:      self.picker_submit_handlers.clone(),
      text_annotations_providers:  self.text_annotations_providers.clone(),
      render_plan_post_processors: self.render_plan_post_processors.clone(),
      ui_tree_post_processors:     self.ui_tree_post_processors.clone(),
    }
  }
}

impl<Ctx> Default for EditorExtensions<Ctx> {
  fn default() -> Self {
    Self {
      named_actions:               NamedActionRegistry::default(),
      picker_query_handlers:       PickerQueryHandlerRegistry::default(),
      picker_submit_handlers:      PickerSubmitHandlerRegistry::default(),
      text_annotations_providers:  Vec::new(),
      render_plan_post_processors: Vec::new(),
      ui_tree_post_processors:     Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for EditorExtensions<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EditorExtensions")
      .field("named_actions", &self.named_actions)
      .field("picker_query_handlers", &self.picker_query_handlers)
      .field("picker_submit_handlers", &self.picker_submit_handlers)
      .field(
        "text_annotations_providers",
        &self.text_annotations_providers.len(),
      )
      .field(
        "render_plan_post_processors",
        &self.render_plan_post_processors.len(),
      )
      .field(
        "ui_tree_post_processors",
        &self.ui_tree_post_processors.len(),
      )
      .finish()
  }
}

impl<Ctx> EditorExtensions<Ctx> {
  pub fn register_named_action(&mut self, action: NamedAction<Ctx>) {
    self.named_actions.register(action);
  }

  pub fn register_picker_query_handler(&mut self, entry: PickerQueryHandlerEntry<Ctx>) {
    self.picker_query_handlers.register(entry);
  }

  pub fn register_picker_submit_handler(&mut self, entry: PickerSubmitHandlerEntry<Ctx>) {
    self.picker_submit_handlers.register(entry);
  }

  pub fn register_text_annotations_provider(&mut self, provider: TextAnnotationsProvider<Ctx>) {
    self.text_annotations_providers.push(provider);
  }

  pub fn register_render_plan_post_processor(&mut self, processor: RenderPlanPostProcessor<Ctx>) {
    self.render_plan_post_processors.push(processor);
  }

  pub fn register_ui_tree_post_processor(&mut self, processor: UiTreePostProcessor<Ctx>) {
    self.ui_tree_post_processors.push(processor);
  }

  pub fn execute_named_action(&self, ctx: &mut Ctx, name: &str) -> bool {
    let Some(handler) = self.named_actions.handler(name) else {
      return false;
    };
    handler(ctx);
    true
  }

  pub fn handle_picker_query(&self, ctx: &mut Ctx, name: &str, query: &str) -> bool {
    let Some(handler) = self.picker_query_handlers.handler(name) else {
      return false;
    };
    handler(ctx, query);
    true
  }

  pub fn submit_picker_item(
    &self,
    ctx: &mut Ctx,
    name: &str,
    item: &FilePickerItem,
  ) -> PickerSubmitResult {
    let Some(handler) = self.picker_submit_handlers.handler(name) else {
      return PickerSubmitResult::Unhandled;
    };
    handler(ctx, item)
  }

  pub fn extend_text_annotations<'a>(&self, ctx: &'a Ctx, annotations: &mut TextAnnotations<'a>) {
    for provider in &self.text_annotations_providers {
      provider(ctx, annotations);
    }
  }

  pub fn postprocess_render_plan(&self, ctx: &mut Ctx, plan: &mut RenderPlan) {
    for processor in &self.render_plan_post_processors {
      processor(ctx, plan);
    }
  }

  pub fn postprocess_ui_tree(&self, ctx: &mut Ctx, tree: &mut UiTree) {
    for processor in &self.ui_tree_post_processors {
      processor(ctx, tree);
    }
  }
}
