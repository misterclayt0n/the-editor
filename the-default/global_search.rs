use std::{
  collections::HashMap,
  fs,
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
      Receiver,
      Sender,
      TryRecvError,
      channel,
    },
  },
  thread,
};

use grep_regex::RegexMatcherBuilder;
use grep_searcher::{
  BinaryDetection,
  SearcherBuilder,
  sinks,
};
use regex::RegexBuilder;

use crate::{
  FilePickerConfig,
  FilePickerItem,
  FilePickerItemAction,
  file_picker::build_file_walk_builder,
};

const GLOBAL_SEARCH_MAX_RESULTS: usize = 10_000;
const GLOBAL_SEARCH_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
const GLOBAL_SEARCH_MAX_MATCHES_PER_FILE: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSearchDocumentSnapshot {
  pub path: PathBuf,
  pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSearchConfig {
  pub smart_case:  bool,
  pub file_picker: FilePickerConfig,
}

impl Default for GlobalSearchConfig {
  fn default() -> Self {
    Self {
      smart_case:  true,
      file_picker: FilePickerConfig::default(),
    }
  }
}

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
  config:             GlobalSearchConfig,
  generation:         u64,
  pending_generation: Option<u64>,
  cancel:             Option<Arc<AtomicBool>>,
  result_tx:          Option<Sender<GlobalSearchResponse>>,
  result_rx:          Option<Receiver<GlobalSearchResponse>>,
}

impl GlobalSearchState {
  pub fn is_active(&self) -> bool {
    self.active
  }

  pub fn activate(&mut self, root: &Path, config: GlobalSearchConfig) -> Result<(), String> {
    self.cancel_active_worker();
    let (tx, rx) = channel();
    self.active = true;
    self.root = root.to_path_buf();
    self.config = config;
    self.generation = 0;
    self.pending_generation = None;
    self.result_tx = Some(tx);
    self.result_rx = Some(rx);
    Ok(())
  }

  pub fn deactivate(&mut self) {
    self.cancel_active_worker();
    self.active = false;
    self.pending_generation = None;
    self.result_tx = None;
    self.result_rx = None;
  }

  pub fn cancel_pending(&mut self) {
    if !self.active {
      return;
    }
    self.cancel_active_worker();
    self.generation = self.generation.wrapping_add(1);
    self.pending_generation = Some(self.generation);
  }

  pub fn schedule(&mut self, query: String, documents: Vec<GlobalSearchDocumentSnapshot>) {
    if !self.active {
      return;
    }
    let Some(response_tx) = self.result_tx.clone() else {
      return;
    };

    self.cancel_active_worker();
    self.generation = self.generation.wrapping_add(1);
    let generation = self.generation;
    self.pending_generation = Some(generation);

    let cancel = Arc::new(AtomicBool::new(false));
    self.cancel = Some(cancel.clone());

    let root = self.root.clone();
    let config = self.config.clone();
    thread::spawn(move || {
      let result = run_global_search_request(generation, root, query, config, documents, cancel);
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

    if self
      .pending_generation
      .is_some_and(|pending| response.generation == pending)
    {
      self.pending_generation = None;
      self.cancel = None;
    }
    Some(response)
  }

  fn cancel_active_worker(&mut self) {
    if let Some(cancel) = self.cancel.take() {
      cancel.store(true, Ordering::Relaxed);
    }
  }
}

fn normalize_search_path(path: &Path) -> PathBuf {
  path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn display_relative_path(path: &Path, root: &Path) -> String {
  let mut text = path
    .strip_prefix(root)
    .unwrap_or(path)
    .display()
    .to_string();
  if std::path::MAIN_SEPARATOR != '/' {
    text = text.replace(std::path::MAIN_SEPARATOR, "/");
  }
  text
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
  relative_path: &str,
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
  let column_display = column_char.map(|col| col + 1).unwrap_or(1);
  let display = format!(
    "{relative_path}\t{}\t{column_display}\t{}",
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
  relative_path: &str,
) -> FilePickerItem {
  let absolute = if absolute.is_absolute() {
    absolute.to_path_buf()
  } else {
    root.join(absolute)
  };
  let icon = crate::file_picker::file_picker_icon_name_for_path(&absolute).to_string();

  FilePickerItem {
    absolute: absolute.clone(),
    display: relative_path.to_string(),
    icon,
    is_dir: false,
    display_path: false,
    action: FilePickerItemAction::GroupHeader { path: absolute },
    preview_path: None,
    preview_line: None,
    preview_col: None,
  }
}

#[derive(Debug, Clone)]
struct SearchLineMatch {
  line_number_one_based: usize,
  line_text:             String,
  match_bytes:           Option<(usize, usize)>,
}

fn build_display_regex(query: &str, smart_case: bool) -> Option<regex::Regex> {
  let case_insensitive = smart_case && !query.chars().any(|ch| ch.is_uppercase());
  RegexBuilder::new(query)
    .case_insensitive(case_insensitive)
    .build()
    .ok()
}

fn collect_search_matches_in_bytes(
  path: &Path,
  matcher: &grep_regex::RegexMatcher,
  display_regex: Option<&regex::Regex>,
  haystack: &[u8],
  cancel: &Arc<AtomicBool>,
) -> Result<Vec<SearchLineMatch>, String> {
  let mut searcher = SearcherBuilder::new()
    .binary_detection(BinaryDetection::quit(b'\x00'))
    .line_number(true)
    .build();
  let mut matches = Vec::new();

  let sink = sinks::UTF8(|line_num, line_content| {
    if cancel.load(Ordering::Relaxed) || matches.len() >= GLOBAL_SEARCH_MAX_MATCHES_PER_FILE {
      return Ok(false);
    }
    let line_text = line_content.to_string();
    let match_bytes =
      display_regex.and_then(|regex| regex.find(line_content).map(|mat| (mat.start(), mat.end())));
    matches.push(SearchLineMatch {
      line_number_one_based: line_num as usize,
      line_text,
      match_bytes,
    });
    Ok(true)
  });

  searcher
    .search_slice(matcher, haystack, sink)
    .map_err(|err| format!("{}: {err}", path.display()))?;
  Ok(matches)
}

fn collect_search_matches_from_path(
  path: &Path,
  matcher: &grep_regex::RegexMatcher,
  display_regex: Option<&regex::Regex>,
  cancel: &Arc<AtomicBool>,
) -> Result<Vec<SearchLineMatch>, String> {
  let file_len = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
  if file_len > GLOBAL_SEARCH_MAX_FILE_SIZE {
    return Ok(Vec::new());
  }

  let bytes = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
  collect_search_matches_in_bytes(path, matcher, display_regex, &bytes, cancel)
}

fn collect_search_matches_from_document(
  path: &Path,
  matcher: &grep_regex::RegexMatcher,
  display_regex: Option<&regex::Regex>,
  text: &str,
  cancel: &Arc<AtomicBool>,
) -> Result<Vec<SearchLineMatch>, String> {
  if text.len() as u64 > GLOBAL_SEARCH_MAX_FILE_SIZE {
    return Ok(Vec::new());
  }
  collect_search_matches_in_bytes(path, matcher, display_regex, text.as_bytes(), cancel)
}

fn extend_items_with_file_matches(
  items: &mut Vec<FilePickerItem>,
  root: &Path,
  path: &Path,
  relative_path: &str,
  matches: Vec<SearchLineMatch>,
) {
  if matches.is_empty() {
    return;
  }

  items.push(build_global_search_header_item(root, path, relative_path));
  for matched in matches {
    if items.len() >= GLOBAL_SEARCH_MAX_RESULTS {
      break;
    }
    items.push(build_global_search_item(
      root,
      path,
      relative_path,
      matched.line_number_one_based,
      matched.line_text.as_str(),
      matched.match_bytes,
    ));
  }
}

fn run_global_search_request(
  generation: u64,
  root: PathBuf,
  query: String,
  config: GlobalSearchConfig,
  documents: Vec<GlobalSearchDocumentSnapshot>,
  cancel: Arc<AtomicBool>,
) -> GlobalSearchResponse {
  if query.trim().is_empty() {
    return GlobalSearchResponse {
      generation,
      query,
      items: Vec::new(),
      indexing: false,
      error: None,
    };
  }

  let matcher = match RegexMatcherBuilder::new()
    .case_smart(config.smart_case)
    .build(&query)
  {
    Ok(matcher) => matcher,
    Err(err) => {
      return GlobalSearchResponse {
        generation,
        query,
        items: Vec::new(),
        indexing: false,
        error: Some(format!("Failed to compile regex: {err}")),
      };
    },
  };
  let display_regex = build_display_regex(&query, config.smart_case);
  let mut documents_by_path: HashMap<PathBuf, GlobalSearchDocumentSnapshot> = documents
    .into_iter()
    .filter(|document| !document.path.as_os_str().is_empty())
    .map(|document| (normalize_search_path(&document.path), document))
    .collect();

  let mut items = Vec::new();
  let mut walker = build_file_walk_builder(&root, &config.file_picker).build();
  for entry in &mut walker {
    if cancel.load(Ordering::Relaxed) || items.len() >= GLOBAL_SEARCH_MAX_RESULTS {
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
    let normalized = normalize_search_path(&path);
    let document = documents_by_path.remove(&normalized);
    let relative_path = display_relative_path(&path, &root);
    let matches = if let Some(document) = document {
      collect_search_matches_from_document(
        &document.path,
        &matcher,
        display_regex.as_ref(),
        document.text.as_str(),
        &cancel,
      )
    } else {
      collect_search_matches_from_path(&path, &matcher, display_regex.as_ref(), &cancel)
    };

    let matches = match matches {
      Ok(matches) => matches,
      Err(_) => continue,
    };
    extend_items_with_file_matches(&mut items, &root, &path, &relative_path, matches);
  }

  if !cancel.load(Ordering::Relaxed) && items.len() < GLOBAL_SEARCH_MAX_RESULTS {
    for document in documents_by_path.into_values() {
      if cancel.load(Ordering::Relaxed) || items.len() >= GLOBAL_SEARCH_MAX_RESULTS {
        break;
      }
      if !document.path.starts_with(&root) {
        continue;
      }

      let relative_path = display_relative_path(&document.path, &root);
      let matches = match collect_search_matches_from_document(
        &document.path,
        &matcher,
        display_regex.as_ref(),
        document.text.as_str(),
        &cancel,
      ) {
        Ok(matches) => matches,
        Err(_) => continue,
      };
      extend_items_with_file_matches(&mut items, &root, &document.path, &relative_path, matches);
    }
  }

  GlobalSearchResponse {
    generation,
    query,
    items,
    indexing: false,
    error: None,
  }
}

#[cfg(test)]
mod tests {
  use std::fs;

  use tempfile::tempdir;

  use super::*;

  #[test]
  fn global_search_matches_workspace_files_and_groups_results() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("alpha.txt"), "first needle\nsecond\n").unwrap();
    fs::write(root.join("beta.txt"), "needle again\n").unwrap();

    let response = run_global_search_request(
      1,
      root.to_path_buf(),
      "needle".to_string(),
      GlobalSearchConfig::default(),
      Vec::new(),
      Arc::new(AtomicBool::new(false)),
    );

    assert!(response.error.is_none());
    assert_eq!(response.items.len(), 4);
    assert!(matches!(
      response.items[0].action,
      FilePickerItemAction::GroupHeader { .. }
    ));
    assert_eq!(response.items[1].display, "alpha.txt\t1\t7\tfirst needle");
    assert!(matches!(
      response.items[2].action,
      FilePickerItemAction::GroupHeader { .. }
    ));
    assert_eq!(response.items[3].display, "beta.txt\t1\t1\tneedle again");
  }

  #[test]
  fn global_search_uses_unsaved_buffer_text_over_disk_contents() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let path = root.join("alpha.txt");
    fs::write(&path, "disk only\n").unwrap();

    let response = run_global_search_request(
      1,
      root.to_path_buf(),
      "buffer".to_string(),
      GlobalSearchConfig::default(),
      vec![GlobalSearchDocumentSnapshot {
        path: path.clone(),
        text: "buffer match\n".to_string(),
      }],
      Arc::new(AtomicBool::new(false)),
    );

    assert!(response.error.is_none());
    assert_eq!(response.items.len(), 2);
    assert!(matches!(
      response.items[0].action,
      FilePickerItemAction::GroupHeader { .. }
    ));
    assert_eq!(response.items[1].display, "alpha.txt\t1\t1\tbuffer match");
  }
}
