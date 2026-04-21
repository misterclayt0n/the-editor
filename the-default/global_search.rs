use std::{
  collections::HashMap,
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

use fff_search::GrepMode;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{
  BinaryDetection,
  SearcherBuilder,
  sinks,
};
use regex::RegexBuilder;

use crate::{
  FilePickerItem,
  FilePickerItemAction,
  FilePickerOptions,
  fff_backend,
  file_picker::{
    DirectPickerItemMetadata,
    DirectPickerTrackingKind,
    FilePickerSearchMode,
    FilePickerStatusBanner,
    FilePickerStatusBannerKind,
    compute_match_ranges_for_text,
    merge_match_ranges,
    split_picker_path_display,
  },
};

const GLOBAL_SEARCH_MAX_RESULTS: usize = 10_000;
const GLOBAL_SEARCH_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
const GLOBAL_SEARCH_MAX_MATCHES_PER_FILE: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSearchDocumentSnapshot {
  pub path: PathBuf,
  pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlobalSearchMode {
  #[default]
  PlainText,
  Regex,
  Fuzzy,
}

impl GlobalSearchMode {
  pub fn cycle(self) -> Self {
    match self {
      Self::PlainText => Self::Regex,
      Self::Regex => Self::Fuzzy,
      Self::Fuzzy => Self::PlainText,
    }
  }

  pub fn as_fff_mode(self) -> GrepMode {
    match self {
      Self::PlainText => GrepMode::PlainText,
      Self::Regex => GrepMode::Regex,
      Self::Fuzzy => GrepMode::Fuzzy,
    }
  }

  pub fn as_picker_mode(self) -> FilePickerSearchMode {
    match self {
      Self::PlainText => FilePickerSearchMode::PlainText,
      Self::Regex => FilePickerSearchMode::Regex,
      Self::Fuzzy => FilePickerSearchMode::Fuzzy,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSearchOptions {
  pub smart_case:  bool,
  pub file_picker: FilePickerOptions,
  pub mode:        GlobalSearchMode,
}

impl Default for GlobalSearchOptions {
  fn default() -> Self {
    Self {
      smart_case:  true,
      file_picker: FilePickerOptions::default(),
      mode:        GlobalSearchMode::PlainText,
    }
  }
}

#[derive(Debug, Clone)]
pub struct GlobalSearchResponse {
  pub generation:    u64,
  pub query:         String,
  pub items:         Vec<FilePickerItem>,
  pub indexing:      bool,
  pub error:         Option<String>,
  pub search_mode:   FilePickerSearchMode,
  pub status_banner: Option<FilePickerStatusBanner>,
}

#[derive(Debug, Default)]
pub struct GlobalSearchState {
  active:             bool,
  root:               PathBuf,
  options:            GlobalSearchOptions,
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

  pub fn mode(&self) -> GlobalSearchMode {
    self.options.mode
  }

  pub fn cycle_mode(&mut self) -> Option<GlobalSearchMode> {
    if !self.active {
      return None;
    }
    self.options.mode = self.options.mode.cycle();
    Some(self.options.mode)
  }

  pub fn activate(&mut self, root: &Path, options: GlobalSearchOptions) -> Result<(), String> {
    self.cancel_active_worker();
    let (tx, rx) = channel();
    self.active = true;
    self.root = root.to_path_buf();
    self.options = options;
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
    let options = self.options.clone();
    thread::spawn(move || {
      let result = run_global_search_request(generation, root, query, options, documents, cancel);
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
  query: &str,
  absolute: &Path,
  relative_path: &str,
  line_number_one_based: usize,
  line_text: &str,
  match_bytes: &[(usize, usize)],
) -> FilePickerItem {
  let absolute = if absolute.is_absolute() {
    absolute.to_path_buf()
  } else {
    root.join(absolute)
  };

  let line_idx = line_number_one_based.saturating_sub(1);
  let snippet = sanitize_global_search_excerpt(line_text);
  let preview_match_ranges: Vec<(usize, usize)> = match_bytes
    .iter()
    .map(|(start, end)| {
      let start_char = line_byte_to_char_idx(&snippet, *start, false);
      let end_char = line_byte_to_char_idx(&snippet, *end, true).max(start_char.saturating_add(1));
      (start_char, end_char)
    })
    .filter(|(start, end)| end > start)
    .collect();
  let (column_char, preview_col) =
    if let Some((start_char, end_char)) = preview_match_ranges.first().copied() {
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
    row_data: Some(crate::file_picker::FilePickerRowData {
      kind:       crate::file_picker::FilePickerRowKind::LiveGrepMatch,
      severity:   None,
      primary:    snippet,
      secondary:  String::new(),
      tertiary:   String::new(),
      quaternary: String::new(),
      line:       line_number_one_based,
      column:     column_display,
      depth:      0,
    }),
    preview: None,
    payload: Some(crate::file_picker::FilePickerItemPayload::new(
      DirectPickerItemMetadata {
        match_indices:          Arc::from([]),
        primary_match_ranges:   Arc::from(preview_match_ranges.clone()),
        secondary_match_ranges: Arc::from([]),
        preview_match_ranges:   Arc::from(preview_match_ranges),
        tracking:               DirectPickerTrackingKind::FffGrep {
          root:  root.to_path_buf(),
          query: query.to_string(),
        },
      },
    )),
  }
}

fn build_global_search_header_item(
  root: &Path,
  query: &str,
  absolute: &Path,
  relative_path: &str,
) -> FilePickerItem {
  let absolute = if absolute.is_absolute() {
    absolute.to_path_buf()
  } else {
    root.join(absolute)
  };
  let (primary, secondary) = split_picker_path_display(relative_path);

  FilePickerItem {
    absolute:     absolute.clone(),
    display:      relative_path.to_string(),
    icon:         String::new(),
    is_dir:       false,
    display_path: false,
    action:       FilePickerItemAction::GroupHeader { path: absolute },
    preview_path: None,
    preview_line: None,
    preview_col:  None,
    row_data:     Some(crate::file_picker::FilePickerRowData {
      kind: crate::file_picker::FilePickerRowKind::LiveGrepHeader,
      severity: None,
      primary,
      secondary,
      tertiary: String::new(),
      quaternary: String::new(),
      line: 0,
      column: 0,
      depth: 0,
    }),
    preview:      None,
    payload:      Some(crate::file_picker::FilePickerItemPayload::new(
      DirectPickerItemMetadata {
        match_indices:          Arc::from([]),
        primary_match_ranges:   Arc::from([]),
        secondary_match_ranges: Arc::from([]),
        preview_match_ranges:   Arc::from([]),
        tracking:               DirectPickerTrackingKind::FffGrep {
          root:  root.to_path_buf(),
          query: query.to_string(),
        },
      },
    )),
  }
}

#[derive(Debug, Clone)]
struct SearchLineMatch {
  line_number_one_based: usize,
  line_text:             String,
  match_bytes:           Vec<(usize, usize)>,
}

fn build_display_regex(query: &str, smart_case: bool) -> Option<regex::Regex> {
  let case_insensitive = smart_case && !query.chars().any(|ch| ch.is_uppercase());
  RegexBuilder::new(query)
    .case_insensitive(case_insensitive)
    .build()
    .ok()
}

enum UnsavedDocumentMatcher {
  Grep {
    matcher:        grep_regex::RegexMatcher,
    display_regex:  Option<regex::Regex>,
    fallback_error: Option<String>,
  },
  Fuzzy,
}

fn build_unsaved_document_matcher(
  query: &str,
  smart_case: bool,
  mode: GlobalSearchMode,
) -> Result<UnsavedDocumentMatcher, String> {
  match mode {
    GlobalSearchMode::Fuzzy => Ok(UnsavedDocumentMatcher::Fuzzy),
    GlobalSearchMode::PlainText => {
      let escaped = regex::escape(query);
      let matcher = RegexMatcherBuilder::new()
        .case_smart(smart_case)
        .build(&escaped)
        .map_err(|err| format!("Failed to build literal matcher: {err}"))?;
      Ok(UnsavedDocumentMatcher::Grep {
        matcher,
        display_regex: build_display_regex(&escaped, smart_case),
        fallback_error: None,
      })
    },
    GlobalSearchMode::Regex => {
      match RegexMatcherBuilder::new()
        .case_smart(smart_case)
        .build(query)
      {
        Ok(matcher) => {
          Ok(UnsavedDocumentMatcher::Grep {
            matcher,
            display_regex: build_display_regex(query, smart_case),
            fallback_error: None,
          })
        },
        Err(err) => {
          let escaped = regex::escape(query);
          let matcher = RegexMatcherBuilder::new()
            .case_smart(smart_case)
            .build(&escaped)
            .map_err(|fallback_err| {
              format!("Failed to build literal fallback matcher: {fallback_err}")
            })?;
          Ok(UnsavedDocumentMatcher::Grep {
            matcher,
            display_regex: build_display_regex(&escaped, smart_case),
            fallback_error: Some(format!("Invalid regex — showing literal matches: {err}")),
          })
        },
      }
    },
  }
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
    let match_bytes = display_regex
      .map(|regex| {
        regex
          .find_iter(line_content)
          .map(|mat| (mat.start(), mat.end()))
          .collect()
      })
      .unwrap_or_default();
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

fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
  if char_idx == 0 {
    return 0;
  }
  text
    .char_indices()
    .nth(char_idx)
    .map(|(index, _)| index)
    .unwrap_or(text.len())
}

fn collect_fuzzy_search_matches_from_document(
  text: &str,
  query: &str,
  cancel: &Arc<AtomicBool>,
) -> Vec<SearchLineMatch> {
  if text.len() as u64 > GLOBAL_SEARCH_MAX_FILE_SIZE {
    return Vec::new();
  }

  let lower_query = query.to_ascii_lowercase();
  let mut matches = Vec::new();
  for (index, line) in text.lines().enumerate() {
    if cancel.load(Ordering::Relaxed) || matches.len() >= GLOBAL_SEARCH_MAX_MATCHES_PER_FILE {
      break;
    }

    let ranges = compute_match_ranges_for_text(&lower_query, line);
    if ranges.is_empty() {
      continue;
    }

    let match_bytes = merge_match_ranges(
      ranges
        .into_iter()
        .map(|(start, end)| (char_to_byte_idx(line, start), char_to_byte_idx(line, end)))
        .collect(),
    );
    matches.push(SearchLineMatch {
      line_number_one_based: index + 1,
      line_text: line.to_string(),
      match_bytes,
    });
  }
  matches
}

fn extend_items_with_file_matches(
  items: &mut Vec<FilePickerItem>,
  root: &Path,
  query: &str,
  path: &Path,
  relative_path: &str,
  matches: Vec<SearchLineMatch>,
) {
  if matches.is_empty() {
    return;
  }

  items.push(build_global_search_header_item(
    root,
    query,
    path,
    relative_path,
  ));
  for matched in matches {
    if items.len() >= GLOBAL_SEARCH_MAX_RESULTS {
      break;
    }
    items.push(build_global_search_item(
      root,
      query,
      path,
      relative_path,
      matched.line_number_one_based,
      matched.line_text.as_str(),
      &matched.match_bytes,
    ));
  }
}

fn append_backend_grep_matches(
  items: &mut Vec<FilePickerItem>,
  root: &Path,
  query: &str,
  documents_by_path: &HashMap<PathBuf, GlobalSearchDocumentSnapshot>,
  backend_matches: Vec<fff_backend::FffGrepMatch>,
  cancel: &Arc<AtomicBool>,
) {
  let mut grouped_matches: HashMap<PathBuf, Vec<SearchLineMatch>> = HashMap::new();
  let mut file_order = Vec::new();
  for matched in backend_matches {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    let normalized = normalize_search_path(&matched.path);
    if documents_by_path.contains_key(&normalized) {
      continue;
    }
    let entry = grouped_matches
      .entry(matched.path.clone())
      .or_insert_with(|| {
        file_order.push(matched.path.clone());
        Vec::new()
      });
    entry.push(SearchLineMatch {
      line_number_one_based: matched.line_number_one_based,
      line_text:             matched.line_text,
      match_bytes:           matched.match_bytes,
    });
  }

  for path in file_order {
    if cancel.load(Ordering::Relaxed) || items.len() >= GLOBAL_SEARCH_MAX_RESULTS {
      break;
    }
    let Some(matches) = grouped_matches.remove(&path) else {
      continue;
    };
    let relative_path = display_relative_path(&path, root);
    extend_items_with_file_matches(items, root, query, &path, &relative_path, matches);
  }
}

struct GlobalSearchPassResult {
  items:         Vec<FilePickerItem>,
  indexing:      bool,
  status_banner: Option<FilePickerStatusBanner>,
}

fn no_content_matches_banner() -> FilePickerStatusBanner {
  FilePickerStatusBanner {
    kind: FilePickerStatusBannerKind::Info,
    text: "No content matches".to_string(),
  }
}

fn typo_tolerant_banner(mode: GlobalSearchMode) -> FilePickerStatusBanner {
  FilePickerStatusBanner {
    kind: FilePickerStatusBannerKind::Info,
    text: match mode {
      GlobalSearchMode::PlainText => "No exact matches — showing typo-tolerant matches".to_string(),
      GlobalSearchMode::Regex => "No regex matches — showing typo-tolerant matches".to_string(),
      GlobalSearchMode::Fuzzy => "No content matches".to_string(),
    },
  }
}

fn run_global_search_pass(
  root: &Path,
  query: &str,
  smart_case: bool,
  mode: GlobalSearchMode,
  documents_by_path: &HashMap<PathBuf, GlobalSearchDocumentSnapshot>,
  cancel: &Arc<AtomicBool>,
) -> Result<GlobalSearchPassResult, String> {
  let unsaved_matcher = build_unsaved_document_matcher(query, smart_case, mode)?;
  let backend = fff_backend::search_grep_with_mode(
    root,
    query,
    smart_case,
    GLOBAL_SEARCH_MAX_RESULTS,
    mode.as_fff_mode(),
    cancel.as_ref(),
  )?;

  let mut items = Vec::new();
  append_backend_grep_matches(
    &mut items,
    root,
    query,
    documents_by_path,
    backend.matches,
    cancel,
  );

  let overlay_documents: Vec<_> = documents_by_path.values().cloned().collect();
  if !cancel.load(Ordering::Relaxed) && items.len() < GLOBAL_SEARCH_MAX_RESULTS {
    for document in overlay_documents {
      if cancel.load(Ordering::Relaxed) || items.len() >= GLOBAL_SEARCH_MAX_RESULTS {
        break;
      }
      let normalized_document_path = normalize_search_path(&document.path);
      if !normalized_document_path.starts_with(root) {
        continue;
      }

      let relative_path = display_relative_path(&normalized_document_path, root);
      let matches = match &unsaved_matcher {
        UnsavedDocumentMatcher::Grep {
          matcher,
          display_regex,
          ..
        } => {
          match collect_search_matches_from_document(
            &document.path,
            matcher,
            display_regex.as_ref(),
            document.text.as_str(),
            cancel,
          ) {
            Ok(matches) => matches,
            Err(_) => continue,
          }
        },
        UnsavedDocumentMatcher::Fuzzy => {
          collect_fuzzy_search_matches_from_document(document.text.as_str(), query, cancel)
        },
      };
      extend_items_with_file_matches(
        &mut items,
        root,
        query,
        &document.path,
        &relative_path,
        matches,
      );
    }
  }

  let mut status_banner = match &unsaved_matcher {
    UnsavedDocumentMatcher::Grep { fallback_error, .. } => {
      fallback_error.clone().map(|text| {
        FilePickerStatusBanner {
          kind: FilePickerStatusBannerKind::Warning,
          text,
        }
      })
    },
    UnsavedDocumentMatcher::Fuzzy => None,
  };

  if status_banner.is_none() {
    status_banner = backend.error.clone().map(|text| {
      FilePickerStatusBanner {
        kind: FilePickerStatusBannerKind::Warning,
        text,
      }
    });
  }

  Ok(GlobalSearchPassResult {
    items,
    indexing: backend.indexing,
    status_banner,
  })
}

fn run_global_search_request(
  generation: u64,
  root: PathBuf,
  query: String,
  options: GlobalSearchOptions,
  documents: Vec<GlobalSearchDocumentSnapshot>,
  cancel: Arc<AtomicBool>,
) -> GlobalSearchResponse {
  let search_mode = options.mode.as_picker_mode();
  if query.trim().is_empty() {
    return GlobalSearchResponse {
      generation,
      query,
      items: Vec::new(),
      indexing: false,
      error: None,
      search_mode,
      status_banner: None,
    };
  }

  let normalized_root = normalize_search_path(&root);
  let documents_by_path: HashMap<PathBuf, GlobalSearchDocumentSnapshot> = documents
    .into_iter()
    .filter(|document| !document.path.as_os_str().is_empty())
    .map(|document| (normalize_search_path(&document.path), document))
    .collect();

  fff_backend::track_grep_query(normalized_root.as_path(), &query);

  let primary = match run_global_search_pass(
    normalized_root.as_path(),
    &query,
    options.smart_case,
    options.mode,
    &documents_by_path,
    &cancel,
  ) {
    Ok(result) => result,
    Err(err) => {
      return GlobalSearchResponse {
        generation,
        query,
        items: Vec::new(),
        indexing: false,
        error: Some(err),
        search_mode,
        status_banner: None,
      };
    },
  };

  let mut items = primary.items;
  let mut indexing = primary.indexing;
  let mut status_banner = primary.status_banner;

  if !cancel.load(Ordering::Relaxed) && items.is_empty() {
    if options.mode != GlobalSearchMode::Fuzzy {
      if let Ok(fuzzy) = run_global_search_pass(
        normalized_root.as_path(),
        &query,
        options.smart_case,
        GlobalSearchMode::Fuzzy,
        &documents_by_path,
        &cancel,
      ) {
        indexing |= fuzzy.indexing;
        if !fuzzy.items.is_empty() {
          items = fuzzy.items;
          status_banner = Some(typo_tolerant_banner(options.mode));
        }
      }
    }

    if items.is_empty() {
      status_banner = status_banner.or_else(|| Some(no_content_matches_banner()));
    }
  }

  GlobalSearchResponse {
    generation,
    query,
    items,
    indexing,
    error: None,
    search_mode,
    status_banner,
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
      GlobalSearchOptions::default(),
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
      GlobalSearchOptions::default(),
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

  #[test]
  fn global_search_falls_back_to_literal_for_invalid_regex() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("alpha.txt"), "a[needle\n").unwrap();

    let response = run_global_search_request(
      1,
      root.to_path_buf(),
      "a[needle".to_string(),
      GlobalSearchOptions {
        mode: GlobalSearchMode::Regex,
        ..GlobalSearchOptions::default()
      },
      Vec::new(),
      Arc::new(AtomicBool::new(false)),
    );

    assert_eq!(response.items.len(), 2);
    assert!(response.error.is_none());
    assert!(response.status_banner.is_some());
    assert_eq!(response.items[1].display, "alpha.txt\t1\t1\ta[needle");
  }

  #[test]
  fn global_search_reports_no_content_matches_without_switching_to_file_results() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("regexToBigramQuery.rs"), "fn main() {}\n").unwrap();

    let response = run_global_search_request(
      1,
      root.to_path_buf(),
      "regexToBigramQuery".to_string(),
      GlobalSearchOptions::default(),
      Vec::new(),
      Arc::new(AtomicBool::new(false)),
    );

    assert!(response.error.is_none());
    assert_eq!(response.search_mode, FilePickerSearchMode::PlainText);
    assert!(response.items.is_empty());
    assert_eq!(
      response
        .status_banner
        .as_ref()
        .map(|banner| banner.text.as_str()),
      Some("No content matches")
    );
  }

  #[test]
  fn global_search_falls_back_to_typo_tolerant_matches() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("alpha.txt"), "const schema = value;\n").unwrap();

    let response = run_global_search_request(
      1,
      root.to_path_buf(),
      "shcema".to_string(),
      GlobalSearchOptions::default(),
      Vec::new(),
      Arc::new(AtomicBool::new(false)),
    );

    assert!(response.error.is_none());
    assert_eq!(response.search_mode, FilePickerSearchMode::PlainText);
    assert_eq!(
      response
        .status_banner
        .as_ref()
        .map(|banner| banner.text.as_str()),
      Some("No exact matches — showing typo-tolerant matches")
    );
    assert_eq!(response.items.len(), 2);
    assert!(matches!(
      response.items[0].action,
      FilePickerItemAction::GroupHeader { .. }
    ));
    assert!(response.items[1].display.contains("schema"));
  }
}
