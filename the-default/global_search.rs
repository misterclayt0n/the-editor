use std::{
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    RwLock,
    mpsc::{
      Receiver,
      Sender,
      TryRecvError,
      channel,
    },
  },
  thread,
};

use fff_core::{
  SharedFrecency as FffSharedFrecency,
  SharedPicker as FffSharedPicker,
  file_picker::FilePicker as FffFilePicker,
  grep::{
    GrepMode as FffGrepMode,
    GrepSearchOptions as FffGrepSearchOptions,
    grep_search as fff_grep_search,
    parse_grep_query as fff_parse_grep_query,
  },
};

use crate::{
  FilePickerItem,
  FilePickerItemAction,
};

const GLOBAL_SEARCH_MAX_RESULTS: usize = 10_000;
const GLOBAL_SEARCH_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
const GLOBAL_SEARCH_MAX_MATCHES_PER_FILE: usize = 200;
const GLOBAL_SEARCH_TIME_BUDGET_MS: u64 = 120;

#[derive(Debug, Clone)]
pub struct GlobalSearchResponse {
  pub generation: u64,
  pub query:      String,
  pub items:      Vec<FilePickerItem>,
  pub indexing:   bool,
  pub error:      Option<String>,
}

#[derive(Debug, Default)]
pub struct GlobalSearchState {
  active:             bool,
  root:               PathBuf,
  shared_picker:      Option<FffSharedPicker>,
  shared_frecency:    Option<FffSharedFrecency>,
  generation:         u64,
  pending_generation: Option<u64>,
  result_tx:          Option<Sender<GlobalSearchResponse>>,
  result_rx:          Option<Receiver<GlobalSearchResponse>>,
}

impl GlobalSearchState {
  pub fn is_active(&self) -> bool {
    self.active
  }

  pub fn activate(&mut self, root: &Path) -> Result<(), String> {
    self.ensure_index(root)?;
    let (tx, rx) = channel();
    self.result_tx = Some(tx);
    self.result_rx = Some(rx);
    self.active = true;
    self.generation = 0;
    self.pending_generation = None;
    Ok(())
  }

  pub fn deactivate(&mut self) {
    self.active = false;
    self.pending_generation = None;
    self.result_tx = None;
    self.result_rx = None;
  }

  pub fn schedule(&mut self, query: String) {
    if !self.active {
      return;
    }
    let Some(shared_picker) = self.shared_picker.clone() else {
      return;
    };
    let Some(response_tx) = self.result_tx.clone() else {
      return;
    };

    self.generation = self.generation.wrapping_add(1);
    let generation = self.generation;
    self.pending_generation = Some(generation);

    let root = self.root.clone();
    thread::spawn(move || {
      let result = run_global_search_request(generation, root, query, shared_picker);
      let _ = response_tx.send(result);
    });
  }

  pub fn poll_latest(&mut self) -> Option<GlobalSearchResponse> {
    if !self.active {
      return None;
    }

    let mut latest: Option<GlobalSearchResponse> = None;
    {
      let Some(rx) = self.result_rx.as_ref() else {
        return None;
      };
      loop {
        match rx.try_recv() {
          Ok(response) => {
            if latest
              .as_ref()
              .is_none_or(|current| response.generation >= current.generation)
            {
              latest = Some(response);
            }
          },
          Err(TryRecvError::Empty) => break,
          Err(TryRecvError::Disconnected) => {
            self.deactivate();
            return None;
          },
        }
      }
    }

    let response = latest?;
    if self
      .pending_generation
      .is_some_and(|pending| response.generation < pending)
    {
      return None;
    }

    self.pending_generation = None;
    Some(response)
  }

  pub fn root(&self) -> &Path {
    &self.root
  }

  fn stop_index(&mut self) {
    let Some(shared_picker) = self.shared_picker.take() else {
      self.shared_frecency = None;
      return;
    };

    if let Ok(mut guard) = shared_picker.write()
      && let Some(mut picker) = guard.take()
    {
      picker.stop_background_monitor();
    }

    self.shared_frecency = None;
  }

  fn ensure_index(&mut self, root: &Path) -> Result<(), String> {
    if self.shared_picker.is_some() && self.root == root {
      return Ok(());
    }

    self.stop_index();

    let shared_picker: FffSharedPicker = Arc::new(RwLock::new(None));
    let shared_frecency: FffSharedFrecency = Arc::new(RwLock::new(None));
    FffFilePicker::new_with_shared_state(
      root.to_string_lossy().to_string(),
      false,
      Arc::clone(&shared_picker),
      Arc::clone(&shared_frecency),
    )
    .map_err(|err| err.to_string())?;

    self.shared_picker = Some(shared_picker);
    self.shared_frecency = Some(shared_frecency);
    self.root = root.to_path_buf();
    Ok(())
  }
}

impl Drop for GlobalSearchState {
  fn drop(&mut self) {
    self.stop_index();
  }
}

fn clamp_utf8_boundary(text: &str, idx: usize, round_up: bool) -> usize {
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

fn line_byte_to_char_idx(line: &str, byte_idx: usize, round_up: bool) -> usize {
  let clamped = clamp_utf8_boundary(line, byte_idx, round_up);
  line[..clamped].chars().count()
}

fn sanitize_global_search_excerpt(line: &str) -> String {
  line.trim_end_matches(['\r', '\n']).replace('\t', " ")
}

fn build_global_search_item(
  root: &Path,
  absolute: &Path,
  relative_path: Option<&str>,
  line_number_one_based: usize,
  line_text: &str,
  match_bytes: Option<(usize, usize)>,
) -> FilePickerItem {
  let absolute = if absolute.is_absolute() {
    absolute.to_path_buf()
  } else {
    root.join(absolute)
  };

  let line_idx = line_number_one_based.saturating_sub(1);
  let snippet = sanitize_global_search_excerpt(line_text);
  let (column_char, preview_col) = if let Some((start, end)) = match_bytes {
    let start_char = line_byte_to_char_idx(&snippet, start, false);
    let end_char = line_byte_to_char_idx(&snippet, end, true).max(start_char.saturating_add(1));
    (Some(start_char), Some((start_char, end_char)))
  } else {
    (None, None)
  };
  let relative = relative_path.map(str::to_string).unwrap_or_else(|| {
    absolute
      .strip_prefix(root)
      .unwrap_or(&absolute)
      .display()
      .to_string()
  });
  let column_display = column_char.map(|col| col + 1).unwrap_or(1);
  let display = format!(
    "{relative}\t{}\t{column_display}\t{}",
    line_idx.saturating_add(1),
    snippet
  );
  let icon = crate::file_picker::file_picker_icon_name_for_path(&absolute).to_string();

  FilePickerItem {
    absolute: absolute.clone(),
    display,
    icon,
    is_dir: false,
    display_path: false,
    action: FilePickerItemAction::OpenLocation {
      path:        absolute.clone(),
      cursor_char: 0,
      line:        line_idx,
      column:      column_char,
    },
    preview_path: Some(absolute),
    preview_line: Some(line_idx),
    preview_col,
  }
}

fn build_global_search_header_item(
  root: &Path,
  absolute: &Path,
  relative_path: Option<&str>,
) -> FilePickerItem {
  let absolute = if absolute.is_absolute() {
    absolute.to_path_buf()
  } else {
    root.join(absolute)
  };
  let relative = relative_path.map(str::to_string).unwrap_or_else(|| {
    absolute
      .strip_prefix(root)
      .unwrap_or(&absolute)
      .display()
      .to_string()
  });
  let icon = crate::file_picker::file_picker_icon_name_for_path(&absolute).to_string();

  FilePickerItem {
    absolute: absolute.clone(),
    display: relative,
    icon,
    is_dir: false,
    display_path: false,
    action: FilePickerItemAction::GroupHeader { path: absolute },
    preview_path: None,
    preview_line: None,
    preview_col: None,
  }
}

fn run_global_search_request(
  generation: u64,
  root: PathBuf,
  query: String,
  shared_picker: FffSharedPicker,
) -> GlobalSearchResponse {
  let query = query.trim().to_string();
  if query.is_empty() {
    return GlobalSearchResponse {
      generation,
      query,
      items: Vec::new(),
      indexing: false,
      error: None,
    };
  }

  let picker_guard = match shared_picker.read() {
    Ok(guard) => guard,
    Err(_) => {
      return GlobalSearchResponse {
        generation,
        query,
        items: Vec::new(),
        indexing: false,
        error: Some("failed to read global search index".to_string()),
      };
    },
  };
  let Some(picker) = picker_guard.as_ref() else {
    return GlobalSearchResponse {
      generation,
      query,
      items: Vec::new(),
      indexing: true,
      error: None,
    };
  };

  let indexing = picker.is_scan_active();
  let files = picker.get_files();
  if files.is_empty() {
    return GlobalSearchResponse {
      generation,
      query,
      items: Vec::new(),
      indexing,
      error: None,
    };
  }

  let parsed = fff_parse_grep_query(&query);
  let options = FffGrepSearchOptions {
    max_file_size:        GLOBAL_SEARCH_MAX_FILE_SIZE,
    max_matches_per_file: GLOBAL_SEARCH_MAX_MATCHES_PER_FILE,
    smart_case:           true,
    file_offset:          0,
    page_limit:           GLOBAL_SEARCH_MAX_RESULTS,
    mode:                 FffGrepMode::PlainText,
    time_budget_ms:       GLOBAL_SEARCH_TIME_BUDGET_MS,
  };
  let result = fff_grep_search(files, &query, parsed, &options);

  let mut items = Vec::with_capacity(result.matches.len().min(GLOBAL_SEARCH_MAX_RESULTS));
  let mut previous_relative_path: Option<String> = None;
  for matched in result.matches {
    let Some(file) = result.files.get(matched.file_index).copied() else {
      continue;
    };
    let relative_path = file.relative_path.as_str();
    if previous_relative_path.as_deref() != Some(relative_path) {
      items.push(build_global_search_header_item(
        &root,
        file.path.as_path(),
        Some(relative_path),
      ));
      previous_relative_path = Some(relative_path.to_string());
    }
    let line_number_one_based = matched.line_number.min(usize::MAX as u64) as usize;
    let match_bytes = matched
      .match_byte_offsets
      .first()
      .map(|(start, end)| (*start as usize, *end as usize));
    items.push(build_global_search_item(
      &root,
      file.path.as_path(),
      Some(relative_path),
      line_number_one_based,
      matched.line_content.as_str(),
      match_bytes,
    ));
    if items.len() >= GLOBAL_SEARCH_MAX_RESULTS {
      break;
    }
  }

  GlobalSearchResponse {
    generation,
    query,
    items,
    indexing,
    error: None,
  }
}
