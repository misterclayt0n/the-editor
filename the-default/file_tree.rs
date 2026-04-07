use std::{
  collections::{
    BTreeMap,
    BTreeSet,
  },
  env,
  ffi::OsString,
  fs::{
    OpenOptions,
    read_dir,
  },
  io::Write,
  path::{
    Path,
    PathBuf,
  },
  time::{
    Instant,
    SystemTime,
    UNIX_EPOCH,
  },
};

use the_lib::{
  diagnostics::{
    DiagnosticSeverity,
    DiagnosticsState,
  },
  editor::{
    ClientSurfaceId,
    OpenTarget,
  },
  split_tree::{
    PaneDirection,
    PaneId,
    SplitAxis,
  },
  view::scroll_row_to_keep_visible,
};
use the_lsp::text_sync::path_for_file_uri;
use the_vcs::FileChange;

use crate::{
  CommandBuilder,
  CommandEvent,
  CommandRegistry,
  DefaultContext,
  Key,
  KeyEvent,
  Mode,
  file_picker_icon_glyph,
  file_picker_icon_name_for_path,
  open_command_palette_with_input,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileTreeVcsKind {
  Conflict,
  Deleted,
  Modified,
  Renamed,
  Untracked,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileTreeDecorations {
  pub vcs:        Option<FileTreeVcsKind>,
  pub diagnostic: Option<DiagnosticSeverity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeRow {
  pub path:              PathBuf,
  pub display_name:      String,
  pub depth:             usize,
  pub ancestor_branches: Vec<bool>,
  pub is_last_sibling:   bool,
  pub has_children:      bool,
  pub is_dir:            bool,
  pub is_expanded:       bool,
  pub is_current_file:   bool,
  pub decorations:       FileTreeDecorations,
  pub icon_name:         String,
  pub icon_glyph:        &'static str,
}

#[derive(Debug, Clone)]
pub struct FileTreeSnapshot {
  pub surface_id:     ClientSurfaceId,
  pub root:           PathBuf,
  pub rows:           Vec<FileTreeRow>,
  pub selected:       Option<usize>,
  pub scroll_offset:  usize,
  pub show_hidden:    bool,
  pub follow_current: bool,
  pub attached_pane:  Option<PaneId>,
  pub active:         bool,
}

#[derive(Debug, Clone, Default)]
pub struct FileTreeState {
  pub surface_id:          Option<ClientSurfaceId>,
  pub sidebar_pane:        Option<PaneId>,
  pub visible:             bool,
  pub active:              bool,
  pub root:                Option<PathBuf>,
  pub rows:                Vec<FileTreeRow>,
  pub selected:            Option<usize>,
  pub scroll_offset:       usize,
  pub visible_rows:        usize,
  pub selection_follow:    bool,
  pub expanded_dirs:       BTreeSet<PathBuf>,
  pub show_hidden:         bool,
  pub follow_current:      bool,
  pub last_editor_pane:    Option<PaneId>,
  pub clipboard:           Option<FileTreeClipboard>,
  pub vcs_statuses:        BTreeMap<PathBuf, FileTreeVcsKind>,
  pub diagnostic_statuses: BTreeMap<PathBuf, DiagnosticSeverity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeClipboardMode {
  Copy,
  Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeClipboard {
  pub path: PathBuf,
  pub mode: FileTreeClipboardMode,
}

impl FileTreeState {
  fn clear_rows(&mut self) {
    self.rows.clear();
    self.selected = None;
    self.scroll_offset = 0;
    self.selection_follow = false;
  }

  fn combined_decorations(&self) -> BTreeMap<PathBuf, FileTreeDecorations> {
    combine_file_tree_decorations(&self.vcs_statuses, &self.diagnostic_statuses)
  }
}

fn file_tree_perf_enabled() -> bool {
  env::var("THE_TERM_DEBUG_RENDER_PERF").ok().as_deref() == Some("1")
}

fn append_file_tree_perf_line(data: &[u8]) {
  let Some(path) = env::var("THE_TERM_DEBUG_RENDER_PERF_FILE")
    .ok()
    .map(|raw| raw.trim().to_string())
    .filter(|raw| !raw.is_empty())
    .map(PathBuf::from)
  else {
    return;
  };
  if let Some(parent) = path.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
    let _ = file.write_all(data);
  }
}

fn file_tree_perf_log(message: String) {
  if !file_tree_perf_enabled() {
    return;
  }
  let ts_ms = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_millis())
    .unwrap_or(0);
  let line = format!("[filetree {ts_ms}] {message}\n");
  append_file_tree_perf_line(line.as_bytes());
}

pub fn install_builtin_file_tree_commands<Ctx>(registry: &mut CommandRegistry<Ctx>)
where
  Ctx: DefaultContext + 'static,
{
  registry.register(
    CommandBuilder::new(
      "file-tree-add",
      "Create a file or directory in the file tree",
      cmd_add::<Ctx>,
    )
    .required_arg()
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-rename",
      "Rename the selected file-tree entry",
      cmd_rename::<Ctx>,
    )
    .required_arg()
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-delete",
      "Delete the selected file-tree entry (use :file-tree-delete yes to confirm)",
      cmd_delete::<Ctx>,
    )
    .optional_arg()
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-yank",
      "Copy the selected file-tree entry into the tree clipboard",
      cmd_yank::<Ctx>,
    )
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-cut",
      "Move the selected file-tree entry into the tree clipboard",
      cmd_cut::<Ctx>,
    )
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-paste",
      "Paste the tree clipboard into the selected directory",
      cmd_paste::<Ctx>,
    )
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-copy",
      "Copy the selected file-tree entry to a target path",
      cmd_copy::<Ctx>,
    )
    .required_arg()
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-move",
      "Move the selected file-tree entry to a target path",
      cmd_move::<Ctx>,
    )
    .required_arg()
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-trash",
      "Move the selected file-tree entry to trash (use :file-tree-trash yes to confirm)",
      cmd_trash::<Ctx>,
    )
    .optional_arg()
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-up",
      "Retarget the file-tree root to the parent directory",
      cmd_up::<Ctx>,
    )
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-focus",
      "Reveal the current buffer file in the file tree and focus the tree pane",
      cmd_focus::<Ctx>,
    )
    .build(),
  );
  registry.register(
    CommandBuilder::new(
      "file-tree-close-all",
      "Collapse all directories in the file tree",
      cmd_close_all::<Ctx>,
    )
    .build(),
  );
}

pub fn file_tree_snapshot<Ctx: DefaultContext>(ctx: &Ctx) -> Option<FileTreeSnapshot> {
  let state = ctx.file_tree();
  let surface_id = state.surface_id?;
  let root = state.root.clone()?;
  let attached_pane = if ctx.file_tree_uses_split_pane() {
    attached_tree_pane(ctx)
  } else {
    None
  };
  Some(FileTreeSnapshot {
    surface_id,
    root,
    rows: state.rows.clone(),
    selected: state.selected,
    scroll_offset: state.scroll_offset,
    show_hidden: state.show_hidden,
    follow_current: state.follow_current,
    attached_pane,
    active: is_active_file_tree(ctx),
  })
}

pub fn file_tree_surface_id<Ctx: DefaultContext>(ctx: &Ctx) -> Option<ClientSurfaceId> {
  ctx.file_tree().surface_id
}

pub fn is_file_tree_surface<Ctx: DefaultContext>(ctx: &Ctx, surface_id: ClientSurfaceId) -> bool {
  ctx.file_tree().surface_id == Some(surface_id)
}

pub fn is_active_file_tree<Ctx: DefaultContext>(ctx: &Ctx) -> bool {
  if ctx.file_tree_uses_split_pane() {
    let Some(surface_id) = ctx.file_tree().surface_id else {
      return false;
    };
    return ctx.editor_ref().active_client_surface_id() == Some(surface_id);
  }

  let state = ctx.file_tree();
  state.visible && state.active
}

pub fn remember_active_editor_pane<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let pane = ctx.editor_ref().active_pane_id();
  if matches!(
    ctx.editor_ref().pane_content_kind(pane),
    Some(the_lib::editor::PaneContentKind::EditorBuffer)
  ) {
    ctx.file_tree_mut().last_editor_pane = Some(pane);
  }
}

pub fn sync_file_tree_to_active_file<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.file_tree().follow_current {
    return;
  }
  let Some(root) = ctx.file_tree().root.clone() else {
    return;
  };
  let Some(path) = ctx.file_path().map(PathBuf::from) else {
    return;
  };
  if !path.starts_with(&root) {
    return;
  }
  reveal_path(ctx, &path);
}

pub fn toggle_file_tree<Ctx: DefaultContext>(ctx: &mut Ctx) {
  toggle_file_tree_with_root(ctx, false);
}

pub fn toggle_file_tree_in_current_buffer_directory<Ctx: DefaultContext>(ctx: &mut Ctx) {
  toggle_file_tree_with_root(ctx, true);
}

pub fn set_file_tree_visible_rows<Ctx: DefaultContext>(ctx: &mut Ctx, visible_rows: usize) {
  let scrolloff = ctx.scrolloff();
  let state = ctx.file_tree_mut();
  state.visible_rows = visible_rows;
  sync_tree_scroll(state, scrolloff);
}

pub fn set_file_tree_active<Ctx: DefaultContext>(ctx: &mut Ctx, active: bool) -> bool {
  if ctx.file_tree_uses_split_pane() {
    if active {
      focus_file_tree(ctx);
      return true;
    }
    return false;
  }

  let state = ctx.file_tree_mut();
  if state.active == active {
    return false;
  }
  state.active = active;
  true
}

pub fn handle_file_tree_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if ctx.mode() == Mode::Command || ctx.file_picker().active || !is_active_file_tree(ctx) {
    return false;
  }

  match key.key {
    Key::Up => move_selection(ctx, -1),
    Key::Down => move_selection(ctx, 1),
    Key::Left => collapse_selected_or_select_parent(ctx),
    Key::Right | Key::Enter | Key::NumpadEnter => expand_or_open_selected(ctx, None),
    Key::Backspace => root_to_parent(ctx),
    Key::Escape => close_file_tree(ctx),
    Key::Char('-') => root_to_parent(ctx),
    Key::Char('j') => move_selection(ctx, 1),
    Key::Char('k') => move_selection(ctx, -1),
    Key::Char('h') => collapse_selected_or_select_parent(ctx),
    Key::Char('l') => expand_or_open_selected(ctx, None),
    Key::Char('s') => expand_or_open_selected(ctx, Some(SplitAxis::Horizontal)),
    Key::Char('v') => expand_or_open_selected(ctx, Some(SplitAxis::Vertical)),
    Key::Char('f') => focus_file_tree(ctx),
    Key::Char('q') => close_file_tree(ctx),
    Key::Char('.') => reveal_current_file(ctx),
    Key::Char('u') => refresh_file_tree(ctx),
    Key::Char('H') => toggle_hidden(ctx),
    Key::Char('a') => open_command_palette_with_input(ctx, "file-tree-add "),
    Key::Char('r') => open_command_palette_with_input(ctx, "file-tree-rename "),
    Key::Char('d') => open_command_palette_with_input(ctx, "file-tree-delete yes"),
    Key::Char('y') => yank_selected(ctx, FileTreeClipboardMode::Copy),
    Key::Char('x') => yank_selected(ctx, FileTreeClipboardMode::Move),
    Key::Char('p') => paste_into_selected_directory(ctx),
    Key::Char('c') => open_command_palette_with_input(ctx, "file-tree-copy "),
    Key::Char('m') => open_command_palette_with_input(ctx, "file-tree-move "),
    Key::Char('t') => open_command_palette_with_input(ctx, "file-tree-trash yes"),
    Key::Char('z') => close_all_file_tree(ctx),
    _ => return false,
  }

  true
}

pub fn refresh_file_tree<Ctx: DefaultContext>(ctx: &mut Ctx) {
  rebuild_rows(ctx);
  ctx.request_render();
}

fn select_file_tree_index_with_behavior<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  index: usize,
  follow_selection: bool,
) -> bool {
  let len = ctx.file_tree().rows.len();
  if len == 0 {
    return false;
  }

  let uses_split_pane = ctx.file_tree_uses_split_pane();
  let next = index.min(len.saturating_sub(1));
  let scrolloff = ctx.scrolloff();
  let state = ctx.file_tree_mut();
  let changed = state.selected != Some(next) || (!uses_split_pane && !state.active);
  state.selected = Some(next);
  state.selection_follow = follow_selection;
  if !uses_split_pane {
    state.visible = true;
    state.active = true;
  }
  sync_tree_scroll(state, scrolloff);
  changed
}

pub fn select_file_tree_index<Ctx: DefaultContext>(ctx: &mut Ctx, index: usize) -> bool {
  select_file_tree_index_with_behavior(ctx, index, true)
}

pub fn select_file_tree_index_without_follow<Ctx: DefaultContext>(ctx: &mut Ctx, index: usize) -> bool {
  select_file_tree_index_with_behavior(ctx, index, false)
}

pub fn activate_file_tree_index<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  index: usize,
  split: Option<SplitAxis>,
) -> bool {
  let len = ctx.file_tree().rows.len();
  if index >= len {
    return false;
  }
  let _ = select_file_tree_index(ctx, index);
  expand_or_open_selected(ctx, split);
  true
}

pub fn scroll_file_tree<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  delta: isize,
  visible_rows: usize,
) -> bool {
  let state = ctx.file_tree_mut();
  state.visible_rows = visible_rows;
  clamp_tree_state(state);
  state.selection_follow = false;
  let max_offset = tree_max_scroll_offset(state);
  let next = state
    .scroll_offset
    .saturating_add_signed(delta)
    .min(max_offset);
  if next == state.scroll_offset {
    return false;
  }
  state.scroll_offset = next;
  true
}

pub fn reveal_current_file<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(path) = ctx.file_path().map(PathBuf::from) else {
    return;
  };
  reveal_path(ctx, &path);
}

pub fn close_file_tree<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.file_tree_uses_split_pane() {
    let state = ctx.file_tree_mut();
    let changed = state.visible || state.active;
    state.visible = false;
    state.active = false;
    if changed {
      ctx.request_render();
    }
    return;
  }

  let Some(surface_id) = ctx.file_tree().surface_id else {
    return;
  };
  let tree_pane = attached_tree_pane(ctx);
  if tree_pane.is_none() {
    ctx.file_tree_mut().sidebar_pane = None;
    return;
  }
  let tree_pane = tree_pane.expect("checked above");
  let sidebar_pane = ctx.file_tree().sidebar_pane;

  let previous_buffer_id = ctx.editor_ref().active_buffer_id();
  let _ = ctx.editor().set_active_pane(tree_pane);
  let close_sidebar_pane = sidebar_pane == Some(tree_pane) && ctx.editor_ref().pane_count() > 1;
  let closed = if close_sidebar_pane {
    ctx.editor().close_active_pane()
  } else if ctx.editor_ref().active_client_surface_id() == Some(surface_id) {
    ctx.editor().hide_active_client_surface()
  } else {
    false
  };

  if closed {
    ctx.file_tree_mut().sidebar_pane = None;
    ctx.did_change_active_pane(previous_buffer_id);
    ctx.request_render();
  }
}

pub fn focus_file_tree<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.file_tree_uses_split_pane() {
    if !ctx.file_tree().visible {
      toggle_file_tree_with_root(ctx, false);
      return;
    }
    reveal_current_file(ctx);
    ctx.file_tree_mut().active = true;
    ctx.request_render();
    return;
  }

  if attached_tree_pane(ctx).is_none() {
    toggle_file_tree_with_root(ctx, false);
  }
  reveal_current_file(ctx);
  if let Some(tree_pane) = attached_tree_pane(ctx) {
    let previous_buffer_id = ctx.editor_ref().active_buffer_id();
    if ctx.editor().set_active_pane(tree_pane) {
      ctx.did_change_active_pane(previous_buffer_id);
    }
  }
  ctx.request_render();
}

pub fn close_all_file_tree<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(root) = ctx.file_tree().root.clone() else {
    return;
  };
  let state = ctx.file_tree_mut();
  state.expanded_dirs.retain(|path| path == &root);
  rebuild_rows(ctx);
  ctx.request_render();
}

fn toggle_file_tree_with_root<Ctx: DefaultContext>(ctx: &mut Ctx, current_buffer_directory: bool) {
  let tree_is_open = if ctx.file_tree_uses_split_pane() {
    attached_tree_pane(ctx).is_some()
  } else {
    ctx.file_tree().visible
  };
  if tree_is_open {
    close_file_tree(ctx);
    return;
  }

  remember_active_editor_pane(ctx);
  let root = desired_root(ctx, current_buffer_directory);
  let surface_id = ensure_surface(ctx);

  let uses_split_pane = ctx.file_tree_uses_split_pane();
  {
    let state = ctx.file_tree_mut();
    let root_changed = state.root.as_deref() != Some(root.as_path());
    if root_changed {
      state.root = Some(root.clone());
      state.expanded_dirs.clear();
      state.expanded_dirs.insert(root.clone());
      state.selected = None;
      state.scroll_offset = 0;
    }
    if !state.follow_current {
      state.follow_current = true;
    }
    if !uses_split_pane {
      state.visible = true;
      state.active = true;
    }
  }

  if uses_split_pane {
    open_tree_in_sidebar(ctx, surface_id);
  }
  rebuild_rows(ctx);
  reveal_current_file(ctx);
  ctx.request_render();
}

fn open_tree_in_sidebar<Ctx: DefaultContext>(ctx: &mut Ctx, surface_id: ClientSurfaceId) {
  let previous_pane = ctx.editor_ref().active_pane_id();
  let viewport = ctx.editor_ref().layout_viewport();
  let leftmost_pane = ctx
    .editor_ref()
    .frame_pane_snapshots(viewport)
    .into_iter()
    .min_by_key(|pane| (pane.rect.x, pane.rect.y))
    .map(|pane| pane.pane_id)
    .unwrap_or(previous_pane);
  remember_active_editor_pane(ctx);
  let target = ctx.editor().resolve_open_target(OpenTarget::Split {
    axis:      SplitAxis::Vertical,
    focus_new: true,
  });
  if let Some(target) = target {
    let _ = ctx
      .editor()
      .move_pane(target.pane, leftmost_pane, PaneDirection::Left);
    let _ = ctx.editor().set_active_pane(target.pane);
    if ctx.editor().open_client_surface_in_active_pane(surface_id) {
      ctx.file_tree_mut().sidebar_pane = Some(target.pane);
    }
  }
  if matches!(
    ctx.editor_ref().pane_content_kind(previous_pane),
    Some(the_lib::editor::PaneContentKind::EditorBuffer)
  ) {
    ctx.file_tree_mut().last_editor_pane = Some(previous_pane);
  }
}

fn move_selection<Ctx: DefaultContext>(ctx: &mut Ctx, amount: isize) {
  let len = ctx.file_tree().rows.len();
  if len == 0 {
    return;
  }

  let current = ctx.file_tree().selected.unwrap_or(0);
  let next = current
    .saturating_add_signed(amount)
    .min(len.saturating_sub(1));
  let scrolloff = ctx.scrolloff();
  let state = ctx.file_tree_mut();
  state.selected = Some(next);
  state.selection_follow = true;
  sync_tree_scroll(state, scrolloff);
  ctx.request_render();
}

fn collapse_selected_or_select_parent<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(index) = ctx.file_tree().selected else {
    return;
  };
  let Some(row) = ctx.file_tree().rows.get(index).cloned() else {
    return;
  };

  if row.is_dir && row.is_expanded {
    ctx.file_tree_mut().expanded_dirs.remove(&row.path);
    rebuild_rows(ctx);
    ctx.request_render();
    return;
  }

  let current_depth = row.depth;
  if current_depth == 0 {
    return;
  }
  if let Some(parent_index) = (0..index).rev().find(|candidate| {
    ctx
      .file_tree()
      .rows
      .get(*candidate)
      .is_some_and(|candidate_row| candidate_row.depth < current_depth)
  }) {
    let scrolloff = ctx.scrolloff();
    let state = ctx.file_tree_mut();
    state.selected = Some(parent_index);
    state.selection_follow = true;
    sync_tree_scroll(state, scrolloff);
    ctx.request_render();
  }
}

fn expand_or_open_selected<Ctx: DefaultContext>(ctx: &mut Ctx, split: Option<SplitAxis>) {
  let Some(index) = ctx.file_tree().selected else {
    return;
  };
  let Some(row) = ctx.file_tree().rows.get(index).cloned() else {
    return;
  };

  if row.is_dir {
    if row.is_expanded {
      ctx.file_tree_mut().expanded_dirs.remove(&row.path);
    } else {
      ctx.file_tree_mut().expanded_dirs.insert(row.path);
    }
    rebuild_rows(ctx);
    ctx.request_render();
    return;
  }

  if let Err(err) = open_file_from_tree(ctx, &row.path, split) {
    let _ = ctx.push_error(
      "file_tree",
      format!("failed to open '{}': {err}", row.path.display()),
    );
  }
}

fn root_to_parent<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(root) = ctx.file_tree().root.clone() else {
    return;
  };
  let Some(parent) = root.parent().map(Path::to_path_buf) else {
    return;
  };
  let state = ctx.file_tree_mut();
  state.root = Some(parent.clone());
  state.expanded_dirs.clear();
  state.expanded_dirs.insert(parent);
  state.selected = None;
  state.scroll_offset = 0;
  state.selection_follow = true;
  rebuild_rows(ctx);
  ctx.request_render();
}

fn toggle_hidden<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let show_hidden = !ctx.file_tree().show_hidden;
  ctx.file_tree_mut().show_hidden = show_hidden;
  rebuild_rows(ctx);
  ctx.request_render();
}

fn yank_selected<Ctx: DefaultContext>(ctx: &mut Ctx, mode: FileTreeClipboardMode) {
  let Some(path) = selected_path(ctx) else {
    ctx.push_warning("file_tree", "no file-tree entry selected");
    return;
  };
  ctx.file_tree_mut().clipboard = Some(FileTreeClipboard {
    path: path.clone(),
    mode,
  });
  let verb = match mode {
    FileTreeClipboardMode::Copy => "copied",
    FileTreeClipboardMode::Move => "cut",
  };
  ctx.push_info("file_tree", format!("{verb}: {}", path.display()));
  ctx.request_render();
}

fn paste_into_selected_directory<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if let Err(err) = paste_clipboard(ctx) {
    ctx.push_error("file_tree", err);
  } else {
    ctx.request_render();
  }
}

fn cmd_add<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let input = args
    .first()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| crate::CommandError::new("usage: :file-tree-add <name>"))?;
  let base = add_base_directory(ctx)
    .ok_or_else(|| crate::CommandError::new("file tree is not available"))?;
  let target = resolve_tree_input_path(&base, input);

  if target.exists() {
    return Err(crate::CommandError::new(format!(
      "'{}' already exists",
      target.display()
    )));
  }

  if input.ends_with('/') {
    std::fs::create_dir_all(&target).map_err(|err| {
      crate::CommandError::new(format!(
        "failed to create directory '{}': {err}",
        target.display()
      ))
    })?;
  } else {
    if let Some(parent) = target.parent()
      && !parent.as_os_str().is_empty()
    {
      std::fs::create_dir_all(parent).map_err(|err| {
        crate::CommandError::new(format!(
          "failed to create directory '{}': {err}",
          parent.display()
        ))
      })?;
    }
    OpenOptions::new()
      .create_new(true)
      .write(true)
      .open(&target)
      .map_err(|err| {
        crate::CommandError::new(format!("failed to create '{}': {err}", target.display()))
      })?;
  }

  rebuild_rows(ctx);
  reveal_path(ctx, &target);
  Ok(())
}

fn cmd_yank<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  yank_selected(ctx, FileTreeClipboardMode::Copy);
  Ok(())
}

fn cmd_cut<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  yank_selected(ctx, FileTreeClipboardMode::Move);
  Ok(())
}

fn cmd_paste<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  paste_clipboard(ctx).map_err(crate::CommandError::new)
}

fn cmd_copy<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  let source =
    selected_path(ctx).ok_or_else(|| crate::CommandError::new("no file-tree entry selected"))?;
  let input = args
    .first()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| crate::CommandError::new("usage: :file-tree-copy <target>"))?;
  let base = source
    .parent()
    .map(Path::to_path_buf)
    .or_else(|| ctx.file_tree().root.clone())
    .ok_or_else(|| crate::CommandError::new("file tree is not available"))?;
  let target = resolve_tree_input_path(&base, input);
  copy_selected_entry(ctx, &source, &target).map_err(crate::CommandError::new)
}

fn cmd_move<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  let source =
    selected_path(ctx).ok_or_else(|| crate::CommandError::new("no file-tree entry selected"))?;
  let input = args
    .first()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| crate::CommandError::new("usage: :file-tree-move <target>"))?;
  let base = source
    .parent()
    .map(Path::to_path_buf)
    .or_else(|| ctx.file_tree().root.clone())
    .ok_or_else(|| crate::CommandError::new("file tree is not available"))?;
  let target = resolve_tree_input_path(&base, input);
  move_selected_entry(ctx, &source, &target).map_err(crate::CommandError::new)
}

fn cmd_rename<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  let input = args
    .first()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| crate::CommandError::new("usage: :file-tree-rename <new-name>"))?;
  let source =
    selected_path(ctx).ok_or_else(|| crate::CommandError::new("no file-tree entry selected"))?;
  let parent = source
    .parent()
    .ok_or_else(|| crate::CommandError::new("selected entry has no parent directory"))?;
  let target = resolve_tree_input_path(parent, input);
  if source == target {
    return Ok(());
  }
  if target.exists() {
    return Err(crate::CommandError::new(format!(
      "'{}' already exists",
      target.display()
    )));
  }

  std::fs::rename(&source, &target).map_err(|err| {
    crate::CommandError::new(format!(
      "failed to rename '{}' to '{}': {err}",
      source.display(),
      target.display()
    ))
  })?;

  retarget_expanded_dirs(ctx.file_tree_mut(), &source, &target);
  rebuild_rows(ctx);
  reveal_path(ctx, &target);
  Ok(())
}

fn cmd_delete<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  if args.first().map(str::trim) != Some("yes") {
    return Err(crate::CommandError::new(
      "delete is permanent; use :file-tree-delete yes to confirm",
    ));
  }

  let target =
    selected_path(ctx).ok_or_else(|| crate::CommandError::new("no file-tree entry selected"))?;
  let parent = target.parent().map(Path::to_path_buf);

  if target.is_dir() {
    std::fs::remove_dir_all(&target).map_err(|err| {
      crate::CommandError::new(format!(
        "failed to delete directory '{}': {err}",
        target.display()
      ))
    })?;
  } else {
    std::fs::remove_file(&target).map_err(|err| {
      crate::CommandError::new(format!(
        "failed to delete file '{}': {err}",
        target.display()
      ))
    })?;
  }

  let state = ctx.file_tree_mut();
  state
    .expanded_dirs
    .retain(|path| !path.starts_with(&target));
  if let Some(parent) = parent {
    state.expanded_dirs.insert(parent);
  }
  rebuild_rows(ctx);
  Ok(())
}

fn cmd_trash<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }

  if args.first().map(str::trim) != Some("yes") {
    return Err(crate::CommandError::new(
      "trash moves the entry out of the workspace; use :file-tree-trash yes to confirm",
    ));
  }

  let target =
    selected_path(ctx).ok_or_else(|| crate::CommandError::new("no file-tree entry selected"))?;
  trash_selected_entry(ctx, &target).map_err(crate::CommandError::new)
}

fn cmd_up<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  root_to_parent(ctx);
  Ok(())
}

fn cmd_focus<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  focus_file_tree(ctx);
  Ok(())
}

fn cmd_close_all<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  _args: the_lib::command_line::Args,
  event: CommandEvent,
) -> crate::CommandResult {
  if event != CommandEvent::Validate {
    return Ok(());
  }
  close_all_file_tree(ctx);
  Ok(())
}

fn ensure_surface<Ctx: DefaultContext>(ctx: &mut Ctx) -> ClientSurfaceId {
  if let Some(surface_id) = ctx.file_tree().surface_id {
    return surface_id;
  }
  let surface_id = ctx.editor().create_client_surface();
  ctx.file_tree_mut().surface_id = Some(surface_id);
  surface_id
}

fn desired_root<Ctx: DefaultContext>(ctx: &Ctx, current_buffer_directory: bool) -> PathBuf {
  if current_buffer_directory {
    ctx
      .file_path()
      .and_then(Path::parent)
      .map(Path::to_path_buf)
      .unwrap_or_else(|| ctx.effective_working_directory())
  } else {
    ctx.workspace_root()
  }
}

fn attached_tree_pane<Ctx: DefaultContext>(ctx: &Ctx) -> Option<PaneId> {
  let surface_id = ctx.file_tree().surface_id?;
  ctx
    .editor_ref()
    .client_surface_snapshots()
    .into_iter()
    .find(|surface| surface.client_surface_id == surface_id)
    .and_then(|surface| surface.attached_pane)
}

fn add_base_directory<Ctx: DefaultContext>(ctx: &Ctx) -> Option<PathBuf> {
  let selected = selected_path(ctx);
  match selected {
    Some(path) if path.is_dir() => Some(path),
    Some(path) => path.parent().map(Path::to_path_buf),
    None => ctx.file_tree().root.clone(),
  }
}

fn selected_path<Ctx: DefaultContext>(ctx: &Ctx) -> Option<PathBuf> {
  let state = ctx.file_tree();
  state
    .selected
    .and_then(|index| state.rows.get(index))
    .map(|row| row.path.clone())
}

fn selected_directory<Ctx: DefaultContext>(ctx: &Ctx) -> Option<PathBuf> {
  match selected_path(ctx) {
    Some(path) if path.is_dir() => Some(path),
    Some(path) => path.parent().map(Path::to_path_buf),
    None => ctx.file_tree().root.clone(),
  }
}

fn resolve_tree_input_path(base: &Path, input: &str) -> PathBuf {
  let path = Path::new(input.trim_end_matches('/'));
  if path.is_absolute() {
    path.to_path_buf()
  } else {
    base.join(path)
  }
}

fn copy_selected_entry<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  source: &Path,
  target: &Path,
) -> Result<(), String> {
  ensure_target_available(source, target)?;
  copy_path_recursively(source, target)?;
  rebuild_rows(ctx);
  reveal_path(ctx, target);
  ctx.push_info("file_tree", format!("copied to {}", target.display()));
  Ok(())
}

fn move_selected_entry<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  source: &Path,
  target: &Path,
) -> Result<(), String> {
  ensure_target_available(source, target)?;
  move_path(source, target)?;
  retarget_expanded_dirs(ctx.file_tree_mut(), source, target);
  rebuild_rows(ctx);
  reveal_path(ctx, target);
  ctx.push_info("file_tree", format!("moved to {}", target.display()));
  Ok(())
}

fn trash_selected_entry<Ctx: DefaultContext>(ctx: &mut Ctx, source: &Path) -> Result<(), String> {
  let trash_dir = trash_directory().ok_or_else(|| {
    "trash is not available on this platform; use :file-tree-delete yes instead".to_string()
  })?;
  std::fs::create_dir_all(&trash_dir).map_err(|err| {
    format!(
      "failed to create trash directory '{}': {err}",
      trash_dir.display()
    )
  })?;
  let target = unique_destination_in_dir(&trash_dir, file_name_for_path(source)?)?;
  move_path(source, &target)?;
  ctx
    .file_tree_mut()
    .expanded_dirs
    .retain(|path| !path.starts_with(source));
  rebuild_rows(ctx);
  ctx.push_info("file_tree", format!("trashed {}", source.display()));
  Ok(())
}

fn rebuild_rows<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let perf_start = file_tree_perf_enabled().then(Instant::now);
  let current_file = ctx.file_path().map(PathBuf::from);
  let Some(root) = ctx.file_tree().root.clone() else {
    ctx.file_tree_mut().clear_rows();
    return;
  };
  let (selected_path, root, show_hidden, mut expanded_dirs, decorations) = {
    let state = ctx.file_tree();
    let selected_path = state
      .selected
      .and_then(|index| state.rows.get(index))
      .map(|row| row.path.clone());
    (
      selected_path,
      root.clone(),
      state.show_hidden,
      state.expanded_dirs.clone(),
      state.combined_decorations(),
    )
  };

  expanded_dirs.retain(|path| path == &root || path.starts_with(&root));
  expanded_dirs.insert(root.clone());
  let mut rows = Vec::new();
  append_rows(
    &root,
    &[],
    show_hidden,
    &expanded_dirs,
    current_file.as_deref(),
    &decorations,
    &mut rows,
  );
  let scrolloff = ctx.scrolloff();
  let state = ctx.file_tree_mut();
  state.expanded_dirs = expanded_dirs;
  state.rows = rows;
  state.selected = selected_path
    .as_ref()
    .and_then(|path| state.rows.iter().position(|row| &row.path == path))
    .or_else(|| {
      current_file
        .as_ref()
        .and_then(|path| state.rows.iter().position(|row| row.path == *path))
    })
    .or_else(|| (!state.rows.is_empty()).then_some(0));
  clamp_tree_state(state);
  state.selection_follow = true;
  sync_tree_scroll(state, scrolloff);

  if let Some(perf_start) = perf_start {
    let rebuild_ms = perf_start.elapsed().as_secs_f64() * 1000.0;
    file_tree_perf_log(format!(
      "kind=file_tree_refresh total={rebuild_ms:.2}ms root={} rows={} expanded={} selected={} \
       scroll={} visible_rows={} show_hidden={} follow_current={}",
      root.display(),
      state.rows.len(),
      state.expanded_dirs.len(),
      state
        .selected
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string()),
      state.scroll_offset,
      state.visible_rows,
      if state.show_hidden { 1 } else { 0 },
      if state.follow_current { 1 } else { 0 },
    ));
  }
}

pub fn collapse_file_tree_vcs_statuses(
  changes: &[FileChange],
  root: &Path,
) -> BTreeMap<PathBuf, FileTreeVcsKind> {
  let mut statuses = BTreeMap::new();
  for change in changes {
    let (path, kind) = match change {
      FileChange::Untracked { path } => (path.as_path(), FileTreeVcsKind::Untracked),
      FileChange::Modified { path } => (path.as_path(), FileTreeVcsKind::Modified),
      FileChange::Conflict { path } => (path.as_path(), FileTreeVcsKind::Conflict),
      FileChange::Deleted { path } => (path.as_path(), FileTreeVcsKind::Deleted),
      FileChange::Renamed { to_path, .. } => (to_path.as_path(), FileTreeVcsKind::Renamed),
    };
    if !path.starts_with(root) {
      continue;
    }
    apply_tree_hierarchy_status(&mut statuses, path, root, kind, choose_vcs_status);
  }

  statuses
}

pub fn rebuild_file_tree_diagnostic_statuses(
  diagnostics: &DiagnosticsState,
  root: &Path,
) -> BTreeMap<PathBuf, DiagnosticSeverity> {
  let mut statuses = BTreeMap::new();
  for document in diagnostics.documents() {
    let Some(path) = path_for_file_uri(&document.uri) else {
      continue;
    };
    if !path.starts_with(root) {
      continue;
    }
    let Some(severity) = document
      .diagnostics
      .iter()
      .filter_map(|diagnostic| diagnostic.severity)
      .max_by_key(|severity| file_tree_diagnostic_rank(*severity))
    else {
      continue;
    };
    apply_tree_hierarchy_status(
      &mut statuses,
      &path,
      root,
      severity,
      choose_diagnostic_severity,
    );
  }
  statuses
}

pub fn combine_file_tree_decorations(
  vcs_statuses: &BTreeMap<PathBuf, FileTreeVcsKind>,
  diagnostic_statuses: &BTreeMap<PathBuf, DiagnosticSeverity>,
) -> BTreeMap<PathBuf, FileTreeDecorations> {
  let mut decorations: BTreeMap<PathBuf, FileTreeDecorations> = BTreeMap::new();
  for (path, &vcs) in vcs_statuses {
    decorations.entry(path.clone()).or_default().vcs = Some(vcs);
  }
  for (path, &diagnostic) in diagnostic_statuses {
    decorations.entry(path.clone()).or_default().diagnostic = Some(diagnostic);
  }
  decorations
}

pub fn set_file_tree_vcs_statuses<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  statuses: BTreeMap<PathBuf, FileTreeVcsKind>,
) -> bool {
  let state = ctx.file_tree_mut();
  if state.vcs_statuses == statuses {
    return false;
  }
  state.vcs_statuses = statuses;
  apply_file_tree_row_decorations(state)
}

pub fn set_file_tree_diagnostic_statuses<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  statuses: BTreeMap<PathBuf, DiagnosticSeverity>,
) -> bool {
  let state = ctx.file_tree_mut();
  if state.diagnostic_statuses == statuses {
    return false;
  }
  state.diagnostic_statuses = statuses;
  apply_file_tree_row_decorations(state)
}

pub fn clear_file_tree_decorations<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let state = ctx.file_tree_mut();
  let had_decorations = !state.vcs_statuses.is_empty() || !state.diagnostic_statuses.is_empty();
  state.vcs_statuses.clear();
  state.diagnostic_statuses.clear();
  apply_file_tree_row_decorations(state) || had_decorations
}

fn apply_file_tree_row_decorations(state: &mut FileTreeState) -> bool {
  let decorations = state.combined_decorations();
  let mut changed = false;
  for row in &mut state.rows {
    let next = decorations.get(&row.path).copied().unwrap_or_default();
    if row.decorations != next {
      row.decorations = next;
      changed = true;
    }
  }
  changed
}

fn apply_tree_hierarchy_status<T: Copy>(
  statuses: &mut BTreeMap<PathBuf, T>,
  path: &Path,
  root: &Path,
  value: T,
  choose: fn(T, T) -> T,
) {
  let mut current = Some(path.to_path_buf());
  while let Some(candidate) = current {
    if !candidate.starts_with(root) {
      break;
    }
    statuses
      .entry(candidate.clone())
      .and_modify(|existing| *existing = choose(*existing, value))
      .or_insert(value);
    if candidate == root {
      break;
    }
    current = candidate.parent().map(Path::to_path_buf);
  }
}

fn choose_vcs_status(left: FileTreeVcsKind, right: FileTreeVcsKind) -> FileTreeVcsKind {
  if file_tree_vcs_rank(left) >= file_tree_vcs_rank(right) {
    left
  } else {
    right
  }
}

fn choose_diagnostic_severity(
  left: DiagnosticSeverity,
  right: DiagnosticSeverity,
) -> DiagnosticSeverity {
  if file_tree_diagnostic_rank(left) >= file_tree_diagnostic_rank(right) {
    left
  } else {
    right
  }
}

fn file_tree_vcs_rank(kind: FileTreeVcsKind) -> u8 {
  match kind {
    FileTreeVcsKind::Conflict => 5,
    FileTreeVcsKind::Deleted => 4,
    FileTreeVcsKind::Modified => 3,
    FileTreeVcsKind::Renamed => 2,
    FileTreeVcsKind::Untracked => 1,
  }
}

fn file_tree_diagnostic_rank(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 4,
    DiagnosticSeverity::Warning => 3,
    DiagnosticSeverity::Information => 2,
    DiagnosticSeverity::Hint => 1,
  }
}

fn paste_clipboard<Ctx: DefaultContext>(ctx: &mut Ctx) -> Result<(), String> {
  let clipboard = ctx
    .file_tree()
    .clipboard
    .clone()
    .ok_or_else(|| "file-tree clipboard is empty".to_string())?;
  if !clipboard.path.exists() {
    ctx.file_tree_mut().clipboard = None;
    return Err(format!(
      "clipboard entry no longer exists: {}",
      clipboard.path.display()
    ));
  }

  let destination_dir = selected_directory(ctx)
    .ok_or_else(|| "no destination directory available in the file tree".to_string())?;
  let source_name = file_name_for_path(&clipboard.path)?;
  let target = unique_destination_in_dir(&destination_dir, source_name)?;
  match clipboard.mode {
    FileTreeClipboardMode::Copy => {
      copy_path_recursively(&clipboard.path, &target)?;
      ctx.push_info("file_tree", format!("pasted to {}", target.display()));
    },
    FileTreeClipboardMode::Move => {
      move_path(&clipboard.path, &target)?;
      retarget_expanded_dirs(ctx.file_tree_mut(), &clipboard.path, &target);
      ctx.file_tree_mut().clipboard = None;
      ctx.push_info("file_tree", format!("moved to {}", target.display()));
    },
  }
  rebuild_rows(ctx);
  reveal_path(ctx, &target);
  Ok(())
}

fn append_rows(
  directory: &Path,
  ancestor_branches: &[bool],
  show_hidden: bool,
  expanded_dirs: &BTreeSet<PathBuf>,
  current_file: Option<&Path>,
  decorations: &BTreeMap<PathBuf, FileTreeDecorations>,
  rows: &mut Vec<FileTreeRow>,
) {
  let entries = visible_directory_entries(directory, show_hidden);

  for (index, (path, name, is_dir)) in entries.iter().enumerate() {
    let is_last_sibling = index + 1 == entries.len();
    let expanded = *is_dir && expanded_dirs.contains(path.as_path());
    let icon_name = if *is_dir {
      if expanded {
        "folder_open".to_string()
      } else {
        "folder".to_string()
      }
    } else {
      file_picker_icon_name_for_path(path.as_path()).to_string()
    };
    let has_children = *is_dir && directory_has_visible_entries(path, show_hidden);
    rows.push(FileTreeRow {
      path: path.clone(),
      display_name: name.clone(),
      depth: ancestor_branches.len(),
      ancestor_branches: ancestor_branches.to_vec(),
      is_last_sibling,
      has_children,
      is_dir: *is_dir,
      is_expanded: expanded,
      is_current_file: current_file.is_some_and(|current| current == path.as_path()),
      decorations: decorations.get(path).copied().unwrap_or_default(),
      icon_glyph: file_picker_icon_glyph(&icon_name, *is_dir),
      icon_name,
    });

    if *is_dir && expanded {
      let mut next_ancestor_branches = ancestor_branches.to_vec();
      next_ancestor_branches.push(!is_last_sibling);
      append_rows(
        path,
        &next_ancestor_branches,
        show_hidden,
        expanded_dirs,
        current_file,
        decorations,
        rows,
      );
    }
  }
}

fn visible_directory_entries(directory: &Path, show_hidden: bool) -> Vec<(PathBuf, String, bool)> {
  let Ok(read_dir) = std::fs::read_dir(directory) else {
    return Vec::new();
  };

  let mut entries = read_dir
    .flatten()
    .filter_map(|entry| {
      let file_type = entry.file_type().ok()?;
      let name = entry.file_name();
      let name = name.to_str()?.to_string();
      if !show_hidden && name.starts_with('.') {
        return None;
      }
      Some((entry.path(), name, file_type.is_dir()))
    })
    .collect::<Vec<_>>();

  entries.sort_by(|a, b| {
    b.2
      .cmp(&a.2)
      .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
  });

  entries
}

fn directory_has_visible_entries(directory: &Path, show_hidden: bool) -> bool {
  !visible_directory_entries(directory, show_hidden).is_empty()
}

fn reveal_path<Ctx: DefaultContext>(ctx: &mut Ctx, path: &Path) {
  let Some(root) = ctx.file_tree().root.clone() else {
    return;
  };
  if !path.starts_with(&root) {
    return;
  }

  let mut current = path.parent();
  while let Some(dir) = current {
    if dir.starts_with(&root) {
      ctx.file_tree_mut().expanded_dirs.insert(dir.to_path_buf());
    }
    if dir == root.as_path() {
      break;
    }
    current = dir.parent();
  }

  rebuild_rows(ctx);
  if let Some(index) = ctx.file_tree().rows.iter().position(|row| row.path == path) {
    let scrolloff = ctx.scrolloff();
    let state = ctx.file_tree_mut();
    state.selected = Some(index);
    state.selection_follow = true;
    sync_tree_scroll(state, scrolloff);
  }
}

fn clamp_tree_state(state: &mut FileTreeState) {
  if state.rows.is_empty() {
    state.selected = None;
    state.scroll_offset = 0;
    return;
  }

  let max_index = state.rows.len().saturating_sub(1);
  state.selected = Some(state.selected.unwrap_or(0).min(max_index));
  clamp_tree_scroll_offset(state);
}

fn clamp_tree_scroll_offset(state: &mut FileTreeState) {
  state.scroll_offset = state.scroll_offset.min(tree_max_scroll_offset(state));
}

fn sync_tree_scroll(state: &mut FileTreeState, scrolloff: usize) {
  clamp_tree_state(state);
  if state.selection_follow
    && let Some(selected) = state.selected
    && let Some(next) = scroll_row_to_keep_visible(
      selected,
      state.scroll_offset,
      state.visible_rows.max(1),
      scrolloff,
    )
  {
    state.scroll_offset = next;
  }
  state.selection_follow = false;
  clamp_tree_scroll_offset(state);
}

fn tree_max_scroll_offset(state: &FileTreeState) -> usize {
  let visible_rows = state.visible_rows.max(1);
  state.rows.len().saturating_sub(visible_rows)
}

fn retarget_expanded_dirs(state: &mut FileTreeState, source: &Path, target: &Path) {
  let replacement = state
    .expanded_dirs
    .iter()
    .map(|path| {
      if path.starts_with(source) {
        target.join(path.strip_prefix(source).unwrap_or(Path::new("")))
      } else {
        path.clone()
      }
    })
    .collect::<BTreeSet<_>>();
  state.expanded_dirs = replacement;
}

fn ensure_target_available(source: &Path, target: &Path) -> Result<(), String> {
  if source == target {
    return Ok(());
  }
  if target.starts_with(source) {
    return Err(format!(
      "cannot move '{}' into its own descendant '{}'",
      source.display(),
      target.display()
    ));
  }
  if target.exists() {
    return Err(format!("'{}' already exists", target.display()));
  }
  if let Some(parent) = target.parent()
    && !parent.as_os_str().is_empty()
  {
    std::fs::create_dir_all(parent)
      .map_err(|err| format!("failed to create directory '{}': {err}", parent.display()))?;
  }
  Ok(())
}

fn copy_path_recursively(source: &Path, target: &Path) -> Result<(), String> {
  if source.is_dir() {
    std::fs::create_dir_all(target)
      .map_err(|err| format!("failed to create directory '{}': {err}", target.display()))?;
    for entry in read_dir(source)
      .map_err(|err| format!("failed to read directory '{}': {err}", source.display()))?
    {
      let entry = entry.map_err(|err| {
        format!(
          "failed to read directory entry '{}': {err}",
          source.display()
        )
      })?;
      let child_source = entry.path();
      let child_target = target.join(entry.file_name());
      copy_path_recursively(&child_source, &child_target)?;
    }
    return Ok(());
  }

  std::fs::copy(source, target).map_err(|err| {
    format!(
      "failed to copy '{}' to '{}': {err}",
      source.display(),
      target.display()
    )
  })?;
  Ok(())
}

fn move_path(source: &Path, target: &Path) -> Result<(), String> {
  match std::fs::rename(source, target) {
    Ok(()) => Ok(()),
    Err(err) if err.raw_os_error() == Some(18) => {
      copy_path_recursively(source, target)?;
      remove_path_recursively(source)?;
      Ok(())
    },
    Err(err) => {
      Err(format!(
        "failed to move '{}' to '{}': {err}",
        source.display(),
        target.display()
      ))
    },
  }
}

fn remove_path_recursively(path: &Path) -> Result<(), String> {
  if path.is_dir() {
    std::fs::remove_dir_all(path)
      .map_err(|err| format!("failed to delete directory '{}': {err}", path.display()))
  } else {
    std::fs::remove_file(path)
      .map_err(|err| format!("failed to delete file '{}': {err}", path.display()))
  }
}

fn file_name_for_path(path: &Path) -> Result<OsString, String> {
  path
    .file_name()
    .map(OsString::from)
    .ok_or_else(|| format!("'{}' has no file name", path.display()))
}

fn unique_destination_in_dir(dir: &Path, file_name: OsString) -> Result<PathBuf, String> {
  let candidate = dir.join(&file_name);
  if !candidate.exists() {
    return Ok(candidate);
  }

  let path = PathBuf::from(&file_name);
  let stem = path
    .file_stem()
    .map(OsString::from)
    .unwrap_or_else(|| file_name.clone());
  let extension = path.extension().map(OsString::from);

  for index in 1.. {
    let mut candidate_name = stem.clone();
    if index == 1 {
      candidate_name.push(" copy");
    } else {
      candidate_name.push(format!(" copy {index}"));
    }
    if let Some(ext) = &extension {
      candidate_name.push(".");
      candidate_name.push(ext);
    }
    let candidate = dir.join(&candidate_name);
    if !candidate.exists() {
      return Ok(candidate);
    }
  }

  Err(format!(
    "failed to derive unique destination under '{}'",
    dir.display()
  ))
}

fn trash_directory() -> Option<PathBuf> {
  #[cfg(target_os = "macos")]
  {
    let home = std::env::var_os("HOME")?;
    return Some(PathBuf::from(home).join(".Trash"));
  }

  #[cfg(all(unix, not(target_os = "macos")))]
  {
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
      return Some(PathBuf::from(data_home).join("Trash/files"));
    }
    let home = std::env::var_os("HOME")?;
    return Some(PathBuf::from(home).join(".local/share/Trash/files"));
  }

  #[cfg(not(unix))]
  {
    None
  }
}

fn open_file_from_tree<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  path: &Path,
  split: Option<SplitAxis>,
) -> std::io::Result<()> {
  let pane = resolve_editor_target_pane(ctx, split);
  if let Some(pane) = pane {
    let previous_buffer_id = ctx.editor_ref().active_buffer_id();
    let _ = ctx.editor().set_active_pane(pane);
    ctx.did_change_active_pane(previous_buffer_id);
  }

  if let Some(axis) = split {
    let previous_buffer_id = ctx.editor_ref().active_buffer_id();
    let _ = ctx.editor().resolve_open_target(OpenTarget::Split {
      axis,
      focus_new: true,
    });
    ctx.did_change_active_pane(previous_buffer_id);
  }

  let result = ctx.open_file(path);
  remember_active_editor_pane(ctx);
  sync_file_tree_to_active_file(ctx);
  result
}

fn resolve_editor_target_pane<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  split: Option<SplitAxis>,
) -> Option<PaneId> {
  let tree_pane = attached_tree_pane(ctx);
  if let Some(pane) = ctx.file_tree().last_editor_pane
    && matches!(
      ctx.editor_ref().pane_content_kind(pane),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    )
  {
    return Some(pane);
  }

  if let Some(tree_pane) = tree_pane {
    if split.is_none()
      && let Some(right) = ctx
        .editor_ref()
        .pane_in_direction(tree_pane, PaneDirection::Right)
      && matches!(
        ctx.editor_ref().pane_content_kind(right),
        Some(the_lib::editor::PaneContentKind::EditorBuffer)
      )
    {
      return Some(right);
    }

    let previous_buffer_id = ctx.editor_ref().active_buffer_id();
    let _ = ctx.editor().set_active_pane(tree_pane);
    let target = OpenTarget::Neighbor {
      direction:         PaneDirection::Right,
      create_if_missing: true,
    };
    if let Some(resolved) = ctx.editor().resolve_open_target(target) {
      ctx.did_change_active_pane(previous_buffer_id);
      return Some(resolved.pane);
    }
  }

  Some(ctx.editor_ref().active_pane_id())
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    time::{
      SystemTime,
      UNIX_EPOCH,
    },
  };

  use the_lib::diagnostics::{
    Diagnostic,
    DiagnosticPosition,
    DiagnosticRange,
    DocumentDiagnostics,
  };

  use super::*;

  fn row(name: &str) -> FileTreeRow {
    FileTreeRow {
      path:              PathBuf::from(name),
      display_name:      name.to_string(),
      depth:             0,
      ancestor_branches: Vec::new(),
      is_last_sibling:   true,
      has_children:      false,
      is_dir:            false,
      is_expanded:       false,
      is_current_file:   false,
      decorations:       FileTreeDecorations::default(),
      icon_name:         "file".to_string(),
      icon_glyph:        "f",
    }
  }

  #[test]
  fn selection_follow_uses_scrolloff_padding() {
    let mut state = FileTreeState {
      rows: (0..20).map(|idx| row(&format!("row-{idx}"))).collect(),
      selected: Some(4),
      visible_rows: 5,
      selection_follow: true,
      ..FileTreeState::default()
    };

    sync_tree_scroll(&mut state, 2);

    assert_eq!(state.scroll_offset, 2);
    assert!(!state.selection_follow);
  }

  #[test]
  fn manual_scroll_only_clamps_when_selection_follow_is_disabled() {
    let mut state = FileTreeState {
      rows: (0..20).map(|idx| row(&format!("row-{idx}"))).collect(),
      selected: Some(4),
      scroll_offset: 99,
      visible_rows: 5,
      selection_follow: false,
      ..FileTreeState::default()
    };

    sync_tree_scroll(&mut state, 2);

    assert_eq!(state.scroll_offset, 15);
    assert!(!state.selection_follow);
  }

  #[test]
  fn append_rows_tracks_branch_metadata() {
    let unique = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time")
      .as_nanos();
    let root = std::env::temp_dir().join(format!("the-editor-file-tree-{unique}"));
    let first = root.join("alpha");
    let second = root.join("beta");
    let nested = first.join("child.txt");

    fs::create_dir_all(&first).expect("create alpha");
    fs::create_dir_all(&second).expect("create beta");
    fs::write(&nested, "child").expect("create child");

    let mut expanded_dirs = BTreeSet::new();
    expanded_dirs.insert(root.clone());
    expanded_dirs.insert(first.clone());

    let mut rows = Vec::new();
    append_rows(
      &root,
      &[],
      false,
      &expanded_dirs,
      None,
      &BTreeMap::new(),
      &mut rows,
    );

    let alpha = rows
      .iter()
      .find(|row| row.display_name == "alpha")
      .expect("alpha row");
    assert_eq!(alpha.depth, 0);
    assert_eq!(alpha.ancestor_branches, Vec::<bool>::new());
    assert!(!alpha.is_last_sibling);
    assert!(alpha.has_children);

    let child = rows
      .iter()
      .find(|row| row.display_name == "child.txt")
      .expect("child row");
    assert_eq!(child.depth, 1);
    assert_eq!(child.ancestor_branches, vec![true]);
    assert!(child.is_last_sibling);
    assert!(!child.has_children);

    let beta = rows
      .iter()
      .find(|row| row.display_name == "beta")
      .expect("beta row");
    assert_eq!(beta.depth, 0);
    assert!(beta.is_last_sibling);

    fs::remove_dir_all(&root).expect("cleanup tree");
  }

  #[test]
  fn unique_destination_in_dir_appends_copy_suffix() {
    let unique = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time")
      .as_nanos();
    let root = std::env::temp_dir().join(format!("the-editor-file-tree-copy-{unique}"));
    fs::create_dir_all(&root).expect("create root");
    let original = root.join("file.txt");
    fs::write(&original, "one").expect("write original");

    let target = unique_destination_in_dir(&root, OsString::from("file.txt")).expect("target");

    assert_eq!(
      target.file_name().and_then(|name| name.to_str()),
      Some("file copy.txt")
    );

    fs::remove_dir_all(&root).expect("cleanup");
  }

  #[test]
  fn collapse_file_tree_vcs_statuses_aggregate_to_parent_directories() {
    let root = PathBuf::from("/workspace");
    let nested = root.join("src/lib.rs");
    let statuses = collapse_file_tree_vcs_statuses(
      &[FileChange::Modified {
        path: nested.clone(),
      }],
      &root,
    );

    assert_eq!(statuses.get(&nested), Some(&FileTreeVcsKind::Modified));
    assert_eq!(statuses.get(&root.join("src")), Some(&FileTreeVcsKind::Modified));
    assert_eq!(statuses.get(&root), Some(&FileTreeVcsKind::Modified));
  }

  #[test]
  fn combine_file_tree_decorations_keeps_vcs_and_diagnostics() {
    let root = PathBuf::from("/workspace");
    let path = root.join("src/lib.rs");
    let mut vcs = BTreeMap::new();
    vcs.insert(path.clone(), FileTreeVcsKind::Renamed);
    let mut diagnostics = BTreeMap::new();
    diagnostics.insert(path.clone(), DiagnosticSeverity::Warning);

    let decorations = combine_file_tree_decorations(&vcs, &diagnostics);

    assert_eq!(
      decorations.get(&path),
      Some(&FileTreeDecorations {
        vcs: Some(FileTreeVcsKind::Renamed),
        diagnostic: Some(DiagnosticSeverity::Warning),
      })
    );
  }

  #[test]
  fn rebuild_file_tree_diagnostic_statuses_aggregate_to_parent_directories() {
    let unique = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time")
      .as_nanos();
    let root = std::env::temp_dir().join(format!("the-editor-file-tree-diagnostics-{unique}"));
    let file = root.join("src/main.rs");
    fs::create_dir_all(file.parent().expect("parent dir")).expect("create root");
    fs::write(&file, "fn main() {}\n").expect("write file");

    let uri = the_lsp::text_sync::file_uri_for_path(&file).expect("file uri");
    let mut diagnostics = DiagnosticsState::default();
    diagnostics.apply_document(DocumentDiagnostics {
      uri,
      version: None,
      diagnostics: vec![Diagnostic {
        range: DiagnosticRange {
          start: DiagnosticPosition {
            line: 0,
            character: 0,
          },
          end: DiagnosticPosition {
            line: 0,
            character: 2,
          },
        },
        severity: Some(DiagnosticSeverity::Error),
        code: None,
        source: Some("test".to_string()),
        message: "boom".to_string(),
      }],
    });

    let statuses = rebuild_file_tree_diagnostic_statuses(&diagnostics, &root);

    assert_eq!(statuses.get(&file), Some(&DiagnosticSeverity::Error));
    assert_eq!(
      statuses.get(&root.join("src")),
      Some(&DiagnosticSeverity::Error)
    );
    assert_eq!(statuses.get(&root), Some(&DiagnosticSeverity::Error));

    fs::remove_dir_all(&root).expect("cleanup");
  }

  #[test]
  fn copy_path_recursively_copies_directory_tree() {
    let unique = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time")
      .as_nanos();
    let root = std::env::temp_dir().join(format!("the-editor-file-tree-recursive-{unique}"));
    let source = root.join("src");
    let nested_dir = source.join("nested");
    let nested_file = nested_dir.join("child.txt");
    let target = root.join("dst");

    fs::create_dir_all(&nested_dir).expect("create nested source");
    fs::write(&nested_file, "hello").expect("write nested file");

    copy_path_recursively(&source, &target).expect("copy tree");

    assert_eq!(
      fs::read_to_string(target.join("nested/child.txt")).expect("copied child"),
      "hello"
    );

    fs::remove_dir_all(&root).expect("cleanup");
  }
}
