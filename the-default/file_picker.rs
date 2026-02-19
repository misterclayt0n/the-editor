use std::{
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
const MAX_PREVIEW_BYTES: usize = 256 * 1024;
const MAX_PREVIEW_LINES: usize = 512;
const PREVIEW_CACHE_CAPACITY: usize = 128;
const PAGE_SIZE: usize = 12;
const MATCHER_TICK_TIMEOUT_MS: u64 = 10;
const DEFAULT_LIST_VISIBLE_ROWS: usize = 32;
const WARMUP_SCAN_BUDGET_MS: u64 = 30;

thread_local! {
  static MATCH_INDEX_SCRATCH: RefCell<(NucleoMatcher, Vec<u32>)> =
    RefCell::new((NucleoMatcher::default(), Vec::new()));
}

#[derive(Debug, Clone)]
pub struct FilePickerItem {
  pub absolute: PathBuf,
  pub display:  String,
  pub icon:     String,
  pub is_dir:   bool,
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
  pub lines:       Arc<[String]>,
  pub line_starts: Arc<[usize]>,
  pub highlights:  Arc<[(Highlight, Range<usize>)]>,
  pub truncated:   bool,
}

struct PreviewRequest {
  request_id: u64,
  path:       PathBuf,
  is_dir:     bool,
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
  pub config:             FilePickerConfig,
  pub query:              String,
  pub cursor:             usize,
  pub selected:           Option<usize>,
  pub hovered:            Option<usize>,
  pub list_offset:        usize,
  pub list_visible:       usize,
  pub preview_scroll:     usize,
  pub show_preview:       bool,
  pub preview_path:       Option<PathBuf>,
  pub preview:            FilePickerPreview,
  pub error:              Option<String>,
  pub scanning:           bool,
  pub matcher_running:    bool,
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
      config: FilePickerConfig::default(),
      query: String::new(),
      cursor: 0,
      selected: None,
      hovered: None,
      list_offset: 0,
      list_visible: DEFAULT_LIST_VISIBLE_ROWS,
      preview_scroll: 0,
      show_preview: true,
      preview_path: None,
      preview: FilePickerPreview::Empty,
      error: None,
      scanning: false,
      matcher_running: false,
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
        source.lines.len().saturating_add(source.truncated as usize)
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
  let root = picker_root(ctx);
  let show_preview = ctx.file_picker().show_preview;
  let config = ctx.file_picker().config.clone();
  let wake_tx = ctx.file_picker().wake_tx.clone();
  let syntax_loader = ctx.file_picker().syntax_loader.clone();
  if let Some(cancel) = ctx.file_picker().scan_cancel.as_ref() {
    cancel.store(true, Ordering::Relaxed);
  }

  let mut state = FilePickerState::default();
  state.active = true;
  state.show_preview = show_preview;
  state.config = config;
  state.wake_tx = wake_tx.clone();
  state.syntax_loader = syntax_loader;
  state.matcher = new_matcher(wake_tx);
  state.preview = FilePickerPreview::Message("Scanning files…".to_string());
  start_preview_worker(&mut state);
  start_scan(&mut state, root);
  poll_scan_results(&mut state);

  *ctx.file_picker_mut() = state;
  ctx.request_render();
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
  picker.preview_path = None;
  picker.preview = FilePickerPreview::Empty;
  picker.scanning = false;
  picker.matcher_running = false;
  picker.preview_req_tx = None;
  picker.preview_res_rx = None;
  picker.preview_pending_id = None;
  picker.preview_latest_request.store(0, Ordering::Relaxed);
  picker.scan_rx = None;
  picker.scan_cancel = None;
  ctx.request_render();
}

pub fn move_selection<Ctx: DefaultContext>(ctx: &mut Ctx, amount: isize) {
  let picker = ctx.file_picker_mut();
  let matched_count = picker.matched_count();
  if matched_count == 0 {
    picker.selected = None;
    picker.list_offset = 0;
    return;
  }

  let len = matched_count as isize;
  let selected = picker.selected.unwrap_or(0) as isize;
  let next = (selected + amount).rem_euclid(len) as usize;
  picker.selected = Some(next);
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
      normalize_selection_and_scroll(picker);
      refresh_preview(picker);
      ctx.request_render();
      true
    },
    Key::End => {
      let picker = ctx.file_picker_mut();
      picker.selected = picker.matched_count().checked_sub(1);
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
      let picker = ctx.file_picker_mut();
      if picker.cursor > 0 && picker.cursor <= picker.query.len() {
        let old_query = picker.query.clone();
        let prev = prev_char_boundary(&picker.query, picker.cursor);
        picker.query.replace_range(prev..picker.cursor, "");
        picker.cursor = prev;
        handle_query_change(picker, &old_query);
        refresh_preview(picker);
      }
      ctx.request_render();
      true
    },
    Key::Delete => {
      let picker = ctx.file_picker_mut();
      if picker.cursor < picker.query.len() {
        let old_query = picker.query.clone();
        let next = next_char_boundary(&picker.query, picker.cursor);
        picker.query.replace_range(picker.cursor..next, "");
        handle_query_change(picker, &old_query);
        refresh_preview(picker);
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
      let picker = ctx.file_picker_mut();
      let old_query = picker.query.clone();
      picker.query.insert(picker.cursor, ch);
      picker.cursor += ch.len_utf8();
      handle_query_change(picker, &old_query);
      refresh_preview(picker);
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

  if let Err(err) = ctx.open_file(&item.absolute) {
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
  panel.title = Some(format!("File Picker · {}", picker.root.display()));
  panel.style = panel.style.with_role("file_picker");
  panel.style.border = None;
  panel.constraints = UiConstraints::floating_default();
  panel.constraints.min_width = Some(72);
  panel.constraints.min_height = Some(18);
  panel.constraints.max_width = None;
  panel.constraints.max_height = None;

  vec![UiNode::Panel(panel)]
}

fn picker_root<Ctx: DefaultContext>(ctx: &Ctx) -> PathBuf {
  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  let mut base = ctx
    .file_path()
    .map(Path::to_path_buf)
    .unwrap_or_else(|| cwd.clone());
  if base.as_os_str().is_empty() {
    base = cwd.clone();
  }
  if base.is_relative() {
    base = cwd.join(base);
  }

  let start = if base.is_dir() {
    base
  } else {
    base
      .parent()
      .filter(|path| !path.as_os_str().is_empty())
      .map(Path::to_path_buf)
      .unwrap_or(cwd)
  };

  workspace_root(&start)
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
    absolute: path,
    display,
    icon,
    is_dir: false,
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

      match preview_for_path_base(&request.path, request.is_dir) {
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
  let selected = state.selected.unwrap_or(0).min(matched_count - 1);
  state.selected = Some(selected);

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
  let next_selected = Some(state.selected.unwrap_or(0).min(last));
  if state.selected != next_selected {
    state.selected = next_selected;
    changed = true;
  }

  let next_hovered = state.hovered.map(|hovered| hovered.min(last));
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
    state.preview_scroll = 0;
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

  if state
    .preview_path
    .as_ref()
    .is_some_and(|path| path == &item.absolute)
  {
    return;
  }

  state.preview_path = Some(item.absolute.clone());
  state.preview_scroll = 0;
  if let Some(preview) = state.preview_cache.get(&item.absolute) {
    state.preview = preview;
    state.preview_pending_id = None;
    state.preview_latest_request.store(0, Ordering::Relaxed);
    return;
  }

  match preview_for_path_base(&item.absolute, item.is_dir) {
    PreviewBuild::Final(preview) => {
      state
        .preview_cache
        .insert(item.absolute.clone(), preview.clone());
      state.preview = preview;
      state.preview_pending_id = None;
      state.preview_latest_request.store(0, Ordering::Relaxed);
    },
    PreviewBuild::Source(mut source_preview) => {
      let base_preview = FilePickerPreview::Source(source_preview.source.clone());
      state
        .preview_cache
        .insert(item.absolute.clone(), base_preview.clone());
      state.preview = base_preview;

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
        path: item.absolute.clone(),
        is_dir: item.is_dir,
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
        &item.absolute,
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
        .insert(item.absolute.clone(), preview.clone());
      state.preview = preview;
      state.preview_pending_id = None;
      state.preview_latest_request.store(0, Ordering::Relaxed);
    },
  }
}

fn preview_for_path_base(path: &Path, is_dir: bool) -> PreviewBuild {
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
  let Some((source, end_byte)) = source_preview(&text, truncated) else {
    return PreviewBuild::Final(FilePickerPreview::Message("<Empty file>".to_string()));
  };

  PreviewBuild::Source(SourcePreviewData {
    source,
    text,
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
  for entry in read_dir.take(MAX_PREVIEW_LINES) {
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

fn source_preview(
  text: &str,
  truncated_by_bytes: bool,
) -> Option<(FilePickerSourcePreview, usize)> {
  let mut lines = Vec::new();
  let mut line_starts = Vec::new();
  let mut consumed_bytes = 0usize;

  let mut segments = text.split_inclusive('\n').peekable();
  while lines.len() < MAX_PREVIEW_LINES {
    let Some(segment) = segments.next() else {
      break;
    };
    let line = segment
      .strip_suffix('\n')
      .map(|line| line.strip_suffix('\r').unwrap_or(line))
      .unwrap_or(segment);
    line_starts.push(consumed_bytes);
    lines.push(line.to_string());
    consumed_bytes = consumed_bytes.saturating_add(segment.len());
  }

  if lines.is_empty() {
    return None;
  }

  let truncated = truncated_by_bytes || segments.peek().is_some();
  Some((
    FilePickerSourcePreview {
      lines: lines.into(),
      line_starts: line_starts.into(),
      highlights: Vec::<(Highlight, Range<usize>)>::new().into(),
      truncated,
    },
    consumed_bytes,
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
  let width = source.lines.len().max(1).to_string().len();
  let mut output = String::new();
  for (line_idx, line) in source.lines.iter().enumerate() {
    let _ = std::fmt::Write::write_fmt(
      &mut output,
      format_args!("{:>width$} {}\n", line_idx + 1, line, width = width),
    );
  }
  if source.truncated {
    output.push('…');
  }
  output
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::*;

  fn sample_item(display: &str) -> FilePickerItem {
    FilePickerItem {
      absolute: PathBuf::from(display),
      display:  display.to_string(),
      icon:     "file_generic".to_string(),
      is_dir:   false,
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
}
