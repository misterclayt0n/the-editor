use the_lib::render::{
  LineNumberMode,
  graphics::CursorKind,
};

use crate::{
  CommandRegistry,
  DefaultContext,
  FilePickerOptions,
  install_builtin_commands,
  install_builtin_file_tree_commands,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinCompletionMenuKind {
  LspCompletion,
  CodeActions,
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
  pub file_picker:   Option<FilePickerOptions>,
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

  pub fn file_picker(mut self, options: FilePickerOptions) -> Self {
    self.file_picker = Some(options);
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
pub struct Defaults {
  pub theme:  Option<String>,
  pub editor: EditorDefaults,
  pub term:   TermDefaults,
}

impl Defaults {
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

  pub fn file_picker(mut self, options: FilePickerOptions) -> Self {
    self.editor = self.editor.file_picker(options);
    self
  }

  pub fn term(mut self, defaults: TermDefaults) -> Self {
    self.term.merge(defaults);
    self
  }

  pub fn merge(&mut self, other: Self) {
    if other.theme.is_some() {
      self.theme = other.theme;
    }
    self.editor.merge(other.editor);
    self.term.merge(other.term);
  }
}

pub fn default_defaults() -> Defaults {
  Defaults::default()
}

pub fn install_default_wiring<Ctx>(command_registry: &mut CommandRegistry<Ctx>)
where
  Ctx: DefaultContext + 'static,
{
  install_builtin_commands(command_registry);
  install_builtin_file_tree_commands(command_registry);
}
