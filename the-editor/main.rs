use crate::editor::Editor;

mod core;
mod editor;

fn main() -> anyhow::Result<()> {
  let editor = Editor::new();
  the_editor_renderer::run("The Editor - Modern Text Editor", 1024, 768, editor)
    .map_err(|e| anyhow::anyhow!("Failed to run renderer: {}", e))
}
