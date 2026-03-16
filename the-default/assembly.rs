use crate::{
  CommandRegistry,
  DefaultContext,
  DefaultDispatchStatic,
  EditorExtensions,
  KeyAction,
  Keymaps,
  Mode,
  NamedAction,
  ParseKeyBindingError,
  PickerQueryHandlerEntry,
  PickerSubmitHandlerEntry,
  RenderPlanPostProcessor,
  TextAnnotationsProvider,
  TypableCommand,
  UiTreePostProcessor,
};

/// Installs additional typable commands into a command registry during
/// assembly finalization.
pub type CommandRegistryInstaller<Ctx> = fn(&mut CommandRegistry<Ctx>);

/// Runs after the client has created its context/app and installed assembled
/// dispatch + keymaps + command registry.
pub type StartupHook<Ctx> = fn(&mut Ctx);

/// Compile-time assembly surface for shared editor composition.
///
/// This intentionally starts small. The first version only assembles:
///
/// - dispatch
/// - keymaps
/// - typable command installers
/// - startup hooks
///
/// The goal is to establish a first-class compile-time assembly pattern without
/// taking flexibility away from clients.
pub struct EditorAssembly<Ctx: 'static, Dispatch = DefaultDispatchStatic<Ctx>> {
  dispatch:                    Dispatch,
  keymaps:                     Keymaps,
  commands:                    Vec<TypableCommand<Ctx>>,
  extensions:                  EditorExtensions<Ctx>,
  command_registry_installers: Vec<CommandRegistryInstaller<Ctx>>,
  startup_hooks:               Vec<StartupHook<Ctx>>,
}

impl<Ctx: 'static, Dispatch> EditorAssembly<Ctx, Dispatch> {
  pub fn new(dispatch: Dispatch, keymaps: Keymaps) -> Self {
    Self {
      dispatch,
      keymaps,
      commands: Vec::new(),
      extensions: EditorExtensions::default(),
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

  pub fn keymaps(&self) -> &Keymaps {
    &self.keymaps
  }

  pub fn keymaps_mut(&mut self) -> &mut Keymaps {
    &mut self.keymaps
  }

  pub fn extensions(&self) -> &EditorExtensions<Ctx> {
    &self.extensions
  }

  pub fn extensions_mut(&mut self) -> &mut EditorExtensions<Ctx> {
    &mut self.extensions
  }

  pub fn command_registry_installers(&self) -> &[CommandRegistryInstaller<Ctx>] {
    &self.command_registry_installers
  }

  pub fn startup_hooks(&self) -> &[StartupHook<Ctx>] {
    &self.startup_hooks
  }

  pub fn with_dispatch<NewDispatch>(
    self,
    dispatch: NewDispatch,
  ) -> EditorAssembly<Ctx, NewDispatch> {
    EditorAssembly {
      dispatch,
      keymaps: self.keymaps,
      commands: self.commands,
      extensions: self.extensions,
      command_registry_installers: self.command_registry_installers,
      startup_hooks: self.startup_hooks,
    }
  }

  pub fn with_keymaps(mut self, keymaps: Keymaps) -> Self {
    self.keymaps = keymaps;
    self
  }

  pub fn merge_keymaps(mut self, keymaps: Keymaps) -> Self {
    self.keymaps.merge(keymaps);
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

  pub fn install_command(mut self, command: TypableCommand<Ctx>) -> Self {
    self.commands.push(command);
    self
  }

  pub fn install_named_action(mut self, action: NamedAction<Ctx>) -> Self {
    self.extensions.register_named_action(action);
    self
  }

  pub fn install_picker_query_handler(mut self, entry: PickerQueryHandlerEntry<Ctx>) -> Self {
    self.extensions.register_picker_query_handler(entry);
    self
  }

  pub fn install_picker_submit_handler(mut self, entry: PickerSubmitHandlerEntry<Ctx>) -> Self {
    self.extensions.register_picker_submit_handler(entry);
    self
  }

  pub fn install_text_annotations_provider(
    mut self,
    provider: TextAnnotationsProvider<Ctx>,
  ) -> Self {
    self.extensions.register_text_annotations_provider(provider);
    self
  }

  pub fn install_render_plan_post_processor(
    mut self,
    processor: RenderPlanPostProcessor<Ctx>,
  ) -> Self {
    self
      .extensions
      .register_render_plan_post_processor(processor);
    self
  }

  pub fn install_ui_tree_post_processor(mut self, processor: UiTreePostProcessor<Ctx>) -> Self {
    self.extensions.register_ui_tree_post_processor(processor);
    self
  }

  pub fn install_command_registry(mut self, installer: CommandRegistryInstaller<Ctx>) -> Self {
    self.command_registry_installers.push(installer);
    self
  }

  pub fn install_startup_hook(mut self, hook: StartupHook<Ctx>) -> Self {
    self.startup_hooks.push(hook);
    self
  }

  pub fn build(self) -> BuiltEditorAssembly<Ctx, Dispatch>
  where
    Ctx: DefaultContext,
  {
    let mut command_registry = CommandRegistry::new();
    for command in self.commands {
      command_registry.register(command);
    }
    for installer in self.command_registry_installers {
      installer(&mut command_registry);
    }

    BuiltEditorAssembly {
      dispatch: self.dispatch,
      keymaps: self.keymaps,
      command_registry,
      extensions: self.extensions,
      startup_hooks: self.startup_hooks,
    }
  }
}

/// Final assembled editor surface consumed by clients.
pub struct BuiltEditorAssembly<Ctx: 'static, Dispatch = DefaultDispatchStatic<Ctx>> {
  pub dispatch:         Dispatch,
  pub keymaps:          Keymaps,
  pub command_registry: CommandRegistry<Ctx>,
  pub extensions:       EditorExtensions<Ctx>,
  startup_hooks:        Vec<StartupHook<Ctx>>,
}

impl<Ctx: 'static, Dispatch> BuiltEditorAssembly<Ctx, Dispatch> {
  pub fn startup_hooks(&self) -> &[StartupHook<Ctx>] {
    &self.startup_hooks
  }

  pub fn run_startup_hooks(&self, ctx: &mut Ctx) {
    for hook in &self.startup_hooks {
      hook(ctx);
    }
  }

  pub fn into_parts(
    self,
  ) -> (
    Dispatch,
    Keymaps,
    CommandRegistry<Ctx>,
    EditorExtensions<Ctx>,
    Vec<StartupHook<Ctx>>,
  ) {
    (
      self.dispatch,
      self.keymaps,
      self.command_registry,
      self.extensions,
      self.startup_hooks,
    )
  }
}
