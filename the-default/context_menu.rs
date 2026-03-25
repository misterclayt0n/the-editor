use std::{
  fmt,
  str::FromStr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextMenuActionId {
  EditorGotoDefinition,
  EditorGotoTypeDefinition,
  EditorGotoImplementation,
  EditorFindReferences,
  EditorRenameSymbol,
  EditorShowCodeActions,
  EditorFormatBuffer,
}

impl ContextMenuActionId {
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::EditorGotoDefinition => "editor.goto_definition",
      Self::EditorGotoTypeDefinition => "editor.goto_type_definition",
      Self::EditorGotoImplementation => "editor.goto_implementation",
      Self::EditorFindReferences => "editor.find_references",
      Self::EditorRenameSymbol => "editor.rename_symbol",
      Self::EditorShowCodeActions => "editor.show_code_actions",
      Self::EditorFormatBuffer => "editor.format_buffer",
    }
  }

  #[must_use]
  pub const fn default_title(self) -> &'static str {
    match self {
      Self::EditorGotoDefinition => "Go to Definition",
      Self::EditorGotoTypeDefinition => "Go to Type Definition",
      Self::EditorGotoImplementation => "Go to Implementation",
      Self::EditorFindReferences => "Find References",
      Self::EditorRenameSymbol => "Rename Symbol...",
      Self::EditorShowCodeActions => "Show Code Actions",
      Self::EditorFormatBuffer => "Format Buffer",
    }
  }
}

impl fmt::Display for ContextMenuActionId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

impl FromStr for ContextMenuActionId {
  type Err = ();

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    match value {
      "editor.goto_definition" => Ok(Self::EditorGotoDefinition),
      "editor.goto_type_definition" => Ok(Self::EditorGotoTypeDefinition),
      "editor.goto_implementation" => Ok(Self::EditorGotoImplementation),
      "editor.find_references" => Ok(Self::EditorFindReferences),
      "editor.rename_symbol" => Ok(Self::EditorRenameSymbol),
      "editor.show_code_actions" => Ok(Self::EditorShowCodeActions),
      "editor.format_buffer" => Ok(Self::EditorFormatBuffer),
      _ => Err(()),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenuItem {
  pub id:          ContextMenuActionId,
  pub title:       String,
  pub enabled:     bool,
  pub destructive: bool,
}

impl ContextMenuItem {
  #[must_use]
  pub fn new(id: ContextMenuActionId) -> Self {
    Self {
      id,
      title: id.default_title().to_string(),
      enabled: true,
      destructive: false,
    }
  }

  #[must_use]
  pub fn title(mut self, title: impl Into<String>) -> Self {
    self.title = title.into();
    self
  }

  #[must_use]
  pub fn disabled(mut self) -> Self {
    self.enabled = false;
    self
  }

  #[must_use]
  pub fn destructive(mut self) -> Self {
    self.destructive = true;
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextMenuSection {
  pub title: Option<String>,
  pub items: Vec<ContextMenuItem>,
}

impl ContextMenuSection {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub fn title(mut self, title: impl Into<String>) -> Self {
    self.title = Some(title.into());
    self
  }

  #[must_use]
  pub fn item(mut self, item: ContextMenuItem) -> Self {
    self.items.push(item);
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextMenuSnapshot {
  pub sections: Vec<ContextMenuSection>,
}

impl ContextMenuSnapshot {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub fn section(mut self, section: ContextMenuSection) -> Self {
    self.sections.push(section);
    self
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditorContextMenuOptions {
  pub can_goto_definition:      bool,
  pub can_goto_type_definition: bool,
  pub can_goto_implementation:  bool,
  pub can_find_references:      bool,
  pub can_rename_symbol:        bool,
  pub can_show_code_actions:    bool,
  pub can_format_buffer:        bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditorContextMenuRequest {
  pub char_index: Option<usize>,
  pub options:    EditorContextMenuOptions,
}

#[must_use]
pub fn build_editor_context_menu(options: EditorContextMenuOptions) -> ContextMenuSnapshot {
  let mut sections = Vec::new();

  let navigation = [
    (
      options.can_goto_definition,
      ContextMenuActionId::EditorGotoDefinition,
    ),
    (
      options.can_goto_type_definition,
      ContextMenuActionId::EditorGotoTypeDefinition,
    ),
    (
      options.can_goto_implementation,
      ContextMenuActionId::EditorGotoImplementation,
    ),
    (
      options.can_find_references,
      ContextMenuActionId::EditorFindReferences,
    ),
  ]
  .into_iter()
  .filter_map(|(enabled, id)| enabled.then(|| ContextMenuItem::new(id)))
  .collect::<Vec<_>>();
  if !navigation.is_empty() {
    sections.push(ContextMenuSection {
      title: None,
      items: navigation,
    });
  }

  let symbol_actions = [
    (
      options.can_rename_symbol,
      ContextMenuActionId::EditorRenameSymbol,
    ),
    (
      options.can_show_code_actions,
      ContextMenuActionId::EditorShowCodeActions,
    ),
    (
      options.can_format_buffer,
      ContextMenuActionId::EditorFormatBuffer,
    ),
  ]
  .into_iter()
  .filter_map(|(enabled, id)| enabled.then(|| ContextMenuItem::new(id)))
  .collect::<Vec<_>>();
  if !symbol_actions.is_empty() {
    sections.push(ContextMenuSection {
      title: None,
      items: symbol_actions,
    });
  }

  ContextMenuSnapshot { sections }
}
