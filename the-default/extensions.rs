use the_lib::render::{
  RenderPlan,
  UiTree,
  text_annotations::TextAnnotations,
};

use crate::{
  CommandPaletteItem,
  CommandPaletteSource,
  CompletionMenuItem,
  ContextMenuSnapshot,
  EditorContextMenuRequest,
  FileTreeNodeDecoration,
  FileTreeNodeRequest,
  FileTreeContextMenuRequest,
  Mode,
  SignatureHelpPresentation,
  file_picker::FilePickerItem,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NamedActionHandle {
  name: &'static str,
}

impl NamedActionHandle {
  pub const fn new(name: &'static str) -> Self {
    Self { name }
  }

  pub const fn name(self) -> &'static str {
    self.name
  }
}

pub type NamedActionFn<Ctx> = dyn Fn(&mut Ctx) + 'static;
pub type CommandPaletteItemProvider<Ctx> =
  dyn Fn(&mut Ctx, CommandPaletteSource, Mode, &str) -> Vec<CommandPaletteItem> + 'static;
pub type CompletionMenuItemsProvider<Ctx> = dyn Fn(&mut Ctx) -> Vec<CompletionMenuItem> + 'static;
pub type CompletionMenuSelectionHandler<Ctx> =
  dyn Fn(&mut Ctx, usize, &CompletionMenuItem) + 'static;
pub type CompletionMenuAcceptHandler<Ctx> =
  dyn Fn(&mut Ctx, usize, &CompletionMenuItem) -> bool + 'static;
pub type SignatureHelpProviderFn<Ctx> = dyn Fn(&mut Ctx) -> SignatureHelpPresentation + 'static;
pub type EditorContextMenuProvider<Ctx> =
  dyn Fn(&mut Ctx, &EditorContextMenuRequest, &mut ContextMenuSnapshot) + 'static;
pub type FileTreeContextMenuProvider<Ctx> =
  dyn Fn(&mut Ctx, &FileTreeContextMenuRequest, &mut ContextMenuSnapshot) + 'static;
pub type FileTreeNodeDecorator<Ctx> =
  dyn for<'a> Fn(&mut Ctx, &FileTreeNodeRequest<'a>, &mut FileTreeNodeDecoration) + 'static;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompletionMenuProviderId(usize);

impl CompletionMenuProviderId {
  pub const fn new(raw: usize) -> Self {
    Self(raw)
  }

  pub const fn get(self) -> usize {
    self.0
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignatureHelpProviderId(usize);

impl SignatureHelpProviderId {
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

  pub const fn handle(&self) -> NamedActionHandle {
    NamedActionHandle::new(self.name)
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
  pub handle: NamedActionHandle,
  pub name:   &'static str,
  pub doc:    &'static str,
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
  pub fn register(&mut self, action: NamedAction<Ctx>) -> NamedActionHandle {
    let handle = action.handle();
    if let Some(existing) = self
      .actions
      .iter_mut()
      .find(|existing| existing.name() == action.name())
    {
      *existing = action;
      return handle;
    }
    self.actions.push(action);
    handle
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
          handle: action.handle(),
          name:   action.name(),
          doc:    action.doc(),
        }
      })
      .collect()
  }
}

pub struct CompletionMenuProviderEntry<Ctx> {
  items:     Box<CompletionMenuItemsProvider<Ctx>>,
  on_select: Option<Box<CompletionMenuSelectionHandler<Ctx>>>,
  on_accept: Option<Box<CompletionMenuAcceptHandler<Ctx>>>,
}

impl<Ctx> CompletionMenuProviderEntry<Ctx> {
  pub fn new<F>(items: F) -> Self
  where
    F: Fn(&mut Ctx) -> Vec<CompletionMenuItem> + 'static,
  {
    Self {
      items:     Box::new(items),
      on_select: None,
      on_accept: None,
    }
  }

  pub fn on_select<F>(mut self, handler: F) -> Self
  where
    F: Fn(&mut Ctx, usize, &CompletionMenuItem) + 'static,
  {
    self.on_select = Some(Box::new(handler));
    self
  }

  pub fn on_accept<F>(mut self, handler: F) -> Self
  where
    F: Fn(&mut Ctx, usize, &CompletionMenuItem) -> bool + 'static,
  {
    self.on_accept = Some(Box::new(handler));
    self
  }
}

impl<Ctx> std::fmt::Debug for CompletionMenuProviderEntry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("CompletionMenuProviderEntry(..)")
  }
}

pub struct CompletionMenuProviderRegistry<Ctx> {
  providers: Vec<CompletionMenuProviderEntry<Ctx>>,
}

impl<Ctx> Default for CompletionMenuProviderRegistry<Ctx> {
  fn default() -> Self {
    Self {
      providers: Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for CompletionMenuProviderRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CompletionMenuProviderRegistry")
      .field("providers", &self.providers.len())
      .finish()
  }
}

impl<Ctx> CompletionMenuProviderRegistry<Ctx> {
  pub fn register(&mut self, entry: CompletionMenuProviderEntry<Ctx>) -> CompletionMenuProviderId {
    self.providers.push(entry);
    CompletionMenuProviderId::new(self.providers.len() - 1)
  }

  pub fn items(
    &self,
    id: CompletionMenuProviderId,
    ctx: &mut Ctx,
  ) -> Option<Vec<CompletionMenuItem>> {
    let entry = self.providers.get(id.get())?;
    Some((entry.items)(ctx))
  }

  pub fn selection_changed(&self, id: CompletionMenuProviderId, ctx: &mut Ctx, index: usize)
  where
    Ctx: crate::DefaultContext,
  {
    let Some(entry) = self.providers.get(id.get()) else {
      return;
    };
    let Some(handler) = &entry.on_select else {
      return;
    };
    let Some(item) = ctx_completion_menu_item(ctx, index) else {
      return;
    };
    handler(ctx, index, &item);
  }

  pub fn accept_selected(&self, id: CompletionMenuProviderId, ctx: &mut Ctx, index: usize) -> bool
  where
    Ctx: crate::DefaultContext,
  {
    let Some(entry) = self.providers.get(id.get()) else {
      return false;
    };
    let Some(handler) = &entry.on_accept else {
      return false;
    };
    let Some(item) = ctx_completion_menu_item(ctx, index) else {
      return false;
    };
    handler(ctx, index, &item)
  }
}

pub struct SignatureHelpProviderEntry<Ctx> {
  provider: Box<SignatureHelpProviderFn<Ctx>>,
}

impl<Ctx> SignatureHelpProviderEntry<Ctx> {
  pub fn new<F>(provider: F) -> Self
  where
    F: Fn(&mut Ctx) -> SignatureHelpPresentation + 'static,
  {
    Self {
      provider: Box::new(provider),
    }
  }
}

impl<Ctx> std::fmt::Debug for SignatureHelpProviderEntry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("SignatureHelpProviderEntry(..)")
  }
}

pub struct SignatureHelpProviderRegistry<Ctx> {
  providers: Vec<SignatureHelpProviderEntry<Ctx>>,
}

impl<Ctx> Default for SignatureHelpProviderRegistry<Ctx> {
  fn default() -> Self {
    Self {
      providers: Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for SignatureHelpProviderRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("SignatureHelpProviderRegistry")
      .field("providers", &self.providers.len())
      .finish()
  }
}

impl<Ctx> SignatureHelpProviderRegistry<Ctx> {
  pub fn register(&mut self, entry: SignatureHelpProviderEntry<Ctx>) -> SignatureHelpProviderId {
    self.providers.push(entry);
    SignatureHelpProviderId::new(self.providers.len() - 1)
  }

  pub fn presentation(
    &self,
    id: SignatureHelpProviderId,
    ctx: &mut Ctx,
  ) -> Option<SignatureHelpPresentation> {
    let entry = self.providers.get(id.get())?;
    Some((entry.provider)(ctx))
  }
}

pub struct EditorContextMenuProviderRegistry<Ctx> {
  providers: Vec<Box<EditorContextMenuProvider<Ctx>>>,
}

impl<Ctx> Default for EditorContextMenuProviderRegistry<Ctx> {
  fn default() -> Self {
    Self {
      providers: Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for EditorContextMenuProviderRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EditorContextMenuProviderRegistry")
      .field("providers", &self.providers.len())
      .finish()
  }
}

impl<Ctx> EditorContextMenuProviderRegistry<Ctx> {
  pub fn register<F>(&mut self, provider: F)
  where
    F: Fn(&mut Ctx, &EditorContextMenuRequest, &mut ContextMenuSnapshot) + 'static,
  {
    self.providers.push(Box::new(provider));
  }

  pub fn postprocess(
    &self,
    ctx: &mut Ctx,
    request: &EditorContextMenuRequest,
    snapshot: &mut ContextMenuSnapshot,
  ) {
    for provider in &self.providers {
      provider(ctx, request, snapshot);
    }
  }
}

pub struct FileTreeContextMenuProviderRegistry<Ctx> {
  providers: Vec<Box<FileTreeContextMenuProvider<Ctx>>>,
}

impl<Ctx> Default for FileTreeContextMenuProviderRegistry<Ctx> {
  fn default() -> Self {
    Self {
      providers: Vec::new(),
    }
  }
}

pub struct FileTreeNodeDecoratorRegistry<Ctx> {
  providers: Vec<Box<FileTreeNodeDecorator<Ctx>>>,
}

impl<Ctx> Default for FileTreeNodeDecoratorRegistry<Ctx> {
  fn default() -> Self {
    Self {
      providers: Vec::new(),
    }
  }
}

impl<Ctx> std::fmt::Debug for FileTreeNodeDecoratorRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FileTreeNodeDecoratorRegistry")
      .field("providers", &self.providers.len())
      .finish()
  }
}

impl<Ctx> FileTreeNodeDecoratorRegistry<Ctx> {
  pub fn register<F>(&mut self, provider: F)
  where
    F: for<'a> Fn(&mut Ctx, &FileTreeNodeRequest<'a>, &mut FileTreeNodeDecoration) + 'static,
  {
    self.providers.push(Box::new(provider));
  }

  pub fn decorate(
    &self,
    ctx: &mut Ctx,
    request: &FileTreeNodeRequest<'_>,
    decoration: &mut FileTreeNodeDecoration,
  ) {
    for provider in &self.providers {
      provider(ctx, request, decoration);
    }
  }
}

impl<Ctx> std::fmt::Debug for FileTreeContextMenuProviderRegistry<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FileTreeContextMenuProviderRegistry")
      .field("providers", &self.providers.len())
      .finish()
  }
}

impl<Ctx> FileTreeContextMenuProviderRegistry<Ctx> {
  pub fn register<F>(&mut self, provider: F)
  where
    F: Fn(&mut Ctx, &FileTreeContextMenuRequest, &mut ContextMenuSnapshot) + 'static,
  {
    self.providers.push(Box::new(provider));
  }

  pub fn postprocess(
    &self,
    ctx: &mut Ctx,
    request: &FileTreeContextMenuRequest,
    snapshot: &mut ContextMenuSnapshot,
  ) {
    for provider in &self.providers {
      provider(ctx, request, snapshot);
    }
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

fn ctx_completion_menu_item<Ctx>(ctx: &mut Ctx, index: usize) -> Option<CompletionMenuItem>
where
  Ctx: crate::DefaultContext,
{
  ctx.completion_menu().items.get(index).cloned()
}

#[cfg(test)]
mod tests {
  use super::{
    CommandPaletteItemProvider,
    CompletionMenuProviderEntry,
    CompletionMenuProviderRegistry,
    EditorContextMenuProviderRegistry,
    FileTreeContextMenuProviderRegistry,
    NamedAction,
    NamedActionRegistry,
    SignatureHelpProviderEntry,
    SignatureHelpProviderRegistry,
  };
  use crate::{
    CommandPaletteItem,
    CommandPaletteSource,
    CompletionMenuItem,
    ContextMenuActionId,
    ContextMenuItem,
    ContextMenuSection,
    ContextMenuSnapshot,
    EditorContextMenuOptions,
    EditorContextMenuRequest,
    FileTreeContextMenuOptions,
    FileTreeContextMenuRequest,
    Mode,
    SignatureHelpItem,
    SignatureHelpPresentation,
  };

  #[derive(Default)]
  struct TestCtx {
    queries: Vec<String>,
  }

  #[test]
  fn command_palette_providers_can_contribute_items() {
    let providers: Vec<Box<CommandPaletteItemProvider<TestCtx>>> =
      vec![Box::new(|ctx: &mut TestCtx, source, mode, query| {
        ctx.queries.push(format!("{source:?}:{mode:?}:{query}"));
        vec![CommandPaletteItem::new("demo").description("from provider")]
      })];

    let mut ctx = TestCtx::default();
    let items = providers
      .iter()
      .flat_map(|provider| {
        provider(
          &mut ctx,
          CommandPaletteSource::ActionPalette,
          Mode::Normal,
          "de",
        )
      })
      .collect::<Vec<_>>();

    assert_eq!(ctx.queries, vec!["ActionPalette:Normal:de"]);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "demo");
    assert_eq!(items[0].description.as_deref(), Some("from provider"));
  }

  #[test]
  fn named_action_registration_returns_stable_handle() {
    let mut actions = NamedActionRegistry::<TestCtx>::default();

    let first = actions.register(NamedAction::new("demo.open", "open demo", |_ctx| {}));
    let second = actions.register(NamedAction::new(
      "demo.open",
      "open demo override",
      |_ctx| {},
    ));

    assert_eq!(first, second);

    let infos = actions.infos();
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].handle, first);
    assert_eq!(infos[0].name, "demo.open");
    assert_eq!(infos[0].doc, "open demo override");
  }

  #[test]
  fn completion_menu_provider_returns_items() {
    let mut providers = CompletionMenuProviderRegistry::<TestCtx>::default();
    let provider = providers.register(CompletionMenuProviderEntry::new(|ctx: &mut TestCtx| {
      ctx.queries.push("completion".to_string());
      vec![CompletionMenuItem::new("demo-item").detail("detail")]
    }));

    let mut ctx = TestCtx::default();
    let items = providers.items(provider, &mut ctx).expect("provider items");

    assert_eq!(ctx.queries, vec!["completion"]);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "demo-item");
    assert_eq!(items[0].detail.as_deref(), Some("detail"));
  }

  #[test]
  fn signature_help_provider_returns_presentation() {
    let mut providers = SignatureHelpProviderRegistry::<TestCtx>::default();
    let provider = providers.register(SignatureHelpProviderEntry::new(|_ctx: &mut TestCtx| {
      SignatureHelpPresentation::new(
        vec![
          SignatureHelpItem::new("demo(arg)")
            .documentation("demo docs")
            .active_parameter(0),
        ],
        0,
      )
    }));

    let presentation = providers
      .presentation(provider, &mut TestCtx::default())
      .expect("signature help presentation");

    assert_eq!(presentation.active_signature, 0);
    assert_eq!(presentation.items.len(), 1);
    assert_eq!(presentation.items[0].label, "demo(arg)");
    assert_eq!(
      presentation.items[0].documentation.as_deref(),
      Some("demo docs")
    );
  }

  #[test]
  fn context_menu_providers_can_postprocess_snapshots() {
    let mut editor_providers = EditorContextMenuProviderRegistry::<TestCtx>::default();
    editor_providers.register(|ctx, request, snapshot| {
      ctx.queries.push(format!("editor:{:?}", request.char_index));
      snapshot.sections.push(
        ContextMenuSection::new()
          .title("Demo")
          .item(ContextMenuItem::new(ContextMenuActionId::EditorFormatBuffer).title("Format Demo")),
      );
    });
    let mut tree_providers = FileTreeContextMenuProviderRegistry::<TestCtx>::default();
    tree_providers.register(|ctx, request, snapshot| {
      ctx.queries.push(format!("tree:{}", request.path.display()));
      snapshot.sections.push(
        ContextMenuSection::new()
          .item(ContextMenuItem::new(ContextMenuActionId::FileTreeRename).title("Rename Demo")),
      );
    });

    let mut ctx = TestCtx::default();
    let mut editor_snapshot = ContextMenuSnapshot::new();
    let editor_request = EditorContextMenuRequest {
      char_index: Some(42),
      options:    EditorContextMenuOptions::default(),
    };
    editor_providers.postprocess(&mut ctx, &editor_request, &mut editor_snapshot);

    let mut tree_snapshot = ContextMenuSnapshot::new();
    let tree_request = FileTreeContextMenuRequest {
      path:    "demo.txt".into(),
      options: FileTreeContextMenuOptions {
        is_directory:      false,
        expanded:          false,
        is_workspace_root: false,
      },
    };
    tree_providers.postprocess(&mut ctx, &tree_request, &mut tree_snapshot);

    assert_eq!(ctx.queries, vec!["editor:Some(42)", "tree:demo.txt"]);
    assert_eq!(editor_snapshot.sections.len(), 1);
    assert_eq!(editor_snapshot.sections[0].title.as_deref(), Some("Demo"));
    assert_eq!(editor_snapshot.sections[0].items[0].title, "Format Demo");
    assert_eq!(tree_snapshot.sections.len(), 1);
    assert_eq!(tree_snapshot.sections[0].items[0].title, "Rename Demo");
  }
}
