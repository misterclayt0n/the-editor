use the_lib::render::{
  LineNumberMode,
  OwnedTextAnnotations,
  RenderPlan,
  graphics::CursorKind,
  text_annotations::TextAnnotations,
};

use crate::{
  CommandPaletteItem,
  CommandPaletteItemProvider,
  CommandPaletteSource,
  CommandRegistry,
  CompletionMenuItem,
  CompletionMenuProviderEntry,
  CompletionMenuProviderId,
  ContextMenuSnapshot,
  DefaultApi,
  DefaultContext,
  EditorContextMenuRequest,
  ExtensionStateStore,
  FilePickerConfig,
  KeyAction,
  Keymaps,
  Mode,
  NamedAction,
  NamedActionHandle,
  OwnedTextAnnotationsProvider,
  ParseKeyBindingError,
  PickerQueryHandlerEntry,
  PickerSubmitHandlerEntry,
  RenderPlanPostProcessor,
  SignatureHelpPresentation,
  SignatureHelpProviderEntry,
  SignatureHelpProviderId,
  TextAnnotationsProvider,
  TypableCommand,
  extensions::{
    CompletionMenuProviderRegistry,
    EditorContextMenuProviderRegistry,
    NamedActionRegistry,
    PickerQueryHandlerRegistry,
    PickerSubmitHandlerRegistry,
    SignatureHelpProviderRegistry,
  },
};

pub struct CommandRegistryInstaller<Ctx: 'static> {
  installer: Box<dyn Fn(&mut CommandRegistry<Ctx>) + 'static>,
}

impl<Ctx: 'static> CommandRegistryInstaller<Ctx> {
  pub fn new<F>(installer: F) -> Self
  where
    F: Fn(&mut CommandRegistry<Ctx>) + 'static,
  {
    Self {
      installer: Box::new(installer),
    }
  }

  pub fn apply(&self, registry: &mut CommandRegistry<Ctx>) {
    (self.installer)(registry);
  }
}

impl<Ctx: 'static> std::fmt::Debug for CommandRegistryInstaller<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("CommandRegistryInstaller(..)")
  }
}

pub struct StartupHook<Ctx: 'static> {
  hook: Box<dyn Fn(&mut Ctx) + 'static>,
}

impl<Ctx: 'static> StartupHook<Ctx> {
  pub fn new<F>(hook: F) -> Self
  where
    F: Fn(&mut Ctx) + 'static,
  {
    Self {
      hook: Box::new(hook),
    }
  }

  pub fn run(&self, ctx: &mut Ctx) {
    (self.hook)(ctx);
  }
}

impl<Ctx: 'static> std::fmt::Debug for StartupHook<Ctx> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("StartupHook(..)")
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinCompletionMenuKind {
  LspCompletion,
  CodeActions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultPresetHandles {
  pub lsp_completion_menu: CompletionMenuProviderId,
  pub code_action_menu:    CompletionMenuProviderId,
  pub lsp_signature_help:  SignatureHelpProviderId,
}

pub struct CompletionMenuProviderBuilder<'a, Ctx: 'static, Dispatch> {
  preset: &'a mut EditorPreset<Ctx, Dispatch>,
  entry:  CompletionMenuProviderEntry<Ctx>,
}

impl<'a, Ctx: 'static, Dispatch> CompletionMenuProviderBuilder<'a, Ctx, Dispatch> {
  fn new<F>(preset: &'a mut EditorPreset<Ctx, Dispatch>, items: F) -> Self
  where
    F: Fn(&mut Ctx) -> Vec<CompletionMenuItem> + 'static,
  {
    Self {
      preset,
      entry: CompletionMenuProviderEntry::new(items),
    }
  }

  pub fn on_select<F>(mut self, handler: F) -> Self
  where
    F: Fn(&mut Ctx, usize, &CompletionMenuItem) + 'static,
  {
    self.entry = self.entry.on_select(handler);
    self
  }

  pub fn on_accept<F>(mut self, handler: F) -> Self
  where
    F: Fn(&mut Ctx, usize, &CompletionMenuItem) -> bool + 'static,
  {
    self.entry = self.entry.on_accept(handler);
    self
  }

  pub fn register(self) -> CompletionMenuProviderId {
    self.preset.completion_menu_providers.register(self.entry)
  }
}

impl<Ctx: 'static, Dispatch> std::fmt::Debug for CompletionMenuProviderBuilder<'_, Ctx, Dispatch> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("CompletionMenuProviderBuilder(..)")
  }
}

pub struct SignatureHelpProviderBuilder<'a, Ctx: 'static, Dispatch> {
  preset: &'a mut EditorPreset<Ctx, Dispatch>,
  entry:  SignatureHelpProviderEntry<Ctx>,
}

impl<'a, Ctx: 'static, Dispatch> SignatureHelpProviderBuilder<'a, Ctx, Dispatch> {
  fn new<F>(preset: &'a mut EditorPreset<Ctx, Dispatch>, provider: F) -> Self
  where
    F: Fn(&mut Ctx) -> SignatureHelpPresentation + 'static,
  {
    Self {
      preset,
      entry: SignatureHelpProviderEntry::new(provider),
    }
  }

  pub fn register(self) -> SignatureHelpProviderId {
    self.preset.signature_help_providers.register(self.entry)
  }
}

impl<Ctx: 'static, Dispatch> std::fmt::Debug for SignatureHelpProviderBuilder<'_, Ctx, Dispatch> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("SignatureHelpProviderBuilder(..)")
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorShapes {
  pub insert: CursorKind,
  pub normal: CursorKind,
  pub select: CursorKind,
}

impl CursorShapes {
  pub const fn new(insert: CursorKind, normal: CursorKind, select: CursorKind) -> Self {
    Self {
      insert,
      normal,
      select,
    }
  }
}

impl Default for CursorShapes {
  fn default() -> Self {
    Self::new(CursorKind::Bar, CursorKind::Block, CursorKind::Underline)
  }
}

#[derive(Debug, Clone, Default)]
pub struct EditorDefaults {
  pub line_numbers:  Option<LineNumberMode>,
  pub cursor_shapes: Option<CursorShapes>,
  pub file_picker:   Option<FilePickerConfig>,
}

impl EditorDefaults {
  pub fn line_numbers(mut self, mode: LineNumberMode) -> Self {
    self.line_numbers = Some(mode);
    self
  }

  pub fn cursor_shapes(mut self, shapes: CursorShapes) -> Self {
    self.cursor_shapes = Some(shapes);
    self
  }

  pub fn file_picker(mut self, config: FilePickerConfig) -> Self {
    self.file_picker = Some(config);
    self
  }

  fn merge(&mut self, other: Self) {
    if other.line_numbers.is_some() {
      self.line_numbers = other.line_numbers;
    }
    if other.cursor_shapes.is_some() {
      self.cursor_shapes = other.cursor_shapes;
    }
    if other.file_picker.is_some() {
      self.file_picker = other.file_picker;
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct TermDefaults {
  pub mouse: Option<bool>,
}

impl TermDefaults {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn mouse(mut self, enabled: bool) -> Self {
    self.mouse = Some(enabled);
    self
  }

  fn merge(&mut self, other: Self) {
    if other.mouse.is_some() {
      self.mouse = other.mouse;
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct ConfigDefaults {
  pub theme:  Option<String>,
  pub editor: EditorDefaults,
  pub term:   TermDefaults,
}

impl ConfigDefaults {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn theme(mut self, theme: impl Into<String>) -> Self {
    self.theme = Some(theme.into());
    self
  }

  pub fn line_numbers(mut self, mode: LineNumberMode) -> Self {
    self.editor = self.editor.line_numbers(mode);
    self
  }

  pub fn cursor_shapes(mut self, shapes: CursorShapes) -> Self {
    self.editor = self.editor.cursor_shapes(shapes);
    self
  }

  pub fn file_picker(mut self, config: FilePickerConfig) -> Self {
    self.editor = self.editor.file_picker(config);
    self
  }

  pub fn term(mut self, defaults: TermDefaults) -> Self {
    self.term.merge(defaults);
    self
  }

  fn merge(&mut self, other: Self) {
    if other.theme.is_some() {
      self.theme = other.theme;
    }
    self.editor.merge(other.editor);
    self.term.merge(other.term);
  }
}

pub struct EditorPreset<Ctx: 'static, Dispatch> {
  dispatch:                         Dispatch,
  defaults:                         ConfigDefaults,
  keymaps:                          Keymaps,
  completion_menu_keymaps:          Keymaps,
  commands:                         Vec<TypableCommand<Ctx>>,
  named_actions:                    NamedActionRegistry<Ctx>,
  command_palette_providers:        Vec<Box<CommandPaletteItemProvider<Ctx>>>,
  completion_menu_providers:        CompletionMenuProviderRegistry<Ctx>,
  signature_help_providers:         SignatureHelpProviderRegistry<Ctx>,
  editor_context_menu_providers:    EditorContextMenuProviderRegistry<Ctx>,
  picker_query_handlers:            PickerQueryHandlerRegistry<Ctx>,
  picker_submit_handlers:           PickerSubmitHandlerRegistry<Ctx>,
  text_annotations_providers:       Vec<Box<TextAnnotationsProvider<Ctx>>>,
  owned_text_annotations_providers: Vec<Box<OwnedTextAnnotationsProvider<Ctx>>>,
  render_plan_post_processors:      Vec<Box<RenderPlanPostProcessor<Ctx>>>,
  extension_state:                  ExtensionStateStore,
  command_registry_installers:      Vec<CommandRegistryInstaller<Ctx>>,
  startup_hooks:                    Vec<StartupHook<Ctx>>,
}

impl<Ctx: 'static, Dispatch> EditorPreset<Ctx, Dispatch> {
  pub fn new(dispatch: Dispatch, keymaps: Keymaps) -> Self {
    Self {
      dispatch,
      defaults: ConfigDefaults::default(),
      keymaps,
      completion_menu_keymaps: crate::builtin_completion_menu_keymaps(),
      commands: Vec::new(),
      named_actions: NamedActionRegistry::default(),
      command_palette_providers: Vec::new(),
      completion_menu_providers: CompletionMenuProviderRegistry::default(),
      signature_help_providers: SignatureHelpProviderRegistry::default(),
      editor_context_menu_providers: EditorContextMenuProviderRegistry::default(),
      picker_query_handlers: PickerQueryHandlerRegistry::default(),
      picker_submit_handlers: PickerSubmitHandlerRegistry::default(),
      text_annotations_providers: Vec::new(),
      owned_text_annotations_providers: Vec::new(),
      render_plan_post_processors: Vec::new(),
      extension_state: ExtensionStateStore::default(),
      command_registry_installers: Vec::new(),
      startup_hooks: Vec::new(),
    }
  }

  pub fn dispatch(&self) -> &Dispatch {
    &self.dispatch
  }

  pub fn dispatch_mut(&mut self) -> &mut Dispatch {
    &mut self.dispatch
  }

  pub fn defaults(&self) -> &ConfigDefaults {
    &self.defaults
  }

  pub fn keymaps(&self) -> &Keymaps {
    &self.keymaps
  }

  pub fn keymaps_mut(&mut self) -> &mut Keymaps {
    &mut self.keymaps
  }

  pub fn extension_state(&self) -> &ExtensionStateStore {
    &self.extension_state
  }

  pub fn extension_state_mut(&mut self) -> &mut ExtensionStateStore {
    &mut self.extension_state
  }

  pub fn command_registry_installers(&self) -> &[CommandRegistryInstaller<Ctx>] {
    &self.command_registry_installers
  }

  pub fn startup_hooks(&self) -> &[StartupHook<Ctx>] {
    &self.startup_hooks
  }

  pub fn with_dispatch<NewDispatch>(self, dispatch: NewDispatch) -> EditorPreset<Ctx, NewDispatch> {
    EditorPreset {
      dispatch,
      defaults: self.defaults,
      keymaps: self.keymaps,
      completion_menu_keymaps: self.completion_menu_keymaps,
      commands: self.commands,
      named_actions: self.named_actions,
      command_palette_providers: self.command_palette_providers,
      completion_menu_providers: self.completion_menu_providers,
      signature_help_providers: self.signature_help_providers,
      editor_context_menu_providers: self.editor_context_menu_providers,
      picker_query_handlers: self.picker_query_handlers,
      picker_submit_handlers: self.picker_submit_handlers,
      text_annotations_providers: self.text_annotations_providers,
      owned_text_annotations_providers: self.owned_text_annotations_providers,
      render_plan_post_processors: self.render_plan_post_processors,
      extension_state: self.extension_state,
      command_registry_installers: self.command_registry_installers,
      startup_hooks: self.startup_hooks,
    }
  }

  pub fn with_keymaps(mut self, keymaps: Keymaps) -> Self {
    self.keymaps = keymaps;
    self
  }

  pub fn with_defaults(mut self, defaults: ConfigDefaults) -> Self {
    self.defaults.merge(defaults);
    self
  }

  pub fn with_completion_menu_keymaps(mut self, keymaps: Keymaps) -> Self {
    self.completion_menu_keymaps = keymaps;
    self
  }

  pub fn merge_keymaps(mut self, keymaps: Keymaps) -> Self {
    self.keymaps.merge(keymaps);
    self
  }

  pub fn merge_completion_menu_keymaps(mut self, keymaps: Keymaps) -> Self {
    self.completion_menu_keymaps.merge(keymaps);
    self
  }

  pub fn bind_key<L>(
    mut self,
    mode: Mode,
    binding: L,
    action: KeyAction,
  ) -> Result<Self, ParseKeyBindingError>
  where
    L: crate::IntoKeyBinding,
  {
    self.keymaps.bind(mode, binding, action)?;
    Ok(self)
  }

  pub fn bind_key_sequence<I, L>(
    mut self,
    mode: Mode,
    bindings: I,
    action: KeyAction,
  ) -> Result<Self, ParseKeyBindingError>
  where
    I: IntoIterator<Item = L>,
    L: crate::IntoKeyBinding,
  {
    self.keymaps.bind_sequence(mode, bindings, action)?;
    Ok(self)
  }

  pub fn bind_completion_menu_key<L>(
    mut self,
    mode: Mode,
    binding: L,
    action: KeyAction,
  ) -> Result<Self, ParseKeyBindingError>
  where
    L: crate::IntoKeyBinding,
  {
    self.completion_menu_keymaps.bind(mode, binding, action)?;
    Ok(self)
  }

  pub fn bind_completion_menu_key_sequence<I, L>(
    mut self,
    mode: Mode,
    bindings: I,
    action: KeyAction,
  ) -> Result<Self, ParseKeyBindingError>
  where
    I: IntoIterator<Item = L>,
    L: crate::IntoKeyBinding,
  {
    self
      .completion_menu_keymaps
      .bind_sequence(mode, bindings, action)?;
    Ok(self)
  }

  pub fn install_command(mut self, command: TypableCommand<Ctx>) -> Self {
    self.commands.push(command);
    self
  }

  pub fn register_named_action(&mut self, action: NamedAction<Ctx>) -> NamedActionHandle {
    self.named_actions.register(action)
  }

  pub fn install_named_action(mut self, action: NamedAction<Ctx>) -> Self {
    self.named_actions.register(action);
    self
  }

  pub fn install_command_palette_item_provider<F>(mut self, provider: F) -> Self
  where
    F: Fn(&mut Ctx, CommandPaletteSource, Mode, &str) -> Vec<CommandPaletteItem> + 'static,
  {
    self.command_palette_providers.push(Box::new(provider));
    self
  }

  pub fn install_named_action_with_binding<I, L>(
    mut self,
    action: NamedAction<Ctx>,
    mode: Mode,
    bindings: I,
  ) -> Result<Self, ParseKeyBindingError>
  where
    I: IntoIterator<Item = L>,
    L: crate::IntoKeyBinding,
  {
    let handle = self.named_actions.register(action);
    self
      .keymaps
      .bind_sequence(mode, bindings, KeyAction::NamedHandle(handle))?;
    Ok(self)
  }

  pub fn register_named_action_with_binding<I, L>(
    &mut self,
    action: NamedAction<Ctx>,
    mode: Mode,
    bindings: I,
  ) -> Result<NamedActionHandle, ParseKeyBindingError>
  where
    I: IntoIterator<Item = L>,
    L: crate::IntoKeyBinding,
  {
    let handle = self.named_actions.register(action);
    self
      .keymaps
      .bind_sequence(mode, bindings, KeyAction::NamedHandle(handle))?;
    Ok(handle)
  }

  pub fn install_extension_state<T>(mut self, state: T) -> Self
  where
    T: 'static,
  {
    let _ = self.extension_state.insert(state);
    self
  }

  pub fn install_picker_query_handler(mut self, entry: PickerQueryHandlerEntry<Ctx>) -> Self {
    self.picker_query_handlers.register(entry);
    self
  }

  pub fn install_picker_submit_handler(mut self, entry: PickerSubmitHandlerEntry<Ctx>) -> Self {
    self.picker_submit_handlers.register(entry);
    self
  }

  pub fn register_completion_menu_provider(
    &mut self,
    provider: CompletionMenuProviderEntry<Ctx>,
  ) -> CompletionMenuProviderId {
    self.completion_menu_providers.register(provider)
  }

  pub fn completion_menu_provider<F>(
    &mut self,
    items: F,
  ) -> CompletionMenuProviderBuilder<'_, Ctx, Dispatch>
  where
    F: Fn(&mut Ctx) -> Vec<CompletionMenuItem> + 'static,
  {
    CompletionMenuProviderBuilder::new(self, items)
  }

  pub fn install_completion_menu_provider(
    mut self,
    provider: CompletionMenuProviderEntry<Ctx>,
  ) -> Self {
    self.completion_menu_providers.register(provider);
    self
  }

  pub fn register_signature_help_provider(
    &mut self,
    provider: SignatureHelpProviderEntry<Ctx>,
  ) -> SignatureHelpProviderId {
    self.signature_help_providers.register(provider)
  }

  pub fn signature_help_provider<F>(
    &mut self,
    provider: F,
  ) -> SignatureHelpProviderBuilder<'_, Ctx, Dispatch>
  where
    F: Fn(&mut Ctx) -> SignatureHelpPresentation + 'static,
  {
    SignatureHelpProviderBuilder::new(self, provider)
  }

  pub fn install_signature_help_provider(
    mut self,
    provider: SignatureHelpProviderEntry<Ctx>,
  ) -> Self {
    self.signature_help_providers.register(provider);
    self
  }

  pub fn install_editor_context_menu_provider<F>(mut self, provider: F) -> Self
  where
    F: Fn(&mut Ctx, &EditorContextMenuRequest, &mut ContextMenuSnapshot) + 'static,
  {
    self.editor_context_menu_providers.register(provider);
    self
  }

  pub fn install_text_annotations_provider<F>(mut self, provider: F) -> Self
  where
    F: for<'a> Fn(&'a Ctx, &mut TextAnnotations<'a>) + 'static,
  {
    self.text_annotations_providers.push(Box::new(provider));
    self
  }

  pub fn install_owned_text_annotations_provider<F>(mut self, provider: F) -> Self
  where
    F: Fn(&Ctx, &mut OwnedTextAnnotations) + 'static,
  {
    self
      .owned_text_annotations_providers
      .push(Box::new(provider));
    self
  }

  pub fn install_render_plan_post_processor<F>(mut self, processor: F) -> Self
  where
    F: Fn(&mut Ctx, &mut RenderPlan) + 'static,
  {
    self.render_plan_post_processors.push(Box::new(processor));
    self
  }

  pub fn install_command_registry<F>(mut self, installer: F) -> Self
  where
    F: Fn(&mut CommandRegistry<Ctx>) + 'static,
  {
    self
      .command_registry_installers
      .push(CommandRegistryInstaller::new(installer));
    self
  }

  pub fn install_startup_hook<F>(mut self, hook: F) -> Self
  where
    F: Fn(&mut Ctx) + 'static,
  {
    self.startup_hooks.push(StartupHook::new(hook));
    self
  }

  pub fn build(self) -> BuiltEditorPreset<Ctx, Dispatch>
  where
    Ctx: DefaultContext,
  {
    let mut command_registry = CommandRegistry::new();
    for command in self.commands {
      command_registry.register(command);
    }
    for installer in self.command_registry_installers {
      installer.apply(&mut command_registry);
    }

    BuiltEditorPreset {
      dispatch: self.dispatch,
      defaults: self.defaults,
      keymaps: self.keymaps,
      completion_menu_keymaps: self.completion_menu_keymaps,
      command_registry,
      named_actions: self.named_actions,
      command_palette_providers: self.command_palette_providers,
      completion_menu_providers: self.completion_menu_providers,
      signature_help_providers: self.signature_help_providers,
      editor_context_menu_providers: self.editor_context_menu_providers,
      picker_query_handlers: self.picker_query_handlers,
      picker_submit_handlers: self.picker_submit_handlers,
      text_annotations_providers: self.text_annotations_providers,
      owned_text_annotations_providers: self.owned_text_annotations_providers,
      render_plan_post_processors: self.render_plan_post_processors,
      extension_state: self.extension_state,
      startup_hooks: self.startup_hooks,
    }
  }
}

pub struct BuiltEditorPreset<Ctx: 'static, Dispatch> {
  dispatch:                         Dispatch,
  defaults:                         ConfigDefaults,
  keymaps:                          Keymaps,
  completion_menu_keymaps:          Keymaps,
  command_registry:                 CommandRegistry<Ctx>,
  named_actions:                    NamedActionRegistry<Ctx>,
  command_palette_providers:        Vec<Box<CommandPaletteItemProvider<Ctx>>>,
  completion_menu_providers:        CompletionMenuProviderRegistry<Ctx>,
  signature_help_providers:         SignatureHelpProviderRegistry<Ctx>,
  editor_context_menu_providers:    EditorContextMenuProviderRegistry<Ctx>,
  picker_query_handlers:            PickerQueryHandlerRegistry<Ctx>,
  picker_submit_handlers:           PickerSubmitHandlerRegistry<Ctx>,
  text_annotations_providers:       Vec<Box<TextAnnotationsProvider<Ctx>>>,
  owned_text_annotations_providers: Vec<Box<OwnedTextAnnotationsProvider<Ctx>>>,
  render_plan_post_processors:      Vec<Box<RenderPlanPostProcessor<Ctx>>>,
  extension_state:                  ExtensionStateStore,
  startup_hooks:                    Vec<StartupHook<Ctx>>,
}

impl<Ctx: 'static, Dispatch> BuiltEditorPreset<Ctx, Dispatch> {
  pub fn dispatch(&self) -> &Dispatch {
    &self.dispatch
  }

  pub fn keymaps(&self) -> &Keymaps {
    &self.keymaps
  }

  pub fn defaults(&self) -> &ConfigDefaults {
    &self.defaults
  }

  pub fn keymaps_mut(&mut self) -> &mut Keymaps {
    &mut self.keymaps
  }

  pub fn completion_menu_keymaps(&self) -> &Keymaps {
    &self.completion_menu_keymaps
  }

  pub fn completion_menu_keymaps_mut(&mut self) -> &mut Keymaps {
    &mut self.completion_menu_keymaps
  }

  pub fn command_registry(&self) -> &CommandRegistry<Ctx> {
    &self.command_registry
  }

  pub fn command_registry_mut(&mut self) -> &mut CommandRegistry<Ctx> {
    &mut self.command_registry
  }

  pub fn extension_state(&self) -> &ExtensionStateStore {
    &self.extension_state
  }

  pub fn extension_state_mut(&mut self) -> &mut ExtensionStateStore {
    &mut self.extension_state
  }

  pub fn startup_hooks(&self) -> &[StartupHook<Ctx>] {
    &self.startup_hooks
  }

  pub fn take_startup_hooks(&mut self) -> Vec<StartupHook<Ctx>> {
    std::mem::take(&mut self.startup_hooks)
  }

  pub fn run_startup_hooks(&self, ctx: &mut Ctx) {
    for hook in &self.startup_hooks {
      hook.run(ctx);
    }
  }

  pub fn named_action_names(&self) -> Vec<&'static str> {
    self
      .named_actions
      .infos()
      .into_iter()
      .map(|info| info.name)
      .collect()
  }

  pub fn named_action_doc(&self, name: &str) -> Option<&'static str> {
    self.named_actions.doc(name)
  }

  pub fn execute_named_action(&self, ctx: &mut Ctx, name: &str) -> bool {
    match self.named_actions.get(name) {
      Some(action) => {
        action.execute(ctx);
        true
      },
      None => false,
    }
  }

  pub fn command_palette_items(
    &self,
    ctx: &mut Ctx,
    source: CommandPaletteSource,
    mode: Mode,
    query: &str,
  ) -> Vec<CommandPaletteItem> {
    let mut items = Vec::new();
    for provider in &self.command_palette_providers {
      items.extend(provider(ctx, source, mode, query));
    }
    items
  }

  pub fn completion_menu_items(
    &self,
    ctx: &mut Ctx,
    provider: CompletionMenuProviderId,
  ) -> Option<Vec<crate::CompletionMenuItem>>
  where
    Ctx: DefaultContext,
  {
    self.completion_menu_providers.items(provider, ctx)
  }

  pub fn completion_menu_selection_changed(
    &self,
    ctx: &mut Ctx,
    provider: CompletionMenuProviderId,
    index: usize,
  ) where
    Ctx: DefaultContext,
  {
    self
      .completion_menu_providers
      .selection_changed(provider, ctx, index);
  }

  pub fn completion_menu_accept_selected(
    &self,
    ctx: &mut Ctx,
    provider: CompletionMenuProviderId,
    index: usize,
  ) -> bool
  where
    Ctx: DefaultContext,
  {
    self
      .completion_menu_providers
      .accept_selected(provider, ctx, index)
  }

  pub fn signature_help_presentation(
    &self,
    ctx: &mut Ctx,
    provider: SignatureHelpProviderId,
  ) -> Option<SignatureHelpPresentation> {
    self.signature_help_providers.presentation(provider, ctx)
  }

  pub fn postprocess_editor_context_menu(
    &self,
    ctx: &mut Ctx,
    request: &EditorContextMenuRequest,
    snapshot: &mut ContextMenuSnapshot,
  ) {
    self
      .editor_context_menu_providers
      .postprocess(ctx, request, snapshot);
  }

  pub fn picker_query_handler_id(&self, name: &str) -> Option<crate::PickerQueryHandlerId> {
    self.picker_query_handlers.id_for_name(name)
  }

  pub fn picker_submit_handler_id(&self, name: &str) -> Option<crate::PickerSubmitHandlerId> {
    self.picker_submit_handlers.id_for_name(name)
  }

  pub fn handle_picker_query(
    &self,
    ctx: &mut Ctx,
    id: crate::PickerQueryHandlerId,
    query: &str,
  ) -> bool {
    self.picker_query_handlers.execute(id, ctx, query)
  }

  pub fn submit_picker_item(
    &self,
    ctx: &mut Ctx,
    id: crate::PickerSubmitHandlerId,
    item: &crate::file_picker::FilePickerItem,
  ) -> crate::PickerSubmitResult {
    self.picker_submit_handlers.execute(id, ctx, item)
  }

  pub fn extend_text_annotations<'a>(
    &'a self,
    ctx: &'a Ctx,
    annotations: &mut TextAnnotations<'a>,
  ) {
    for provider in &self.text_annotations_providers {
      provider(ctx, annotations);
    }
  }

  pub fn extend_owned_text_annotations(&self, ctx: &Ctx, annotations: &mut OwnedTextAnnotations) {
    for provider in &self.owned_text_annotations_providers {
      provider(ctx, annotations);
    }
  }

  pub fn postprocess_render_plan(&self, ctx: &mut Ctx, plan: &mut RenderPlan) {
    for processor in &self.render_plan_post_processors {
      processor(ctx, plan);
    }
  }
}

impl<Ctx: 'static, Dispatch> BuiltEditorPreset<Ctx, Dispatch>
where
  Dispatch: DefaultApi<Ctx> + 'static,
{
  pub fn box_dispatch(self) -> BuiltEditorPreset<Ctx, Box<dyn DefaultApi<Ctx>>> {
    BuiltEditorPreset {
      dispatch:                         Box::new(self.dispatch),
      defaults:                         self.defaults,
      keymaps:                          self.keymaps,
      completion_menu_keymaps:          self.completion_menu_keymaps,
      command_registry:                 self.command_registry,
      named_actions:                    self.named_actions,
      command_palette_providers:        self.command_palette_providers,
      completion_menu_providers:        self.completion_menu_providers,
      signature_help_providers:         self.signature_help_providers,
      editor_context_menu_providers:    self.editor_context_menu_providers,
      picker_query_handlers:            self.picker_query_handlers,
      picker_submit_handlers:           self.picker_submit_handlers,
      text_annotations_providers:       self.text_annotations_providers,
      owned_text_annotations_providers: self.owned_text_annotations_providers,
      render_plan_post_processors:      self.render_plan_post_processors,
      extension_state:                  self.extension_state,
      startup_hooks:                    self.startup_hooks,
    }
  }
}

fn default_editor_context_menu_provider<Ctx: DefaultContext>(
  _ctx: &mut Ctx,
  request: &EditorContextMenuRequest,
  snapshot: &mut ContextMenuSnapshot,
) {
  snapshot.sections.extend(
    crate::build_editor_context_menu(request.options)
      .sections
      .into_iter(),
  );
}

pub fn default_preset_handles<Ctx: DefaultContext>(ctx: &Ctx) -> Option<&DefaultPresetHandles> {
  ctx.extension_state::<DefaultPresetHandles>()
}

pub fn show_builtin_completion_menu<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  kind: BuiltinCompletionMenuKind,
) {
  let Some(handles) = default_preset_handles(ctx).copied() else {
    crate::close_completion_menu(ctx);
    return;
  };
  let provider = match kind {
    BuiltinCompletionMenuKind::LspCompletion => handles.lsp_completion_menu,
    BuiltinCompletionMenuKind::CodeActions => handles.code_action_menu,
  };
  crate::show_completion_menu_provider(ctx, provider);
}

pub fn show_builtin_signature_help<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(handles) = default_preset_handles(ctx).copied() else {
    crate::close_signature_help(ctx);
    return;
  };
  crate::show_signature_help_provider(ctx, handles.lsp_signature_help);
}

pub fn default_editor_preset<Ctx>() -> EditorPreset<Ctx, impl DefaultApi<Ctx>>
where
  Ctx: DefaultContext,
{
  let mut preset = EditorPreset::new(crate::build_dispatch::<Ctx>(), crate::builtin_keymaps())
    .install_command_registry(crate::install_builtin_commands::<Ctx>)
    .install_editor_context_menu_provider(default_editor_context_menu_provider::<Ctx>);

  let handles = DefaultPresetHandles {
    lsp_completion_menu: preset
      .completion_menu_provider(|ctx| {
        ctx.builtin_completion_menu_items(BuiltinCompletionMenuKind::LspCompletion)
      })
      .on_select(|ctx, index, _item| {
        ctx.completion_selection_changed(index);
      })
      .on_accept(|ctx, index, _item| ctx.completion_accept_selected(index))
      .register(),
    code_action_menu:    preset
      .completion_menu_provider(|ctx| {
        ctx.builtin_completion_menu_items(BuiltinCompletionMenuKind::CodeActions)
      })
      .on_accept(|ctx, index, _item| ctx.completion_accept_selected(index))
      .register(),
    lsp_signature_help:  preset
      .signature_help_provider(|ctx| {
        ctx
          .builtin_signature_help_presentation()
          .unwrap_or_default()
      })
      .register(),
  };

  preset.install_extension_state(handles)
}

#[cfg(test)]
mod tests {
  use the_lib::render::LineNumberMode;

  use super::{
    ConfigDefaults,
    CursorShapes,
    EditorPreset,
  };
  use crate::{
    CompletionMenuItem,
    FilePickerConfig,
    IntoKeyBinding,
    KeyAction,
    KeyTrie,
    Keymaps,
    Mode,
    NamedAction,
    SignatureHelpPresentation,
  };

  struct TestCtx;

  #[test]
  fn register_named_action_with_binding_uses_typed_handle_binding() {
    let mut preset = EditorPreset::<TestCtx, ()>::new((), Keymaps::default());

    let handle = preset
      .register_named_action_with_binding(
        NamedAction::new("demo.open", "Open demo", |_ctx: &mut TestCtx| {}),
        Mode::Normal,
        ['g', 'p'],
      )
      .expect("valid keybinding");

    let root = preset
      .keymaps()
      .map
      .get(&Mode::Normal)
      .expect("normal-mode keymap");
    let g = 'g'.into_binding().expect("g binding");
    let p = 'p'.into_binding().expect("p binding");
    let Some(KeyTrie::Node(prefix)) = root.search(&[g]) else {
      panic!("missing g prefix binding");
    };
    let Some(KeyTrie::Command(KeyAction::NamedHandle(bound))) = prefix.map.get(&p) else {
      panic!("missing typed named-action binding");
    };

    assert_eq!(*bound, handle);
    assert_eq!(bound.name(), "demo.open");
  }

  #[test]
  fn completion_menu_provider_builder_infers_ctx_from_preset() {
    let mut preset = EditorPreset::<TestCtx, ()>::new((), Keymaps::default());

    let provider = preset
      .completion_menu_provider(|_ctx| vec![CompletionMenuItem::new("Demo")])
      .on_accept(|_ctx, _index, _item| true)
      .register();

    assert_eq!(provider.get(), 0);
  }

  #[test]
  fn signature_help_provider_builder_infers_ctx_from_preset() {
    let mut preset = EditorPreset::<TestCtx, ()>::new((), Keymaps::default());

    let provider = preset
      .signature_help_provider(|_ctx| SignatureHelpPresentation::default())
      .register();

    assert_eq!(provider.get(), 0);
  }

  #[test]
  fn preset_defaults_merge_and_survive_dispatch_replacement() {
    let preset = EditorPreset::<TestCtx, ()>::new((), Keymaps::default())
      .with_defaults(
        ConfigDefaults::new()
          .theme("onedark")
          .line_numbers(LineNumberMode::Relative),
      )
      .with_defaults(
        ConfigDefaults::new()
          .cursor_shapes(CursorShapes::new(
            crate::CursorKind::Underline,
            crate::CursorKind::Bar,
            crate::CursorKind::Block,
          ))
          .file_picker(FilePickerConfig {
            hidden: false,
            ..Default::default()
          }),
      )
      .with_dispatch(());

    let defaults = preset.defaults();
    assert_eq!(defaults.theme.as_deref(), Some("onedark"));
    assert_eq!(defaults.editor.line_numbers, Some(LineNumberMode::Relative));
    assert_eq!(
      defaults.editor.cursor_shapes,
      Some(CursorShapes::new(
        crate::CursorKind::Underline,
        crate::CursorKind::Bar,
        crate::CursorKind::Block,
      ))
    );
    assert_eq!(
      defaults
        .editor
        .file_picker
        .as_ref()
        .map(|config| config.hidden),
      Some(false)
    );
  }
}
