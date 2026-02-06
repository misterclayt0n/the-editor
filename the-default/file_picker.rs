use std::{
  fs,
  io::Read,
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    atomic::{
      AtomicBool,
      Ordering,
    },
    mpsc::{
      self,
      Receiver,
      Sender,
      TryRecvError,
    },
  },
};

use ignore::DirEntry;
use nucleo::{
  Config as NucleoConfig,
  Nucleo,
  pattern::{
    CaseMatching,
    Normalization,
  },
};
use the_core::chars::{
  next_char_boundary,
  prev_char_boundary,
};
use the_lib::render::{
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
};

use crate::{
  DefaultContext,
  Key,
  KeyEvent,
};

const MAX_SCAN_ITEMS: usize = 100_000;
const MAX_RESULTS: usize = 2_000;
const MAX_FILE_SIZE_FOR_PREVIEW: u64 = 10 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 256 * 1024;
const MAX_PREVIEW_LINES: usize = 512;
const PAGE_SIZE: usize = 12;
const DEDUP_SYMLINKS: bool = true;
const SCAN_BATCH_SIZE: usize = 256;
const MATCHER_TICK_TIMEOUT_MS: u64 = 10;

#[derive(Debug, Clone)]
pub struct FilePickerItem {
  pub absolute: PathBuf,
  pub display:  String,
  pub is_dir:   bool,
}

#[derive(Debug, Clone)]
pub enum FilePickerPreview {
  Empty,
  Text(String),
  Message(String),
}

pub struct FilePickerState {
  pub active:          bool,
  pub root:            PathBuf,
  pub query:           String,
  pub cursor:          usize,
  pub items:           Vec<FilePickerItem>,
  pub filtered:        Vec<usize>,
  pub selected:        Option<usize>,
  pub max_results:     usize,
  pub show_preview:    bool,
  pub preview_path:    Option<PathBuf>,
  pub preview:         FilePickerPreview,
  pub error:           Option<String>,
  pub scanning:        bool,
  pub matcher_running: bool,
  scan_generation:     u64,
  scan_rx:             Option<Receiver<(u64, ScanMessage)>>,
  scan_cancel:         Option<Arc<AtomicBool>>,
  matcher:             Nucleo<usize>,
}

enum ScanMessage {
  Batch(Vec<FilePickerItem>),
  Done,
  Error(String),
}

impl Default for FilePickerState {
  fn default() -> Self {
    let matcher = new_matcher();
    Self {
      active: false,
      root: PathBuf::new(),
      query: String::new(),
      cursor: 0,
      items: Vec::new(),
      filtered: Vec::new(),
      selected: None,
      max_results: MAX_RESULTS,
      show_preview: true,
      preview_path: None,
      preview: FilePickerPreview::Empty,
      error: None,
      scanning: false,
      matcher_running: false,
      scan_generation: 0,
      scan_rx: None,
      scan_cancel: None,
      matcher,
    }
  }
}

impl FilePickerState {
  pub fn current_item(&self) -> Option<&FilePickerItem> {
    let selected = self.selected?;
    let idx = *self.filtered.get(selected)?;
    self.items.get(idx)
  }

  pub fn matched_count(&self) -> usize {
    self.filtered.len()
  }

  pub fn total_count(&self) -> usize {
    self.items.len()
  }
}

pub fn open_file_picker<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let root = picker_root(ctx);
  let show_preview = ctx.file_picker().show_preview;
  if let Some(cancel) = ctx.file_picker().scan_cancel.as_ref() {
    cancel.store(true, Ordering::Relaxed);
  }

  let mut state = FilePickerState::default();
  state.active = true;
  state.show_preview = show_preview;
  state.preview = FilePickerPreview::Message("Scanning files…".to_string());
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
  picker.preview_path = None;
  picker.preview = FilePickerPreview::Empty;
  picker.scanning = false;
  picker.matcher_running = false;
  picker.scan_rx = None;
  picker.scan_cancel = None;
  ctx.request_render();
}

pub fn move_selection<Ctx: DefaultContext>(ctx: &mut Ctx, amount: isize) {
  let picker = ctx.file_picker_mut();
  if picker.filtered.is_empty() {
    picker.selected = None;
    return;
  }

  let len = picker.filtered.len() as isize;
  let selected = picker.selected.unwrap_or(0) as isize;
  let next = (selected + amount).rem_euclid(len) as usize;
  picker.selected = Some(next);
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
      picker.selected = if picker.filtered.is_empty() {
        None
      } else {
        Some(0)
      };
      refresh_preview(picker);
      ctx.request_render();
      true
    },
    Key::End => {
      let picker = ctx.file_picker_mut();
      picker.selected = picker.filtered.len().checked_sub(1);
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
  let selected = ctx.file_picker().current_item().cloned();
  let Some(item) = selected else {
    return;
  };

  if let Err(err) = ctx.open_file(&item.absolute) {
    let picker = ctx.file_picker_mut();
    picker.error = Some(err.to_string());
    picker.preview = FilePickerPreview::Message(format!("Failed to open file: {err}"));
    ctx.request_render();
    return;
  }

  close_file_picker(ctx);
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
  let is_running = picker.scanning || picker.matcher_running;

  if is_running {
    ctx.request_render();
  }
  let picker = ctx.file_picker();

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
  input.placeholder = Some("Find file".to_string());
  input.cursor = picker.query[..picker.cursor.min(picker.query.len())]
    .chars()
    .count();
  input.style = input.style.with_role("file_picker");
  input.style.accent = Some(UiColor::Token(UiColorToken::Placeholder));

  let list_items: Vec<UiListItem> = picker
    .filtered
    .iter()
    .filter_map(|idx| picker.items.get(*idx))
    .map(|item| {
      let mut row = UiListItem::new(item.display.clone());
      row.emphasis = item.is_dir;
      row
    })
    .collect();

  let mut list = UiList::new("file_picker_list", list_items);
  list.selected = picker.selected;
  list.max_visible = Some(32);
  list.style = list.style.with_role("file_picker");
  list.style.accent = Some(UiColor::Token(UiColorToken::SelectedBg));
  list.style.border = Some(UiColor::Token(UiColorToken::SelectedText));
  let list = UiNode::List(list);

  let preview_content = match &picker.preview {
    FilePickerPreview::Empty => String::new(),
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

fn workspace_root(start: &Path) -> PathBuf {
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

fn has_workspace_marker(path: &Path) -> bool {
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
  state.items.clear();
  state.filtered.clear();
  state.selected = None;
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

  let (scan_tx, scan_rx) = mpsc::channel();
  let cancel = Arc::new(AtomicBool::new(false));
  state.scan_rx = Some(scan_rx);
  state.scan_cancel = Some(cancel.clone());

  spawn_scan_thread(root, MAX_SCAN_ITEMS, generation, scan_tx, cancel);
}

fn spawn_scan_thread(
  root: PathBuf,
  max_items: usize,
  generation: u64,
  scan_tx: Sender<(u64, ScanMessage)>,
  cancel: Arc<AtomicBool>,
) {
  std::thread::spawn(move || {
    if !root.exists() {
      let _ = scan_tx.send((
        generation,
        ScanMessage::Error("workspace directory does not exist".to_string()),
      ));
      return;
    }

    if let Err(err) = fs::read_dir(&root) {
      let _ = scan_tx.send((generation, ScanMessage::Error(err.to_string())));
      return;
    }

    let absolute_root = root.canonicalize().unwrap_or_else(|_| root.clone());
    let mut walk_builder = ignore::WalkBuilder::new(&root);
    let mut walker = walk_builder
      .hidden(true)
      .parents(true)
      .ignore(true)
      .follow_links(true)
      .git_ignore(true)
      .git_global(true)
      .git_exclude(true)
      .sort_by_file_name(|name1, name2| name1.cmp(name2))
      .filter_entry(move |entry| filter_picker_entry(entry, &absolute_root, DEDUP_SYMLINKS))
      .types(excluded_types())
      .build();

    let mut total = 0usize;
    let mut batch = Vec::with_capacity(SCAN_BATCH_SIZE);
    for entry in &mut walker {
      if cancel.load(Ordering::Relaxed) {
        break;
      }

      let entry = match entry {
        Ok(entry) => entry,
        Err(_) => continue,
      };
      if !entry
        .file_type()
        .is_some_and(|file_type| file_type.is_file())
      {
        continue;
      }

      let path = entry.into_path();
      let rel = match path.strip_prefix(&root) {
        Ok(rel) => rel,
        Err(_) => continue,
      };
      let mut display = rel.to_string_lossy().to_string();
      if std::path::MAIN_SEPARATOR != '/' {
        display = display.replace(std::path::MAIN_SEPARATOR, "/");
      }
      batch.push(FilePickerItem {
        absolute: path,
        display,
        is_dir: false,
      });
      total += 1;

      if batch.len() >= SCAN_BATCH_SIZE {
        if scan_tx
          .send((generation, ScanMessage::Batch(std::mem::take(&mut batch))))
          .is_err()
        {
          return;
        }
      }
      if total >= max_items {
        break;
      }
    }

    if !batch.is_empty() {
      if scan_tx
        .send((generation, ScanMessage::Batch(batch)))
        .is_err()
      {
        return;
      }
    }

    let _ = scan_tx.send((generation, ScanMessage::Done));
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

fn poll_scan_results(state: &mut FilePickerState) -> bool {
  let mut changed = false;
  let mut processed = 0usize;
  while processed < 64 {
    let scan_result = match state.scan_rx.as_ref() {
      Some(scan_rx) => scan_rx.try_recv(),
      None => break,
    };
    match scan_result {
      Ok((generation, message)) => {
        if generation != state.scan_generation {
          processed += 1;
          continue;
        }
        match message {
          ScanMessage::Batch(batch) => {
            inject_batch(state, batch);
            changed = true;
          },
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
        if state.items.is_empty() {
          state.error = Some("Scan interrupted".to_string());
          state.preview = FilePickerPreview::Message("Scan interrupted".to_string());
        }
        changed = true;
        break;
      },
    }
    processed += 1;
  }

  if refresh_filtered(state) {
    refresh_preview(state);
    changed = true;
  }

  let running = state.matcher_running || state.scanning;
  if running {
    changed = true;
  }

  changed
}

fn inject_batch(state: &mut FilePickerState, batch: Vec<FilePickerItem>) {
  if batch.is_empty() {
    return;
  }

  let injector = state.matcher.injector();
  for item in batch {
    let idx = state.items.len();
    let text = item.display.clone();
    state.items.push(item);
    injector.push(idx, move |_idx, dst| {
      dst[0] = text.into();
    });
  }
}

fn refresh_filtered(state: &mut FilePickerState) -> bool {
  let status = state.matcher.tick(MATCHER_TICK_TIMEOUT_MS);
  state.matcher_running = status.running || state.matcher.active_injectors() > 0;

  let snapshot = state.matcher.snapshot();
  let count = snapshot.matched_item_count().min(state.max_results as u32);
  let mut filtered = Vec::with_capacity(count as usize);
  for item in snapshot.matched_items(0..count) {
    filtered.push(*item.data);
  }

  let mut changed = status.changed || filtered != state.filtered;
  if filtered != state.filtered {
    state.filtered = filtered;
  }

  let prev_selected = state.selected;
  if state.filtered.is_empty() {
    state.selected = None;
  } else {
    let selected = state.selected.unwrap_or(0).min(state.filtered.len() - 1);
    state.selected = Some(selected);
  }
  if prev_selected != state.selected {
    changed = true;
  }

  changed
}

fn handle_query_change(state: &mut FilePickerState, old_query: &str) {
  if state.query == old_query {
    return;
  }

  state.selected = Some(0);
  let is_append = state.query.starts_with(old_query);
  state.matcher.pattern.reparse(
    0,
    &state.query,
    CaseMatching::Smart,
    Normalization::Smart,
    is_append,
  );
  let _ = refresh_filtered(state);
}

fn new_matcher() -> Nucleo<usize> {
  let mut config = NucleoConfig::DEFAULT;
  config.set_match_paths();
  Nucleo::new(config, Arc::new(|| {}), None, 1)
}

fn refresh_preview(state: &mut FilePickerState) {
  let item = state.current_item().cloned();
  let Some(item) = item else {
    state.preview_path = None;
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
  state.preview = preview_for_path(&item.absolute, item.is_dir);
}

fn preview_for_path(path: &Path, is_dir: bool) -> FilePickerPreview {
  if is_dir {
    return directory_preview(path);
  }

  let metadata = match fs::metadata(path) {
    Ok(metadata) => metadata,
    Err(_) => {
      return FilePickerPreview::Message("<File not found>".to_string());
    },
  };

  if metadata.len() > MAX_FILE_SIZE_FOR_PREVIEW {
    return FilePickerPreview::Message("<File too large to preview>".to_string());
  }

  let file = match fs::File::open(path) {
    Ok(file) => file,
    Err(_) => {
      return FilePickerPreview::Message("<Could not read file>".to_string());
    },
  };
  let mut bytes = Vec::new();
  if file
    .take((MAX_PREVIEW_BYTES + 1) as u64)
    .read_to_end(&mut bytes)
    .is_err()
  {
    return FilePickerPreview::Message("<Could not read file>".to_string());
  }

  if bytes.contains(&0) {
    return FilePickerPreview::Message("<Binary file>".to_string());
  }

  let truncated = bytes.len() > MAX_PREVIEW_BYTES;
  if truncated {
    bytes.truncate(MAX_PREVIEW_BYTES);
  }

  let text = String::from_utf8_lossy(&bytes);
  let mut output = String::new();
  for (line_idx, line) in text.lines().take(MAX_PREVIEW_LINES).enumerate() {
    let _ = std::fmt::Write::write_fmt(&mut output, format_args!("{:>4} {}\n", line_idx + 1, line));
  }
  if output.is_empty() {
    output.push_str("<Empty file>");
  } else if truncated {
    output.push_str("\n…");
  }

  FilePickerPreview::Text(output)
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
