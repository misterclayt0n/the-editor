use std::{
  fmt,
  str::FromStr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextMenuActionId {
  FileTreeOpen,
  FileTreeOpenSplitRight,
  FileTreeOpenSplitDown,
  FileTreeExpand,
  FileTreeCollapse,
  FileTreeNewFile,
  FileTreeNewFolder,
  FileTreeRename,
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
      Self::FileTreeOpen => "file_tree.open",
      Self::FileTreeOpenSplitRight => "file_tree.open_split_right",
      Self::FileTreeOpenSplitDown => "file_tree.open_split_down",
      Self::FileTreeExpand => "file_tree.expand",
      Self::FileTreeCollapse => "file_tree.collapse",
      Self::FileTreeNewFile => "file_tree.new_file",
      Self::FileTreeNewFolder => "file_tree.new_folder",
      Self::FileTreeRename => "file_tree.rename",
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
      Self::FileTreeOpen => "Open",
      Self::FileTreeOpenSplitRight => "Open in Split Right",
      Self::FileTreeOpenSplitDown => "Open in Split Down",
      Self::FileTreeExpand => "Expand",
      Self::FileTreeCollapse => "Collapse",
      Self::FileTreeNewFile => "New File...",
      Self::FileTreeNewFolder => "New Folder...",
      Self::FileTreeRename => "Rename...",
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
      "file_tree.open" => Ok(Self::FileTreeOpen),
      "file_tree.open_split_right" => Ok(Self::FileTreeOpenSplitRight),
      "file_tree.open_split_down" => Ok(Self::FileTreeOpenSplitDown),
      "file_tree.expand" => Ok(Self::FileTreeExpand),
      "file_tree.collapse" => Ok(Self::FileTreeCollapse),
      "file_tree.new_file" => Ok(Self::FileTreeNewFile),
      "file_tree.new_folder" => Ok(Self::FileTreeNewFolder),
      "file_tree.rename" => Ok(Self::FileTreeRename),
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextMenuSnapshot {
  pub sections: Vec<ContextMenuSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileTreeContextMenuOptions {
  pub is_directory:      bool,
  pub expanded:          bool,
  pub is_workspace_root: bool,
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

#[must_use]
pub fn build_file_tree_context_menu(options: FileTreeContextMenuOptions) -> ContextMenuSnapshot {
  let mut sections = Vec::new();

  if options.is_directory {
    sections.push(ContextMenuSection {
      title: None,
      items: vec![ContextMenuItem::new(if options.expanded {
        ContextMenuActionId::FileTreeCollapse
      } else {
        ContextMenuActionId::FileTreeExpand
      })],
    });
  } else {
    sections.push(ContextMenuSection {
      title: None,
      items: vec![
        ContextMenuItem::new(ContextMenuActionId::FileTreeOpen),
        ContextMenuItem::new(ContextMenuActionId::FileTreeOpenSplitRight),
        ContextMenuItem::new(ContextMenuActionId::FileTreeOpenSplitDown),
      ],
    });
  }

  sections.push(ContextMenuSection {
    title: None,
    items: vec![
      ContextMenuItem::new(ContextMenuActionId::FileTreeNewFile),
      ContextMenuItem::new(ContextMenuActionId::FileTreeNewFolder),
    ],
  });

  sections.push(ContextMenuSection {
    title: None,
    items: vec![if options.is_workspace_root {
      ContextMenuItem::new(ContextMenuActionId::FileTreeRename).disabled()
    } else {
      ContextMenuItem::new(ContextMenuActionId::FileTreeRename)
    }],
  });

  ContextMenuSnapshot { sections }
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
