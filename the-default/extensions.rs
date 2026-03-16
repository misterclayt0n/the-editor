use the_lib::render::{
  RenderPlan,
  UiTree,
  text_annotations::TextAnnotations,
};

use crate::file_picker::FilePickerItem;

pub type NamedActionFn<Ctx> = dyn Fn(&mut Ctx) + 'static;
pub type PickerQueryHandler<Ctx> = dyn Fn(&mut Ctx, &str) + 'static;
pub type PickerSubmitHandler<Ctx> =
  dyn Fn(&mut Ctx, &FilePickerItem) -> PickerSubmitResult + 'static;
pub type TextAnnotationsProvider<Ctx> = dyn for<'a> Fn(&'a Ctx, &mut TextAnnotations<'a>) + 'static;
pub type RenderPlanPostProcessor<Ctx> = dyn Fn(&mut Ctx, &mut RenderPlan) + 'static;
pub type UiTreePostProcessor<Ctx> = dyn Fn(&mut Ctx, &mut UiTree) + 'static;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PickerQueryHandlerId(usize);

impl PickerQueryHandlerId {
  pub const fn new(raw: usize) -> Self {
    Self(raw)
  }

  pub const fn get(self) -> usize {
    self.0
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PickerSubmitHandlerId(usize);

impl PickerSubmitHandlerId {
  pub const fn new(raw: usize) -> Self {
    Self(raw)
  }

  pub const fn get(self) -> usize {
    self.0
  }
}

pub struct NamedAction<Ctx> {
  name:    &'static str,
  doc:     &'static str,
  handler: Box<NamedActionFn<Ctx>>,
}

impl<Ctx> NamedAction<Ctx> {
  pub fn new<F>(name: &'static str, doc: &'static str, handler: F) -> Self
  where
    F: Fn(&mut Ctx) + 'static,
  {
    Self {
      name,
      doc,
      handler: Box::new(handler),
    }
  }

  pub fn name(&self) -> &'static str {
    self.name
  }

  pub fn doc(&self) -> &'static str {
    self.doc
  }

  pub fn execute(&self, ctx: &mut Ctx) {
    (self.handler)(ctx);
  }
}

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
      .find(|existing| existing.name() == action.name())
    {
      *existing = action;
      return;
    }
    self.actions.push(action);
  }

  pub fn get(&self, name: &str) -> Option<&NamedAction<Ctx>> {
    self.actions.iter().find(|action| action.name() == name)
  }

  pub fn doc(&self, name: &str) -> Option<&'static str> {
    self.get(name).map(NamedAction::doc)
  }

  pub fn infos(&self) -> Vec<NamedActionInfo> {
    self
      .actions
      .iter()
      .map(|action| {
        NamedActionInfo {
          name: action.name(),
          doc:  action.doc(),
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
  name:    &'static str,
  handler: Box<PickerQueryHandler<Ctx>>,
}

impl<Ctx> PickerQueryHandlerEntry<Ctx> {
  pub fn new<F>(name: &'static str, handler: F) -> Self
  where
    F: Fn(&mut Ctx, &str) + 'static,
  {
    Self {
      name,
      handler: Box::new(handler),
    }
  }

  pub fn name(&self) -> &'static str {
    self.name
  }
}

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
  pub fn register(&mut self, entry: PickerQueryHandlerEntry<Ctx>) -> PickerQueryHandlerId {
    if let Some(index) = self
      .handlers
      .iter()
      .position(|existing| existing.name() == entry.name())
    {
      self.handlers[index] = entry;
      return PickerQueryHandlerId::new(index);
    }
    self.handlers.push(entry);
    PickerQueryHandlerId::new(self.handlers.len() - 1)
  }

  pub fn id_for_name(&self, name: &str) -> Option<PickerQueryHandlerId> {
    self
      .handlers
      .iter()
      .position(|entry| entry.name() == name)
      .map(PickerQueryHandlerId::new)
  }

  pub fn execute(&self, id: PickerQueryHandlerId, ctx: &mut Ctx, query: &str) -> bool {
    let Some(entry) = self.handlers.get(id.get()) else {
      return false;
    };
    (entry.handler)(ctx, query);
    true
  }
}

pub struct PickerSubmitHandlerEntry<Ctx> {
  name:    &'static str,
  handler: Box<PickerSubmitHandler<Ctx>>,
}

impl<Ctx> PickerSubmitHandlerEntry<Ctx> {
  pub fn new<F>(name: &'static str, handler: F) -> Self
  where
    F: Fn(&mut Ctx, &FilePickerItem) -> PickerSubmitResult + 'static,
  {
    Self {
      name,
      handler: Box::new(handler),
    }
  }

  pub fn name(&self) -> &'static str {
    self.name
  }
}

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
  pub fn register(&mut self, entry: PickerSubmitHandlerEntry<Ctx>) -> PickerSubmitHandlerId {
    if let Some(index) = self
      .handlers
      .iter()
      .position(|existing| existing.name() == entry.name())
    {
      self.handlers[index] = entry;
      return PickerSubmitHandlerId::new(index);
    }
    self.handlers.push(entry);
    PickerSubmitHandlerId::new(self.handlers.len() - 1)
  }

  pub fn id_for_name(&self, name: &str) -> Option<PickerSubmitHandlerId> {
    self
      .handlers
      .iter()
      .position(|entry| entry.name() == name)
      .map(PickerSubmitHandlerId::new)
  }

  pub fn execute(
    &self,
    id: PickerSubmitHandlerId,
    ctx: &mut Ctx,
    item: &FilePickerItem,
  ) -> PickerSubmitResult {
    let Some(entry) = self.handlers.get(id.get()) else {
      return PickerSubmitResult::Unhandled;
    };
    (entry.handler)(ctx, item)
  }
}

pub struct EditorExtensions<Ctx> {
  pub named_actions:           NamedActionRegistry<Ctx>,
  pub picker_query_handlers:   PickerQueryHandlerRegistry<Ctx>,
  pub picker_submit_handlers:  PickerSubmitHandlerRegistry<Ctx>,
  text_annotations_providers:  Vec<Box<TextAnnotationsProvider<Ctx>>>,
  render_plan_post_processors: Vec<Box<RenderPlanPostProcessor<Ctx>>>,
  ui_tree_post_processors:     Vec<Box<UiTreePostProcessor<Ctx>>>,
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

  pub fn register_picker_query_handler(
    &mut self,
    entry: PickerQueryHandlerEntry<Ctx>,
  ) -> PickerQueryHandlerId {
    self.picker_query_handlers.register(entry)
  }

  pub fn register_picker_submit_handler(
    &mut self,
    entry: PickerSubmitHandlerEntry<Ctx>,
  ) -> PickerSubmitHandlerId {
    self.picker_submit_handlers.register(entry)
  }

  pub fn register_text_annotations_provider<F>(&mut self, provider: F)
  where
    F: for<'a> Fn(&'a Ctx, &mut TextAnnotations<'a>) + 'static,
  {
    self.text_annotations_providers.push(Box::new(provider));
  }

  pub fn register_render_plan_post_processor<F>(&mut self, processor: F)
  where
    F: Fn(&mut Ctx, &mut RenderPlan) + 'static,
  {
    self.render_plan_post_processors.push(Box::new(processor));
  }

  pub fn register_ui_tree_post_processor<F>(&mut self, processor: F)
  where
    F: Fn(&mut Ctx, &mut UiTree) + 'static,
  {
    self.ui_tree_post_processors.push(Box::new(processor));
  }

  pub fn picker_query_handler_id(&self, name: &str) -> Option<PickerQueryHandlerId> {
    self.picker_query_handlers.id_for_name(name)
  }

  pub fn picker_submit_handler_id(&self, name: &str) -> Option<PickerSubmitHandlerId> {
    self.picker_submit_handlers.id_for_name(name)
  }

  pub fn execute_named_action(&self, ctx: &mut Ctx, name: &str) -> bool {
    let Some(action) = self.named_actions.get(name) else {
      return false;
    };
    action.execute(ctx);
    true
  }

  pub fn handle_picker_query(&self, ctx: &mut Ctx, id: PickerQueryHandlerId, query: &str) -> bool {
    self.picker_query_handlers.execute(id, ctx, query)
  }

  pub fn submit_picker_item(
    &self,
    ctx: &mut Ctx,
    id: PickerSubmitHandlerId,
    item: &FilePickerItem,
  ) -> PickerSubmitResult {
    self.picker_submit_handlers.execute(id, ctx, item)
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
