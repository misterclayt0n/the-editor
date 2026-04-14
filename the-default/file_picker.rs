use std::{
  any::Any,
  borrow::Cow,
  cell::RefCell,
  cmp::Reverse,
  collections::{
    HashMap,
    VecDeque,
    hash_map::DefaultHasher,
  },
  fs,
  hash::{
    Hash,
    Hasher,
  },
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
  editor::BufferId,
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
  command_registry::{
    CommandEvent,
    TypableCommand,
  },
  fff_backend,
};

const MAX_SCAN_ITEMS: usize = 100_000;
const MAX_FILE_SIZE_FOR_PREVIEW: u64 = 10 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = MAX_FILE_SIZE_FOR_PREVIEW as usize;
const MAX_PREVIEW_DIRECTORY_ENTRIES: usize = 1024;
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
  pub row_data:     Option<FilePickerRowData>,
  pub preview:      Option<FilePickerPreview>,
  pub payload:      Option<FilePickerItemPayload>,
}

#[derive(Debug, Clone)]
pub enum FilePickerItemAction {
  OpenFile(PathBuf),
  GroupHeader {
    path: PathBuf,
  },
  SwitchBuffer {
    buffer_id: BufferId,
  },
  RestoreJump {
    buffer_id:     BufferId,
    selection:     Selection,
    active_cursor: Option<CursorId>,
  },
  OpenLocation {
    path:        PathBuf,
    cursor_char: usize,
    line:        usize,
    column:      Option<usize>,
  },
  Custom {
    handler:    PickerSubmitHandlerRef,
    selectable: bool,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickerSubmitHandlerRef {
  Missing,
  Runtime(PickerRuntimeSessionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerSubmitResult {
  Unhandled,
  KeepOpen,
  Close,
}

impl FilePickerItemAction {
  fn is_selectable(&self) -> bool {
    !matches!(self, Self::GroupHeader { .. })
      && !matches!(self, Self::Custom {
        selectable: false,
        ..
      })
  }

  fn stable_id(&self) -> u64 {
    let mut hasher = DefaultHasher::new();
    match self {
      Self::OpenFile(path) => {
        0_u8.hash(&mut hasher);
        path.hash(&mut hasher);
      },
      Self::GroupHeader { path } => {
        1_u8.hash(&mut hasher);
        path.hash(&mut hasher);
      },
      Self::SwitchBuffer { buffer_id } => {
        2_u8.hash(&mut hasher);
        buffer_id.hash(&mut hasher);
      },
      Self::RestoreJump {
        buffer_id,
        selection,
        active_cursor,
      } => {
        3_u8.hash(&mut hasher);
        buffer_id.hash(&mut hasher);
        for range in selection.ranges() {
          range.anchor.hash(&mut hasher);
          range.head.hash(&mut hasher);
        }
        for cursor_id in selection.cursor_ids() {
          cursor_id.hash(&mut hasher);
        }
        active_cursor.map(CursorId::get).hash(&mut hasher);
      },
      Self::OpenLocation {
        path,
        cursor_char,
        line,
        column,
      } => {
        4_u8.hash(&mut hasher);
        path.hash(&mut hasher);
        cursor_char.hash(&mut hasher);
        line.hash(&mut hasher);
        column.hash(&mut hasher);
      },
      Self::Custom {
        handler,
        selectable,
      } => {
        5_u8.hash(&mut hasher);
        match handler {
          PickerSubmitHandlerRef::Missing => {
            0_u8.hash(&mut hasher);
          },
          PickerSubmitHandlerRef::Runtime(id) => {
            1_u8.hash(&mut hasher);
            id.hash(&mut hasher);
          },
        }
        selectable.hash(&mut hasher);
      },
    }
    let id = hasher.finish();
    if id == 0 { 1 } else { id }
  }
}

#[derive(Clone)]
pub struct FilePickerItemPayload {
  type_name: &'static str,
  value:     Arc<dyn Any + Send + Sync>,
}

impl FilePickerItemPayload {
  pub fn new<T>(value: T) -> Self
  where
    T: Any + Send + Sync,
  {
    Self {
      type_name: std::any::type_name::<T>(),
      value:     Arc::new(value),
    }
  }

  pub fn get<T>(&self) -> Option<&T>
  where
    T: Any + Send + Sync,
  {
    self.value.as_ref().downcast_ref::<T>()
  }
}

#[derive(Debug, Clone)]
pub enum DirectPickerTrackingKind {
  FffFileSearch {
    root:  PathBuf,
    query: String,
  },
  FffGrep {
    root:  PathBuf,
    query: String,
  },
}

#[derive(Debug, Clone)]
pub struct DirectPickerItemMetadata {
  pub match_indices:          Arc<[usize]>,
  pub primary_match_ranges:   Arc<[(usize, usize)]>,
  pub secondary_match_ranges: Arc<[(usize, usize)]>,
  pub preview_match_ranges:   Arc<[(usize, usize)]>,
  pub tracking:               DirectPickerTrackingKind,
}

impl std::fmt::Debug for FilePickerItemPayload {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FilePickerItemPayload")
      .field("type_name", &self.type_name)
      .finish()
  }
}

impl FilePickerItem {
  fn is_selectable(&self) -> bool {
    self.action.is_selectable()
  }

  pub fn stable_id(&self) -> u64 {
    let mut hasher = DefaultHasher::new();
    if let Some(payload) = self.payload::<FilePickerVcsDiffPayload>() {
      255_u8.hash(&mut hasher);
      payload.path.hash(&mut hasher);
      payload.entry_index.hash(&mut hasher);
      payload.hunk_index.hash(&mut hasher);
      let id = hasher.finish();
      return if id == 0 { 1 } else { id };
    }
    self.action.stable_id().hash(&mut hasher);
    self.absolute.hash(&mut hasher);
    self.display.hash(&mut hasher);
    self.icon.hash(&mut hasher);
    self.is_dir.hash(&mut hasher);
    self.display_path.hash(&mut hasher);
    self.preview_path.hash(&mut hasher);
    self.preview_line.hash(&mut hasher);
    self.preview_col.hash(&mut hasher);
    let id = hasher.finish();
    if id == 0 { 1 } else { id }
  }

  pub fn with_row_data(mut self, row_data: FilePickerRowData) -> Self {
    self.row_data = Some(row_data);
    self
  }

  pub fn with_preview(mut self, preview: FilePickerPreview) -> Self {
    self.preview = Some(preview);
    self
  }

  pub fn with_payload<T>(mut self, payload: T) -> Self
  where
    T: Any + Send + Sync,
  {
    self.payload = Some(FilePickerItemPayload::new(payload));
    self
  }

  pub fn payload<T>(&self) -> Option<&T>
  where
    T: Any + Send + Sync,
  {
    self
      .payload
      .as_ref()
      .and_then(FilePickerItemPayload::get::<T>)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerKind {
  Generic,
  Diagnostics,
  Symbols,
  LiveGrep,
  VcsDiff,
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
  VcsDiffHeader,
  VcsDiffHunk,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilePickerSearchMode {
  #[default]
  None,
  PlainText,
  Regex,
  Fuzzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilePickerStatusBannerKind {
  #[default]
  Info,
  Warning,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerStatusBanner {
  pub kind: FilePickerStatusBannerKind,
  pub text: String,
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
  pub change_kind:  Option<FilePickerPreviewChangeKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerPreviewChangeKind {
  Added,
  Removed,
  Modified,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilePickerPreviewWindowKind {
  #[default]
  Empty,
  Source,
  Text,
  Message,
  VcsDiff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilePickerVcsDiffPreviewRowKind {
  #[default]
  Context,
  Added,
  Removed,
  Modified,
  SectionHeader,
  Info,
  CollapsedAbove,
  CollapsedBelow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickerVcsDiffPreviewRow {
  pub kind:              FilePickerVcsDiffPreviewRowKind,
  pub left_line_index:   Option<usize>,
  pub right_line_index:  Option<usize>,
  pub left_line_number:  Option<usize>,
  pub right_line_number: Option<usize>,
  pub message:           String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickerVcsDiffPreview {
  pub title:        String,
  pub from_title:   Option<String>,
  pub left_label:   String,
  pub right_label:  String,
  pub left:         FilePickerSourcePreview,
  pub right:        FilePickerSourcePreview,
  pub rows:         Vec<FilePickerVcsDiffPreviewRow>,
  pub cached_lines: Arc<[FilePickerVcsDiffPreviewWindowLine]>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerVcsDiffPreviewWindowLine {
  pub virtual_row: usize,
  pub kind:        FilePickerVcsDiffPreviewRowKind,
  pub source:      FilePickerVcsDiffPreviewLineSource,
  pub line_number: Option<usize>,
  pub segments:    Vec<FilePickerPreviewSegment>,
  pub message:     String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerVcsDiffPreviewWindow {
  pub total_virtual_rows: usize,
  pub lines:              Vec<FilePickerVcsDiffPreviewWindowLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilePickerVcsDiffPreviewLineSource {
  Base,
  Worktree,
  #[default]
  Meta,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePickerPreviewWindow {
  pub navigation_mode:    FilePickerPreviewNavigationMode,
  pub kind:               FilePickerPreviewWindowKind,
  pub total_virtual_rows: usize,
  pub offset:             usize,
  pub window_start:       usize,
  pub lines:              Vec<FilePickerPreviewWindowLine>,
  pub vcs_diff:           Option<FilePickerVcsDiffPreviewWindow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilePickerPreviewNavigationMode {
  #[default]
  Static,
  Scrollable,
  Anchored,
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

#[derive(Debug, Clone)]
pub enum FilePickerVcsDiffBootstrap {
  Ready {
    root:    PathBuf,
    changed: Vec<FilePickerChangedFileItem>,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickerOptions {
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

impl Default for FilePickerOptions {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilePickerQueryMode {
  #[default]
  Static,
  Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[doc(hidden)]
pub struct PickerRuntimeSessionId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PickerQueryHandlerRef {
  Runtime(PickerRuntimeSessionId),
}

pub type PickerQuerySource<Ctx> = dyn Fn(&mut Ctx, &str) -> Vec<PickerItemSpec> + 'static;
pub type PickerRuntimeSubmit<Ctx> =
  dyn Fn(&mut Ctx, &FilePickerItem) -> PickerSubmitResult + 'static;
pub(crate) type PickerPathFilter = dyn Fn(&Path, &Path) -> bool + Send + Sync + 'static;

pub struct PickerRuntimeSession<Ctx> {
  query:  Option<Box<PickerQuerySource<Ctx>>>,
  submit: Option<Box<PickerRuntimeSubmit<Ctx>>>,
}

pub struct PickerRuntimeStore<Ctx> {
  next_id:  u64,
  sessions: HashMap<PickerRuntimeSessionId, PickerRuntimeSession<Ctx>>,
}

impl<Ctx> Default for PickerRuntimeStore<Ctx> {
  fn default() -> Self {
    Self {
      next_id:  0,
      sessions: HashMap::new(),
    }
  }
}

impl<Ctx> PickerRuntimeStore<Ctx> {
  pub fn register(&mut self, session: PickerRuntimeSession<Ctx>) -> PickerRuntimeSessionId {
    let next = self.next_id.wrapping_add(1).max(1);
    self.next_id = next;
    let id = PickerRuntimeSessionId(next);
    self.sessions.insert(id, session);
    id
  }

  pub fn remove(&mut self, id: PickerRuntimeSessionId) {
    self.sessions.remove(&id);
  }
}

#[derive(Debug, Clone)]
pub enum FilePickerPreview {
  Empty,
  Source(FilePickerSourcePreview),
  Text(String),
  Message(String),
  VcsDiff(FilePickerVcsDiffPreview),
}

#[derive(Debug, Clone)]
pub struct FilePickerVcsDiffHunk {
  pub summary:            String,
  pub target_line:        Option<usize>,
  pub target_cursor_char: Option<usize>,
  pub before_start:       usize,
  pub before_end:         usize,
  pub after_start:        usize,
  pub after_end:          usize,
  pub preview:            FilePickerPreview,
}

#[derive(Debug, Clone)]
pub struct FilePickerVcsDiffEntry {
  pub kind:      FilePickerChangedKind,
  pub path:      PathBuf,
  pub from_path: Option<PathBuf>,
  pub hunks:     Vec<FilePickerVcsDiffHunk>,
}

#[derive(Debug, Clone)]
pub struct FilePickerVcsDiffPayload {
  pub path:        PathBuf,
  pub entry_index: usize,
  pub hunk_index:  Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickerSourcePreview {
  pub text:               Arc<str>,
  pub line_starts:        Arc<[usize]>,
  pub highlights:         Arc<[(Highlight, Range<usize>)]>,
  pub truncated_by_bytes: bool,
}

impl FilePickerSourcePreview {
  pub fn total_lines(&self) -> usize {
    self.line_starts.len()
  }

  pub fn line_start(&self, line_index: usize) -> Option<usize> {
    self.line_starts.get(line_index).copied()
  }

  pub fn line_text(&self, line_index: usize) -> Option<&str> {
    let start = self.line_start(line_index)?;
    let end = self
      .line_starts
      .get(line_index + 1)
      .copied()
      .unwrap_or(self.text.len());
    let raw = self.text.get(start..end)?;
    Some(
      raw
        .strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or(raw),
    )
  }
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

#[derive(Clone)]
struct FilePickerScanSource {
  options:        FilePickerOptions,
  max_results:    usize,
  extensions:     Option<Arc<[String]>>,
  path_filter:    Option<Arc<PickerPathFilter>>,
  preview:        bool,
  submit_handler: Option<PickerSubmitHandlerRef>,
}

pub struct FilePickerState {
  pub active:             bool,
  pub kind:               FilePickerKind,
  pub root:               PathBuf,
  pub title:              String,
  pub options:            FilePickerOptions,
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
  pub status_banner:      Option<FilePickerStatusBanner>,
  pub scanning:           bool,
  pub matcher_running:    bool,
  pub search_mode:        FilePickerSearchMode,
  pub query_mode:         FilePickerQueryMode,
  custom_query_handler:   Option<PickerQueryHandlerRef>,
  pub dynamic_running:    bool,
  runtime_session:        Option<PickerRuntimeSessionId>,
  scan_source:            Option<FilePickerScanSource>,
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
  direct_items:           Vec<Arc<FilePickerItem>>,
  use_direct_items:       bool,
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
      kind: FilePickerKind::Generic,
      root: PathBuf::new(),
      title: "File Picker".to_string(),
      options: FilePickerOptions::default(),
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
      status_banner: None,
      scanning: false,
      matcher_running: false,
      search_mode: FilePickerSearchMode::None,
      query_mode: FilePickerQueryMode::Static,
      custom_query_handler: None,
      dynamic_running: false,
      runtime_session: None,
      scan_source: None,
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
      direct_items: Vec::new(),
      use_direct_items: false,
      matcher,
    }
  }
}

pub fn set_file_picker_wake_sender(state: &mut FilePickerState, wake_tx: Option<Sender<()>>) {
  state.wake_tx = wake_tx;
}

pub fn set_file_picker_options(state: &mut FilePickerState, options: FilePickerOptions) {
  state.options = options;
}

pub fn set_file_picker_syntax_loader(state: &mut FilePickerState, loader: Option<Arc<Loader>>) {
  state.syntax_loader = loader;
}

impl FilePickerState {
  pub fn current_item(&self) -> Option<Arc<FilePickerItem>> {
    let selected = self.selected?;
    self.matched_item(selected)
  }

  pub fn matched_item(&self, matched_index: usize) -> Option<Arc<FilePickerItem>> {
    if self.use_direct_items {
      return self.direct_items.get(matched_index).cloned();
    }
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
    indices_out.clear();

    if self.use_direct_items {
      let item = self.direct_items.get(matched_index)?.clone();
      if let Some(metadata) = item.payload::<DirectPickerItemMetadata>() {
        indices_out.extend(metadata.match_indices.iter().copied());
      }
      return Some(item);
    }

    let snapshot = self.matcher.snapshot();
    let item = snapshot.get_matched_item(matched_index as u32)?;

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

  pub fn matched_item_stable_id(&self, matched_index: usize) -> Option<u64> {
    self
      .matched_item(matched_index)
      .map(|item| item.stable_id())
  }

  pub fn selected_item_stable_id(&self) -> Option<u64> {
    self
      .selected
      .and_then(|matched_index| self.matched_item_stable_id(matched_index))
  }

  pub fn matched_index_for_stable_id(&self, stable_id: u64) -> Option<usize> {
    if self.use_direct_items {
      return self
        .direct_items
        .iter()
        .position(|item| item.stable_id() == stable_id);
    }

    let snapshot = self.matcher.snapshot();
    let matched_count = snapshot.matched_item_count() as usize;
    (0..matched_count).find(|&matched_index| {
      snapshot
        .get_matched_item(matched_index as u32)
        .is_some_and(|item| item.data.stable_id() == stable_id)
    })
  }

  pub fn matched_count(&self) -> usize {
    if self.use_direct_items {
      return self.direct_items.len();
    }
    self.matcher.snapshot().matched_item_count() as usize
  }

  pub fn total_count(&self) -> usize {
    if self.use_direct_items {
      return self.direct_items.len();
    }
    self.matcher.snapshot().item_count() as usize
  }

  pub fn preview_loading(&self) -> bool {
    self.preview_pending_id.is_some()
  }

  pub fn runtime_session(&self) -> Option<PickerRuntimeSessionId> {
    self.runtime_session
  }

  pub fn preview_navigation_mode(&self) -> FilePickerPreviewNavigationMode {
    match &self.preview {
      FilePickerPreview::Empty | FilePickerPreview::Message(_) => {
        FilePickerPreviewNavigationMode::Static
      },
      FilePickerPreview::VcsDiff(_)
      | FilePickerPreview::Source(_)
      | FilePickerPreview::Text(_) => FilePickerPreviewNavigationMode::Scrollable,
    }
  }

  pub fn preview_line_count(&self) -> usize {
    match &self.preview {
      FilePickerPreview::Empty => 0,
      FilePickerPreview::Source(source) => source.total_lines(),
      FilePickerPreview::Text(text) => text.lines().count().max(1),
      FilePickerPreview::Message(message) => message.lines().count().max(1),
      FilePickerPreview::VcsDiff(preview) => vcs_diff_preview_line_count(preview),
    }
  }

  pub fn clamp_preview_scroll(&mut self, visible_rows: usize) {
    if self.preview_navigation_mode() != FilePickerPreviewNavigationMode::Scrollable {
      self.preview_scroll = 0;
      return;
    }
    let max_offset = self
      .preview_line_count()
      .saturating_sub(visible_rows.max(1));
    if self.preview_scroll > max_offset {
      self.preview_scroll = max_offset;
    }
  }
}

#[derive(Debug, Clone)]
pub enum PickerRoot {
  Workspace,
  EffectiveWorkingDirectory,
  Fixed(PathBuf),
}

impl PickerRoot {
  fn resolve<Ctx: DefaultContext>(&self, ctx: &Ctx) -> PathBuf {
    match self {
      Self::Workspace => ctx.workspace_root(),
      Self::EffectiveWorkingDirectory => ctx.effective_working_directory(),
      Self::Fixed(path) => path.clone(),
    }
  }
}

#[derive(Debug, Clone)]
pub enum PickerItemSpecAction {
  OpenFile(PathBuf),
  OpenLocation {
    path:        PathBuf,
    cursor_char: usize,
    line:        usize,
    column:      Option<usize>,
  },
  Custom {
    selectable: bool,
  },
}

#[derive(Debug, Clone)]
pub struct PickerItemSpec {
  absolute:     PathBuf,
  display:      String,
  icon:         String,
  is_dir:       bool,
  display_path: bool,
  action:       PickerItemSpecAction,
  preview_path: Option<PathBuf>,
  preview_line: Option<usize>,
  preview_col:  Option<(usize, usize)>,
  row_data:     Option<FilePickerRowData>,
  preview:      Option<FilePickerPreview>,
  payload:      Option<FilePickerItemPayload>,
}

impl PickerItemSpec {
  pub fn custom(display: impl Into<String>) -> Self {
    let display = display.into();
    Self {
      absolute: PathBuf::from(display.clone()),
      display,
      icon: "sparkle".to_string(),
      is_dir: false,
      display_path: false,
      action: PickerItemSpecAction::Custom { selectable: true },
      preview_path: None,
      preview_line: None,
      preview_col: None,
      row_data: None,
      preview: None,
      payload: None,
    }
  }

  pub fn file(root: impl AsRef<Path>, path: impl AsRef<Path>) -> Option<Self> {
    let root = root.as_ref();
    let path = path.as_ref();
    let relative = path.strip_prefix(root).ok()?;
    let mut display = relative.to_string_lossy().into_owned();
    if std::path::MAIN_SEPARATOR != '/' {
      display = display.replace(std::path::MAIN_SEPARATOR, "/");
    }
    let path_buf = path.to_path_buf();
    Some(Self {
      absolute: path_buf.clone(),
      display,
      icon: file_picker_icon_name_for_path(relative).to_string(),
      is_dir: false,
      display_path: false,
      action: PickerItemSpecAction::OpenFile(path_buf.clone()),
      preview_path: Some(path_buf),
      preview_line: None,
      preview_col: None,
      row_data: None,
      preview: None,
      payload: None,
    })
  }

  pub fn location(
    display: impl Into<String>,
    path: impl Into<PathBuf>,
    cursor_char: usize,
    line: usize,
    column: Option<usize>,
  ) -> Self {
    let path = path.into();
    Self {
      absolute:     path.clone(),
      display:      display.into(),
      icon:         file_picker_icon_name_for_path(&path).to_string(),
      is_dir:       false,
      display_path: false,
      action:       PickerItemSpecAction::OpenLocation {
        path: path.clone(),
        cursor_char,
        line,
        column,
      },
      preview_path: Some(path),
      preview_line: Some(line),
      preview_col:  column.map(|col| (col, col)),
      row_data:     None,
      preview:      None,
      payload:      None,
    }
  }

  pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
    self.icon = icon.into();
    self
  }

  pub fn with_preview_path(mut self, path: impl Into<PathBuf>) -> Self {
    self.preview_path = Some(path.into());
    self
  }

  pub fn with_preview_line(mut self, line: usize) -> Self {
    self.preview_line = Some(line);
    self
  }

  pub fn with_preview(mut self, preview: FilePickerPreview) -> Self {
    self.preview = Some(preview);
    self
  }

  pub fn with_selectable(mut self, selectable: bool) -> Self {
    if let PickerItemSpecAction::Custom {
      selectable: spec_selectable,
    } = &mut self.action
    {
      *spec_selectable = selectable;
    }
    self
  }

  pub fn with_row_data(mut self, row_data: FilePickerRowData) -> Self {
    self.row_data = Some(row_data);
    self
  }

  pub fn with_payload<T>(mut self, payload: T) -> Self
  where
    T: Any + Send + Sync,
  {
    self.payload = Some(FilePickerItemPayload::new(payload));
    self
  }

  fn into_item(self, submit_handler: Option<PickerSubmitHandlerRef>) -> FilePickerItem {
    let action = match self.action {
      PickerItemSpecAction::OpenFile(path) => FilePickerItemAction::OpenFile(path),
      PickerItemSpecAction::OpenLocation {
        path,
        cursor_char,
        line,
        column,
      } => {
        FilePickerItemAction::OpenLocation {
          path,
          cursor_char,
          line,
          column,
        }
      },
      PickerItemSpecAction::Custom { selectable } => {
        FilePickerItemAction::Custom {
          handler:    submit_handler.unwrap_or(PickerSubmitHandlerRef::Missing),
          selectable: selectable && submit_handler.is_some(),
        }
      },
    };

    FilePickerItem {
      absolute: self.absolute,
      display: self.display,
      icon: self.icon,
      is_dir: self.is_dir,
      display_path: self.display_path,
      action,
      preview_path: self.preview_path,
      preview_line: self.preview_line,
      preview_col: self.preview_col,
      row_data: self.row_data,
      preview: self.preview,
      payload: self.payload,
    }
  }
}

enum PickerSource<Ctx> {
  Files {
    scan_source:    FilePickerScanSource,
    submit_handler: Option<Arc<PickerRuntimeSubmit<Ctx>>>,
  },
  Static {
    items:          Arc<[PickerItemSpec]>,
    submit_handler: Option<Arc<PickerRuntimeSubmit<Ctx>>>,
  },
  Dynamic {
    query:          Arc<PickerQuerySource<Ctx>>,
    submit_handler: Option<Arc<PickerRuntimeSubmit<Ctx>>>,
  },
}

pub struct PickerBuilder<Ctx> {
  kind:          FilePickerKind,
  title:         String,
  root:          PickerRoot,
  open_split:    Option<SplitAxis>,
  initial_query: String,
  source:        PickerSource<Ctx>,
}

impl<Ctx> Clone for PickerSource<Ctx> {
  fn clone(&self) -> Self {
    match self {
      Self::Files {
        scan_source,
        submit_handler,
      } => {
        Self::Files {
          scan_source:    scan_source.clone(),
          submit_handler: submit_handler.clone(),
        }
      },
      Self::Static {
        items,
        submit_handler,
      } => {
        Self::Static {
          items:          items.clone(),
          submit_handler: submit_handler.clone(),
        }
      },
      Self::Dynamic {
        query,
        submit_handler,
      } => {
        Self::Dynamic {
          query:          query.clone(),
          submit_handler: submit_handler.clone(),
        }
      },
    }
  }
}

impl<Ctx> Clone for PickerBuilder<Ctx> {
  fn clone(&self) -> Self {
    Self {
      kind:          self.kind,
      title:         self.title.clone(),
      root:          self.root.clone(),
      open_split:    self.open_split,
      initial_query: self.initial_query.clone(),
      source:        self.source.clone(),
    }
  }
}

impl<Ctx> PickerBuilder<Ctx>
where
  Ctx: DefaultContext,
{
  pub fn files(title: impl Into<String>) -> Self {
    Self {
      kind:          FilePickerKind::Generic,
      title:         title.into(),
      root:          PickerRoot::EffectiveWorkingDirectory,
      open_split:    None,
      initial_query: String::new(),
      source:        PickerSource::Files {
        scan_source:    FilePickerScanSource {
          options:        FilePickerOptions::default(),
          max_results:    MAX_SCAN_ITEMS,
          extensions:     None,
          path_filter:    None,
          preview:        true,
          submit_handler: None,
        },
        submit_handler: None,
      },
    }
  }

  pub fn static_items<I>(title: impl Into<String>, items: I) -> Self
  where
    I: IntoIterator<Item = PickerItemSpec>,
  {
    Self {
      kind:          FilePickerKind::Generic,
      title:         title.into(),
      root:          PickerRoot::EffectiveWorkingDirectory,
      open_split:    None,
      initial_query: String::new(),
      source:        PickerSource::Static {
        items:          items.into_iter().collect::<Vec<_>>().into(),
        submit_handler: None,
      },
    }
  }

  pub fn dynamic<F>(title: impl Into<String>, query: F) -> Self
  where
    F: Fn(&mut Ctx, &str) -> Vec<PickerItemSpec> + 'static,
  {
    Self {
      kind:          FilePickerKind::Generic,
      title:         title.into(),
      root:          PickerRoot::EffectiveWorkingDirectory,
      open_split:    None,
      initial_query: String::new(),
      source:        PickerSource::Dynamic {
        query:          Arc::new(query),
        submit_handler: None,
      },
    }
  }

  pub fn root(mut self, root: PickerRoot) -> Self {
    self.root = root;
    self
  }

  pub fn kind(mut self, kind: FilePickerKind) -> Self {
    self.kind = kind;
    self
  }

  pub fn open_split(mut self, open_split: Option<SplitAxis>) -> Self {
    self.open_split = open_split;
    self
  }

  pub fn initial_query(mut self, query: impl Into<String>) -> Self {
    self.initial_query = query.into();
    self
  }

  pub fn extension(mut self, extension: impl Into<String>) -> Self {
    self = self.extensions([extension]);
    self
  }

  pub fn extensions<I, S>(mut self, extensions: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    if let PickerSource::Files { scan_source, .. } = &mut self.source {
      let values = extensions
        .into_iter()
        .map(Into::into)
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
      scan_source.extensions = (!values.is_empty()).then_some(values.into());
    }
    self
  }

  pub fn max_results(mut self, max_results: usize) -> Self {
    if let PickerSource::Files { scan_source, .. } = &mut self.source {
      scan_source.max_results = max_results.max(1);
    }
    self
  }

  pub fn max_depth(mut self, max_depth: Option<usize>) -> Self {
    if let PickerSource::Files { scan_source, .. } = &mut self.source {
      scan_source.options.max_depth = max_depth;
    }
    self
  }

  pub fn path_filter<F>(mut self, filter: F) -> Self
  where
    F: Fn(&Path, &Path) -> bool + Send + Sync + 'static,
  {
    if let PickerSource::Files { scan_source, .. } = &mut self.source {
      scan_source.path_filter = Some(Arc::new(filter));
    }
    self
  }

  pub fn preview(mut self, enabled: bool) -> Self {
    if let PickerSource::Files { scan_source, .. } = &mut self.source {
      scan_source.preview = enabled;
    }
    self
  }

  pub fn on_submit<F>(mut self, submit: F) -> Self
  where
    F: Fn(&mut Ctx, &FilePickerItem) -> PickerSubmitResult + 'static,
  {
    let submit = Arc::new(submit);
    match &mut self.source {
      PickerSource::Files { submit_handler, .. } => {
        *submit_handler = Some(submit);
      },
      PickerSource::Static { submit_handler, .. } => {
        *submit_handler = Some(submit);
      },
      PickerSource::Dynamic { submit_handler, .. } => {
        *submit_handler = Some(submit);
      },
    }
    self
  }

  pub fn open(&self, ctx: &mut Ctx) {
    let root = self.root.resolve(ctx);
    match &self.source {
      PickerSource::Files {
        scan_source,
        submit_handler,
      } => {
        open_picker_from_builder_files(
          ctx,
          self.kind,
          &self.title,
          root,
          self.open_split,
          scan_source.clone(),
          submit_handler.clone(),
        );
      },
      PickerSource::Static {
        items,
        submit_handler,
      } => {
        open_picker_from_builder_items(
          ctx,
          self.kind,
          &self.title,
          root,
          self.open_split,
          items.iter().cloned().collect(),
          submit_handler.clone(),
        );
      },
      PickerSource::Dynamic {
        query,
        submit_handler,
      } => {
        open_picker_from_builder_query(
          ctx,
          self.kind,
          &self.title,
          root,
          self.open_split,
          self.initial_query.clone(),
          query.clone(),
          submit_handler.clone(),
        );
      },
    }
  }

  pub fn command(self, name: &'static str, doc: &'static str) -> TypableCommand<Ctx> {
    crate::CommandBuilder::new(name, doc, move |ctx, _args, _event: CommandEvent| {
      self.open(ctx);
      Ok(())
    })
    .build()
  }
}

pub fn open_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  PickerBuilder::<Ctx>::files("File Picker").open(ctx);
}

pub fn open_file_picker_with_split<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  open_split: Option<SplitAxis>,
) {
  PickerBuilder::<Ctx>::files("File Picker")
    .open_split(open_split)
    .open(ctx);
}

pub fn open_file_picker_with_root_and_split<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  root: PathBuf,
  open_split: Option<SplitAxis>,
) {
  PickerBuilder::<Ctx>::files("File Picker")
    .root(PickerRoot::Fixed(root))
    .open_split(open_split)
    .open(ctx);
}

pub fn open_file_picker_in_current_directory<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc_dir = ctx
    .file_path()
    .and_then(|path| path.parent().map(Path::to_path_buf));
  let root = match doc_dir {
    Some(path) => path,
    None => {
      let cwd = ctx.effective_working_directory();
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
  PickerBuilder::<Ctx>::files("File Picker")
    .root(PickerRoot::Fixed(root))
    .open(ctx);
}

pub fn open_buffer_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let cwd = ctx.effective_working_directory();
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
        .unwrap_or_else(|| PathBuf::from(format!("<buffer:{}>", snapshot.buffer_id.as_u64())));
      let preview_line = editor.buffer_document(snapshot.buffer_id).map(|doc| {
        selection_focus_line(
          doc.selection(),
          editor
            .buffer_view(snapshot.buffer_id)
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
          buffer_id: snapshot.buffer_id,
        },
        preview_path: snapshot.file_path,
        preview_line,
        preview_col: None,
        row_data: None,
        preview: None,
        payload: None,
      }
    })
    .collect();

  open_static_picker(ctx, "Buffer Picker", root, None, items, initial_cursor);
}

pub fn open_jumplist_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let cwd = ctx.effective_working_directory();
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
      let snapshot = editor.buffer_snapshot(jump.buffer_id)?;
      let doc = editor.buffer_document(jump.buffer_id)?;
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
          buffer_id:     jump.buffer_id,
          selection:     jump.selection.clone(),
          active_cursor: jump.active_cursor,
        },
        preview_path: snapshot.file_path,
        preview_line: Some(preview_line),
        preview_col: None,
        row_data: None,
        preview: None,
        payload: None,
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
  let cwd = ctx.effective_working_directory();
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
        row_data: None,
        preview: None,
        payload: None,
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

pub fn open_vcs_diff_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let (root, changed) = match ctx.file_picker_vcs_diff_bootstrap() {
    Ok(FilePickerVcsDiffBootstrap::Ready { root, changed }) => (root, changed),
    Err(err) => {
      ctx.push_warning("vcs_diff_picker", err);
      return;
    },
  };
  if changed.is_empty() {
    ctx.push_warning("vcs_diff_picker", "no changed files");
    return;
  }

  let entries: Vec<_> = changed
    .iter()
    .map(file_picker_vcs_diff_placeholder_entry)
    .collect();
  let items = file_picker_vcs_diff_specs(&root, &entries);

  PickerBuilder::static_items("VCS Diff Picker", items)
    .root(PickerRoot::Fixed(root))
    .kind(FilePickerKind::VcsDiff)
    .open(ctx);
  ctx.file_picker_vcs_diff_did_open();
  ctx.file_picker_selection_changed();
}

pub fn open_changed_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open_vcs_diff_picker(ctx);
}

fn vcs_diff_icon_name(kind: FilePickerChangedKind) -> &'static str {
  match kind {
    FilePickerChangedKind::Untracked => "git_untracked",
    FilePickerChangedKind::Modified => "git_modified",
    FilePickerChangedKind::Conflict => "git_conflict",
    FilePickerChangedKind::Deleted => "git_deleted",
    FilePickerChangedKind::Renamed => "git_renamed",
  }
}

fn vcs_diff_status_label(kind: FilePickerChangedKind) -> &'static str {
  match kind {
    FilePickerChangedKind::Untracked => "untracked",
    FilePickerChangedKind::Modified => "modified",
    FilePickerChangedKind::Conflict => "conflict",
    FilePickerChangedKind::Deleted => "deleted",
    FilePickerChangedKind::Renamed => "renamed",
  }
}

pub fn file_picker_vcs_diff_placeholder_entry(
  item: &FilePickerChangedFileItem,
) -> FilePickerVcsDiffEntry {
  FilePickerVcsDiffEntry {
    kind:      item.kind,
    path:      item.path.clone(),
    from_path: item.from_path.clone(),
    hunks:     vec![FilePickerVcsDiffHunk {
      summary:            "loading diff…".to_string(),
      target_line:        None,
      target_cursor_char: None,
      before_start:       0,
      before_end:         0,
      after_start:        0,
      after_end:          0,
      preview:            FilePickerPreview::Message("Loading diff…".to_string()),
    }],
  }
}

pub fn file_picker_vcs_diff_specs(
  root: &Path,
  entries: &[FilePickerVcsDiffEntry],
) -> Vec<PickerItemSpec> {
  let mut items = Vec::new();
  for (entry_index, entry) in entries.iter().enumerate() {
    let display_path = display_relative_path(&entry.path, root);
    let from_display = entry
      .from_path
      .as_ref()
      .map(|path| display_relative_path(path, root));

    items.push(
      PickerItemSpec::custom(display_path.clone())
        .with_selectable(false)
        .with_icon(vcs_diff_icon_name(entry.kind))
        .with_row_data(FilePickerRowData {
          kind:       FilePickerRowKind::VcsDiffHeader,
          severity:   None,
          primary:    display_path,
          secondary:  vcs_diff_status_label(entry.kind).to_string(),
          tertiary:   from_display.clone().unwrap_or_default(),
          quaternary: String::new(),
          line:       0,
          column:     0,
          depth:      0,
        }),
    );

    for (hunk_index, hunk) in entry.hunks.iter().enumerate() {
      let row_data = FilePickerRowData {
        kind:       FilePickerRowKind::VcsDiffHunk,
        severity:   None,
        primary:    hunk.summary.clone(),
        secondary:  display_relative_path(&entry.path, root),
        tertiary:   vcs_diff_status_label(entry.kind).to_string(),
        quaternary: from_display.clone().unwrap_or_default(),
        line:       hunk.target_line.unwrap_or(0).saturating_add(1),
        column:     1,
        depth:      1,
      };

      let item = if let (Some(target_line), Some(cursor_char)) =
        (hunk.target_line, hunk.target_cursor_char)
      {
        PickerItemSpec::location(
          hunk.summary.clone(),
          entry.path.clone(),
          cursor_char,
          target_line,
          None,
        )
        .with_icon(vcs_diff_icon_name(entry.kind))
        .with_preview(FilePickerPreview::Message("Loading diff…".to_string()))
        .with_row_data(row_data)
        .with_payload(FilePickerVcsDiffPayload {
          path: entry.path.clone(),
          entry_index,
          hunk_index: Some(hunk_index),
        })
      } else {
        PickerItemSpec::custom(hunk.summary.clone())
          .with_icon(vcs_diff_icon_name(entry.kind))
          .with_preview(hunk.preview.clone())
          .with_row_data(row_data)
          .with_payload(FilePickerVcsDiffPayload {
            path: entry.path.clone(),
            entry_index,
            hunk_index: Some(hunk_index),
          })
      };
      items.push(item);
    }
  }
  items
}

fn open_static_picker<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  let mut state = base_picker_state(ctx, file_picker_kind_from_title(title), title, open_split);
  state.root = root;
  replace_picker_items(&mut state, items, initial_cursor);

  *ctx.file_picker_mut() = state;
  ctx.file_picker_selection_changed();
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

pub fn open_custom_picker_with_query_handler<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  initial_query: String,
  query_handler: &'static str,
) {
  open_dynamic_picker_with_handler(ctx, title, root, open_split, initial_query, query_handler);
}

pub fn open_dynamic_picker<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  initial_query: String,
) {
  let mut state = base_picker_state(ctx, file_picker_kind_from_title(title), title, open_split);
  state.root = root;
  state.query_mode = FilePickerQueryMode::Dynamic;
  state.query = initial_query;
  state.cursor = state.query.len();
  prepare_dynamic_query_change(&mut state);
  start_preview_worker(&mut state);

  *ctx.file_picker_mut() = state;
  ctx.request_render();
}

pub fn open_dynamic_picker_with_handler<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  initial_query: String,
  _query_handler: &'static str,
) {
  open_dynamic_picker(ctx, title, root, open_split, initial_query);
}

fn register_picker_runtime_session<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  session: PickerRuntimeSession<Ctx>,
) -> PickerRuntimeSessionId {
  ctx.picker_runtime_store_mut().register(session)
}

fn drop_picker_runtime_session<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  session_id: PickerRuntimeSessionId,
) {
  ctx.picker_runtime_store_mut().remove(session_id);
}

fn run_picker_runtime_query<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  session_id: PickerRuntimeSessionId,
  query: &str,
) -> Option<Vec<PickerItemSpec>> {
  let registry = ctx.picker_runtime_store() as *const PickerRuntimeStore<Ctx>;
  let handler = unsafe {
    let session = (&*registry).sessions.get(&session_id)?;
    session
      .query
      .as_ref()
      .map(|handler| &**handler as *const PickerQuerySource<Ctx>)?
  };
  Some(unsafe { (&*handler)(ctx, query) })
}

fn run_picker_runtime_submit<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  session_id: PickerRuntimeSessionId,
  item: &FilePickerItem,
) -> PickerSubmitResult {
  let registry = ctx.picker_runtime_store() as *const PickerRuntimeStore<Ctx>;
  let Some(handler) = (unsafe { (&*registry).sessions.get(&session_id) }).and_then(|session| {
    session
      .submit
      .as_ref()
      .map(|handler| &**handler as *const PickerRuntimeSubmit<Ctx>)
  }) else {
    return PickerSubmitResult::Unhandled;
  };
  unsafe { (&*handler)(ctx, item) }
}

pub fn file_picker_items_from_specs(
  specs: Vec<PickerItemSpec>,
  submit_handler: Option<PickerSubmitHandlerRef>,
) -> Vec<FilePickerItem> {
  specs
    .into_iter()
    .map(|spec| spec.into_item(submit_handler))
    .collect()
}

fn direct_picker_metadata(tracking: DirectPickerTrackingKind) -> DirectPickerItemMetadata {
  DirectPickerItemMetadata {
    match_indices:          Arc::from([]),
    primary_match_ranges:   Arc::from([]),
    secondary_match_ranges: Arc::from([]),
    preview_match_ranges:   Arc::from([]),
    tracking,
  }
}

fn file_depth_within_limit(root: &Path, path: &Path, max_depth: Option<usize>) -> bool {
  let Some(max_depth) = max_depth else {
    return true;
  };
  let Ok(relative) = path.strip_prefix(root) else {
    return false;
  };
  relative.components().count() <= max_depth
}

fn path_matches_scan_source(path: &Path, root: &Path, scan_source: &FilePickerScanSource) -> bool {
  let Ok(relative) = path.strip_prefix(root) else {
    return false;
  };

  if !file_depth_within_limit(root, path, scan_source.options.max_depth) {
    return false;
  }

  if let Some(extensions) = &scan_source.extensions {
    let extension = relative
      .extension()
      .and_then(|extension| extension.to_str())
      .map(|value| value.to_ascii_lowercase());
    if extension
      .as_deref()
      .is_none_or(|candidate| !extensions.iter().any(|value| value == candidate))
    {
      return false;
    }
  }

  if let Some(filter) = &scan_source.path_filter
    && !filter(path, relative)
  {
    return false;
  }

  true
}

fn picker_spec_for_fff_file_hit(
  root: &Path,
  query: &str,
  path: &Path,
  location: Option<fff_search::Location>,
) -> Option<PickerItemSpec> {
  let display = display_relative_path(path, root);
  let (primary, secondary) = split_picker_path_display(&display);
  let (primary_match_ranges, secondary_match_ranges) = map_match_ranges_to_row_fields(
    &display,
    &primary,
    &secondary,
    &compute_match_ranges_for_text(query, &display),
  );

  let tracking = DirectPickerTrackingKind::FffFileSearch {
    root:  root.to_path_buf(),
    query: query.to_string(),
  };
  let metadata = DirectPickerItemMetadata {
    match_indices: Arc::from([]),
    primary_match_ranges,
    secondary_match_ranges,
    preview_match_ranges: Arc::from([]),
    tracking,
  };
  let row_data = FilePickerRowData {
    kind:       FilePickerRowKind::Generic,
    severity:   None,
    primary,
    secondary,
    tertiary:   String::new(),
    quaternary: String::new(),
    line:       0,
    column:     0,
    depth:      0,
  };

  let spec = match location {
    Some(fff_search::Location::Line(line)) => {
      let line = line.max(1) as usize - 1;
      PickerItemSpec::location(display.clone(), path, 0, line, None).with_preview_line(line)
    },
    Some(fff_search::Location::Position { line, col }) => {
      let line = line.max(1) as usize - 1;
      let col = col.max(1) as usize - 1;
      PickerItemSpec::location(display.clone(), path, 0, line, Some(col)).with_preview_line(line)
    },
    Some(fff_search::Location::Range { start, .. }) => {
      let line = start.0.max(1) as usize - 1;
      let col = start.1.max(1) as usize - 1;
      PickerItemSpec::location(display, path, 0, line, Some(col)).with_preview_line(line)
    },
    None => PickerItemSpec::file(root, path)?,
  };

  Some(spec.with_row_data(row_data).with_payload(metadata))
}

fn sanitize_picker_grep_excerpt(line: &str) -> String {
  line.trim_end_matches(['\r', '\n']).replace('\t', " ")
}

fn grep_suggestion_header_spec(root: &Path, query: &str, path: &Path) -> PickerItemSpec {
  let display = display_relative_path(path, root);
  let (primary, secondary) = split_picker_path_display(&display);
  let metadata = direct_picker_metadata(DirectPickerTrackingKind::FffGrep {
    root:  root.to_path_buf(),
    query: query.to_string(),
  });

  PickerItemSpec::custom(display)
    .with_selectable(false)
    .with_row_data(FilePickerRowData {
      kind:       FilePickerRowKind::LiveGrepHeader,
      severity:   None,
      primary,
      secondary,
      tertiary:   String::new(),
      quaternary: String::new(),
      line:       0,
      column:     0,
      depth:      0,
    })
    .with_payload(metadata)
}

fn grep_suggestion_match_spec(
  root: &Path,
  query: &str,
  matched: &fff_backend::FffGrepMatch,
) -> PickerItemSpec {
  let snippet = sanitize_picker_grep_excerpt(&matched.line_text);
  let preview_match_ranges: Vec<(usize, usize)> = matched
    .match_bytes
    .iter()
    .map(|(start, end)| {
      (
        line_byte_to_char_idx(&snippet, *start, false),
        line_byte_to_char_idx(&snippet, *end, true),
      )
    })
    .filter(|(start, end)| end > start)
    .collect();
  let metadata = DirectPickerItemMetadata {
    match_indices:          Arc::from([]),
    primary_match_ranges:   Arc::from(preview_match_ranges.clone()),
    secondary_match_ranges: Arc::from([]),
    preview_match_ranges:   Arc::from(preview_match_ranges.clone()),
    tracking:               DirectPickerTrackingKind::FffGrep {
      root:  root.to_path_buf(),
      query: query.to_string(),
    },
  };

  PickerItemSpec::location(
    display_relative_path(matched.path.as_path(), root),
    matched.path.clone(),
    0,
    matched.line_number_one_based.saturating_sub(1),
    preview_match_ranges.first().map(|(start, _)| *start),
  )
  .with_preview_line(matched.line_number_one_based.saturating_sub(1))
  .with_row_data(FilePickerRowData {
    kind:       FilePickerRowKind::LiveGrepMatch,
    severity:   None,
    primary:    snippet,
    secondary:  String::new(),
    tertiary:   String::new(),
    quaternary: String::new(),
    line:       matched.line_number_one_based,
    column:     preview_match_ranges.first().map(|(start, _)| start + 1).unwrap_or(1),
    depth:      0,
  })
  .with_payload(metadata)
}

fn build_fff_grep_suggestion_specs(root: &Path, query: &str, limit: usize) -> Vec<PickerItemSpec> {
  let cancel = AtomicBool::new(false);
  let response = match fff_backend::search_grep_with_mode(
    root,
    query,
    true,
    limit,
    fff_search::GrepMode::PlainText,
    &cancel,
  ) {
    Ok(response) => response,
    Err(_) => return Vec::new(),
  };

  let mut grouped: HashMap<PathBuf, Vec<fff_backend::FffGrepMatch>> = HashMap::new();
  let mut file_order = Vec::new();
  for matched in response.matches {
    let entry = grouped.entry(matched.path.clone()).or_insert_with(|| {
      file_order.push(matched.path.clone());
      Vec::new()
    });
    entry.push(matched);
  }

  let mut specs = Vec::new();
  for path in file_order {
    let Some(matches) = grouped.remove(&path) else {
      continue;
    };
    specs.push(grep_suggestion_header_spec(root, query, &path));
    for matched in matches {
      if specs.len() >= limit {
        break;
      }
      specs.push(grep_suggestion_match_spec(root, query, &matched));
    }
    if specs.len() >= limit {
      break;
    }
  }
  specs
}

fn build_fff_file_search_specs<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  root: &Path,
  query: &str,
  scan_source: &FilePickerScanSource,
) -> Vec<PickerItemSpec> {
  ctx.file_picker_mut().status_banner = None;
  ctx.file_picker_mut().search_mode = FilePickerSearchMode::None;

  let current_file = ctx.file_path().filter(|path| path.starts_with(root));
  let overfetch_limit = fff_backend::file_search_overfetch_limit(scan_source.max_results);
  let response = match fff_backend::search_files(root, query, current_file, overfetch_limit) {
    Ok(response) => response,
    Err(_) => return Vec::new(),
  };

  let hits: Vec<_> = response
    .hits
    .into_iter()
    .filter(|hit| path_matches_scan_source(hit.path.as_path(), root, scan_source))
    .filter_map(|hit| picker_spec_for_fff_file_hit(root, query, hit.path.as_path(), hit.location))
    .take(scan_source.max_results)
    .collect();

  if !hits.is_empty() || query.trim().is_empty() {
    return hits;
  }

  let suggestions = build_fff_grep_suggestion_specs(root, query, scan_source.max_results);
  if !suggestions.is_empty() {
    ctx.file_picker_mut().status_banner = Some(FilePickerStatusBanner {
      kind: FilePickerStatusBannerKind::Warning,
      text: "No file matches — showing content matches".to_string(),
    });
  }
  suggestions
}

fn open_picker_from_builder_items<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  kind: FilePickerKind,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  items: Vec<PickerItemSpec>,
  submit_handler: Option<Arc<PickerRuntimeSubmit<Ctx>>>,
) {
  let runtime_session = submit_handler.map(|submit| {
    register_picker_runtime_session(ctx, PickerRuntimeSession {
      query:  None,
      submit: Some(Box::new(move |ctx: &mut Ctx, item: &FilePickerItem| {
        submit(ctx, item)
      })),
    })
  });
  let mut state = base_picker_state(ctx, kind, title, open_split);
  state.root = root;
  state.runtime_session = runtime_session;
  replace_picker_items(
    &mut state,
    file_picker_items_from_specs(items, runtime_session.map(PickerSubmitHandlerRef::Runtime)),
    0,
  );

  *ctx.file_picker_mut() = state;
  ctx.request_render();
}

fn open_picker_from_builder_query<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  kind: FilePickerKind,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  initial_query: String,
  query: Arc<PickerQuerySource<Ctx>>,
  submit_handler: Option<Arc<PickerRuntimeSubmit<Ctx>>>,
) {
  let runtime_session = register_picker_runtime_session(ctx, PickerRuntimeSession {
    query:  Some(Box::new(move |ctx: &mut Ctx, input: &str| {
      query(ctx, input)
    })),
    submit: submit_handler.map(|submit| {
      Box::new(move |ctx: &mut Ctx, item: &FilePickerItem| submit(ctx, item))
        as Box<PickerRuntimeSubmit<Ctx>>
    }),
  });

  let mut state = base_picker_state(ctx, kind, title, open_split);
  state.root = root;
  state.query_mode = FilePickerQueryMode::Dynamic;
  state.custom_query_handler = Some(PickerQueryHandlerRef::Runtime(runtime_session));
  state.query = initial_query.clone();
  state.cursor = state.query.len();
  state.runtime_session = Some(runtime_session);
  prepare_dynamic_query_change(&mut state);
  start_preview_worker(&mut state);

  *ctx.file_picker_mut() = state;
  notify_file_picker_query_changed(ctx, &initial_query);
  ctx.file_picker_selection_changed();
  ctx.request_render();
}

fn open_picker_from_builder_files<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  kind: FilePickerKind,
  title: &str,
  root: PathBuf,
  open_split: Option<SplitAxis>,
  mut scan_source: FilePickerScanSource,
  submit_handler: Option<Arc<PickerRuntimeSubmit<Ctx>>>,
) {
  if kind == FilePickerKind::Generic {
    let query_root = root.clone();
    let query_scan_source = scan_source.clone();
    open_picker_from_builder_query(
      ctx,
      kind,
      title,
      root,
      open_split,
      String::new(),
      Arc::new(move |ctx: &mut Ctx, query: &str| {
        build_fff_file_search_specs(ctx, query_root.as_path(), query, &query_scan_source)
      }),
      submit_handler,
    );
    return;
  }

  let runtime_session = submit_handler.map(|submit| {
    register_picker_runtime_session(ctx, PickerRuntimeSession {
      query:  None,
      submit: Some(Box::new(move |ctx: &mut Ctx, item: &FilePickerItem| {
        submit(ctx, item)
      })),
    })
  });
  scan_source.submit_handler = runtime_session.map(PickerSubmitHandlerRef::Runtime);

  let mut state = base_picker_state(ctx, kind, title, open_split);
  state.preview = FilePickerPreview::Message("Scanning files…".to_string());
  state.runtime_session = runtime_session;
  state.scan_source = Some(scan_source);
  start_preview_worker(&mut state);
  start_scan(&mut state, root);
  poll_scan_results(&mut state);

  *ctx.file_picker_mut() = state;
  ctx.file_picker_selection_changed();
  ctx.request_render();
}

pub fn set_file_picker_query_text(state: &mut FilePickerState, query: &str) -> bool {
  let old_query = state.query.clone();
  state.query = query.to_string();
  state.cursor = state.query.len();
  if state.query_mode == FilePickerQueryMode::Dynamic {
    prepare_dynamic_query_change(state);
    true
  } else {
    handle_query_change(state, &old_query);
    refresh_preview(state);
    false
  }
}

fn reset_picker_matcher(state: &mut FilePickerState) {
  state.matcher = new_matcher(state.wake_tx.clone());
  state.matcher_running = false;
  state.direct_items.clear();
  state.use_direct_items = false;
}

fn prepare_dynamic_query_change(state: &mut FilePickerState) {
  reset_picker_matcher(state);
  state.selected = None;
  state.hovered = None;
  state.list_offset = 0;
  state.preview_scroll = 0;
  state.error = None;
  state.status_banner = None;
  state.dynamic_running = !state.query.trim().is_empty();
  state.preview_path = None;
  state.preview_focus_line = None;
  state.preview_pending_id = None;
  state.preview_latest_request.store(0, Ordering::Relaxed);
  state.preview = FilePickerPreview::Message(if state.dynamic_running {
    "Searching…".to_string()
  } else {
    "Type to search".to_string()
  });
}

pub fn notify_file_picker_query_changed<Ctx: DefaultContext>(ctx: &mut Ctx, query: &str) {
  let handler = ctx.file_picker().custom_query_handler;
  if let Some(handler) = handler {
    match handler {
      PickerQueryHandlerRef::Runtime(session_id) => {
        if let Some(items) = run_picker_runtime_query(ctx, session_id, query) {
          replace_file_picker_items(
            ctx,
            file_picker_items_from_specs(items, Some(PickerSubmitHandlerRef::Runtime(session_id))),
            0,
          );
          return;
        }
      },
    }
  }
  ctx.file_picker_query_changed(query);
}

fn finalize_query_edit<Ctx: DefaultContext>(ctx: &mut Ctx, old_query: &str) {
  let mut changed_query = None;
  {
    let picker = ctx.file_picker_mut();
    if picker.query_mode == FilePickerQueryMode::Dynamic {
      prepare_dynamic_query_change(picker);
      changed_query = Some(picker.query.clone());
    } else {
      handle_query_change(picker, old_query);
      refresh_preview(picker);
    }
  }
  ctx.file_picker_selection_changed();
  if let Some(query) = changed_query {
    notify_file_picker_query_changed(ctx, &query);
  }
}

pub fn replace_file_picker_items<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  let picker = ctx.file_picker_mut();
  replace_picker_items(picker, items, initial_cursor);
  ctx.file_picker_selection_changed();
  ctx.request_render();
}

fn replace_picker_items(
  state: &mut FilePickerState,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  state.dynamic_running = false;
  state.preview = FilePickerPreview::Message("No matches".to_string());
  if state.preview_req_tx.is_none() || state.preview_res_rx.is_none() {
    start_preview_worker(state);
  }

  state.direct_items.clear();
  state.use_direct_items = items
    .iter()
    .any(|item| item.payload::<DirectPickerItemMetadata>().is_some());

  state.matcher.restart(true);
  state.matcher.pattern.reparse(
    0,
    state.query.as_str(),
    CaseMatching::Smart,
    Normalization::Smart,
    false,
  );

  if state.use_direct_items {
    state.direct_items = items.into_iter().map(Arc::new).collect();
    state.matcher_running = false;
  } else {
    let injector = state.matcher.injector();
    for item in items {
      inject_item(&injector, item);
    }
    drop(injector);
    let _ = refresh_matcher_state(state);
  }

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

pub fn replace_file_picker_items_preserving_selection<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  let selected_item_stable_id = ctx.file_picker().selected_item_stable_id();
  let picker = ctx.file_picker_mut();
  replace_picker_items(picker, items, initial_cursor);
  let _ = restore_selection_by_stable_id(picker, selected_item_stable_id);
  refresh_preview(picker);
  ctx.file_picker_selection_changed();
  ctx.request_render();
}

pub fn replace_file_picker_items_preserving_selection_and_viewport<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  items: Vec<FilePickerItem>,
  initial_cursor: usize,
) {
  let selected_item_stable_id = ctx.file_picker().selected_item_stable_id();
  let list_offset = ctx.file_picker().list_offset;
  let preview_scroll = ctx.file_picker().preview_scroll;
  {
    let picker = ctx.file_picker_mut();
    replace_picker_items(picker, items, initial_cursor);
    let _ = restore_selection_by_stable_id(picker, selected_item_stable_id);

    let visible = picker.list_visible.max(1);
    let max_offset = picker.matched_count().saturating_sub(visible);
    picker.list_offset = list_offset.min(max_offset);

    refresh_preview(picker);
  }

  ctx.file_picker_selection_changed();

  let picker = ctx.file_picker_mut();
  picker.preview_scroll = match picker.preview_navigation_mode() {
    FilePickerPreviewNavigationMode::Scrollable => {
      preview_scroll.min(picker.preview_line_count().saturating_sub(1))
    },
    _ => 0,
  };
  ctx.request_render();
}

fn base_picker_state<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  kind: FilePickerKind,
  title: &str,
  open_split: Option<SplitAxis>,
) -> FilePickerState {
  if let Some(runtime_session) = ctx.file_picker().runtime_session {
    drop_picker_runtime_session(ctx, runtime_session);
  }
  let show_preview = ctx.file_picker().show_preview;
  let options = ctx.file_picker().options.clone();
  let wake_tx = ctx.file_picker().wake_tx.clone();
  let syntax_loader = ctx.file_picker().syntax_loader.clone();
  if let Some(cancel) = ctx.file_picker().scan_cancel.as_ref() {
    cancel.store(true, Ordering::Relaxed);
  }

  let mut state = FilePickerState::default();
  state.active = true;
  state.kind = kind;
  state.title = title.to_string();
  state.show_preview = show_preview;
  state.open_split = open_split;
  state.options = options;
  state.wake_tx = wake_tx.clone();
  state.syntax_loader = syntax_loader;
  state.matcher = new_matcher(wake_tx);
  state
}

pub fn close_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let runtime_session = ctx.file_picker().runtime_session;
  let picker = ctx.file_picker_mut();
  if let Some(cancel) = picker.scan_cancel.as_ref() {
    cancel.store(true, Ordering::Relaxed);
  }
  picker.active = false;
  picker.error = None;
  picker.status_banner = None;
  picker.search_mode = FilePickerSearchMode::None;
  picker.hovered = None;
  picker.preview_scroll = 0;
  picker.open_split = None;
  picker.preview_path = None;
  picker.preview_focus_line = None;
  picker.preview = FilePickerPreview::Empty;
  picker.scanning = false;
  picker.matcher_running = false;
  picker.query_mode = FilePickerQueryMode::Static;
  picker.custom_query_handler = None;
  picker.dynamic_running = false;
  picker.runtime_session = None;
  picker.scan_source = None;
  picker.preview_req_tx = None;
  picker.preview_res_rx = None;
  picker.preview_pending_id = None;
  picker.preview_latest_request.store(0, Ordering::Relaxed);
  picker.scan_rx = None;
  picker.scan_cancel = None;
  picker.direct_items.clear();
  picker.use_direct_items = false;
  let _ = picker;
  if let Some(runtime_session) = runtime_session {
    drop_picker_runtime_session(ctx, runtime_session);
  }
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
  ctx.file_picker_selection_changed();
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
      ctx.file_picker_selection_changed();
      ctx.request_render();
      true
    },
    Key::End => {
      let picker = ctx.file_picker_mut();
      picker.selected = picker.matched_count().checked_sub(1);
      snap_selection_to_selectable(picker, -1);
      normalize_selection_and_scroll(picker);
      refresh_preview(picker);
      ctx.file_picker_selection_changed();
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
      let mut old_query = None;
      {
        let picker = ctx.file_picker_mut();
        if picker.cursor > 0 && picker.cursor <= picker.query.len() {
          old_query = Some(picker.query.clone());
          let prev = prev_char_boundary(&picker.query, picker.cursor);
          picker.query.replace_range(prev..picker.cursor, "");
          picker.cursor = prev;
        }
      }
      if let Some(old_query) = old_query {
        finalize_query_edit(ctx, &old_query);
      }
      ctx.request_render();
      true
    },
    Key::Delete => {
      let mut old_query = None;
      {
        let picker = ctx.file_picker_mut();
        if picker.cursor < picker.query.len() {
          old_query = Some(picker.query.clone());
          let next = next_char_boundary(&picker.query, picker.cursor);
          picker.query.replace_range(picker.cursor..next, "");
        }
      }
      if let Some(old_query) = old_query {
        finalize_query_edit(ctx, &old_query);
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
      let old_query = {
        let picker = ctx.file_picker_mut();
        let old_query = picker.query.clone();
        picker.query.insert(picker.cursor, ch);
        picker.cursor += ch.len_utf8();
        old_query
      };
      finalize_query_edit(ctx, &old_query);
      ctx.request_render();
      true
    },
    _ => true,
  }
}

fn track_direct_picker_selection(item: &FilePickerItem) {
  let Some(metadata) = item.payload::<DirectPickerItemMetadata>() else {
    return;
  };
  match &metadata.tracking {
    DirectPickerTrackingKind::FffFileSearch { root, query } => {
      fff_backend::track_file_selection(root.as_path(), query, item.absolute.as_path());
    },
    DirectPickerTrackingKind::FffGrep { root, query } => {
      fff_backend::track_grep_selection(root.as_path(), query, item.absolute.as_path());
    },
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
      track_direct_picker_selection(&item);
      close_file_picker(ctx);
    },
    FilePickerItemAction::GroupHeader { .. } => {},
    FilePickerItemAction::SwitchBuffer { buffer_id } => {
      if !ctx
        .editor()
        .set_active_buffer_preserving_terminal(*buffer_id)
      {
        ctx.push_warning("buffer_picker", "selected buffer is no longer available");
        return;
      }
      track_direct_picker_selection(&item);
      close_file_picker(ctx);
    },
    FilePickerItemAction::RestoreJump {
      buffer_id,
      selection,
      active_cursor,
    } => {
      if !ctx
        .editor()
        .set_active_buffer_preserving_terminal(*buffer_id)
      {
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
      track_direct_picker_selection(&item);
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
      track_direct_picker_selection(&item);
      close_file_picker(ctx);
    },
    FilePickerItemAction::Custom { handler, .. } => {
      let result = match handler {
        PickerSubmitHandlerRef::Missing => PickerSubmitResult::Unhandled,
        PickerSubmitHandlerRef::Runtime(session_id) => {
          run_picker_runtime_submit(ctx, *session_id, &item)
        },
      };

      match result {
        PickerSubmitResult::Unhandled => {
          ctx.push_warning("file_picker", "picker action handler not found");
          ctx.request_render();
        },
        PickerSubmitResult::KeepOpen => {
          ctx.request_render();
        },
        PickerSubmitResult::Close => {
          close_file_picker(ctx);
        },
      }
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
  if picker.preview_navigation_mode() != FilePickerPreviewNavigationMode::Scrollable {
    if picker.preview_scroll != 0 {
      picker.preview_scroll = 0;
      ctx.request_render();
    }
    return;
  }
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
  if picker.preview_navigation_mode() != FilePickerPreviewNavigationMode::Scrollable {
    return;
  }
  let current = picker.preview_scroll;
  let target = if delta < 0 {
    current.saturating_sub(delta.unsigned_abs())
  } else {
    current.saturating_add(delta as usize)
  };
  set_file_picker_preview_offset(ctx, target, visible_rows);
}

fn display_relative_path(path: &Path, cwd: &Path) -> String {
  path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

fn line_byte_to_char_idx(line: &str, byte_idx: usize, round_up: bool) -> usize {
  let clamped = clamp_preview_char_boundary(line, byte_idx, round_up);
  line[..clamped].chars().count()
}

pub(crate) fn split_picker_path_display(display: &str) -> (String, String) {
  let path = Path::new(display);
  let primary = path
    .file_name()
    .and_then(|name| name.to_str())
    .filter(|name| !name.is_empty())
    .unwrap_or(display)
    .to_string();
  let secondary = path
    .parent()
    .and_then(|parent| parent.to_str())
    .filter(|parent| !parent.is_empty() && *parent != ".")
    .unwrap_or_default()
    .to_string();
  (primary, secondary)
}

pub(crate) fn query_match_tokens(query: &str) -> Vec<String> {
  query
    .trim()
    .split(|ch: char| ch.is_whitespace() || matches!(ch, '/' | '\\' | ':'))
    .filter(|token| !token.is_empty())
    .map(|token| token.to_ascii_lowercase())
    .collect()
}

pub(crate) fn char_count(text: &str) -> usize {
  text.chars().count()
}

pub(crate) fn contiguous_char_ranges(indices: &[usize]) -> Vec<(usize, usize)> {
  if indices.is_empty() {
    return Vec::new();
  }
  let mut sorted = indices.to_vec();
  sorted.sort_unstable();
  sorted.dedup();

  let mut out = Vec::new();
  let mut start = sorted[0];
  let mut end = start + 1;
  for index in sorted.into_iter().skip(1) {
    if index == end {
      end += 1;
    } else {
      out.push((start, end));
      start = index;
      end = index + 1;
    }
  }
  out.push((start, end));
  out
}

fn substring_char_range(text: &str, needle: &str) -> Option<(usize, usize)> {
  let lower_text = text.to_ascii_lowercase();
  let start = lower_text.find(needle)?;
  let start_chars = lower_text[..start].chars().count();
  let len_chars = lower_text[start..start + needle.len()].chars().count();
  Some((start_chars, start_chars + len_chars))
}

fn subsequence_char_indices(text: &str, needle: &str) -> Option<Vec<usize>> {
  if needle.is_empty() {
    return Some(Vec::new());
  }

  let mut out = Vec::new();
  let mut needle_chars = needle.chars();
  let mut next = needle_chars.next()?;
  for (index, ch) in text.chars().enumerate() {
    if ch == next {
      out.push(index);
      if let Some(next_char) = needle_chars.next() {
        next = next_char;
      } else {
        return Some(out);
      }
    }
  }
  None
}

pub(crate) fn merge_match_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
  if ranges.is_empty() {
    return ranges;
  }
  ranges.sort_unstable_by_key(|range| range.0);
  let mut merged = Vec::with_capacity(ranges.len());
  let mut current = ranges[0];
  for range in ranges.into_iter().skip(1) {
    if range.0 <= current.1 {
      current.1 = current.1.max(range.1);
    } else {
      merged.push(current);
      current = range;
    }
  }
  merged.push(current);
  merged
}

pub(crate) fn compute_match_ranges_for_text(query: &str, text: &str) -> Vec<(usize, usize)> {
  if query.trim().is_empty() || text.is_empty() {
    return Vec::new();
  }

  let lower_text = text.to_ascii_lowercase();
  let mut ranges = Vec::new();
  for token in query_match_tokens(query) {
    if let Some(range) = substring_char_range(text, &token) {
      ranges.push(range);
      continue;
    }
    if let Some(indices) = subsequence_char_indices(&lower_text, &token) {
      ranges.extend(contiguous_char_ranges(&indices));
    }
  }
  merge_match_ranges(ranges)
}

pub(crate) fn map_match_ranges_to_row_fields(
  display: &str,
  primary: &str,
  secondary: &str,
  ranges: &[(usize, usize)],
) -> (Arc<[(usize, usize)]>, Arc<[(usize, usize)]>) {
  if ranges.is_empty() {
    return (Arc::from([]), Arc::from([]));
  }

  let filename_start = char_count(display).saturating_sub(char_count(primary));
  let secondary_end = if secondary.is_empty() {
    0
  } else {
    filename_start.saturating_sub(1)
  };

  let mut primary_ranges = Vec::new();
  let mut secondary_ranges = Vec::new();
  for (start, end) in ranges.iter().copied() {
    if end <= secondary_end {
      secondary_ranges.push((start, end.min(secondary_end)));
      continue;
    }
    if start < secondary_end {
      secondary_ranges.push((start, secondary_end));
    }
    if end > filename_start {
      primary_ranges.push((start.max(filename_start) - filename_start, end - filename_start));
    }
  }

  (merge_match_ranges(primary_ranges).into(), merge_match_ranges(secondary_ranges).into())
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
  if title.starts_with("VCS Diff Picker") {
    return FilePickerKind::VcsDiff;
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

pub fn file_picker_item_selectable(item: &FilePickerItem) -> bool {
  item.is_selectable()
}

pub fn file_picker_row_data_for_kind(
  kind: FilePickerKind,
  item: &FilePickerItem,
) -> FilePickerRowData {
  if let Some(row_data) = &item.row_data {
    return row_data.clone();
  }

  match kind {
    FilePickerKind::Diagnostics => parse_diagnostics_row(item.display.as_str(), item.icon.as_str()),
    FilePickerKind::Symbols => parse_symbols_row(item.display.as_str()),
    FilePickerKind::LiveGrep => parse_live_grep_row(item),
    FilePickerKind::VcsDiff => {
      FilePickerRowData {
        kind:       FilePickerRowKind::VcsDiffHunk,
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
    FilePickerKind::Generic => {
      let (primary, secondary) = split_picker_path_display(&item.display);
      FilePickerRowData {
        kind:       FilePickerRowKind::Generic,
        severity:   None,
        primary,
        secondary,
        tertiary:   String::new(),
        quaternary: String::new(),
        line:       0,
        column:     0,
        depth:      0,
      }
    },
  }
}

pub fn file_picker_row_data(title: &str, item: &FilePickerItem) -> FilePickerRowData {
  file_picker_row_data_for_kind(file_picker_kind_from_title(title), item)
}

fn picker_root<Ctx: DefaultContext>(_ctx: &Ctx) -> PathBuf {
  _ctx.workspace_root()
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
  state.query_mode = FilePickerQueryMode::Static;
  state.custom_query_handler = None;
  state.dynamic_running = false;
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

  let scan_source = state.scan_source.clone();
  let walk_config = scan_source
    .as_ref()
    .map(|source| &source.options)
    .unwrap_or(&state.options);
  let mut walker = build_file_walk_builder(&root, walk_config).build();
  let timeout = Instant::now() + Duration::from_millis(WARMUP_SCAN_BUDGET_MS);
  let mut scanned = 0usize;
  let mut hit_timeout = false;
  let max_results = scan_source
    .as_ref()
    .map(|source| source.max_results)
    .unwrap_or(MAX_SCAN_ITEMS)
    .min(MAX_SCAN_ITEMS);

  for entry in &mut walker {
    if cancel.load(Ordering::Relaxed) {
      break;
    }

    let entry = match entry {
      Ok(entry) => entry,
      Err(_) => continue,
    };
    let Some(item) = entry_to_picker_item(entry, &root, scan_source.as_ref()) else {
      continue;
    };
    inject_item(&injector, item);
    scanned += 1;

    if scanned >= max_results {
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
    max_results.saturating_sub(scanned),
    generation,
    scan_tx,
    cancel,
    injector,
    state.wake_tx.clone(),
    scan_source,
  );
}

pub(crate) fn build_file_walk_builder(
  root: &Path,
  options: &FilePickerOptions,
) -> ignore::WalkBuilder {
  let absolute_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
  let deduplicate_links = options.deduplicate_links;
  let mut walk_builder = ignore::WalkBuilder::new(root);
  walk_builder
    .hidden(options.hidden)
    .parents(options.parents)
    .ignore(options.ignore)
    .follow_links(options.follow_symlinks)
    .git_ignore(options.git_ignore)
    .git_global(options.git_global)
    .git_exclude(options.git_exclude)
    .sort_by_file_name(|name1, name2| name1.cmp(name2))
    .max_depth(options.max_depth)
    .filter_entry(move |entry| filter_picker_entry(entry, &absolute_root, deduplicate_links))
    .add_custom_ignore_filename(the_loader::config_dir().join("ignore"))
    .add_custom_ignore_filename(".helix/ignore")
    .types(excluded_types());
  walk_builder
}

fn entry_to_picker_item(
  entry: DirEntry,
  root: &Path,
  scan_source: Option<&FilePickerScanSource>,
) -> Option<FilePickerItem> {
  if !entry
    .file_type()
    .is_some_and(|file_type| file_type.is_file())
  {
    return None;
  }

  let path = entry.into_path();
  let rel = path.strip_prefix(root).ok()?;
  if let Some(scan_source) = scan_source {
    if let Some(extensions) = &scan_source.extensions {
      let extension = rel
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|value| value.to_ascii_lowercase());
      if extension
        .as_deref()
        .is_none_or(|candidate| !extensions.iter().any(|value| value == candidate))
      {
        return None;
      }
    }
    if let Some(filter) = &scan_source.path_filter
      && !filter(&path, rel)
    {
      return None;
    }
  }

  let mut display = rel.to_string_lossy().to_string();
  if std::path::MAIN_SEPARATOR != '/' {
    display = display.replace(std::path::MAIN_SEPARATOR, "/");
  }
  let icon = file_picker_icon_name_for_path(rel).to_string();
  let submit_handler = scan_source.and_then(|source| source.submit_handler);
  let preview_path = scan_source
    .map(|source| source.preview.then_some(path.clone()))
    .unwrap_or_else(|| Some(path.clone()));

  Some(FilePickerItem {
    action: submit_handler.map_or_else(
      || FilePickerItemAction::OpenFile(path.clone()),
      |handler| {
        FilePickerItemAction::Custom {
          handler,
          selectable: true,
        }
      },
    ),
    absolute: path,
    display,
    icon,
    is_dir: false,
    display_path: true,
    preview_path,
    preview_line: None,
    preview_col: None,
    row_data: None,
    preview: None,
    payload: None,
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
  match icon {
    "folder" => "",
    "folder_open" => "",
    "folder_search" => "",
    "archive" => "",
    "book" => "󰂺",
    "c" => "",
    "cpp" => "",
    "css" => "",
    "database" => "",
    "copilot" | "copilot_init" | "copilot_error" | "copilot_disabled" => "",
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
    "pi" => "π",
    "python" => "",
    "sass" => "",
    "settings" => "",
    "swift" => "",
    "supermaven" | "supermaven_init" | "supermaven_error" | "supermaven_disabled" => "",
    "terminal" => "",
    "tool_hammer" => "󰛶",
    "typescript" => "",
    _ if is_dir => "",
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
  scan_source: Option<FilePickerScanSource>,
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
      let Some(item) = entry_to_picker_item(entry, &root, scan_source.as_ref()) else {
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
  let selected_item_stable_id = state.selected_item_stable_id();
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
    let _ = restore_selection_by_stable_id(state, selected_item_stable_id);
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
  if state.use_direct_items {
    state.matcher_running = false;
    return clamp_selection_and_offsets(state);
  }

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

pub fn refresh_file_picker_preview(state: &mut FilePickerState) {
  refresh_preview(state);
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

fn restore_selection_by_stable_id(state: &mut FilePickerState, stable_id: Option<u64>) -> bool {
  let Some(stable_id) = stable_id else {
    return false;
  };
  let Some(next_selected) = state.matched_index_for_stable_id(stable_id) else {
    return false;
  };
  if state.selected == Some(next_selected) {
    return false;
  }
  state.selected = Some(next_selected);
  normalize_selection_and_scroll(state);
  true
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

  if let Some(preview) = item.preview.clone() {
    state.preview_path = None;
    state.preview = preview;
    set_preview_focus_line(state, item.preview_line);
    state.preview_pending_id = None;
    state.preview_latest_request.store(0, Ordering::Relaxed);
    return;
  }

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
  if let Some(preview) = state
    .preview_cache
    .get(preview_target)
    .filter(preview_is_cacheable)
  {
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
      if preview_is_cacheable(&preview) {
        state
          .preview_cache
          .insert(preview_target.clone(), preview.clone());
      }
      state.preview = preview;
      set_preview_focus_line(state, preview_focus_line);
      state.preview_pending_id = None;
      state.preview_latest_request.store(0, Ordering::Relaxed);
    },
    PreviewBuild::Source(mut source_preview) => {
      let base_preview = FilePickerPreview::Source(source_preview.source.clone());
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
      state.preview = preview;
      set_preview_focus_line(state, preview_focus_line);
      state.preview_pending_id = None;
      state.preview_latest_request.store(0, Ordering::Relaxed);
    },
  }
}

fn set_preview_focus_line(state: &mut FilePickerState, line: Option<usize>) {
  state.preview_focus_line = line;
  if state.preview_navigation_mode() != FilePickerPreviewNavigationMode::Scrollable {
    state.preview_scroll = 0;
    return;
  }
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
    FilePickerPreview::Source(source) => (focus_line < source.total_lines()).then_some(focus_line),
    FilePickerPreview::Text(_) | FilePickerPreview::Message(_) => Some(focus_line),
    FilePickerPreview::VcsDiff(_) => Some(focus_line),
    FilePickerPreview::Empty => None,
  }
}

fn preview_contains_focus_line(preview: &FilePickerPreview, focus_line: Option<usize>) -> bool {
  let Some(focus_line) = focus_line else {
    return true;
  };
  match preview {
    FilePickerPreview::Source(source) => focus_line < source.total_lines(),
    FilePickerPreview::Text(text) => focus_line < text.lines().count().max(1),
    FilePickerPreview::Message(message) => focus_line < message.lines().count().max(1),
    FilePickerPreview::VcsDiff(preview) => focus_line < preview.rows.len().max(1),
    FilePickerPreview::Empty => false,
  }
}

fn preview_is_cacheable(preview: &FilePickerPreview) -> bool {
  !matches!(preview, FilePickerPreview::Source(_))
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

pub fn file_picker_preview_line_segments(
  line: &str,
  line_start: usize,
  highlights: &[(Highlight, Range<usize>)],
) -> Vec<FilePickerPreviewSegment> {
  if line.is_empty() {
    return vec![FilePickerPreviewSegment {
      text:         String::new(),
      highlight_id: None,
      is_match:     false,
      change_kind:  None,
    }];
  }

  if highlights.is_empty() {
    return vec![FilePickerPreviewSegment {
      text:         line.to_string(),
      highlight_id: None,
      is_match:     false,
      change_kind:  None,
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
      change_kind: None,
    });
  }

  if segments.is_empty() {
    segments.push(FilePickerPreviewSegment {
      text:         line.to_string(),
      highlight_id: None,
      is_match:     false,
      change_kind:  None,
    });
  }

  segments
}

fn apply_match_to_preview_segments(
  segments: Vec<FilePickerPreviewSegment>,
  match_ranges: &[(usize, usize)],
) -> Vec<FilePickerPreviewSegment> {
  if match_ranges.is_empty() {
    return segments;
  }

  let mut out = segments;
  for (match_start, match_end) in match_ranges.iter().copied() {
    if match_end <= match_start {
      continue;
    }

    let mut next = Vec::new();
    let mut segment_start_char = 0usize;
    for segment in out {
      let segment_char_len = segment.text.chars().count();
      if segment_char_len == 0 {
        next.push(segment);
        continue;
      }
      let segment_end_char = segment_start_char.saturating_add(segment_char_len);
      let overlap_start = match_start.max(segment_start_char);
      let overlap_end = match_end.min(segment_end_char);
      if overlap_start >= overlap_end {
        next.push(segment);
        segment_start_char = segment_end_char;
        continue;
      }

      let local_overlap_start = overlap_start.saturating_sub(segment_start_char);
      let local_overlap_end = overlap_end.saturating_sub(segment_start_char);

      if local_overlap_start > 0 {
        let prefix_end = preview_char_to_byte_idx(&segment.text, local_overlap_start);
        next.push(FilePickerPreviewSegment {
          text:         segment.text[..prefix_end].to_string(),
          highlight_id: segment.highlight_id,
          is_match:     segment.is_match,
          change_kind:  segment.change_kind,
        });
      }

      let overlap_start_byte = preview_char_to_byte_idx(&segment.text, local_overlap_start);
      let overlap_end_byte = preview_char_to_byte_idx(&segment.text, local_overlap_end);
      next.push(FilePickerPreviewSegment {
        text:         segment.text[overlap_start_byte..overlap_end_byte].to_string(),
        highlight_id: segment.highlight_id,
        is_match:     true,
        change_kind:  segment.change_kind,
      });

      if local_overlap_end < segment_char_len {
        next.push(FilePickerPreviewSegment {
          text:         segment.text[overlap_end_byte..].to_string(),
          highlight_id: segment.highlight_id,
          is_match:     segment.is_match,
          change_kind:  segment.change_kind,
        });
      }

      segment_start_char = segment_end_char;
    }
    out = next;
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
  let raw_overscan = overscan;
  let overscan = overscan.max(1);
  let focus_line = state.preview_focus_line;
  let focus_match_ranges = state
    .current_item()
    .and_then(|item| {
      item
        .payload::<DirectPickerItemMetadata>()
        .map(|metadata| metadata.preview_match_ranges.clone())
        .filter(|ranges| !ranges.is_empty())
        .or_else(|| item.preview_col.map(|range| Arc::from([range])))
    })
    .unwrap_or_else(|| Arc::from([]));
  let navigation_mode = state.preview_navigation_mode();

  match &state.preview {
    FilePickerPreview::Empty => {
      FilePickerPreviewWindow {
        navigation_mode,
        kind: FilePickerPreviewWindowKind::Empty,
        total_virtual_rows: 0,
        offset: 0,
        window_start: 0,
        lines: Vec::new(),
        vcs_diff: None,
      }
    },
    FilePickerPreview::Source(source) => {
      build_source_preview_window(
        source,
        navigation_mode,
        focus_line,
        focus_match_ranges.as_ref(),
        offset,
        visible_rows,
        overscan,
      )
    },
    FilePickerPreview::Text(text) => {
      build_plain_preview_window(
        text,
        FilePickerPreviewWindowKind::Text,
        navigation_mode,
        focus_line,
        focus_match_ranges.as_ref(),
        offset,
        visible_rows,
        overscan,
      )
    },
    FilePickerPreview::Message(text) => {
      build_plain_preview_window(
        text,
        FilePickerPreviewWindowKind::Message,
        FilePickerPreviewNavigationMode::Static,
        focus_line,
        focus_match_ranges.as_ref(),
        offset,
        visible_rows,
        overscan,
      )
    },
    FilePickerPreview::VcsDiff(preview) => {
      build_vcs_diff_preview_window(preview, navigation_mode, offset, visible_rows, raw_overscan)
    },
  }
}

fn build_source_preview_window(
  source: &FilePickerSourcePreview,
  navigation_mode: FilePickerPreviewNavigationMode,
  focus_line: Option<usize>,
  focus_match_ranges: &[(usize, usize)],
  offset: usize,
  visible_rows: usize,
  overscan: usize,
) -> FilePickerPreviewWindow {
  let total_virtual_rows = source.total_lines();
  if total_virtual_rows == 0 {
    return FilePickerPreviewWindow {
      navigation_mode,
      kind: FilePickerPreviewWindowKind::Source,
      total_virtual_rows: 0,
      offset: 0,
      window_start: 0,
      lines: Vec::new(),
      vcs_diff: None,
    };
  }

  match navigation_mode {
    FilePickerPreviewNavigationMode::Scrollable => {
      let max_offset = total_virtual_rows.saturating_sub(visible_rows);
      let offset = offset.min(max_offset);
      let window_start = offset.saturating_sub(overscan);
      let window_end = offset
        .saturating_add(visible_rows)
        .saturating_add(overscan)
        .min(total_virtual_rows);
      let mut lines = Vec::with_capacity(window_end.saturating_sub(window_start));
      for line_index in window_start..window_end {
        let Some(line) = source.line_text(line_index) else {
          continue;
        };
        let line_start = source.line_start(line_index).unwrap_or(0);
        let mut segments = file_picker_preview_line_segments(line, line_start, &source.highlights);
        let focused = focus_line.is_some_and(|focus| focus == line_index);
        if focused {
          segments = apply_match_to_preview_segments(segments, focus_match_ranges);
        }
        lines.push(FilePickerPreviewWindowLine {
          virtual_row: line_index,
          kind: FilePickerPreviewLineKind::Content,
          line_number: Some(line_index.saturating_add(1)),
          focused,
          marker: String::new(),
          segments,
        });
      }

      FilePickerPreviewWindow {
        navigation_mode,
        kind: FilePickerPreviewWindowKind::Source,
        total_virtual_rows,
        offset,
        window_start,
        lines,
        vcs_diff: None,
      }
    },
    FilePickerPreviewNavigationMode::Anchored | FilePickerPreviewNavigationMode::Static => {
      let target_line = focus_line
        .map(|line| line.min(total_virtual_rows.saturating_sub(1)))
        .unwrap_or(0);
      let target_content_rows = visible_rows.saturating_sub(2).max(1);
      let (window_start, window_end) =
        centered_window(target_line, total_virtual_rows, target_content_rows);
      let hidden_above = window_start;
      let hidden_below = total_virtual_rows.saturating_sub(window_end);
      let has_top_marker = hidden_above > 0;
      let has_bottom_marker = hidden_below > 0 || source.truncated_by_bytes;
      let mut lines = Vec::with_capacity(
        window_end
          .saturating_sub(window_start)
          .saturating_add(has_top_marker as usize)
          .saturating_add(has_bottom_marker as usize),
      );

      if has_top_marker {
        lines.push(FilePickerPreviewWindowLine {
          virtual_row: window_start.saturating_sub(1),
          kind:        FilePickerPreviewLineKind::TruncatedAbove,
          line_number: None,
          focused:     false,
          marker:      format!("… {} lines above", hidden_above),
          segments:    Vec::new(),
        });
      }

      for line_index in window_start..window_end {
        let Some(line) = source.line_text(line_index) else {
          continue;
        };
        let line_start = source.line_start(line_index).unwrap_or(0);
        let mut segments = file_picker_preview_line_segments(line, line_start, &source.highlights);
        let focused = focus_line.is_some_and(|focus| focus == line_index);
        if focused {
          segments = apply_match_to_preview_segments(segments, focus_match_ranges);
        }
        lines.push(FilePickerPreviewWindowLine {
          virtual_row: line_index,
          kind: FilePickerPreviewLineKind::Content,
          line_number: Some(line_index.saturating_add(1)),
          focused,
          marker: String::new(),
          segments,
        });
      }

      if has_bottom_marker {
        lines.push(FilePickerPreviewWindowLine {
          virtual_row: window_end,
          kind:        FilePickerPreviewLineKind::TruncatedBelow,
          line_number: None,
          focused:     false,
          marker:      source_bottom_marker(hidden_below, source.truncated_by_bytes),
          segments:    Vec::new(),
        });
      }

      FilePickerPreviewWindow {
        navigation_mode,
        kind: FilePickerPreviewWindowKind::Source,
        total_virtual_rows,
        offset: window_start,
        window_start,
        lines,
        vcs_diff: None,
      }
    },
  }
}

fn build_plain_preview_window(
  text: &str,
  kind: FilePickerPreviewWindowKind,
  navigation_mode: FilePickerPreviewNavigationMode,
  focus_line: Option<usize>,
  focus_match_ranges: &[(usize, usize)],
  offset: usize,
  visible_rows: usize,
  overscan: usize,
) -> FilePickerPreviewWindow {
  let mut plain_lines: Vec<&str> = text.lines().collect();
  if plain_lines.is_empty() {
    plain_lines.push("");
  }
  let total_virtual_rows = plain_lines.len();

  match navigation_mode {
    FilePickerPreviewNavigationMode::Scrollable => {
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
          change_kind:  None,
        }];
        if focused {
          segments = apply_match_to_preview_segments(segments, focus_match_ranges);
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
        navigation_mode,
        kind,
        total_virtual_rows,
        offset,
        window_start,
        lines,
        vcs_diff: None,
      }
    },
    FilePickerPreviewNavigationMode::Anchored => {
      let target_line = focus_line
        .map(|line| line.min(total_virtual_rows.saturating_sub(1)))
        .unwrap_or(0);
      let target_content_rows = visible_rows.saturating_sub(2).max(1);
      let (window_start, window_end) =
        centered_window(target_line, total_virtual_rows, target_content_rows);
      let hidden_above = window_start;
      let hidden_below = total_virtual_rows.saturating_sub(window_end);
      let has_top_marker = hidden_above > 0;
      let has_bottom_marker = hidden_below > 0;
      let mut lines = Vec::with_capacity(
        window_end
          .saturating_sub(window_start)
          .saturating_add(has_top_marker as usize)
          .saturating_add(has_bottom_marker as usize),
      );

      if has_top_marker {
        lines.push(FilePickerPreviewWindowLine {
          virtual_row: window_start.saturating_sub(1),
          kind:        FilePickerPreviewLineKind::TruncatedAbove,
          line_number: None,
          focused:     false,
          marker:      format!("… {} lines above", hidden_above),
          segments:    Vec::new(),
        });
      }

      for virtual_row in window_start..window_end {
        let focused = focus_line.is_some_and(|focus| focus == virtual_row);
        let mut segments = vec![FilePickerPreviewSegment {
          text:         plain_lines[virtual_row].to_string(),
          highlight_id: None,
          is_match:     false,
          change_kind:  None,
        }];
        if focused {
          segments = apply_match_to_preview_segments(segments, focus_match_ranges);
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

      if has_bottom_marker {
        lines.push(FilePickerPreviewWindowLine {
          virtual_row: window_end,
          kind:        FilePickerPreviewLineKind::TruncatedBelow,
          line_number: None,
          focused:     false,
          marker:      format!("… {} lines below", hidden_below),
          segments:    Vec::new(),
        });
      }

      FilePickerPreviewWindow {
        navigation_mode,
        kind,
        total_virtual_rows,
        offset: window_start,
        window_start,
        lines,
        vcs_diff: None,
      }
    },
    FilePickerPreviewNavigationMode::Static => {
      let lines = plain_lines
        .into_iter()
        .enumerate()
        .map(|(virtual_row, line)| {
          FilePickerPreviewWindowLine {
            virtual_row,
            kind: FilePickerPreviewLineKind::Content,
            line_number: None,
            focused: false,
            marker: String::new(),
            segments: vec![FilePickerPreviewSegment {
              text:         line.to_string(),
              highlight_id: None,
              is_match:     false,
              change_kind:  None,
            }],
          }
        })
        .collect();

      FilePickerPreviewWindow {
        navigation_mode,
        kind,
        total_virtual_rows,
        offset: 0,
        window_start: 0,
        lines,
        vcs_diff: None,
      }
    },
  }
}

pub fn finalize_vcs_diff_preview(
  mut preview: FilePickerVcsDiffPreview,
) -> FilePickerVcsDiffPreview {
  preview.cached_lines = compute_vcs_diff_preview_window_lines(&preview).into();
  preview
}

fn vcs_diff_preview_window_lines(
  preview: &FilePickerVcsDiffPreview,
) -> Cow<'_, [FilePickerVcsDiffPreviewWindowLine]> {
  if preview.cached_lines.is_empty() {
    Cow::Owned(compute_vcs_diff_preview_window_lines(preview))
  } else {
    Cow::Borrowed(&preview.cached_lines)
  }
}

fn compute_vcs_diff_preview_window_lines(
  preview: &FilePickerVcsDiffPreview,
) -> Vec<FilePickerVcsDiffPreviewWindowLine> {
  let mut lines = Vec::new();
  let mut changed_rows = Vec::new();
  let mut trailing_rows = Vec::new();
  let mut in_changed_block = false;

  for row in &preview.rows {
    match row.kind {
      FilePickerVcsDiffPreviewRowKind::CollapsedAbove
      | FilePickerVcsDiffPreviewRowKind::CollapsedBelow
      | FilePickerVcsDiffPreviewRowKind::Info => {
        let target = if in_changed_block {
          &mut trailing_rows
        } else {
          &mut lines
        };
        target.push(FilePickerVcsDiffPreviewWindowLine {
          virtual_row: 0,
          kind:        row.kind,
          source:      FilePickerVcsDiffPreviewLineSource::Meta,
          line_number: None,
          segments:    Vec::new(),
          message:     row.message.clone(),
        });
      },
      FilePickerVcsDiffPreviewRowKind::Context => {
        let (source, line_number, segments) = if let Some(line_index) = row.right_line_index {
          (
            FilePickerVcsDiffPreviewLineSource::Worktree,
            row.right_line_number,
            preview.right.line_text(line_index).map(|line| {
              let line_start = preview.right.line_start(line_index).unwrap_or(0);
              file_picker_preview_line_segments(line, line_start, &preview.right.highlights)
            }),
          )
        } else if let Some(line_index) = row.left_line_index {
          (
            FilePickerVcsDiffPreviewLineSource::Base,
            row.left_line_number,
            preview.left.line_text(line_index).map(|line| {
              let line_start = preview.left.line_start(line_index).unwrap_or(0);
              file_picker_preview_line_segments(line, line_start, &preview.left.highlights)
            }),
          )
        } else {
          (
            FilePickerVcsDiffPreviewLineSource::Meta,
            None,
            Some(Vec::new()),
          )
        };
        let target = if in_changed_block {
          &mut trailing_rows
        } else {
          &mut lines
        };
        target.push(FilePickerVcsDiffPreviewWindowLine {
          virtual_row: 0,
          kind: FilePickerVcsDiffPreviewRowKind::Context,
          source,
          line_number,
          segments: segments.unwrap_or_default(),
          message: String::new(),
        });
      },
      FilePickerVcsDiffPreviewRowKind::Added
      | FilePickerVcsDiffPreviewRowKind::Removed
      | FilePickerVcsDiffPreviewRowKind::Modified => {
        in_changed_block = true;
        changed_rows.push(row.clone());
      },
      FilePickerVcsDiffPreviewRowKind::SectionHeader => {},
    }
  }

  if !changed_rows.is_empty() {
    let has_base = changed_rows.iter().any(|row| row.left_line_index.is_some());
    let has_worktree = changed_rows
      .iter()
      .any(|row| row.right_line_index.is_some());

    if has_base {
      lines.push(FilePickerVcsDiffPreviewWindowLine {
        virtual_row: 0,
        kind:        FilePickerVcsDiffPreviewRowKind::SectionHeader,
        source:      FilePickerVcsDiffPreviewLineSource::Base,
        line_number: None,
        segments:    Vec::new(),
        message:     preview.left_label.clone(),
      });
      for row in &changed_rows {
        let Some(line_index) = row.left_line_index else {
          continue;
        };
        let line = preview.left.line_text(line_index).unwrap_or_default();
        let line_start = preview.left.line_start(line_index).unwrap_or(0);
        lines.push(FilePickerVcsDiffPreviewWindowLine {
          virtual_row: 0,
          kind:        match row.kind {
            FilePickerVcsDiffPreviewRowKind::Removed
            | FilePickerVcsDiffPreviewRowKind::Modified => FilePickerVcsDiffPreviewRowKind::Removed,
            other => other,
          },
          source:      FilePickerVcsDiffPreviewLineSource::Base,
          line_number: row.left_line_number,
          segments:    file_picker_preview_line_segments(
            line,
            line_start,
            &preview.left.highlights,
          ),
          message:     String::new(),
        });
      }
    }

    if has_base && has_worktree {
      lines.push(FilePickerVcsDiffPreviewWindowLine {
        virtual_row: 0,
        kind:        FilePickerVcsDiffPreviewRowKind::Info,
        source:      FilePickerVcsDiffPreviewLineSource::Meta,
        line_number: None,
        segments:    Vec::new(),
        message:     String::new(),
      });
    }

    if has_worktree {
      lines.push(FilePickerVcsDiffPreviewWindowLine {
        virtual_row: 0,
        kind:        FilePickerVcsDiffPreviewRowKind::SectionHeader,
        source:      FilePickerVcsDiffPreviewLineSource::Worktree,
        line_number: None,
        segments:    Vec::new(),
        message:     preview.right_label.clone(),
      });
      for row in &changed_rows {
        let Some(line_index) = row.right_line_index else {
          continue;
        };
        let line = preview.right.line_text(line_index).unwrap_or_default();
        let line_start = preview.right.line_start(line_index).unwrap_or(0);
        lines.push(FilePickerVcsDiffPreviewWindowLine {
          virtual_row: 0,
          kind:        match row.kind {
            FilePickerVcsDiffPreviewRowKind::Added | FilePickerVcsDiffPreviewRowKind::Modified => {
              FilePickerVcsDiffPreviewRowKind::Added
            },
            other => other,
          },
          source:      FilePickerVcsDiffPreviewLineSource::Worktree,
          line_number: row.right_line_number,
          segments:    file_picker_preview_line_segments(
            line,
            line_start,
            &preview.right.highlights,
          ),
          message:     String::new(),
        });
      }
    }
  }

  lines.extend(trailing_rows);

  if lines.is_empty() {
    lines.push(FilePickerVcsDiffPreviewWindowLine {
      virtual_row: 0,
      kind:        FilePickerVcsDiffPreviewRowKind::Info,
      source:      FilePickerVcsDiffPreviewLineSource::Meta,
      line_number: None,
      segments:    Vec::new(),
      message:     "No diff available".to_string(),
    });
  }

  for (virtual_row, line) in lines.iter_mut().enumerate() {
    line.virtual_row = virtual_row;
  }

  lines
}

fn vcs_diff_preview_line_count(preview: &FilePickerVcsDiffPreview) -> usize {
  vcs_diff_preview_window_lines(preview).len().max(1)
}

fn build_vcs_diff_preview_window(
  preview: &FilePickerVcsDiffPreview,
  navigation_mode: FilePickerPreviewNavigationMode,
  offset: usize,
  visible_rows: usize,
  overscan: usize,
) -> FilePickerPreviewWindow {
  let visible_rows = visible_rows.max(1);
  let lines = vcs_diff_preview_window_lines(preview);
  let total_virtual_rows = lines.len().max(1);
  let (offset, window_start, lines) = match navigation_mode {
    FilePickerPreviewNavigationMode::Scrollable => {
      let max_offset = total_virtual_rows.saturating_sub(visible_rows);
      let offset = offset.min(max_offset);
      let window_start = offset.saturating_sub(overscan);
      let window_end = offset
        .saturating_add(visible_rows)
        .saturating_add(overscan)
        .min(total_virtual_rows);
      (
        offset,
        window_start,
        lines[window_start..window_end].to_vec(),
      )
    },
    FilePickerPreviewNavigationMode::Anchored | FilePickerPreviewNavigationMode::Static => {
      let window_end = visible_rows.min(total_virtual_rows);
      (0, 0, lines[..window_end].to_vec())
    },
  };

  FilePickerPreviewWindow {
    navigation_mode,
    kind: FilePickerPreviewWindowKind::VcsDiff,
    total_virtual_rows,
    offset,
    window_start,
    lines: Vec::new(),
    vcs_diff: Some(FilePickerVcsDiffPreviewWindow {
      total_virtual_rows,
      lines,
    }),
  }
}

fn centered_window(focus_line: usize, total_rows: usize, target_rows: usize) -> (usize, usize) {
  let target_rows = target_rows.max(1).min(total_rows.max(1));
  let mut window_start = focus_line.saturating_sub(target_rows / 2);
  if window_start.saturating_add(target_rows) > total_rows {
    window_start = total_rows.saturating_sub(target_rows);
  }
  let window_end = window_start.saturating_add(target_rows).min(total_rows);
  (window_start, window_end)
}

fn source_bottom_marker(hidden_below: usize, truncated_by_bytes: bool) -> String {
  if truncated_by_bytes {
    if hidden_below > 0 {
      format!("… {} lines below (truncated file)", hidden_below)
    } else {
      "… truncated file below".to_string()
    }
  } else {
    format!("… {} lines below", hidden_below)
  }
}

fn preview_for_path_base(path: &Path, is_dir: bool, focus_line: Option<usize>) -> PreviewBuild {
  let _ = focus_line;
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
  let Some((source, end_byte, preview_text)) = source_preview(path, &metadata, &text, truncated)
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

fn line_start_index_for_text(text: &str) -> Arc<[usize]> {
  if text.is_empty() {
    return Arc::from([]);
  }
  let mut line_starts = Vec::new();
  line_starts.push(0);
  for (idx, ch) in text.char_indices() {
    if ch == '\n' && idx + 1 < text.len() {
      line_starts.push(idx + 1);
    }
  }
  line_starts.into()
}

fn source_preview(
  path: &Path,
  metadata: &fs::Metadata,
  text: &str,
  truncated_by_bytes: bool,
) -> Option<(FilePickerSourcePreview, usize, String)> {
  if text.is_empty() {
    return None;
  }

  let all_line_starts = line_start_index_for_path(path, metadata, text);
  if all_line_starts.is_empty() {
    return None;
  }

  let owned_text = text.to_string();

  Some((
    FilePickerSourcePreview {
      text: Arc::<str>::from(owned_text.clone()),
      line_starts: all_line_starts,
      highlights: Vec::<(Highlight, Range<usize>)>::new().into(),
      truncated_by_bytes,
    },
    owned_text.len(),
    owned_text,
  ))
}

pub fn file_picker_source_preview_from_text(
  path: &Path,
  text: &str,
  loader: Option<&Loader>,
) -> FilePickerSourcePreview {
  let owned_text = text.to_string();
  let mut preview = FilePickerSourcePreview {
    text:               Arc::<str>::from(owned_text.clone()),
    line_starts:        line_start_index_for_text(&owned_text),
    highlights:         Vec::<(Highlight, Range<usize>)>::new().into(),
    truncated_by_bytes: false,
  };
  if let Some(loader) = loader {
    let highlights = collect_source_highlights(path, &owned_text, owned_text.len(), loader);
    if !highlights.is_empty() {
      preview.highlights = highlights.into();
    }
  }
  preview
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
      row_data:     None,
      preview:      None,
      payload:      None,
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
      row_data:     None,
      preview:      None,
      payload:      None,
    }
  }

  #[test]
  fn file_picker_row_data_prefers_custom_row_data() {
    let item = sample_item("demo.rs").with_row_data(FilePickerRowData {
      kind:       FilePickerRowKind::Generic,
      severity:   None,
      primary:    "custom".to_string(),
      secondary:  "secondary".to_string(),
      tertiary:   String::new(),
      quaternary: String::new(),
      line:       0,
      column:     0,
      depth:      0,
    });

    let row = file_picker_row_data("Generic", &item);

    assert_eq!(row.primary, "custom");
    assert_eq!(row.secondary, "secondary");
  }

  #[test]
  fn refresh_preview_prefers_item_preview_over_path_preview() {
    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(
      &injector,
      sample_item("virtual").with_preview(FilePickerPreview::Message("custom preview".to_string())),
    );
    drop(injector);

    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);
    state.selected = Some(0);

    refresh_preview(&mut state);

    assert!(matches!(
      &state.preview,
      FilePickerPreview::Message(message) if message == "custom preview"
    ));
    assert_eq!(state.preview_path, None);
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
  fn restore_selection_by_stable_id_keeps_same_item_after_reorder() {
    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(&injector, sample_item("cargo.toml"));
    inject_item(&injector, sample_item("font_bug.md"));
    drop(injector);

    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);

    state.selected = Some(1);
    let stable_id = state
      .selected_item_stable_id()
      .expect("selected item should have an id");

    state.matcher.restart(true);
    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let injector = state.matcher.injector();
    inject_item(&injector, sample_item("font_bug.md"));
    inject_item(&injector, sample_item("cargo.toml"));
    drop(injector);

    let _ = refresh_matcher_state(&mut state);
    assert_eq!(state.selected, Some(1));
    assert_eq!(
      state.current_item().map(|item| item.display.clone()),
      Some("cargo.toml".to_string())
    );

    assert!(restore_selection_by_stable_id(&mut state, Some(stable_id)));
    assert_eq!(state.selected, Some(0));
    assert_eq!(
      state.current_item().map(|item| item.display.clone()),
      Some("font_bug.md".to_string())
    );
  }

  #[test]
  fn refresh_preview_anchors_new_focus_line_in_same_file() {
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
      row_data:     None,
      preview:      None,
      payload:      None,
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
      row_data:     None,
      preview:      None,
      payload:      None,
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
      state.preview_navigation_mode(),
      FilePickerPreviewNavigationMode::Scrollable
    );
    assert_eq!(state.preview_scroll, 37);

    let window = file_picker_preview_window(&state, state.preview_scroll, 10, 4);
    assert_eq!(
      window.navigation_mode,
      FilePickerPreviewNavigationMode::Scrollable
    );
    assert_eq!(window.offset, 37);
    assert!(
      window
        .lines
        .iter()
        .any(|line| line.focused && line.virtual_row == 40)
    );

    let _ = std::fs::remove_file(path);
  }

  #[test]
  fn generic_source_preview_windows_full_document() {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock should be after epoch")
      .as_nanos();
    path.push(format!("the-editor-file-picker-scroll-preview-{stamp}.txt"));

    let mut text = String::new();
    for idx in 1..=200 {
      text.push_str(&format!("line {idx}\n"));
    }
    std::fs::write(&path, text).expect("should create preview file");

    let mut state = FilePickerState::default();
    let injector = state.matcher.injector();
    inject_item(&injector, FilePickerItem {
      absolute:     path.clone(),
      display:      "plain".to_string(),
      icon:         "file_generic".to_string(),
      is_dir:       false,
      display_path: false,
      action:       FilePickerItemAction::OpenFile(path.clone()),
      preview_path: Some(path.clone()),
      preview_line: None,
      preview_col:  None,
      row_data:     None,
      preview:      None,
      payload:      None,
    });
    drop(injector);

    state
      .matcher
      .pattern
      .reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
    let _ = refresh_matcher_state(&mut state);
    state.selected = Some(0);
    normalize_selection_and_scroll(&mut state);
    refresh_preview(&mut state);

    assert_eq!(
      state.preview_navigation_mode(),
      FilePickerPreviewNavigationMode::Scrollable
    );
    assert_eq!(state.preview_line_count(), 200);

    let window = file_picker_preview_window(&state, 69, 8, 0);
    assert_eq!(
      window.navigation_mode,
      FilePickerPreviewNavigationMode::Scrollable
    );
    assert_eq!(window.offset, 69);
    assert_eq!(window.window_start, 68);
    assert_eq!(window.lines.first().map(|line| line.virtual_row), Some(68));
    assert_eq!(window.lines.last().map(|line| line.virtual_row), Some(77));

    let _ = std::fs::remove_file(path);
  }

  #[test]
  fn vcs_diff_preview_window_builds_stacked_rows() {
    let preview = finalize_vcs_diff_preview(FilePickerVcsDiffPreview {
      title:        "main.c".to_string(),
      from_title:   Some("old-main.c".to_string()),
      left_label:   "BASE".to_string(),
      right_label:  "WORKTREE".to_string(),
      left:         file_picker_source_preview_from_text(
        Path::new("main.c"),
        "int main() {\n  return 0;\n}\n",
        None,
      ),
      right:        file_picker_source_preview_from_text(
        Path::new("main.c"),
        "int main() {\n  puts(\"hi\");\n  return 0;\n}\n",
        None,
      ),
      rows:         vec![
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Context,
          left_line_index:   Some(0),
          right_line_index:  Some(0),
          left_line_number:  Some(1),
          right_line_number: Some(1),
          message:           String::new(),
        },
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Added,
          left_line_index:   None,
          right_line_index:  Some(1),
          left_line_number:  None,
          right_line_number: Some(2),
          message:           String::new(),
        },
      ],
      cached_lines: Arc::new([]),
    });
    let mut state = FilePickerState::default();
    state.preview = FilePickerPreview::VcsDiff(preview);

    let window = file_picker_preview_window(&state, 0, 4, 0);
    let vcs_window = window.vcs_diff.expect("vcs diff preview window");

    assert_eq!(window.kind, FilePickerPreviewWindowKind::VcsDiff);
    assert_eq!(
      window.navigation_mode,
      FilePickerPreviewNavigationMode::Scrollable
    );
    assert_eq!(vcs_window.lines.len(), 3);
    assert_eq!(
      vcs_window.lines[0].kind,
      FilePickerVcsDiffPreviewRowKind::Context
    );
    assert_eq!(
      vcs_window.lines[0].source,
      FilePickerVcsDiffPreviewLineSource::Worktree
    );
    assert_eq!(vcs_window.lines[0].line_number, Some(1));
    assert_eq!(vcs_window.lines[0].segments[0].text, "int main() {");
    assert_eq!(
      vcs_window.lines[1].kind,
      FilePickerVcsDiffPreviewRowKind::SectionHeader
    );
    assert_eq!(
      vcs_window.lines[1].source,
      FilePickerVcsDiffPreviewLineSource::Worktree
    );
    assert_eq!(vcs_window.lines[1].message, "WORKTREE");
    assert_eq!(
      vcs_window.lines[2].kind,
      FilePickerVcsDiffPreviewRowKind::Added
    );
    assert_eq!(
      vcs_window.lines[2].source,
      FilePickerVcsDiffPreviewLineSource::Worktree
    );
    assert_eq!(vcs_window.lines[2].line_number, Some(2));
    assert_eq!(vcs_window.lines[2].segments[0].text, "  puts(\"hi\");");
  }

  #[test]
  fn vcs_diff_preview_window_collapses_overflow_rows() {
    let preview = finalize_vcs_diff_preview(FilePickerVcsDiffPreview {
      title:        "main.c".to_string(),
      from_title:   None,
      left_label:   "BASE".to_string(),
      right_label:  "WORKTREE".to_string(),
      left:         file_picker_source_preview_from_text(Path::new("main.c"), "a\nb\nc\nd\n", None),
      right:        file_picker_source_preview_from_text(Path::new("main.c"), "a\nb\nc\nd\n", None),
      rows:         vec![
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Context,
          left_line_index:   Some(0),
          right_line_index:  Some(0),
          left_line_number:  Some(1),
          right_line_number: Some(1),
          message:           String::new(),
        },
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Context,
          left_line_index:   Some(1),
          right_line_index:  Some(1),
          left_line_number:  Some(2),
          right_line_number: Some(2),
          message:           String::new(),
        },
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Context,
          left_line_index:   Some(2),
          right_line_index:  Some(2),
          left_line_number:  Some(3),
          right_line_number: Some(3),
          message:           String::new(),
        },
      ],
      cached_lines: Arc::new([]),
    });
    let mut state = FilePickerState::default();
    state.preview = FilePickerPreview::VcsDiff(preview);

    let window = file_picker_preview_window(&state, 0, 2, 0);
    let vcs_window = window.vcs_diff.expect("vcs diff preview window");

    assert_eq!(window.kind, FilePickerPreviewWindowKind::VcsDiff);
    assert_eq!(
      window.navigation_mode,
      FilePickerPreviewNavigationMode::Scrollable
    );
    assert_eq!(window.total_virtual_rows, 3);
    assert_eq!(vcs_window.lines.len(), 2);
    assert_eq!(
      vcs_window.lines[0].kind,
      FilePickerVcsDiffPreviewRowKind::Context
    );
    assert_eq!(
      vcs_window.lines[1].kind,
      FilePickerVcsDiffPreviewRowKind::Context
    );
    assert_eq!(vcs_window.lines[1].line_number, Some(2));
  }

  #[test]
  fn vcs_diff_preview_window_scrolls_large_hunks() {
    let preview = finalize_vcs_diff_preview(FilePickerVcsDiffPreview {
      title:        "main.c".to_string(),
      from_title:   None,
      left_label:   "BASE".to_string(),
      right_label:  "WORKTREE".to_string(),
      left:         file_picker_source_preview_from_text(
        Path::new("main.c"),
        "old1\nold2\nold3\n",
        None,
      ),
      right:        file_picker_source_preview_from_text(
        Path::new("main.c"),
        "new1\nnew2\nnew3\n",
        None,
      ),
      rows:         vec![
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Removed,
          left_line_index:   Some(0),
          right_line_index:  None,
          left_line_number:  Some(1),
          right_line_number: None,
          message:           String::new(),
        },
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Removed,
          left_line_index:   Some(1),
          right_line_index:  None,
          left_line_number:  Some(2),
          right_line_number: None,
          message:           String::new(),
        },
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Added,
          left_line_index:   None,
          right_line_index:  Some(0),
          left_line_number:  None,
          right_line_number: Some(1),
          message:           String::new(),
        },
        FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Added,
          left_line_index:   None,
          right_line_index:  Some(1),
          left_line_number:  None,
          right_line_number: Some(2),
          message:           String::new(),
        },
      ],
      cached_lines: Arc::new([]),
    });
    let mut state = FilePickerState::default();
    state.preview = FilePickerPreview::VcsDiff(preview);

    let top = file_picker_preview_window(&state, 0, 3, 0);
    let top_window = top.vcs_diff.expect("top vcs diff preview");
    assert_eq!(top.total_virtual_rows, 7);
    assert_eq!(top_window.lines.len(), 3);
    assert_eq!(
      top_window.lines[0].kind,
      FilePickerVcsDiffPreviewRowKind::SectionHeader
    );
    assert_eq!(top_window.lines[0].message, "BASE");
    assert_eq!(top_window.lines[1].line_number, Some(1));
    assert_eq!(top_window.lines[2].line_number, Some(2));

    let scrolled = file_picker_preview_window(&state, 4, 3, 0);
    let scrolled_window = scrolled.vcs_diff.expect("scrolled vcs diff preview");
    assert_eq!(scrolled.offset, 4);
    assert_eq!(scrolled_window.lines.len(), 3);
    assert_eq!(
      scrolled_window.lines[0].kind,
      FilePickerVcsDiffPreviewRowKind::SectionHeader
    );
    assert_eq!(scrolled_window.lines[0].message, "WORKTREE");
    assert_eq!(scrolled_window.lines[1].line_number, Some(1));
    assert_eq!(scrolled_window.lines[2].line_number, Some(2));
  }
}
