use the_lib::render::{
  RenderPlan,
  UiTree,
  text_annotations::TextAnnotations,
};

use crate::{
  CommandRegistry,
  DefaultApi,
  DefaultContext,
  EditorExtensions,
  ExtensionStateStore,
  KeyAction,
  Keymaps,
  Mode,
  NamedAction,
  ParseKeyBindingError,
  PickerQueryHandlerEntry,
  PickerSubmitHandlerEntry,
  TypableCommand,
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

pub struct EditorPreset<Ctx: 'static, Dispatch> {
  dispatch:                    Dispatch,
  keymaps:                     Keymaps,
  commands:                    Vec<TypableCommand<Ctx>>,
  extensions:                  EditorExtensions<Ctx>,
  extension_state:             ExtensionStateStore,
  command_registry_installers: Vec<CommandRegistryInstaller<Ctx>>,
  startup_hooks:               Vec<StartupHook<Ctx>>,
}

impl<Ctx: 'static, Dispatch> EditorPreset<Ctx, Dispatch> {
  pub fn new(dispatch: Dispatch, keymaps: Keymaps) -> Self {
    Self {
      dispatch,
      keymaps,
      commands: Vec::new(),
      extensions: EditorExtensions::default(),
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
      keymaps: self.keymaps,
      commands: self.commands,
      extensions: self.extensions,
      extension_state: self.extension_state,
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

  pub fn install_extension_state<T>(mut self, state: T) -> Self
  where
    T: 'static,
  {
    let _ = self.extension_state.insert(state);
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

  pub fn install_text_annotations_provider<F>(mut self, provider: F) -> Self
  where
    F: for<'a> Fn(&'a Ctx, &mut TextAnnotations<'a>) + 'static,
  {
    self.extensions.register_text_annotations_provider(provider);
    self
  }

  pub fn install_render_plan_post_processor<F>(mut self, processor: F) -> Self
  where
    F: Fn(&mut Ctx, &mut RenderPlan) + 'static,
  {
    self
      .extensions
      .register_render_plan_post_processor(processor);
    self
  }

  pub fn install_ui_tree_post_processor<F>(mut self, processor: F) -> Self
  where
    F: Fn(&mut Ctx, &mut UiTree) + 'static,
  {
    self.extensions.register_ui_tree_post_processor(processor);
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
      keymaps: self.keymaps,
      command_registry,
      extensions: self.extensions,
      extension_state: self.extension_state,
      startup_hooks: self.startup_hooks,
    }
  }
}

pub struct BuiltEditorPreset<Ctx: 'static, Dispatch> {
  pub dispatch:         Dispatch,
  pub keymaps:          Keymaps,
  pub command_registry: CommandRegistry<Ctx>,
  pub extensions:       EditorExtensions<Ctx>,
  pub extension_state:  ExtensionStateStore,
  startup_hooks:        Vec<StartupHook<Ctx>>,
}

impl<Ctx: 'static, Dispatch> BuiltEditorPreset<Ctx, Dispatch> {
  pub fn startup_hooks(&self) -> &[StartupHook<Ctx>] {
    &self.startup_hooks
  }

  pub fn run_startup_hooks(&self, ctx: &mut Ctx) {
    for hook in &self.startup_hooks {
      hook.run(ctx);
    }
  }

  pub fn into_parts(
    self,
  ) -> (
    Dispatch,
    Keymaps,
    CommandRegistry<Ctx>,
    EditorExtensions<Ctx>,
    ExtensionStateStore,
    Vec<StartupHook<Ctx>>,
  ) {
    (
      self.dispatch,
      self.keymaps,
      self.command_registry,
      self.extensions,
      self.extension_state,
      self.startup_hooks,
    )
  }
}

pub fn default_editor_preset<Ctx>() -> EditorPreset<Ctx, impl DefaultApi<Ctx>>
where
  Ctx: DefaultContext,
{
  EditorPreset::new(crate::build_dispatch::<Ctx>(), Keymaps::default())
}
