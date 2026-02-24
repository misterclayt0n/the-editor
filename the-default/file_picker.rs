use std::{
  borrow::Cow,
  cell::RefCell,
  cmp::Reverse,
  collections::{
    HashMap,
    VecDeque,
  },
  fs,
  io::Read,
  ops::Range,
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    Mutex,
    OnceLock,
    atomic::{
      AtomicBool,
      AtomicU64,
      Ordering,
    },
    mpsc::{
      self,
      Receiver,
      Sender,
      TryRecvError,
    },
  },
  time::{
    Duration,
    Instant,
  },
};

use ignore::DirEntry;
use nucleo::{
  Config as NucleoConfig,
  Matcher as NucleoMatcher,
  Nucleo,
  pattern::{
    CaseMatching,
    Normalization,
  },
};
use ropey::Rope;
use the_core::chars::{
  next_char_boundary,
  prev_char_boundary,
};
use the_lib::{
  diagnostics::DiagnosticSeverity,
  render::{
    UiColor,
    UiColorToken,
    UiConstraints,
    UiContainer,
    UiDivider,
    UiInput,
    UiLayout,
    UiList,
    UiListItem,
    UiNode,
    UiPanel,
    UiStyle,
  },
  selection::{
    CursorId,
    Selection,
  },
  split_tree::SplitAxis,
  syntax::{
    Highlight,
    Loader,
    Syntax,
  },
};

use crate::{
  DefaultContext,
  Key,
  KeyEvent,
};

const MAX_SCAN_ITEMS: usize = 100_000;
const MAX_FILE_SIZE_FOR_PREVIEW: u64 = 10 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = MAX_FILE_SIZE_FOR_PREVIEW as usize;
const MAX_PREVIEW_SOURCE_LINES: usize = 16_384;
const MAX_PREVIEW_DIRECTORY_ENTRIES: usize = 1024;
const PREVIEW_FOCUS_WINDOW_LINES: usize = 240;
const PREVIEW_CACHE_CAPACITY: usize = 128;
const PAGE_SIZE: usize = 12;
const MATCHER_TICK_TIMEOUT_MS: u64 = 10;
const DEFAULT_LIST_VISIBLE_ROWS: usize = 32;
const WARMUP_SCAN_BUDGET_MS: u64 = 30;
const PREVIEW_FOCUS_CONTEXT_LINES: usize = 3;

#[derive(Debug, Clone)]
struct CachedLineIndex {
  file_len:      u64,
  modified_secs: u64,
  line_starts:   Arc<[usize]>,
}

static PREVIEW_LINE_INDEX_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedLineIndex>>> =
  OnceLock::new();

thread_local! {
  static MATCH_INDEX_SCRATCH: RefCell<(NucleoMatcher, Vec<u32>)> =
    RefCell::new((NucleoMatcher::default(), Vec::new()));
}

#[derive(Debug, Clone)]
pub struct FilePickerItem {
  pub absolute:     PathBuf,
  pub display:      String,
  pub icon:         String,
  pub is_dir:       bool,
  pub display_path: bool,
  pub action:       FilePickerItemAction,
  pub preview_path: Option<PathBuf>,
  pub preview_line: Option<usize>,
  pub preview_col:  Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
pub enum FilePickerItemAction {
  OpenFile(PathBuf),
  GroupHeader {
    path: PathBuf,
  },
  SwitchBuffer {
    buffer_index: usize,
  },
  RestoreJump {
    buffer_index:  usize,
    selection:     Selection,
    active_cursor: Option<CursorId>,
  },
  OpenLocation {
    path:        PathBuf,
    cursor_char: usize,
    line:        usize,
    column:      Option<usize>,
  },
}

impl FilePickerItemAction {
  fn is_selectable(&self) -> bool {
    !matches!(self, Self::GroupHeader { .. })
  }
}

impl FilePickerItem {
  fn is_selectable(&self) -> bool {
    self.action.is_selectable()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerKind {
  Generic,
  Diagnostics,
  Symbols,
  LiveGrep,
}

impl Default for FilePickerKind {
  fn default() -> Self {
    Self::Generic
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerRowKind {
  Generic,
  Diagnostics,
  Symbols,
  LiveGrepHeader,
  LiveGrepMatch,
}

impl Default for FilePickerRowKind {
  fn default() -> Self {
    Self::Generic
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerRowData {
  pub kind:       FilePickerRowKind,
  pub severity:   Option<DiagnosticSeverity>,
  pub primary:    String,
  pub secondary:  String,
  pub tertiary:   String,
  pub quaternary: String,
  pub line:       usize,
  pub column:     usize,
  pub depth:      usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerPreviewLineKind {
  Content,
  TruncatedAbove,
  TruncatedBelow,
}

impl Default for FilePickerPreviewLineKind {
  fn default() -> Self {
    Self::Content
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerPreviewSegment {
  pub text:         String,
  pub highlight_id: Option<u32>,
  pub is_match:     bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerPreviewWindowLine {
  pub virtual_row: usize,
  pub kind:        FilePickerPreviewLineKind,
  pub line_number: Option<usize>,
  pub focused:     bool,
  pub marker:      String,
  pub segments:    Vec<FilePickerPreviewSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerPreviewWindow {
  pub kind:               u8, // 0=empty, 1=source, 2=text, 3=message
  pub total_virtual_rows: usize,
  pub offset:             usize,
  pub window_start:       usize,
  pub lines:              Vec<FilePickerPreviewWindowLine>,
}

#[derive(Debug, Clone)]
pub struct FilePickerDiagnosticItem {
  pub path:        PathBuf,
  pub line:        usize,
  pub character:   usize,
  pub cursor_char: usize,
  pub severity:    Option<DiagnosticSeverity>,
  pub code:        Option<String>,
  pub source:      Option<String>,
  pub message:     String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerChangedKind {
  Untracked,
  Modified,
  Conflict,
  Deleted,
  Renamed,
}

#[derive(Debug, Clone)]
pub struct FilePickerChangedFileItem {
  pub kind:      FilePickerChangedKind,
  pub path:      PathBuf,
  pub from_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickerConfig {
  pub hidden:            bool,
  pub follow_symlinks:   bool,
  pub deduplicate_links: bool,
  pub parents:           bool,
  pub ignore:            bool,
  pub git_ignore:        bool,
  pub git_global:        bool,
  pub git_exclude:       bool,
  pub max_depth:         Option<usize>,
}

impl Default for FilePickerConfig {
  fn default() -> Self {
    Self {
      hidden:            true,
      follow_symlinks:   true,
      deduplicate_links: true,
      parents:           true,
      ignore:            true,
      git_ignore:        true,
      git_global:        true,
      git_exclude:       true,
      max_depth:         None,
    }
  }
}

#[derive(Debug, Clone)]
pub enum FilePickerPreview {
  Empty,
  Source(FilePickerSourcePreview),
  Text(String),
  Message(String),
}

#[derive(Debug, Clone)]
pub struct FilePickerSourcePreview {
  pub lines:                 Arc<[String]>,
  pub line_starts:           Arc<[usize]>,
  pub highlights:            Arc<[(Highlight, Range<usize>)]>,
  pub base_line:             usize,
  pub truncated_above_lines: usize,
  pub truncated_below_lines: usize,
}

struct PreviewRequest {
  request_id: u64,
  path:       PathBuf,
  is_dir:     bool,
  focus_line: Option<usize>,
}

struct PreviewResult {
  request_id: u64,
  path:       PathBuf,
  preview:    FilePickerPreview,
  is_final:   bool,
}

struct SourcePreviewData {
  source:   FilePickerSourcePreview,
  text:     String,
  end_byte: usize,
}

enum PreviewBuild {
  Final(FilePickerPreview),
  Source(SourcePreviewData),
}

struct PreviewCache {
  capacity: usize,
  items:    HashMap<PathBuf, FilePickerPreview>,
  order:    VecDeque<PathBuf>,
}

impl PreviewCache {
  fn with_capacity(capacity: usize) -> Self {
    Self {
      capacity: capacity.max(1),
      items:    HashMap::new(),
      order:    VecDeque::new(),
    }
  }

  fn get(&mut self, path: &Path) -> Option<FilePickerPreview> {
    let preview = self.items.get(path).cloned()?;
    self.touch(path);
    Some(preview)
  }

  fn insert(&mut self, path: PathBuf, preview: FilePickerPreview) {
    if self.items.insert(path.clone(), preview).is_some() {
      self.touch(&path);
      return;
    }
    self.order.push_back(path.clone());
    while self.items.len() > self.capacity {
      let Some(evicted) = self.order.pop_front() else {
        break;
      };
      self.items.remove(&evicted);
    }
  }

  fn touch(&mut self, path: &Path) {
    let Some(position) = self
      .order
      .iter()
      .position(|candidate| candidate.as_path() == path)
    else {
      return;
    };
    let Some(existing) = self.order.remove(position) else {
      return;
    };
    self.order.push_back(existing);
  }
}

pub struct FilePickerState {
  pub active:             bool,
  pub root:               PathBuf,
  pub title:              String,
  pub config:             FilePickerConfig,
  pub query:              String,
  pub cursor:             usize,
  pub selected:           Option<usize>,
  pub hovered:            Option<usize>,
  pub list_offset:        usize,
  pub list_visible:       usize,
  pub preview_scroll:     usize,
  pub show_preview:       bool,
  pub open_split:         Option<SplitAxis>,
  pub preview_path:       Option<PathBuf>,
  pub preview_focus_line: Option<usize>,
  pub preview:            FilePickerPreview,
  pub error:              Option<String>,
  pub scanning:           bool,
  pub matcher_running:    bool,
  pub query_external:     bool,
  preview_cache:          PreviewCache,
  preview_req_tx:         Option<Sender<PreviewRequest>>,
  preview_res_rx:         Option<Receiver<PreviewResult>>,
  preview_request_id:     u64,
  preview_pending_id:     Option<u64>,
  scan_generation:        u64,
  scan_rx:                Option<Receiver<(u64, ScanMessage)>>,
  scan_cancel:            Option<Arc<AtomicBool>>,
  preview_latest_request: Arc<AtomicU64>,
  wake_tx:                Option<Sender<()>>,
  syntax_loader:          Option<Arc<Loader>>,
  matcher:                Nucleo<Arc<FilePickerItem>>,
}

enum ScanMessage {
  Done,
  Error(String),
}

impl Default for FilePickerState {
  fn default() -> Self {
    let matcher = new_matcher(None);
    Self {
      active: false,
      root: PathBuf::new(),
      title: "File Picker".to_string(),
      config: FilePickerConfig::default(),
      query: String::new(),
      cursor: 0,
      selected: None,
      hovered: None,
      list_offset: 0,
      list_visible: DEFAULT_LIST_VISIBLE_ROWS,
      preview_scroll: 0,
      show_preview: true,
      open_split: None,
      preview_path: None,
      preview_focus_line: None,
      preview: FilePickerPreview::Empty,
      error: None,
      scanning: false,
      matcher_running: false,
      query_external: false,
      preview_cache: PreviewCache::with_capacity(PREVIEW_CACHE_CAPACITY),
      preview_req_tx: None,
      preview_res_rx: None,
      preview_request_id: 0,
      preview_pending_id: None,
      scan_generation: 0,
      scan_rx: None,
      scan_cancel: None,
      preview_latest_request: Arc::new(AtomicU64::new(0)),
      wake_tx: None,
      syntax_loader: None,
      matcher,
    }
  }
}

pub fn set_file_picker_wake_sender(state: &mut FilePickerState, wake_tx: Option<Sender<()>>) {
  state.wake_tx = wake_tx;
}

pub fn set_file_picker_config(state: &mut FilePickerState, config: FilePickerConfig) {
  state.config = config;
}

pub fn set_file_picker_syntax_loader(state: &mut FilePickerState, loader: Option<Arc<Loader>>) {
  state.syntax_loader = loader;
}

impl FilePickerState {
  pub fn current_item(&self) -> Option<Arc<FilePickerItem>> {
    let selected = self.selected?;
    let snapshot = self.matcher.snapshot();
    Some(snapshot.get_matched_item(selected as u32)?.data.clone())
  }

  pub fn matched_item(&self, matched_index: usize) -> Option<Arc<FilePickerItem>> {
    let snapshot = self.matcher.snapshot();
    Some(
      snapshot
        .get_matched_item(matched_index as u32)?
        .data
        .clone(),
    )
  }

  pub fn matched_item_with_match_indices(
    &self,
    matched_index: usize,
    indices_out: &mut Vec<usize>,
  ) -> Option<Arc<FilePickerItem>> {
    let snapshot = self.matcher.snapshot();
    let item = snapshot.get_matched_item(matched_index as u32)?;

    indices_out.clear();
    MATCH_INDEX_SCRATCH.with(|scratch| {
      let mut scratch = scratch.borrow_mut();
      let (matcher, indices) = &mut *scratch;
      matcher.config = NucleoConfig::DEFAULT;
      matcher.config.set_match_paths();

      indices.clear();
      snapshot.pattern().column_pattern(0).indices(
        item.matcher_columns[0].slice(..),
        matcher,
        indices,
      );
      indices.sort_unstable();
      indices.dedup();

      indices_out.extend(indices.iter().map(|index| *index as usize));
    });

    Some(item.data.clone())
  }

  pub fn matched_count(&self) -> usize {
    self.matcher.snapshot().matched_item_count() as usize
  }

  pub fn total_count(&self) -> usize {
    self.matcher.snapshot().item_count() as usize
  }

  pub fn preview_loading(&self) -> bool {
    self.preview_pending_id.is_some()
  }

  pub fn preview_line_count(&self) -> usize {
    match &self.preview {
      FilePickerPreview::Empty => 0,
      FilePickerPreview::Source(source) => {
        source
          .lines
          .len()
          .saturating_add((source.truncated_above_lines > 0) as usize)
          .saturating_add((source.truncated_below_lines > 0) as usize)
      },
      FilePickerPreview::Text(text) => text.lines().count().max(1),
      FilePickerPreview::Message(message) => message.lines().count().max(1),
    }
  }

  pub fn clamp_preview_scroll(&mut self, visible_rows: usize) {
    let max_offset = self
      .preview_line_count()
      .saturating_sub(visible_rows.max(1));
    if self.preview_scroll > max_offset {
      self.preview_scroll = max_offset;
    }
  }
}

pub fn open_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open_file_picker_with_split(ctx, None);
}

pub fn open_file_picker_with_split<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  open_split: Option<SplitAxis>,
) {
  open_file_picker_with_root_and_split(ctx, picker_root(ctx), open_split);
}

pub fn open_file_picker_with_root_and_split<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  root: PathBuf,
  open_split: Option<SplitAxis>,
) {
  open_scanned_picker(ctx, "File Picker", root, open_split);
}

pub fn open_file_picker_in_current_directory<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc_dir = ctx
    .file_path()
    .and_then(|path| path.parent().map(Path::to_path_buf));
  let root = match doc_dir {
    Some(path) => path,
    None => {
      let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
      if !cwd.exists() {
        ctx.push_error(
          "file_picker",
          "current buffer has no parent and current working directory does not exist",
        );
        return;
      }
      ctx.push_warning(
        "file_picker",
        "current buffer has no parent, opening file picker in current working directory",
      );
      cwd
    },
  };
  open_file_picker_with_root_and_split(ctx, root, None);
}

pub fn open_buffer_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  let root = picker_root(ctx);
  let editor = ctx.editor_ref();
  let snapshots = editor.buffer_snapshots_mru();
  if snapshots.is_empty() {
    ctx.push_warning("buffer_picker", "no buffers available");
    return;
  }

  let initial_cursor = if snapshots.len() > 1 { 1 } else { 0 };
  let items = snapshots
    .into_iter()
    .map(|snapshot| {
      let mut flags = String::new();
      if snapshot.modified {
        flags.push('+');
      }
      if snapshot.is_active {
        flags.push('*');
      }
      let path_display = snapshot.file_path.as_ref().map_or_else(
        || snapshot.display_name.clone(),
        |path| display_relative_path(path, &cwd),
      );
      let display = if flags.is_empty() {
        path_display
      } else {
        format!("[{flags}] {path_display}")
      };
      let icon = snapshot.file_path.as_ref().map_or_else(
        || "file_generic".to_string(),
        |path| file_picker_icon_name_for_path(path).to_string(),
      );
      let absolute = snapshot
        .file_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("<buffer:{}>", snapshot.buffer_index)));
      let preview_line = editor.buffer_document(snapshot.buffer_index).map(|doc| {
        selection_focus_line(
          doc.selection(),
          editor
            .buffer_view(snapshot.buffer_index)
            .and_then(|view| view.active_cursor),
          doc.text().slice(..),
        )
      });

      FilePickerItem {
        absolute,
        display,
        icon,
        is_dir: false,
        display_path: true,
        action: FilePickerItemAction::SwitchBuffer {
          buffer_index: snapshot.buffer_index,
        },
        preview_path: snapshot.file_path,
        preview_line,
        preview_col: None,
      }
    })
    .collect();

  open_static_picker(ctx, "Buffer Picker", root, None, items, initial_cursor);
}

pub fn open_jumplist_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  let root = picker_root(ctx);
  let editor = ctx.editor_ref();
  let jumps = editor.jumplist_backward_snapshots();
  if jumps.is_empty() {
    ctx.push_warning("jumplist_picker", "jumplist is empty");
    return;
  }

  let items = jumps
    .into_iter()
    .enumerate()
    .filter_map(|(index, jump)| {
      let snapshot = editor.buffer_snapshot(jump.buffer_index)?;
      let doc = editor.buffer_document(jump.buffer_index)?;
      let text = doc.text().slice(..);
      let excerpt = jump
        .selection
        .fragments(text)
        .map(Cow::into_owned)
        .collect::<Vec<_>>()
        .join(" ");
      let excerpt = excerpt.split_whitespace().collect::<Vec<_>>().join(" ");
      let excerpt = truncate_for_picker(&excerpt, 80);
      let preview_line = selection_focus_line(&jump.selection, jump.active_cursor, text);
      let path_display = snapshot.file_path.as_ref().map_or_else(
        || snapshot.display_name.clone(),
        |path| display_relative_path(path, &cwd),
      );
      let display = if excerpt.is_empty() {
        format!("{path_display} · jump {}", index + 1)
      } else {
        format!("{path_display} · {excerpt}")
      };
      let icon = snapshot.file_path.as_ref().map_or_else(
        || "file_generic".to_string(),
        |path| file_picker_icon_name_for_path(path).to_string(),
      );
      let absolute = snapshot
        .file_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("<jump:{}>", index)));

      Some(FilePickerItem {
        absolute,
        display,
        icon,
        is_dir: false,
        display_path: false,
        action: FilePickerItemAction::RestoreJump {
          buffer_index:  jump.buffer_index,
          selection:     jump.selection.clone(),
          active_cursor: jump.active_cursor,
        },
        preview_path: snapshot.file_path,
        preview_line: Some(preview_line),
        preview_col: None,
      })
    })
    .collect::<Vec<_>>();

  if items.is_empty() {
    ctx.push_warning("jumplist_picker", "jumplist has no valid entries");
    return;
  }

  open_static_picker(ctx, "Jumplist Picker", root, None, items, 0);
}

pub fn open_diagnostics_picker<Ctx: DefaultContext>(ctx: &mut Ctx, workspace: bool) {
  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  let root = picker_root(ctx);
  let diagnostics = ctx.file_picker_diagnostics(workspace);
  if diagnostics.is_empty() {
    let source = if workspace {
      "workspace_diagnostics_picker"
    } else {
      "diagnostics_picker"
    };
    ctx.push_warning(source, "no diagnostics available");
    return;
  }

  let items = diagnostics
    .into_iter()
    .map(|diagnostic| {
      let severity_icon = diagnostic_icon_name(diagnostic.severity);
      let severity_label = diagnostic_severity_label(diagnostic.severity);
      let source_label = truncate_for_column(
        diagnostic
          .source
          .as_deref()
          .filter(|source| !source.is_empty())
          .unwrap_or("-"),
        14,
      );
      let code_label = truncate_for_column(
        diagnostic
          .code
          .as_deref()
          .filter(|code| !code.is_empty())
          .unwrap_or("-"),
        16,
      );
      let path_display = display_relative_path(&diagnostic.path, &cwd);
      let summary = truncate_for_picker(
        diagnostic.message.lines().next().unwrap_or_default().trim(),
        110,
      );
      let location = format!(
        "{}:{}:{}",
        path_display,
        diagnostic.line.saturating_add(1),
        diagnostic.character.saturating_add(1)
      );
      let display = format!(
        "{severity_label:<7} {source_label:<14} {code_label:<16} {}{}",
        if workspace {
          format!("{location}  ")
        } else {
          String::new()
        },
        summary
      );
      let absolute = diagnostic.path.clone();

      FilePickerItem {
        absolute,
        display,
        icon: severity_icon.to_string(),
        is_dir: false,
        display_path: false,
        action: FilePickerItemAction::OpenLocation {
          path:        diagnostic.path.clone(),
          cursor_char: diagnostic.cursor_char,
          line:        diagnostic.line,
          column:      None,
        },
        preview_path: Some(diagnostic.path),
        preview_line: Some(diagnostic.line),
        preview_col: None,
      }
    })
    .collect();

  open_static_picker(
    ctx,
    if workspace {
      "Workspace Diagnostics · severity source code path:line message"
    } else {
      "Diagnostics · severity source code message"
    },
    root,
    None,
    items,
    0,
  );
}

pub fn open_changed_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  if !cwd.exists() {
    ctx.push_error(
      "changed_file_picker",
      "current working directory does not exist",
    );
    return;
  }

  let changed = match ctx.file_picker_changed_files() {
    Ok(changed) => changed,
    Err(err) => {
      ctx.push_warning("changed_file_picker", err);
      return;
    },
  };
  if changed.is_empty() {
    ctx.push_warning("changed_file_picker", "no changed files");
    return;
  }

  let root = workspace_root(&cwd);
  let items = changed
    .into_iter()
    .map(|entry| {
      let (prefix, icon) = match entry.kind {
        FilePickerChangedKind::Untracked => ("+ untracked", "git_untracked"),
        FilePickerChangedKind::Modified => ("~ modified", "git_modified"),
        FilePickerChangedKind::Conflict => ("x conflict", "git_conflict"),
        FilePickerChangedKind::Deleted => ("- deleted", "git_deleted"),
        FilePickerChangedKind::Renamed => ("> renamed", "git_renamed"),
      };
      let display_path = display_relative_path(&entry.path, &cwd);
      let display = if entry.kind == FilePickerChangedKind::Renamed {
        let from_display = entry
          .from_path
          .as_ref()
          .map(|path| display_relative_path(path, &cwd))
          .unwrap_or_else(|| "<unknown>".to_string());
        format!("{prefix} {from_display} -> {display_path}")
      } else {
        format!("{prefix} {display_path}")
      };

      FilePickerItem {
        absolute: entry.path.clone(),
        display,
        icon: icon.to_string(),
        is_dir: false,
        display_path: false,
        action: FilePickerItemAction::OpenFile(entry.path.clone()),
        preview_path: Some(entry.path),
        preview_line: None,
        preview_col: None,
      }
    })
    .collect();

  open_static_picker(ctx, "Changed Files", root, None, items, 0);
}

fn open_scanned_picker<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
) {
  let mut state = base_picker_state(ctx, title, open_split);
  state.preview = FilePickerPreview::Message("Scanning files…".to_string());
  start_preview_worker(&mut state);
  start_scan(&mut state, root);
  poll_scan_results(&mut state);

  *ctx.file_picker_mut() = state;
  ctx.request_render();
}

fn open_static_picker<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  let mut state = base_picker_state(ctx, title, open_split);
  state.root = root;
  replace_picker_items(&mut state, items, initial_cursor);

  *ctx.file_picker_mut() = state;
  ctx.request_render();
}

pub fn open_custom_picker<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  open_static_picker(ctx, title, root, open_split, items, initial_cursor);
}

pub fn set_file_picker_query_external(state: &mut FilePickerState, external: bool) {
  state.query_external = external;
}

pub fn replace_file_picker_items<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  let picker = ctx.file_picker_mut();
  replace_picker_items(picker, items, initial_cursor);
  ctx.request_render();
}

fn replace_picker_items(
  state: &mut FilePickerState,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  state.preview = FilePickerPreview::Message("No matches".to_string());
  if state.preview_req_tx.is_none() || state.preview_res_rx.is_none() {
    start_preview_worker(state);
  }
  state.matcher.restart(true);
  state
    .matcher
    .pattern
    .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
  let injector = state.matcher.injector();
  for item in items {
    inject_item(&injector, item);
  }
  drop(injector);
  let _ = refresh_matcher_state(state);
  if state.matched_count() == 0 {
    state.selected = None;
    set_preview_focus_line(state, None);
    state.preview_path = None;
  } else {
    state.selected = Some(initial_cursor.min(state.matched_count() - 1));
    normalize_selection_and_scroll(state);
    refresh_preview(state);
  }
}

fn base_picker_state<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  open_split: Option<SplitAxis>,
) -> FilePickerState {
  let show_preview = ctx.file_picker().show_preview;
  let config = ctx.file_picker().config.clone();
  let wake_tx = ctx.file_picker().wake_tx.clone();
  let syntax_loader = ctx.file_picker().syntax_loader.clone();
  if let Some(cancel) = ctx.file_picker().scan_cancel.as_ref() {
    cancel.store(true, Ordering::Relaxed);
  }

  let mut state = FilePickerState::default();
  state.active = true;
  state.title = title.to_string();
  state.show_preview = show_preview;
  state.open_split = open_split;
  state.config = config;
  state.wake_tx = wake_tx.clone();
  state.syntax_loader = syntax_loader;
  state.matcher = new_matcher(wake_tx);
  state
}

pub fn close_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let picker = ctx.file_picker_mut();
  if let Some(cancel) = picker.scan_cancel.as_ref() {
    cancel.store(true, Ordering::Relaxed);
  }
  picker.active = false;
  picker.error = None;
  picker.hovered = None;
  picker.preview_scroll = 0;
  picker.open_split = None;
  picker.preview_path = None;
  picker.preview_focus_line = None;
  picker.preview = FilePickerPreview::Empty;
  picker.scanning = false;
  picker.matcher_running = false;
  picker.query_external = false;
  picker.preview_req_tx = None;
  picker.preview_res_rx = None;
  picker.preview_pending_id = None;
  picker.preview_latest_request.store(0, Ordering::Relaxed);
  picker.scan_rx = None;
  picker.scan_cancel = None;
  ctx.file_picker_closed();
  ctx.request_render();
}

fn matched_item_is_selectable(state: &FilePickerState, index: usize) -> bool {
  state
    .matched_item(index)
    .as_deref()
    .is_some_and(FilePickerItem::is_selectable)
}

fn find_selectable_index(state: &FilePickerState, start: usize, direction: isize) -> Option<usize> {
  let matched_count = state.matched_count();
  if matched_count == 0 {
    return None;
  }

  let step = if direction < 0 { -1 } else { 1 };
  let mut probe = start.min(matched_count - 1) as isize;
  for _ in 0..matched_count {
    let index = probe.rem_euclid(matched_count as isize) as usize;
    if matched_item_is_selectable(state, index) {
      return Some(index);
    }
    probe += step;
  }

  None
}

fn snap_selection_to_selectable(state: &mut FilePickerState, direction: isize) {
  let matched_count = state.matched_count();
  if matched_count == 0 {
    state.selected = None;
    return;
  }

  let start = state.selected.unwrap_or(0).min(matched_count - 1);
  state.selected = find_selectable_index(state, start, direction);
}

pub fn move_selection<Ctx: DefaultContext>(ctx: &mut Ctx, amount: isize) {
  if amount == 0 {
    return;
  }
  let picker = ctx.file_picker_mut();
  let matched_count = picker.matched_count();
  if matched_count == 0 {
    picker.selected = None;
    picker.list_offset = 0;
    return;
  }

  let direction = if amount < 0 { -1 } else { 1 };
  let steps = amount.unsigned_abs();
  let fallback = if direction < 0 { matched_count - 1 } else { 0 };
  let start = picker.selected.unwrap_or(fallback).min(matched_count - 1);
  let Some(mut selected) = find_selectable_index(picker, start, direction) else {
    picker.selected = None;
    picker.list_offset = 0;
    return;
  };

  for _ in 0..steps {
    let next_start = (selected as isize + direction).rem_euclid(matched_count as isize) as usize;
    let Some(next) = find_selectable_index(picker, next_start, direction) else {
      break;
    };
    selected = next;
  }

  picker.selected = Some(selected);
  normalize_selection_and_scroll(picker);
  refresh_preview(picker);
}

pub fn move_page<Ctx: DefaultContext>(ctx: &mut Ctx, down: bool) {
  let amount = if down {
    PAGE_SIZE as isize
  } else {
    -(PAGE_SIZE as isize)
  };
  move_selection(ctx, amount);
}

pub fn handle_file_picker_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if !ctx.file_picker().active {
    return false;
  }
  let scan_changed = {
    let picker = ctx.file_picker_mut();
    poll_scan_results(picker)
  };
  if scan_changed {
    ctx.request_render();
  }

  match key.key {
    Key::Escape => {
      close_file_picker(ctx);
      true
    },
    Key::Enter | Key::NumpadEnter => {
      submit_file_picker(ctx);
      true
    },
    Key::Up => {
      move_selection(ctx, -1);
      ctx.request_render();
      true
    },
    Key::Down => {
      move_selection(ctx, 1);
      ctx.request_render();
      true
    },
    Key::PageUp => {
      move_page(ctx, false);
      ctx.request_render();
      true
    },
    Key::PageDown => {
      move_page(ctx, true);
      ctx.request_render();
      true
    },
    Key::Home => {
      let picker = ctx.file_picker_mut();
      picker.selected = if picker.matched_count() == 0 {
        None
      } else {
        Some(0)
      };
      snap_selection_to_selectable(picker, 1);
      normalize_selection_and_scroll(picker);
      refresh_preview(picker);
      ctx.request_render();
      true
    },
    Key::End => {
      let picker = ctx.file_picker_mut();
      picker.selected = picker.matched_count().checked_sub(1);
      snap_selection_to_selectable(picker, -1);
      normalize_selection_and_scroll(picker);
      refresh_preview(picker);
      ctx.request_render();
      true
    },
    Key::Tab => {
      if key.modifiers.shift() {
        move_selection(ctx, -1);
      } else {
        move_selection(ctx, 1);
      }
      ctx.request_render();
      true
    },
    Key::Backspace => {
      let mut changed_query = None;
      {
        let picker = ctx.file_picker_mut();
        if picker.cursor > 0 && picker.cursor <= picker.query.len() {
          let old_query = picker.query.clone();
          let prev = prev_char_boundary(&picker.query, picker.cursor);
          picker.query.replace_range(prev..picker.cursor, "");
          picker.cursor = prev;
          if picker.query_external {
            picker.selected = Some(0);
            picker.list_offset = 0;
            picker.error = None;
            picker.preview = FilePickerPreview::Message("Searching…".to_string());
            set_preview_focus_line(picker, None);
            picker.preview_path = None;
            picker.preview_pending_id = None;
            picker.preview_latest_request.store(0, Ordering::Relaxed);
            changed_query = Some(picker.query.clone());
          } else {
            handle_query_change(picker, &old_query);
            refresh_preview(picker);
          }
        }
      }
      if let Some(query) = changed_query {
        ctx.file_picker_query_changed(&query);
      }
      ctx.request_render();
      true
    },
    Key::Delete => {
      let mut changed_query = None;
      {
        let picker = ctx.file_picker_mut();
        if picker.cursor < picker.query.len() {
          let old_query = picker.query.clone();
          let next = next_char_boundary(&picker.query, picker.cursor);
          picker.query.replace_range(picker.cursor..next, "");
          if picker.query_external {
            picker.selected = Some(0);
            picker.list_offset = 0;
            picker.error = None;
            picker.preview = FilePickerPreview::Message("Searching…".to_string());
            set_preview_focus_line(picker, None);
            picker.preview_path = None;
            picker.preview_pending_id = None;
            picker.preview_latest_request.store(0, Ordering::Relaxed);
            changed_query = Some(picker.query.clone());
          } else {
            handle_query_change(picker, &old_query);
            refresh_preview(picker);
          }
        }
      }
      if let Some(query) = changed_query {
        ctx.file_picker_query_changed(&query);
      }
      ctx.request_render();
      true
    },
    Key::Left => {
      let picker = ctx.file_picker_mut();
      picker.cursor = prev_char_boundary(&picker.query, picker.cursor);
      ctx.request_render();
      true
    },
    Key::Right => {
      let picker = ctx.file_picker_mut();
      picker.cursor = next_char_boundary(&picker.query, picker.cursor);
      ctx.request_render();
      true
    },
    Key::Char('t') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      let picker = ctx.file_picker_mut();
      picker.show_preview = !picker.show_preview;
      ctx.request_render();
      true
    },
    Key::Char('d') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      move_page(ctx, true);
      ctx.request_render();
      true
    },
    Key::Char('u') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      move_page(ctx, false);
      ctx.request_render();
      true
    },
    Key::Char('n') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      move_selection(ctx, 1);
      ctx.request_render();
      true
    },
    Key::Char('p') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      move_selection(ctx, -1);
      ctx.request_render();
      true
    },
    Key::Char('c') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      close_file_picker(ctx);
      true
    },
    Key::Char('s') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      submit_file_picker(ctx);
      true
    },
    Key::Char('v') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      submit_file_picker(ctx);
      true
    },
    Key::Char(ch) => {
      if key.modifiers.ctrl() || key.modifiers.alt() {
        return true;
      }
      let mut changed_query = None;
      {
        let picker = ctx.file_picker_mut();
        let old_query = picker.query.clone();
        picker.query.insert(picker.cursor, ch);
        picker.cursor += ch.len_utf8();
        if picker.query_external {
          picker.selected = Some(0);
          picker.list_offset = 0;
          picker.error = None;
          picker.preview = FilePickerPreview::Message("Searching…".to_string());
          set_preview_focus_line(picker, None);
          picker.preview_path = None;
          picker.preview_pending_id = None;
          picker.preview_latest_request.store(0, Ordering::Relaxed);
          changed_query = Some(picker.query.clone());
        } else {
          handle_query_change(picker, &old_query);
          refresh_preview(picker);
        }
      }
      if let Some(query) = changed_query {
        ctx.file_picker_query_changed(&query);
      }
      ctx.request_render();
      true
    },
    _ => true,
  }
}

pub fn submit_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let selected = ctx.file_picker().current_item();
  let Some(item) = selected else {
    return;
  };

  match &item.action {
    FilePickerItemAction::OpenFile(path) => {
      if !prepare_split_for_submit(ctx) {
        return;
      }
      if let Err(err) = ctx.open_file(path) {
        let message = err.to_string();
        {
          let picker = ctx.file_picker_mut();
          picker.error = Some(message.clone());
          picker.preview = FilePickerPreview::Message(format!("Failed to open file: {message}"));
        }
        ctx.push_error("file_picker", format!("Failed to open file: {message}"));
        ctx.request_render();
        return;
      }
      close_file_picker(ctx);
    },
    FilePickerItemAction::GroupHeader { .. } => {},
    FilePickerItemAction::SwitchBuffer { buffer_index } => {
      if !ctx.editor().set_active_buffer(*buffer_index) {
        ctx.push_warning("buffer_picker", "selected buffer is no longer available");
        return;
      }
      close_file_picker(ctx);
    },
    FilePickerItemAction::RestoreJump {
      buffer_index,
      selection,
      active_cursor,
    } => {
      if !ctx.editor().set_active_buffer(*buffer_index) {
        ctx.push_warning(
          "jumplist_picker",
          "selected jump target is no longer available",
        );
        return;
      }
      if ctx
        .editor()
        .document_mut()
        .set_selection(selection.clone())
        .is_err()
      {
        ctx.push_warning("jumplist_picker", "failed to restore jump selection");
        return;
      }
      ctx.editor().view_mut().active_cursor = *active_cursor;
      close_file_picker(ctx);
    },
    FilePickerItemAction::OpenLocation {
      path,
      cursor_char,
      line,
      column,
    } => {
      if !prepare_split_for_submit(ctx) {
        return;
      }
      if ctx
        .file_path()
        .is_none_or(|current| current != path.as_path())
        && let Err(err) = ctx.open_file(path)
      {
        let message = err.to_string();
        {
          let picker = ctx.file_picker_mut();
          picker.error = Some(message.clone());
          picker.preview = FilePickerPreview::Message(format!("Failed to open file: {message}"));
        }
        ctx.push_error(
          "diagnostics_picker",
          format!("Failed to open file: {message}"),
        );
        ctx.request_render();
        return;
      }

      let (cursor, cursor_line) = {
        let text = ctx.editor_ref().document().text();
        let mut cursor = (*cursor_char).min(text.len_chars());
        if let Some(column) = *column {
          if *line < text.len_lines() {
            let line_start = text.line_to_char(*line);
            let line_end = text.line_to_char((*line + 1).min(text.len_lines()));
            let line_len = line_end.saturating_sub(line_start);
            cursor = line_start.saturating_add(column.min(line_len));
          }
        } else if *cursor_char == 0 && *line > 0 && *line < text.len_lines() {
          cursor = text.line_to_char(*line);
        }
        let cursor_line = text.char_to_line(cursor.min(text.len_chars()));
        (cursor, cursor_line)
      };
      let _ = ctx
        .editor()
        .document_mut()
        .set_selection(Selection::point(cursor));
      let view = ctx.editor_ref().view();
      if let Some(new_row) = the_lib::view::scroll_row_to_keep_visible(
        cursor_line,
        view.scroll.row,
        view.viewport.height.max(1) as usize,
        ctx.scrolloff(),
      ) {
        let mut next = view.scroll;
        next.row = new_row;
        ctx.editor().view_mut().scroll = next;
      }
      close_file_picker(ctx);
    },
  }
}

fn prepare_split_for_submit<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  if let Some(axis) = ctx.file_picker().open_split
    && !ctx.editor().split_active_pane(axis)
  {
    ctx.push_error("file_picker", "Failed to open split");
    ctx.request_render();
    return false;
  }
  true
}

pub fn select_file_picker_index<Ctx: DefaultContext>(ctx: &mut Ctx, index: usize) {
  let picker = ctx.file_picker_mut();
  let matched_count = picker.matched_count();
  if matched_count == 0 {
    picker.selected = None;
    picker.hovered = None;
    picker.list_offset = 0;
    picker.preview_scroll = 0;
    picker.preview = FilePickerPreview::Message("No matches".to_string());
    ctx.request_render();
    return;
  }

  picker.selected = Some(index.min(matched_count - 1));
  snap_selection_to_selectable(picker, 1);
  normalize_selection_and_scroll(picker);
  refresh_preview(picker);
  ctx.request_render();
}

pub fn open_file_picker_index<Ctx: DefaultContext>(ctx: &mut Ctx, index: usize) {
  select_file_picker_index(ctx, index);
  submit_file_picker(ctx);
}

pub fn set_file_picker_list_offset<Ctx: DefaultContext>(ctx: &mut Ctx, offset: usize) {
  let picker = ctx.file_picker_mut();
  let matched_count = picker.matched_count();
  if matched_count == 0 {
    picker.selected = None;
    picker.hovered = None;
    picker.list_offset = 0;
    picker.preview_scroll = 0;
    picker.preview = FilePickerPreview::Message("No matches".to_string());
    ctx.request_render();
    return;
  }

  let visible = picker.list_visible.max(1);
  let max_offset = matched_count.saturating_sub(visible);
  let next_offset = offset.min(max_offset);
  if picker.list_offset == next_offset {
    return;
  }

  picker.list_offset = next_offset;
  ctx.request_render();
}

pub fn scroll_file_picker_list<Ctx: DefaultContext>(ctx: &mut Ctx, delta: isize) {
  let picker = ctx.file_picker();
  let current = picker.list_offset;
  let target = if delta < 0 {
    current.saturating_sub(delta.unsigned_abs())
  } else {
    current.saturating_add(delta as usize)
  };
  set_file_picker_list_offset(ctx, target);
}

pub fn set_file_picker_preview_offset<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  offset: usize,
  visible_rows: usize,
) {
  let picker = ctx.file_picker_mut();
  let max_offset = picker
    .preview_line_count()
    .saturating_sub(visible_rows.max(1));
  let next = offset.min(max_offset);
  if picker.preview_scroll == next {
    return;
  }
  picker.preview_scroll = next;
  ctx.request_render();
}

pub fn scroll_file_picker_preview<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  delta: isize,
  visible_rows: usize,
) {
  let picker = ctx.file_picker();
  let current = picker.preview_scroll;
  let target = if delta < 0 {
    current.saturating_sub(delta.unsigned_abs())
  } else {
    current.saturating_add(delta as usize)
  };
  set_file_picker_preview_offset(ctx, target, visible_rows);
}

pub fn build_file_picker_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Vec<UiNode> {
  let scan_changed = {
    let picker = ctx.file_picker_mut();
    poll_scan_results(picker)
  };
  if scan_changed {
    ctx.request_render();
  }

  let picker = ctx.file_picker();
  if !picker.active {
    return Vec::new();
  }
  let picker = ctx.file_picker();
  let is_running = picker.scanning || picker.matcher_running;

  let mut status = format!(
    "{}{}/{}",
    if is_running { "(running) " } else { "" },
    picker.matched_count(),
    picker.total_count()
  );
  if let Some(err) = picker.error.as_ref().filter(|err| !err.is_empty()) {
    status = format!("{status}  {err}");
  }

  let mut status_text = UiNode::text("file_picker_status", status);
  if let UiNode::Text(text) = &mut status_text {
    text.style = text.style.clone().with_role("file_picker");
  }

  let mut input = UiInput::new("file_picker_input", picker.query.clone());
  input.cursor = picker.query[..picker.cursor.min(picker.query.len())]
    .chars()
    .count();
  input.style = input.style.with_role("file_picker");
  input.style.accent = Some(UiColor::Token(UiColorToken::Placeholder));

  let total_matches = picker.matched_count();
  let visible_rows = picker.list_visible.max(1);
  let window_start = picker
    .list_offset
    .min(total_matches.saturating_sub(visible_rows));
  let window_end = window_start.saturating_add(visible_rows).min(total_matches);
  let mut match_indices = Vec::new();
  let mut list_items = Vec::with_capacity(window_end.saturating_sub(window_start));
  for idx in window_start..window_end {
    let Some(item) = picker.matched_item_with_match_indices(idx, &mut match_indices) else {
      continue;
    };
    let mut row = UiListItem::new(item.display.clone());
    row.emphasis = item.is_dir;
    row.leading_icon = Some(item.icon.clone());
    row.match_indices = Some(match_indices.clone());
    list_items.push(row);
  }

  let mut list = UiList::new("file_picker_list", list_items);
  list.selected = picker.selected;
  list.scroll = window_start;
  list.virtual_total = Some(total_matches);
  list.virtual_start = window_start;
  list.max_visible = Some(picker.list_visible);
  list.style = list.style.with_role("file_picker");
  list.style.accent = Some(UiColor::Token(UiColorToken::SelectedBg));
  list.style.border = Some(UiColor::Token(UiColorToken::SelectedText));
  let list = UiNode::List(list);

  let preview_content = match &picker.preview {
    FilePickerPreview::Empty => String::new(),
    FilePickerPreview::Source(source) => source_preview_text(source),
    FilePickerPreview::Text(text) => text.clone(),
    FilePickerPreview::Message(message) => message.clone(),
  };
  let mut preview = UiNode::text("file_picker_preview", preview_content);
  if let UiNode::Text(text) = &mut preview {
    text.style = UiStyle::default().with_role("file_picker");
    text.clip = true;
  }

  let prompt_row = UiNode::container(
    "file_picker_prompt_row",
    UiLayout::Split {
      axis:   the_lib::render::UiAxis::Horizontal,
      ratios: vec![5, 2],
    },
    vec![UiNode::Input(input), status_text],
  );

  let body = if picker.show_preview {
    UiNode::container(
      "file_picker_body",
      UiLayout::Split {
        axis:   the_lib::render::UiAxis::Horizontal,
        ratios: vec![1, 1],
      },
      vec![list, preview],
    )
  } else {
    UiNode::container(
      "file_picker_body",
      UiLayout::Stack {
        axis: the_lib::render::UiAxis::Vertical,
        gap:  0,
      },
      vec![list],
    )
  };

  let mut container = UiContainer::column("file_picker_container", 0, vec![
    prompt_row,
    UiNode::Divider(UiDivider { id: None }),
    body,
  ]);
  container.style = container.style.with_role("file_picker");
  let container = UiNode::Container(container);

  let mut panel = UiPanel::floating("file_picker", container);
  panel.title = Some(format!("{} · {}", picker.title, picker.root.display()));
  panel.style = panel.style.with_role("file_picker");
  panel.style.border = None;
  panel.constraints = UiConstraints::floating_default();
  panel.constraints.min_width = Some(72);
  panel.constraints.min_height = Some(18);
  panel.constraints.max_width = None;
  panel.constraints.max_height = None;

  vec![UiNode::Panel(panel)]
}

fn display_relative_path(path: &Path, cwd: &Path) -> String {
  path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

fn selection_focus_line(
  selection: &Selection,
  active_cursor: Option<CursorId>,
  text: ropey::RopeSlice<'_>,
) -> usize {
  if let Some(cursor_id) = active_cursor
    && let Some(range) = selection.range_by_id(cursor_id)
  {
    return range.cursor_line(text);
  }

  selection
    .ranges()
    .first()
    .map(|range| range.cursor_line(text))
    .unwrap_or(0)
}

fn truncate_for_picker(text: &str, max_chars: usize) -> String {
  let mut out = String::new();
  for (idx, ch) in text.chars().enumerate() {
    if idx >= max_chars {
      out.push('…');
      return out;
    }
    out.push(ch);
  }
  out
}

fn truncate_for_column(text: &str, max_chars: usize) -> String {
  if max_chars == 0 {
    return String::new();
  }
  let mut out = String::new();
  for (idx, ch) in text.chars().enumerate() {
    if idx >= max_chars {
      if max_chars > 1 {
        let _ = out.pop();
        out.push('…');
      }
      return out;
    }
    out.push(ch);
  }
  out
}

fn diagnostic_icon_name(severity: Option<DiagnosticSeverity>) -> &'static str {
  match severity {
    Some(DiagnosticSeverity::Error) => "diagnostic_error",
    Some(DiagnosticSeverity::Warning) => "diagnostic_warning",
    Some(DiagnosticSeverity::Information) => "diagnostic_info",
    Some(DiagnosticSeverity::Hint) => "diagnostic_hint",
    None => "diagnostic_info",
  }
}

fn diagnostic_severity_label(severity: Option<DiagnosticSeverity>) -> &'static str {
  match severity {
    Some(DiagnosticSeverity::Error) => "ERROR",
    Some(DiagnosticSeverity::Warning) => "WARN",
    Some(DiagnosticSeverity::Information) => "INFO",
    Some(DiagnosticSeverity::Hint) => "HINT",
    None => "DIAG",
  }
}

fn diagnostic_severity_from_icon(icon: &str) -> Option<DiagnosticSeverity> {
  match icon {
    "diagnostic_error" => Some(DiagnosticSeverity::Error),
    "diagnostic_warning" => Some(DiagnosticSeverity::Warning),
    "diagnostic_info" => Some(DiagnosticSeverity::Information),
    "diagnostic_hint" => Some(DiagnosticSeverity::Hint),
    _ => None,
  }
}

pub fn file_picker_kind_from_title(title: &str) -> FilePickerKind {
  if title.starts_with("Diagnostics ·") || title.starts_with("Workspace Diagnostics ·") {
    return FilePickerKind::Diagnostics;
  }
  if title.starts_with("Lsp Symbols")
    || title.starts_with("Document Symbols")
    || title.starts_with("Workspace Symbols")
  {
    return FilePickerKind::Symbols;
  }
  if title.starts_with("Live Grep") || title.starts_with("Global Search") {
    return FilePickerKind::LiveGrep;
  }
  FilePickerKind::Generic
}

fn split_prefix_chars(text: &str, max_chars: usize) -> (&str, &str) {
  if max_chars == 0 || text.is_empty() {
    return ("", text);
  }
  let mut seen = 0usize;
  for (idx, _) in text.char_indices() {
    if seen == max_chars {
      return (&text[..idx], &text[idx..]);
    }
    seen = seen.saturating_add(1);
  }
  (text, "")
}

fn parse_diagnostics_row(display: &str, icon: &str) -> FilePickerRowData {
  let (severity_text, rest) = split_prefix_chars(display, 7);
  let rest = rest.strip_prefix(' ').unwrap_or(rest);
  let (source, rest) = split_prefix_chars(rest, 14);
  let rest = rest.strip_prefix(' ').unwrap_or(rest);
  let (code, rest) = split_prefix_chars(rest, 16);
  let rest = rest.strip_prefix(' ').unwrap_or(rest).trim_start();
  let severity = diagnostic_severity_from_icon(icon);

  let (location, message) = if let Some((location, message)) = rest.split_once("  ") {
    (location.trim().to_string(), message.trim().to_string())
  } else {
    (String::new(), rest.to_string())
  };

  let severity_label = if severity.is_some() {
    severity
      .map(|severity| diagnostic_severity_label(Some(severity)))
      .unwrap_or("DIAG")
      .to_string()
  } else {
    severity_text.trim().to_string()
  };

  FilePickerRowData {
    kind: FilePickerRowKind::Diagnostics,
    severity,
    primary: if message.is_empty() {
      severity_label
    } else {
      message
    },
    secondary: source.trim().to_string(),
    tertiary: code.trim().to_string(),
    quaternary: if location == "-" {
      String::new()
    } else {
      location
    },
    line: 0,
    column: 0,
    depth: 0,
  }
}

fn parse_symbols_row(display: &str) -> FilePickerRowData {
  let mut fields = display.split('\t');
  let mut name = fields.next().unwrap_or_default().trim().to_string();
  let container = fields.next().unwrap_or_default().trim().to_string();
  let detail = fields.next().unwrap_or_default().trim().to_string();
  let kind = fields.next().unwrap_or_default().trim().to_string();
  let _path = fields.next().unwrap_or_default().trim();
  let line = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let column = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let depth = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(0);

  if name.is_empty() {
    name = "<unnamed>".to_string();
  }

  FilePickerRowData {
    kind: FilePickerRowKind::Symbols,
    severity: None,
    primary: name,
    secondary: container,
    tertiary: detail,
    quaternary: kind,
    line,
    column,
    depth,
  }
}

fn parse_live_grep_row(item: &FilePickerItem) -> FilePickerRowData {
  if matches!(&item.action, FilePickerItemAction::GroupHeader { .. }) {
    return FilePickerRowData {
      kind:       FilePickerRowKind::LiveGrepHeader,
      severity:   None,
      primary:    item.display.trim().to_string(),
      secondary:  String::new(),
      tertiary:   String::new(),
      quaternary: String::new(),
      line:       0,
      column:     0,
      depth:      0,
    };
  }

  let mut fields = item.display.splitn(4, '\t');
  let path = fields.next().unwrap_or_default().trim().to_string();
  let line = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let column = fields
    .next()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(1);
  let snippet = fields.next().unwrap_or_default().to_string();
  let snippet = if snippet.is_empty() {
    item.display.trim().to_string()
  } else {
    snippet
  };

  FilePickerRowData {
    kind: FilePickerRowKind::LiveGrepMatch,
    severity: None,
    primary: snippet,
    secondary: path,
    tertiary: String::new(),
    quaternary: String::new(),
    line,
    column,
    depth: 0,
  }
}

pub fn file_picker_row_data(title: &str, item: &FilePickerItem) -> FilePickerRowData {
  match file_picker_kind_from_title(title) {
    FilePickerKind::Diagnostics => parse_diagnostics_row(item.display.as_str(), item.icon.as_str()),
    FilePickerKind::Symbols => parse_symbols_row(item.display.as_str()),
    FilePickerKind::LiveGrep => parse_live_grep_row(item),
    FilePickerKind::Generic => {
      FilePickerRowData {
        kind:       FilePickerRowKind::Generic,
        severity:   None,
        primary:    item.display.clone(),
        secondary:  String::new(),
        tertiary:   String::new(),
        quaternary: String::new(),
        line:       0,
        column:     0,
        depth:      0,
      }
    },
  }
}

fn picker_root<Ctx: DefaultContext>(_ctx: &Ctx) -> PathBuf {
  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  workspace_root(&cwd)
}

pub fn workspace_root(start: &Path) -> PathBuf {
  let mut current = start.to_path_buf();
  loop {
    if has_workspace_marker(&current) {
      return current;
    }
    let Some(parent) = current.parent() else {
      return start.to_path_buf();
    };
    if parent == current {
      return start.to_path_buf();
    }
    current = parent.to_path_buf();
  }
}

pub fn has_workspace_marker(path: &Path) -> bool {
  [".git", ".jj", ".hg", ".pijul", ".svn"]
    .iter()
    .any(|marker| path.join(marker).exists())
}

fn start_scan(state: &mut FilePickerState, root: PathBuf) {
  if let Some(cancel) = state.scan_cancel.as_ref() {
    cancel.store(true, Ordering::Relaxed);
  }

  state.root = root.clone();
  state.query.clear();
  state.cursor = 0;
  state.selected = None;
  state.list_offset = 0;
  state.preview_path = None;
  state.error = None;
  state.scanning = true;
  state.matcher_running = false;
  state.preview = FilePickerPreview::Message("Scanning files…".to_string());

  state.matcher.restart(true);
  state
    .matcher
    .pattern
    .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);

  state.scan_generation = state.scan_generation.wrapping_add(1);
  let generation = state.scan_generation;

  let cancel = Arc::new(AtomicBool::new(false));
  state.scan_rx = None;
  state.scan_cancel = Some(cancel.clone());
  let injector = state.matcher.injector();

  let mut walker = build_file_walker(&root, &state.config);
  let timeout = Instant::now() + Duration::from_millis(WARMUP_SCAN_BUDGET_MS);
  let mut scanned = 0usize;
  let mut hit_timeout = false;

  for entry in &mut walker {
    if cancel.load(Ordering::Relaxed) {
      break;
    }

    let entry = match entry {
      Ok(entry) => entry,
      Err(_) => continue,
    };
    let Some(item) = entry_to_picker_item(entry, &root) else {
      continue;
    };
    inject_item(&injector, item);
    scanned += 1;

    if scanned >= MAX_SCAN_ITEMS {
      break;
    }
    if Instant::now() >= timeout {
      hit_timeout = true;
      break;
    }
  }

  if scanned >= MAX_SCAN_ITEMS || !hit_timeout {
    state.scanning = false;
    state.scan_cancel = None;
    return;
  }

  let (scan_tx, scan_rx) = mpsc::channel();
  state.scan_rx = Some(scan_rx);

  spawn_scan_thread(
    walker,
    root,
    MAX_SCAN_ITEMS.saturating_sub(scanned),
    generation,
    scan_tx,
    cancel,
    injector,
    state.wake_tx.clone(),
  );
}

fn build_file_walker(root: &Path, config: &FilePickerConfig) -> ignore::Walk {
  let absolute_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
  let deduplicate_links = config.deduplicate_links;
  let mut walk_builder = ignore::WalkBuilder::new(root);
  walk_builder
    .hidden(config.hidden)
    .parents(config.parents)
    .ignore(config.ignore)
    .follow_links(config.follow_symlinks)
    .git_ignore(config.git_ignore)
    .git_global(config.git_global)
    .git_exclude(config.git_exclude)
    .sort_by_file_name(|name1, name2| name1.cmp(name2))
    .max_depth(config.max_depth)
    .filter_entry(move |entry| filter_picker_entry(entry, &absolute_root, deduplicate_links))
    .add_custom_ignore_filename(the_loader::config_dir().join("ignore"))
    .add_custom_ignore_filename(".helix/ignore")
    .types(excluded_types())
    .build()
}

fn entry_to_picker_item(entry: DirEntry, root: &Path) -> Option<FilePickerItem> {
  if !entry
    .file_type()
    .is_some_and(|file_type| file_type.is_file())
  {
    return None;
  }

  let path = entry.into_path();
  let rel = path.strip_prefix(root).ok()?;
  let mut display = rel.to_string_lossy().to_string();
  if std::path::MAIN_SEPARATOR != '/' {
    display = display.replace(std::path::MAIN_SEPARATOR, "/");
  }
  let icon = file_picker_icon_name_for_path(rel).to_string();

  Some(FilePickerItem {
    action: FilePickerItemAction::OpenFile(path.clone()),
    absolute: path,
    display,
    icon,
    is_dir: false,
    display_path: true,
    preview_path: None,
    preview_line: None,
    preview_col: None,
  })
}

pub fn file_picker_icon_name_for_path(path: &Path) -> &'static str {
  let file_name = path
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or_default();
  let extension = path
    .extension()
    .and_then(|extension| extension.to_str())
    .unwrap_or_default();

  if matches_ignore_ascii_case(file_name, &["cargo.toml", "cargo.lock"]) {
    return "rust";
  }
  if matches_ignore_ascii_case(file_name, &["justfile", "makefile"]) {
    return "tool_hammer";
  }
  if file_name.eq_ignore_ascii_case("dockerfile") {
    return "docker";
  }
  if matches_ignore_ascii_case(file_name, &["license", "copying"]) {
    return "file_lock";
  }
  if file_name.eq_ignore_ascii_case("readme")
    || file_name.to_ascii_lowercase().starts_with("readme.")
  {
    return "book";
  }
  if file_name.starts_with('.') {
    return "settings";
  }

  if extension.eq_ignore_ascii_case("rs") {
    return "file_rust";
  }
  if extension.eq_ignore_ascii_case("toml") {
    return "file_toml";
  }
  if extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown") {
    return "file_markdown";
  }
  if extension.eq_ignore_ascii_case("json") {
    return "json";
  }
  if extension.eq_ignore_ascii_case("yaml") || extension.eq_ignore_ascii_case("yml") {
    return "file_code";
  }
  if extension.eq_ignore_ascii_case("nix") {
    return "nix";
  }
  if extension.eq_ignore_ascii_case("swift") {
    return "swift";
  }
  if extension.eq_ignore_ascii_case("py") {
    return "python";
  }
  if extension.eq_ignore_ascii_case("js")
    || extension.eq_ignore_ascii_case("mjs")
    || extension.eq_ignore_ascii_case("cjs")
    || extension.eq_ignore_ascii_case("jsx")
  {
    return "javascript";
  }
  if extension.eq_ignore_ascii_case("ts") || extension.eq_ignore_ascii_case("tsx") {
    return "typescript";
  }
  if extension.eq_ignore_ascii_case("go") {
    return "go";
  }
  if extension.eq_ignore_ascii_case("java") {
    return "java";
  }
  if extension.eq_ignore_ascii_case("kt") || extension.eq_ignore_ascii_case("kts") {
    return "kotlin";
  }
  if extension.eq_ignore_ascii_case("c") || extension.eq_ignore_ascii_case("h") {
    return "c";
  }
  if extension.eq_ignore_ascii_case("cc")
    || extension.eq_ignore_ascii_case("cpp")
    || extension.eq_ignore_ascii_case("cxx")
    || extension.eq_ignore_ascii_case("hpp")
    || extension.eq_ignore_ascii_case("hh")
    || extension.eq_ignore_ascii_case("hxx")
  {
    return "cpp";
  }
  if extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm") {
    return "html";
  }
  if extension.eq_ignore_ascii_case("css") {
    return "css";
  }
  if extension.eq_ignore_ascii_case("scss") || extension.eq_ignore_ascii_case("sass") {
    return "sass";
  }
  if extension.eq_ignore_ascii_case("sh")
    || extension.eq_ignore_ascii_case("bash")
    || extension.eq_ignore_ascii_case("zsh")
    || extension.eq_ignore_ascii_case("fish")
    || extension.eq_ignore_ascii_case("nu")
  {
    return "terminal";
  }
  if extension.eq_ignore_ascii_case("png")
    || extension.eq_ignore_ascii_case("jpg")
    || extension.eq_ignore_ascii_case("jpeg")
    || extension.eq_ignore_ascii_case("gif")
    || extension.eq_ignore_ascii_case("bmp")
    || extension.eq_ignore_ascii_case("webp")
    || extension.eq_ignore_ascii_case("svg")
    || extension.eq_ignore_ascii_case("ico")
  {
    return "image";
  }
  if extension.eq_ignore_ascii_case("pdf") {
    return "file_doc";
  }
  if extension.eq_ignore_ascii_case("zip")
    || extension.eq_ignore_ascii_case("gz")
    || extension.eq_ignore_ascii_case("xz")
    || extension.eq_ignore_ascii_case("zst")
    || extension.eq_ignore_ascii_case("tar")
    || extension.eq_ignore_ascii_case("rar")
    || extension.eq_ignore_ascii_case("7z")
  {
    return "archive";
  }
  if extension.eq_ignore_ascii_case("sql")
    || extension.eq_ignore_ascii_case("sqlite")
    || extension.eq_ignore_ascii_case("db")
  {
    return "database";
  }
  if extension.eq_ignore_ascii_case("lock") {
    return "lock";
  }
  if extension.eq_ignore_ascii_case("git")
    || extension.eq_ignore_ascii_case("patch")
    || extension.eq_ignore_ascii_case("diff")
  {
    return "file_git";
  }

  "file_generic"
}

pub fn file_picker_icon_glyph(icon: &str, is_dir: bool) -> &'static str {
  if is_dir {
    return "";
  }

  match icon {
    "folder" | "folder_open" | "folder_search" => "",
    "archive" => "",
    "book" => "󰂺",
    "c" => "",
    "cpp" => "",
    "css" => "",
    "database" => "",
    "diagnostic_error" => "",
    "diagnostic_warning" => "",
    "diagnostic_info" => "",
    "diagnostic_hint" => "󰌵",
    "git_untracked" => "",
    "git_modified" => "",
    "git_conflict" => "",
    "git_deleted" => "",
    "git_renamed" => "",
    "docker" => "",
    "file_doc" => "󰈦",
    "file_git" => "",
    "git_branch" => "",
    "file_lock" | "lock" => "󰌾",
    "file_markdown" => "",
    "file_rust" | "rust" => "",
    "file_toml" | "toml" => "",
    "go" => "",
    "html" => "",
    "image" => "󰈟",
    "java" => "",
    "javascript" => "",
    "json" => "",
    "kotlin" => "",
    "nix" => "",
    "python" => "",
    "sass" => "",
    "settings" => "",
    "swift" => "",
    "terminal" => "",
    "tool_hammer" => "󰛶",
    "typescript" => "",
    _ => "󰈔",
  }
}

fn matches_ignore_ascii_case(candidate: &str, variants: &[&str]) -> bool {
  variants
    .iter()
    .any(|variant| candidate.eq_ignore_ascii_case(variant))
}

fn spawn_scan_thread(
  mut walker: ignore::Walk,
  root: PathBuf,
  remaining_items: usize,
  generation: u64,
  scan_tx: Sender<(u64, ScanMessage)>,
  cancel: Arc<AtomicBool>,
  injector: nucleo::Injector<Arc<FilePickerItem>>,
  wake_tx: Option<Sender<()>>,
) {
  std::thread::spawn(move || {
    if !root.exists() {
      let _ = scan_tx.send((
        generation,
        ScanMessage::Error("workspace directory does not exist".to_string()),
      ));
      notify_wake(&wake_tx);
      return;
    }

    if let Err(err) = fs::read_dir(&root) {
      let _ = scan_tx.send((generation, ScanMessage::Error(err.to_string())));
      notify_wake(&wake_tx);
      return;
    }

    let mut total = 0usize;
    for entry in &mut walker {
      if cancel.load(Ordering::Relaxed) {
        break;
      }

      let entry = match entry {
        Ok(entry) => entry,
        Err(_) => continue,
      };
      let Some(item) = entry_to_picker_item(entry, &root) else {
        continue;
      };
      inject_item(&injector, item);
      total += 1;
      if total >= remaining_items {
        break;
      }
    }

    let _ = scan_tx.send((generation, ScanMessage::Done));
    notify_wake(&wake_tx);
  });
}

fn notify_wake(wake_tx: &Option<Sender<()>>) {
  if let Some(wake_tx) = wake_tx.as_ref() {
    let _ = wake_tx.send(());
  }
}

fn inject_item(injector: &nucleo::Injector<Arc<FilePickerItem>>, item: FilePickerItem) {
  let text = item.display.clone();
  let item = Arc::new(item);
  injector.push(item, move |_item, dst| {
    dst[0] = text.into();
  });
}

fn filter_picker_entry(entry: &DirEntry, root: &Path, dedup_symlinks: bool) -> bool {
  if matches!(
    entry.file_name().to_str(),
    Some(".git" | ".pijul" | ".jj" | ".hg" | ".svn")
  ) {
    return false;
  }

  if dedup_symlinks && entry.path_is_symlink() {
    return entry
      .path()
      .canonicalize()
      .ok()
      .is_some_and(|path| !path.starts_with(root));
  }

  true
}

fn excluded_types() -> ignore::types::Types {
  use ignore::types::TypesBuilder;

  let mut type_builder = TypesBuilder::new();
  type_builder
    .add(
      "compressed",
      "*.{zip,gz,bz2,zst,lzo,sz,tgz,tbz2,lz,lz4,lzma,lzo,z,Z,xz,7z,rar,cab}",
    )
    .expect("invalid compressed type definition");
  type_builder.negate("all");
  type_builder
    .build()
    .expect("failed to build excluded types")
}

pub fn poll_scan_results(state: &mut FilePickerState) -> bool {
  let mut changed = false;
  for _ in 0..64 {
    let scan_result = match state.scan_rx.as_ref() {
      Some(scan_rx) => scan_rx.try_recv(),
      None => break,
    };
    match scan_result {
      Ok((generation, message)) => {
        if generation != state.scan_generation {
          continue;
        }
        match message {
          ScanMessage::Done => {
            state.scanning = false;
            state.scan_rx = None;
            state.scan_cancel = None;
            changed = true;
            break;
          },
          ScanMessage::Error(err) => {
            state.scanning = false;
            state.scan_rx = None;
            state.scan_cancel = None;
            state.error = Some(err.clone());
            state.preview = FilePickerPreview::Message(format!("Failed to read workspace: {err}"));
            changed = true;
            break;
          },
        }
      },
      Err(TryRecvError::Empty) => break,
      Err(TryRecvError::Disconnected) => {
        state.scanning = false;
        state.scan_rx = None;
        state.scan_cancel = None;
        if state.total_count() == 0 {
          state.error = Some("Scan interrupted".to_string());
          state.preview = FilePickerPreview::Message("Scan interrupted".to_string());
        }
        changed = true;
        break;
      },
    }
  }

  if poll_preview_results(state) {
    changed = true;
  }

  if refresh_matcher_state(state) {
    refresh_preview(state);
    changed = true;
  }

  changed
}

fn start_preview_worker(state: &mut FilePickerState) {
  let (request_tx, request_rx) = mpsc::channel::<PreviewRequest>();
  let (result_tx, result_rx) = mpsc::channel::<PreviewResult>();
  let wake_tx = state.wake_tx.clone();
  let syntax_loader = state.syntax_loader.clone();
  let latest_request = state.preview_latest_request.clone();

  let send_preview = |result_tx: &Sender<PreviewResult>,
                      wake_tx: &Option<Sender<()>>,
                      request: &PreviewRequest,
                      preview: FilePickerPreview,
                      is_final: bool| {
    let sent = result_tx.send(PreviewResult {
      request_id: request.request_id,
      path: request.path.clone(),
      preview,
      is_final,
    });
    if sent.is_ok() {
      notify_wake(wake_tx);
    }
    sent
  };

  std::thread::spawn(move || {
    while let Ok(mut request) = request_rx.recv() {
      while let Ok(next) = request_rx.try_recv() {
        request = next;
      }

      if request.request_id != latest_request.load(Ordering::Relaxed) {
        continue;
      }

      match preview_for_path_base(&request.path, request.is_dir, request.focus_line) {
        PreviewBuild::Final(preview) => {
          if request.request_id != latest_request.load(Ordering::Relaxed) {
            continue;
          }
          if send_preview(&result_tx, &wake_tx, &request, preview, true).is_err() {
            break;
          }
        },
        PreviewBuild::Source(source_preview) => {
          if request.request_id != latest_request.load(Ordering::Relaxed) {
            continue;
          }
          let can_highlight = syntax_loader.is_some();
          if send_preview(
            &result_tx,
            &wake_tx,
            &request,
            FilePickerPreview::Source(source_preview.source.clone()),
            !can_highlight,
          )
          .is_err()
          {
            break;
          }

          if !can_highlight || request.request_id != latest_request.load(Ordering::Relaxed) {
            continue;
          }

          let Some(loader) = syntax_loader.as_deref() else {
            continue;
          };
          let highlights = collect_source_highlights(
            &request.path,
            &source_preview.text,
            source_preview.end_byte,
            loader,
          );
          // Always send the highlighted result even if stale — poll_preview_results
          // will cache it by path, so navigating back shows highlighted content.

          let mut preview = source_preview.source;
          if !highlights.is_empty() {
            preview.highlights = highlights.into();
          }
          if send_preview(
            &result_tx,
            &wake_tx,
            &request,
            FilePickerPreview::Source(preview),
            true,
          )
          .is_err()
          {
            break;
          }
        },
      }
    }
  });

  state.preview_req_tx = Some(request_tx);
  state.preview_res_rx = Some(result_rx);
}

fn poll_preview_results(state: &mut FilePickerState) -> bool {
  let mut changed = false;
  for _ in 0..32 {
    let result = match state.preview_res_rx.as_ref() {
      Some(result_rx) => result_rx.try_recv(),
      None => break,
    };
    match result {
      Ok(result) => {
        state
          .preview_cache
          .insert(result.path.clone(), result.preview.clone());
        if state.preview_pending_id == Some(result.request_id)
          && state
            .preview_path
            .as_ref()
            .is_some_and(|path| path == &result.path)
        {
          state.preview = result.preview;
          if result.is_final {
            state.preview_pending_id = None;
          }
          changed = true;
        }
      },
      Err(TryRecvError::Empty) => break,
      Err(TryRecvError::Disconnected) => {
        state.preview_req_tx = None;
        state.preview_res_rx = None;
        state.preview_pending_id = None;
        break;
      },
    }
  }
  changed
}

pub fn refresh_matcher_state(state: &mut FilePickerState) -> bool {
  let status = state.matcher.tick(MATCHER_TICK_TIMEOUT_MS);
  state.matcher_running = status.running || state.matcher.active_injectors() > 0;

  let mut changed = status.changed;

  if clamp_selection_and_offsets(state) {
    changed = true;
  }

  changed
}

pub fn handle_query_change(state: &mut FilePickerState, old_query: &str) {
  if state.query == old_query {
    return;
  }

  state.selected = Some(0);
  state.list_offset = 0;
  let is_append = state.query.starts_with(old_query);
  state.matcher.pattern.reparse(
    0,
    &state.query,
    CaseMatching::Smart,
    Normalization::Smart,
    is_append,
  );
  let _ = refresh_matcher_state(state);
}

pub fn set_picker_visible_rows(state: &mut FilePickerState, visible_rows: usize) {
  state.list_visible = visible_rows.max(1);
  let _ = clamp_selection_and_offsets(state);
}

fn normalize_selection_and_scroll(state: &mut FilePickerState) {
  let matched_count = state.matched_count();
  if matched_count == 0 {
    state.selected = None;
    state.list_offset = 0;
    return;
  }

  let visible = state.list_visible.max(1);
  state.selected = Some(state.selected.unwrap_or(0).min(matched_count - 1));
  snap_selection_to_selectable(state, 1);
  let Some(selected) = state.selected else {
    state.list_offset = 0;
    return;
  };

  if selected < state.list_offset {
    state.list_offset = selected;
  } else if selected >= state.list_offset.saturating_add(visible) {
    state.list_offset = selected.saturating_add(1).saturating_sub(visible);
  }

  let max_offset = matched_count.saturating_sub(visible);
  if state.list_offset > max_offset {
    state.list_offset = max_offset;
  }
}

fn clamp_selection_and_offsets(state: &mut FilePickerState) -> bool {
  let mut changed = false;
  let matched_count = state.matched_count();
  if matched_count == 0 {
    if state.selected.is_some() {
      state.selected = None;
      changed = true;
    }
    if state.hovered.is_some() {
      state.hovered = None;
      changed = true;
    }
    if state.list_offset != 0 {
      state.list_offset = 0;
      changed = true;
    }
    return changed;
  }

  let last = matched_count - 1;
  let selection_start = state.selected.unwrap_or(0).min(last);
  let next_selected = find_selectable_index(state, selection_start, 1);
  if state.selected != next_selected {
    state.selected = next_selected;
    changed = true;
  }

  let next_hovered = state
    .hovered
    .map(|hovered| hovered.min(last))
    .and_then(|hovered| matched_item_is_selectable(state, hovered).then_some(hovered));
  if state.hovered != next_hovered {
    state.hovered = next_hovered;
    changed = true;
  }

  let max_offset = matched_count.saturating_sub(state.list_visible.max(1));
  if state.list_offset > max_offset {
    state.list_offset = max_offset;
    changed = true;
  }

  changed
}

fn new_matcher(wake_tx: Option<Sender<()>>) -> Nucleo<Arc<FilePickerItem>> {
  let mut config = NucleoConfig::DEFAULT;
  config.set_match_paths();
  let notify = Arc::new(move || {
    if let Some(wake_tx) = wake_tx.as_ref() {
      let _ = wake_tx.send(());
    }
  });
  Nucleo::new(config, notify, None, 1)
}

fn refresh_preview(state: &mut FilePickerState) {
  let item = state.current_item();
  let Some(item) = item else {
    set_preview_focus_line(state, None);
    state.preview_path = None;
    state.preview_pending_id = None;
    state.preview_latest_request.store(0, Ordering::Relaxed);
    if state.scanning || state.matcher_running {
      state.preview = FilePickerPreview::Message("Scanning files…".to_string());
    } else {
      state.preview = FilePickerPreview::Message("No matches".to_string());
    }
    return;
  };

  let preview_target = item.preview_path.as_ref().unwrap_or(&item.absolute);
  let preview_focus_line = item.preview_line;
  if state
    .preview_path
    .as_ref()
    .is_some_and(|path| path == preview_target)
  {
    if state.preview_focus_line == preview_focus_line {
      return;
    }
    if preview_contains_focus_line(&state.preview, preview_focus_line) {
      set_preview_focus_line(state, preview_focus_line);
      return;
    }
  }

  state.preview_path = Some(preview_target.clone());
  if let Some(preview) = state.preview_cache.get(preview_target) {
    if !preview_contains_focus_line(&preview, preview_focus_line) {
      // Cache entry for this path is not focused near the selected line.
      // Rebuild so preview recenters around the current hit.
    } else {
      state.preview = preview;
      set_preview_focus_line(state, preview_focus_line);
      state.preview_pending_id = None;
      state.preview_latest_request.store(0, Ordering::Relaxed);
      return;
    }
  }

  match preview_for_path_base(preview_target, item.is_dir, preview_focus_line) {
    PreviewBuild::Final(preview) => {
      state
        .preview_cache
        .insert(preview_target.clone(), preview.clone());
      state.preview = preview;
      set_preview_focus_line(state, preview_focus_line);
      state.preview_pending_id = None;
      state.preview_latest_request.store(0, Ordering::Relaxed);
    },
    PreviewBuild::Source(mut source_preview) => {
      let base_preview = FilePickerPreview::Source(source_preview.source.clone());
      state
        .preview_cache
        .insert(preview_target.clone(), base_preview.clone());
      state.preview = base_preview;
      set_preview_focus_line(state, preview_focus_line);

      if state.syntax_loader.is_none() {
        state.preview_pending_id = None;
        state.preview_latest_request.store(0, Ordering::Relaxed);
        return;
      }

      state.preview_request_id = state.preview_request_id.wrapping_add(1);
      let request_id = state.preview_request_id;
      state.preview_pending_id = Some(request_id);
      state
        .preview_latest_request
        .store(request_id, Ordering::Relaxed);

      let request = PreviewRequest {
        request_id,
        path: preview_target.clone(),
        is_dir: item.is_dir,
        focus_line: preview_focus_line,
      };

      let sent = state
        .preview_req_tx
        .as_ref()
        .is_some_and(|request_tx| request_tx.send(request).is_ok());
      if sent {
        return;
      }

      let Some(loader) = state.syntax_loader.as_deref() else {
        state.preview_pending_id = None;
        state.preview_latest_request.store(0, Ordering::Relaxed);
        return;
      };

      let highlights = collect_source_highlights(
        preview_target,
        &source_preview.text,
        source_preview.end_byte,
        loader,
      );
      if !highlights.is_empty() {
        source_preview.source.highlights = highlights.into();
      }

      let preview = FilePickerPreview::Source(source_preview.source);
      state
        .preview_cache
        .insert(preview_target.clone(), preview.clone());
      state.preview = preview;
      set_preview_focus_line(state, preview_focus_line);
      state.preview_pending_id = None;
      state.preview_latest_request.store(0, Ordering::Relaxed);
    },
  }
}

fn set_preview_focus_line(state: &mut FilePickerState, line: Option<usize>) {
  state.preview_focus_line = line;
  let next_scroll = preview_focus_row(&state.preview, line)
    .map(|row| row.saturating_sub(PREVIEW_FOCUS_CONTEXT_LINES))
    .unwrap_or_else(|| {
      line
        .map(|line| line.saturating_sub(PREVIEW_FOCUS_CONTEXT_LINES))
        .unwrap_or(0)
    });
  state.preview_scroll = next_scroll.min(state.preview_line_count().saturating_sub(1));
}

fn preview_focus_row(preview: &FilePickerPreview, focus_line: Option<usize>) -> Option<usize> {
  let focus_line = focus_line?;
  match preview {
    FilePickerPreview::Source(source) => {
      let start = source.base_line;
      let end = source.base_line.saturating_add(source.lines.len());
      if !(start..end).contains(&focus_line) {
        return None;
      }
      let top_marker = (source.truncated_above_lines > 0) as usize;
      Some(top_marker.saturating_add(focus_line.saturating_sub(start)))
    },
    FilePickerPreview::Text(_) | FilePickerPreview::Message(_) => Some(focus_line),
    FilePickerPreview::Empty => None,
  }
}

fn preview_contains_focus_line(preview: &FilePickerPreview, focus_line: Option<usize>) -> bool {
  let Some(focus_line) = focus_line else {
    return true;
  };
  match preview {
    FilePickerPreview::Source(source) => {
      let start = source.base_line;
      let end = source.base_line.saturating_add(source.lines.len());
      (start..end).contains(&focus_line)
    },
    _ => true,
  }
}

fn preview_highlight_at(highlights: &[(Highlight, Range<usize>)], byte_idx: usize) -> Option<u32> {
  let mut active = None;
  for (highlight, range) in highlights {
    if byte_idx < range.start {
      break;
    }
    if byte_idx < range.end {
      active = Some(highlight.get());
    }
  }
  active
}

fn clamp_preview_char_boundary(text: &str, idx: usize, round_up: bool) -> usize {
  let mut idx = idx.min(text.len());
  if text.is_char_boundary(idx) {
    return idx;
  }
  if round_up {
    while idx < text.len() && !text.is_char_boundary(idx) {
      idx += 1;
    }
    return idx.min(text.len());
  }
  while idx > 0 && !text.is_char_boundary(idx) {
    idx -= 1;
  }
  idx
}

fn preview_char_to_byte_idx(text: &str, char_idx: usize) -> usize {
  if char_idx == 0 {
    return 0;
  }
  text
    .char_indices()
    .nth(char_idx)
    .map(|(idx, _)| idx)
    .unwrap_or(text.len())
}

fn preview_line_segments(
  line: &str,
  line_start: usize,
  highlights: &[(Highlight, Range<usize>)],
) -> Vec<FilePickerPreviewSegment> {
  if line.is_empty() {
    return vec![FilePickerPreviewSegment {
      text:         String::new(),
      highlight_id: None,
      is_match:     false,
    }];
  }

  if highlights.is_empty() {
    return vec![FilePickerPreviewSegment {
      text:         line.to_string(),
      highlight_id: None,
      is_match:     false,
    }];
  }

  let line_end = line_start.saturating_add(line.len());
  let mut boundaries = vec![line_start, line_end];
  for (_highlight, range) in highlights {
    if range.end <= line_start || range.start >= line_end {
      continue;
    }
    boundaries.push(range.start.max(line_start));
    boundaries.push(range.end.min(line_end));
  }
  boundaries.sort_unstable();
  boundaries.dedup();

  let mut segments = Vec::new();
  for pair in boundaries.windows(2) {
    let absolute_start = pair[0];
    let absolute_end = pair[1];
    if absolute_end <= absolute_start {
      continue;
    }
    let local_start =
      clamp_preview_char_boundary(line, absolute_start.saturating_sub(line_start), false);
    let local_end =
      clamp_preview_char_boundary(line, absolute_end.saturating_sub(line_start), true);
    if local_end <= local_start {
      continue;
    }
    let sample_byte = absolute_start + (absolute_end.saturating_sub(absolute_start) / 2);
    let highlight_id = preview_highlight_at(highlights, sample_byte);
    segments.push(FilePickerPreviewSegment {
      text: line[local_start..local_end].to_string(),
      highlight_id,
      is_match: false,
    });
  }

  if segments.is_empty() {
    segments.push(FilePickerPreviewSegment {
      text:         line.to_string(),
      highlight_id: None,
      is_match:     false,
    });
  }

  segments
}

fn apply_match_to_preview_segments(
  segments: Vec<FilePickerPreviewSegment>,
  match_col: Option<(usize, usize)>,
) -> Vec<FilePickerPreviewSegment> {
  let Some((match_start, match_end)) = match_col else {
    return segments;
  };
  if match_end <= match_start {
    return segments;
  }

  let mut out = Vec::new();
  let mut segment_start_char = 0usize;
  for segment in segments {
    let segment_char_len = segment.text.chars().count();
    if segment_char_len == 0 {
      out.push(segment);
      continue;
    }
    let segment_end_char = segment_start_char.saturating_add(segment_char_len);
    let overlap_start = match_start.max(segment_start_char);
    let overlap_end = match_end.min(segment_end_char);
    if overlap_start >= overlap_end {
      out.push(segment);
      segment_start_char = segment_end_char;
      continue;
    }

    let local_overlap_start = overlap_start.saturating_sub(segment_start_char);
    let local_overlap_end = overlap_end.saturating_sub(segment_start_char);

    if local_overlap_start > 0 {
      let prefix_end = preview_char_to_byte_idx(&segment.text, local_overlap_start);
      out.push(FilePickerPreviewSegment {
        text:         segment.text[..prefix_end].to_string(),
        highlight_id: segment.highlight_id,
        is_match:     false,
      });
    }

    let overlap_start_byte = preview_char_to_byte_idx(&segment.text, local_overlap_start);
    let overlap_end_byte = preview_char_to_byte_idx(&segment.text, local_overlap_end);
    out.push(FilePickerPreviewSegment {
      text:         segment.text[overlap_start_byte..overlap_end_byte].to_string(),
      highlight_id: segment.highlight_id,
      is_match:     true,
    });

    if local_overlap_end < segment_char_len {
      out.push(FilePickerPreviewSegment {
        text:         segment.text[overlap_end_byte..].to_string(),
        highlight_id: segment.highlight_id,
        is_match:     false,
      });
    }

    segment_start_char = segment_end_char;
  }

  out
}

pub fn file_picker_preview_window(
  state: &FilePickerState,
  offset: usize,
  visible_rows: usize,
  overscan: usize,
) -> FilePickerPreviewWindow {
  let visible_rows = visible_rows.max(1);
  let overscan = overscan.max(1);
  let focus_line = state.preview_focus_line;
  let focus_col = state.current_item().and_then(|item| item.preview_col);

  match &state.preview {
    FilePickerPreview::Empty => {
      FilePickerPreviewWindow {
        kind:               0,
        total_virtual_rows: 0,
        offset:             0,
        window_start:       0,
        lines:              Vec::new(),
      }
    },
    FilePickerPreview::Source(source) => {
      let has_top_marker = source.truncated_above_lines > 0;
      let has_bottom_marker = source.truncated_below_lines > 0;
      let total_virtual_rows = source
        .lines
        .len()
        .saturating_add(has_top_marker as usize)
        .saturating_add(has_bottom_marker as usize);
      let max_offset = total_virtual_rows.saturating_sub(visible_rows);
      let offset = offset.min(max_offset);
      let window_start = offset.saturating_sub(overscan);
      let window_end = offset
        .saturating_add(visible_rows)
        .saturating_add(overscan)
        .min(total_virtual_rows);

      let mut lines = Vec::with_capacity(window_end.saturating_sub(window_start));
      for virtual_row in window_start..window_end {
        if has_top_marker && virtual_row == 0 {
          lines.push(FilePickerPreviewWindowLine {
            virtual_row,
            kind: FilePickerPreviewLineKind::TruncatedAbove,
            line_number: None,
            focused: false,
            marker: format!("… {} lines above", source.truncated_above_lines),
            segments: Vec::new(),
          });
          continue;
        }

        let local_idx = virtual_row.saturating_sub(has_top_marker as usize);
        if local_idx >= source.lines.len() {
          if has_bottom_marker && local_idx == source.lines.len() {
            lines.push(FilePickerPreviewWindowLine {
              virtual_row,
              kind: FilePickerPreviewLineKind::TruncatedBelow,
              line_number: None,
              focused: false,
              marker: format!("… {} lines below", source.truncated_below_lines),
              segments: Vec::new(),
            });
          }
          continue;
        }

        let absolute_line_idx = source.base_line.saturating_add(local_idx);
        let focused = focus_line.is_some_and(|focus| focus == absolute_line_idx);
        let line = source.lines[local_idx].as_str();
        let line_start = source.line_starts.get(local_idx).copied().unwrap_or(0);
        let mut segments = preview_line_segments(line, line_start, &source.highlights);
        if focused {
          segments = apply_match_to_preview_segments(segments, focus_col);
        }
        lines.push(FilePickerPreviewWindowLine {
          virtual_row,
          kind: FilePickerPreviewLineKind::Content,
          line_number: Some(absolute_line_idx.saturating_add(1)),
          focused,
          marker: String::new(),
          segments,
        });
      }

      FilePickerPreviewWindow {
        kind: 1,
        total_virtual_rows,
        offset,
        window_start,
        lines,
      }
    },
    FilePickerPreview::Text(text) => {
      let mut plain_lines: Vec<&str> = text.lines().collect();
      if plain_lines.is_empty() {
        plain_lines.push("");
      }
      let total_virtual_rows = plain_lines.len();
      let max_offset = total_virtual_rows.saturating_sub(visible_rows);
      let offset = offset.min(max_offset);
      let window_start = offset.saturating_sub(overscan);
      let window_end = offset
        .saturating_add(visible_rows)
        .saturating_add(overscan)
        .min(total_virtual_rows);
      let mut lines = Vec::with_capacity(window_end.saturating_sub(window_start));
      for virtual_row in window_start..window_end {
        let focused = focus_line.is_some_and(|focus| focus == virtual_row);
        let mut segments = vec![FilePickerPreviewSegment {
          text:         plain_lines[virtual_row].to_string(),
          highlight_id: None,
          is_match:     false,
        }];
        if focused {
          segments = apply_match_to_preview_segments(segments, focus_col);
        }
        lines.push(FilePickerPreviewWindowLine {
          virtual_row,
          kind: FilePickerPreviewLineKind::Content,
          line_number: None,
          focused,
          marker: String::new(),
          segments,
        });
      }

      FilePickerPreviewWindow {
        kind: 2,
        total_virtual_rows,
        offset,
        window_start,
        lines,
      }
    },
    FilePickerPreview::Message(text) => {
      let mut plain_lines: Vec<&str> = text.lines().collect();
      if plain_lines.is_empty() {
        plain_lines.push("");
      }
      let total_virtual_rows = plain_lines.len();
      let max_offset = total_virtual_rows.saturating_sub(visible_rows);
      let offset = offset.min(max_offset);
      let window_start = offset.saturating_sub(overscan);
      let window_end = offset
        .saturating_add(visible_rows)
        .saturating_add(overscan)
        .min(total_virtual_rows);
      let mut lines = Vec::with_capacity(window_end.saturating_sub(window_start));
      for virtual_row in window_start..window_end {
        let focused = focus_line.is_some_and(|focus| focus == virtual_row);
        let mut segments = vec![FilePickerPreviewSegment {
          text:         plain_lines[virtual_row].to_string(),
          highlight_id: None,
          is_match:     false,
        }];
        if focused {
          segments = apply_match_to_preview_segments(segments, focus_col);
        }
        lines.push(FilePickerPreviewWindowLine {
          virtual_row,
          kind: FilePickerPreviewLineKind::Content,
          line_number: None,
          focused,
          marker: String::new(),
          segments,
        });
      }

      FilePickerPreviewWindow {
        kind: 3,
        total_virtual_rows,
        offset,
        window_start,
        lines,
      }
    },
  }
}

fn preview_for_path_base(path: &Path, is_dir: bool, focus_line: Option<usize>) -> PreviewBuild {
  if is_dir {
    return PreviewBuild::Final(directory_preview(path));
  }

  let metadata = match fs::metadata(path) {
    Ok(metadata) => metadata,
    Err(_) => {
      return PreviewBuild::Final(FilePickerPreview::Message("<File not found>".to_string()));
    },
  };

  if metadata.len() > MAX_FILE_SIZE_FOR_PREVIEW {
    return PreviewBuild::Final(FilePickerPreview::Message(
      "<File too large to preview>".to_string(),
    ));
  }

  let file = match fs::File::open(path) {
    Ok(file) => file,
    Err(_) => {
      return PreviewBuild::Final(FilePickerPreview::Message(
        "<Could not read file>".to_string(),
      ));
    },
  };
  let mut bytes = Vec::new();
  if file
    .take((MAX_PREVIEW_BYTES + 1) as u64)
    .read_to_end(&mut bytes)
    .is_err()
  {
    return PreviewBuild::Final(FilePickerPreview::Message(
      "<Could not read file>".to_string(),
    ));
  }

  if bytes.contains(&0) {
    return PreviewBuild::Final(FilePickerPreview::Message("<Binary file>".to_string()));
  }

  let truncated = bytes.len() > MAX_PREVIEW_BYTES;
  if truncated {
    bytes.truncate(MAX_PREVIEW_BYTES);
  }

  let text = String::from_utf8_lossy(&bytes).into_owned();
  let Some((source, end_byte, preview_text)) =
    source_preview(path, &metadata, &text, truncated, focus_line)
  else {
    return PreviewBuild::Final(FilePickerPreview::Message("<Empty file>".to_string()));
  };

  PreviewBuild::Source(SourcePreviewData {
    source,
    text: preview_text,
    end_byte,
  })
}

fn directory_preview(path: &Path) -> FilePickerPreview {
  let read_dir = match fs::read_dir(path) {
    Ok(read_dir) => read_dir,
    Err(_) => {
      return FilePickerPreview::Message("<Cannot open directory>".to_string());
    },
  };

  let mut names = Vec::new();
  for entry in read_dir.take(MAX_PREVIEW_DIRECTORY_ENTRIES) {
    let Ok(entry) = entry else {
      continue;
    };
    let file_type = entry.file_type().ok();
    let is_dir = file_type.is_some_and(|ty| ty.is_dir());
    let mut name = entry.file_name().to_string_lossy().to_string();
    if is_dir {
      name.push('/');
    }
    names.push(name);
  }
  names.sort();

  if names.is_empty() {
    return FilePickerPreview::Message("<Empty directory>".to_string());
  }

  FilePickerPreview::Text(names.join("\n"))
}

fn preview_line_index_cache() -> &'static Mutex<HashMap<PathBuf, CachedLineIndex>> {
  PREVIEW_LINE_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn metadata_modified_secs(metadata: &fs::Metadata) -> u64 {
  metadata
    .modified()
    .ok()
    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|duration| duration.as_secs())
    .unwrap_or(0)
}

fn build_line_start_index(text: &str) -> Arc<[usize]> {
  let mut line_starts = Vec::new();
  line_starts.push(0);
  for (idx, byte) in text.bytes().enumerate() {
    if byte == b'\n' && idx + 1 < text.len() {
      line_starts.push(idx + 1);
    }
  }
  line_starts.into()
}

fn line_start_index_for_path(path: &Path, metadata: &fs::Metadata, text: &str) -> Arc<[usize]> {
  let file_len = metadata.len();
  let modified_secs = metadata_modified_secs(metadata);

  if let Ok(cache) = preview_line_index_cache().lock()
    && let Some(cached) = cache.get(path)
    && cached.file_len == file_len
    && cached.modified_secs == modified_secs
  {
    return Arc::clone(&cached.line_starts);
  }

  let line_starts = build_line_start_index(text);
  if let Ok(mut cache) = preview_line_index_cache().lock() {
    cache.insert(path.to_path_buf(), CachedLineIndex {
      file_len,
      modified_secs,
      line_starts: Arc::clone(&line_starts),
    });
  }
  line_starts
}

fn source_preview(
  path: &Path,
  metadata: &fs::Metadata,
  text: &str,
  truncated_by_bytes: bool,
  focus_line: Option<usize>,
) -> Option<(FilePickerSourcePreview, usize, String)> {
  if text.is_empty() {
    return None;
  }

  let all_line_starts = line_start_index_for_path(path, metadata, text);
  let total_lines = all_line_starts.len();
  if total_lines == 0 {
    return None;
  }

  let window_size = PREVIEW_FOCUS_WINDOW_LINES
    .max(1)
    .min(MAX_PREVIEW_SOURCE_LINES.max(1));
  let focus_line = focus_line
    .map(|line| line.min(total_lines.saturating_sub(1)))
    .unwrap_or(0);
  let mut window_start = focus_line.saturating_sub(window_size / 2);
  if window_start.saturating_add(window_size) > total_lines {
    window_start = total_lines.saturating_sub(window_size);
  }
  let window_end = window_start.saturating_add(window_size).min(total_lines);

  let start_byte = all_line_starts[window_start];
  let end_byte = if window_end < total_lines {
    all_line_starts[window_end]
  } else {
    text.len()
  };
  let window_text = &text[start_byte..end_byte];

  let mut lines = Vec::new();
  let mut line_starts = Vec::new();
  let mut consumed_bytes = 0usize;
  for segment in window_text.split_inclusive('\n') {
    let line = segment
      .strip_suffix('\n')
      .map(|line| line.strip_suffix('\r').unwrap_or(line))
      .unwrap_or(segment);
    line_starts.push(consumed_bytes);
    lines.push(line.to_string());
    consumed_bytes = consumed_bytes.saturating_add(segment.len());
  }
  if !window_text.is_empty() && !window_text.ends_with('\n') && lines.is_empty() {
    line_starts.push(0);
    lines.push(window_text.to_string());
    consumed_bytes = window_text.len();
  }
  if lines.is_empty() {
    return None;
  }

  let mut truncated_above_lines = window_start;
  let mut truncated_below_lines = total_lines.saturating_sub(window_end);
  if truncated_by_bytes {
    truncated_below_lines = truncated_below_lines.saturating_add(1);
  }
  if truncated_by_bytes && window_start > 0 {
    truncated_above_lines = truncated_above_lines.saturating_add(1);
  }

  Some((
    FilePickerSourcePreview {
      lines: lines.into(),
      line_starts: line_starts.into(),
      highlights: Vec::<(Highlight, Range<usize>)>::new().into(),
      base_line: window_start,
      truncated_above_lines,
      truncated_below_lines,
    },
    consumed_bytes,
    window_text.to_string(),
  ))
}

fn collect_source_highlights(
  path: &Path,
  text: &str,
  end_byte: usize,
  loader: &Loader,
) -> Vec<(Highlight, Range<usize>)> {
  if end_byte == 0 {
    return Vec::new();
  }

  let Some(language) = loader.language_for_filename(path) else {
    return Vec::new();
  };

  let rope = Rope::from_str(text);
  let end_byte = end_byte.min(rope.len_bytes());
  if end_byte == 0 {
    return Vec::new();
  }

  let syntax = match Syntax::new(rope.slice(..), language, loader) {
    Ok(syntax) => syntax,
    Err(_) => return Vec::new(),
  };

  let mut highlights = syntax.collect_highlights(rope.slice(..), loader, 0..end_byte);
  highlights.retain(|(_highlight, range)| range.start < range.end && range.end <= end_byte);
  highlights.sort_by_key(|(_highlight, range)| (range.start, Reverse(range.end)));
  highlights
}

fn source_preview_text(source: &FilePickerSourcePreview) -> String {
  let total_lines = source
    .base_line
    .saturating_add(source.lines.len())
    .saturating_add(source.truncated_below_lines)
    .max(1);
  let width = total_lines.to_string().len();
  let mut output = String::new();
  if source.truncated_above_lines > 0 {
    let _ = std::fmt::Write::write_fmt(
      &mut output,
      format_args!("… {} lines above\n", source.truncated_above_lines),
    );
  }
  for (line_idx, line) in source.lines.iter().enumerate() {
    let absolute_line = source.base_line.saturating_add(line_idx).saturating_add(1);
    let _ = std::fmt::Write::write_fmt(
      &mut output,
      format_args!("{:>width$} {}\n", absolute_line, line, width = width),
    );
  }
  if source.truncated_below_lines > 0 {
    let _ = std::fmt::Write::write_fmt(
      &mut output,
      format_args!("… {} lines below", source.truncated_below_lines),
    );
  }
  output
}

#[cfg(test)]
mod tests {
  use std::{
    path::PathBuf,
    time::{
      SystemTime,
      UNIX_EPOCH,
    },
  };

  use super::*;

  fn sample_item(display: &str) -> FilePickerItem {
    let absolute = PathBuf::from(display);
    FilePickerItem {
      absolute:     absolute.clone(),
      display:      display.to_string(),
      icon:         "file_generic".to_string(),
      is_dir:       false,
      display_path: true,
      action:       FilePickerItemAction::OpenFile(absolute),
      preview_path: None,
      preview_line: None,
      preview_col:  None,
    }
  }

  fn sample_group_header(display: &str) -> FilePickerItem {
    let absolute = PathBuf::from(display);
    FilePickerItem {
      absolute:     absolute.clone(),
      display:      display.to_string(),
      icon:         "file_generic".to_string(),
      is_dir:       false,
      display_path: false,
      action:       FilePickerItemAction::GroupHeader { path: absolute },
      preview_path: None,
      preview_line: None,
      preview_col:  None,
    }
  }

  #[test]
  fn matched_item_with_match_indices_resets_output_and_returns_sorted_indices() {
    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(&injector, sample_item("src/main.rs"));
    inject_item(&injector, sample_item("src/lib.rs"));

    state
      .matcher
      .pattern
      .reparse(0, "mr", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);

    let mut indices = vec![999];
    let item = state
      .matched_item_with_match_indices(0, &mut indices)
      .expect("first match should exist");

    assert_eq!(item.display, "src/main.rs");
    assert!(!indices.is_empty());
    assert!(indices.iter().all(|&index| index != 999));
    assert!(indices.windows(2).all(|window| window[0] <= window[1]));
  }

  #[test]
  fn matched_item_with_match_indices_empty_query_has_no_indices() {
    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(&injector, sample_item("docs/commands.md"));
    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);

    let mut indices = Vec::new();
    let item = state
      .matched_item_with_match_indices(0, &mut indices)
      .expect("empty query should still match");

    assert_eq!(item.display, "docs/commands.md");
    assert!(indices.is_empty());
  }

  #[test]
  fn normalize_selection_skips_non_selectable_group_headers() {
    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(&injector, sample_group_header("src/main.rs"));
    inject_item(&injector, sample_item("src/main.rs\t10\t1\tfn main()"));
    inject_item(&injector, sample_group_header("src/lib.rs"));
    inject_item(&injector, sample_item("src/lib.rs\t4\t1\tpub fn lib()"));
    drop(injector);

    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);

    state.selected = Some(0);
    normalize_selection_and_scroll(&mut state);
    assert_eq!(state.selected, Some(1));

    state.selected = Some(2);
    normalize_selection_and_scroll(&mut state);
    assert_eq!(state.selected, Some(3));
  }

  #[test]
  fn clamp_selection_is_stable_when_only_group_headers_exist() {
    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(&injector, sample_group_header("src/main.rs"));
    drop(injector);

    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);

    state.selected = Some(0);
    let first_changed = clamp_selection_and_offsets(&mut state);
    let second_changed = clamp_selection_and_offsets(&mut state);
    assert!(first_changed);
    assert!(!second_changed);
    assert_eq!(state.selected, None);
  }

  #[test]
  fn refresh_preview_updates_scroll_for_new_focus_line_in_same_file() {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock should be after epoch")
      .as_nanos();
    path.push(format!("the-editor-file-picker-preview-{stamp}.txt"));

    let mut text = String::new();
    for idx in 1..=80 {
      text.push_str(&format!("line {idx}\n"));
    }
    std::fs::write(&path, text).expect("should create preview file");

    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(&injector, FilePickerItem {
      absolute:     path.clone(),
      display:      "low".to_string(),
      icon:         "file_generic".to_string(),
      is_dir:       false,
      display_path: false,
      action:       FilePickerItemAction::OpenLocation {
        path:        path.clone(),
        cursor_char: 0,
        line:        2,
        column:      Some(0),
      },
      preview_path: Some(path.clone()),
      preview_line: Some(2),
      preview_col:  Some((0, 4)),
    });
    inject_item(&injector, FilePickerItem {
      absolute:     path.clone(),
      display:      "high".to_string(),
      icon:         "file_generic".to_string(),
      is_dir:       false,
      display_path: false,
      action:       FilePickerItemAction::OpenLocation {
        path:        path.clone(),
        cursor_char: 0,
        line:        40,
        column:      Some(0),
      },
      preview_path: Some(path.clone()),
      preview_line: Some(40),
      preview_col:  Some((0, 4)),
    });
    drop(injector);

    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);

    let low_index = (0..state.matched_count())
      .find(|&idx| {
        state
          .matched_item(idx)
          .and_then(|item| item.preview_line)
          .is_some_and(|line| line == 2)
      })
      .expect("low line item should be present");
    let high_index = (0..state.matched_count())
      .find(|&idx| {
        state
          .matched_item(idx)
          .and_then(|item| item.preview_line)
          .is_some_and(|line| line == 40)
      })
      .expect("high line item should be present");

    state.selected = Some(low_index);
    normalize_selection_and_scroll(&mut state);
    refresh_preview(&mut state);
    assert_eq!(state.preview_scroll, 0);

    state.selected = Some(high_index);
    normalize_selection_and_scroll(&mut state);
    refresh_preview(&mut state);
    assert_eq!(
      state.preview_scroll,
      40usize.saturating_sub(PREVIEW_FOCUS_CONTEXT_LINES)
    );

    let _ = std::fs::remove_file(path);
  }
}
