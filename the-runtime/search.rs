use std::{
  collections::BTreeMap,
  fs,
  num::NonZeroU64,
  path::{
    Path,
    PathBuf,
  },
};

use fff_search::{
  FFFMode,
  FilePicker,
  FilePickerOptions,
  FileSearchConfig,
  FrecencyTracker,
  FuzzySearchOptions,
  GrepConfig,
  GrepSearchOptions,
  MixedItemRef,
  PaginationArgs,
  QueryParser,
  QueryTracker,
  SharedFilePicker,
  SharedFrecency,
  SharedQueryTracker,
};
use tempfile::TempDir;
use thiserror::Error;

use crate::resource::normalize_file_path;

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize,
)]
pub struct SearchIndexHandle(NonZeroU64);

impl SearchIndexHandle {
  pub fn new(id: u64) -> Option<Self> {
    NonZeroU64::new(id).map(Self)
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }

  fn next(counter: &mut u64) -> Self {
    *counter = counter.wrapping_add(1).max(1);
    Self::new(*counter).expect("search index handle must be non-zero")
  }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexReady {
  pub index_id:  u64,
  pub root_path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexStatus {
  pub index_id:        u64,
  pub root_path:       String,
  pub scanning:        bool,
  pub watcher_ready:   bool,
  pub warmup_complete: bool,
  pub indexed_files:   usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchItem {
  pub kind:          &'static str,
  pub path:          String,
  pub relative_path: String,
  pub file_name:     String,
  pub parent_path:   String,
  pub score:         i32,
  pub match_type:    String,
  pub exact_match:   bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirSearchItem {
  pub kind:          &'static str,
  pub path:          String,
  pub relative_path: String,
  pub name:          String,
  pub score:         i32,
  pub match_type:    String,
  pub exact_match:   bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MixedSearchItem {
  #[serde(rename = "file")]
  File {
    path:          String,
    relative_path: String,
    file_name:     String,
    parent_path:   String,
    score:         i32,
    match_type:    String,
    exact_match:   bool,
  },
  #[serde(rename = "dir")]
  Dir {
    path:          String,
    relative_path: String,
    name:          String,
    score:         i32,
    match_type:    String,
    exact_match:   bool,
  },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GrepSearchSpan {
  pub start: u32,
  pub end:   u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreviewRow {
  pub text:        String,
  pub line_number: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreviewSyntaxSpan {
  pub row:       usize,
  pub start_col: usize,
  pub end_col:   usize,
  pub kind:      usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepSearchItem {
  pub kind:          &'static str,
  pub path:          String,
  pub relative_path: String,
  pub file_name:     String,
  pub parent_path:   String,
  pub line:          u64,
  pub column:        usize,
  pub text:          String,
  pub score:         i32,
  pub spans:         Vec<GrepSearchSpan>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchResult {
  pub index_id:        u64,
  pub root_path:       String,
  pub query:           String,
  pub total_matched:   usize,
  pub total_files:     usize,
  pub scanning:        bool,
  pub watcher_ready:   bool,
  pub warmup_complete: bool,
  pub items:           Vec<FileSearchItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirSearchResult {
  pub index_id:        u64,
  pub root_path:       String,
  pub query:           String,
  pub total_matched:   usize,
  pub total_dirs:      usize,
  pub scanning:        bool,
  pub watcher_ready:   bool,
  pub warmup_complete: bool,
  pub items:           Vec<DirSearchItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixedSearchResult {
  pub index_id:        u64,
  pub root_path:       String,
  pub query:           String,
  pub total_matched:   usize,
  pub total_files:     usize,
  pub total_dirs:      usize,
  pub scanning:        bool,
  pub watcher_ready:   bool,
  pub warmup_complete: bool,
  pub items:           Vec<MixedSearchItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepSearchResult {
  pub index_id:            u64,
  pub root_path:           String,
  pub query:               String,
  pub total_files:         usize,
  pub total_files_searched: usize,
  pub files_with_matches:  usize,
  pub next_offset:         usize,
  pub regex_fallback_error: Option<String>,
  pub scanning:            bool,
  pub watcher_ready:       bool,
  pub warmup_complete:     bool,
  pub items:               Vec<GrepSearchItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreviewResult {
  pub path:       String,
  pub text:       String,
  pub start_line: usize,
  pub end_line:   usize,
  pub truncated:  bool,
  pub binary:     bool,
  pub rows:       Option<Vec<TextPreviewRow>>,
  pub syntax:     Option<Vec<TextPreviewSyntaxSpan>>,
}

#[derive(Debug, Error)]
pub enum SearchError {
  #[error(transparent)]
  Io(#[from] std::io::Error),

  #[error(transparent)]
  Fff(#[from] fff_search::Error),

  #[error("unknown search index {0:?}")]
  UnknownIndex(SearchIndexHandle),
}

struct SearchIndex {
  handle:               SearchIndexHandle,
  root_path:            PathBuf,
  shared_picker:        SharedFilePicker,
  shared_frecency:      SharedFrecency,
  shared_query_tracker: SharedQueryTracker,
  _db_dir:              TempDir,
}

#[derive(Default)]
pub struct SearchManager {
  next_index_id: u64,
  roots:         BTreeMap<PathBuf, SearchIndexHandle>,
  indices:       BTreeMap<SearchIndexHandle, SearchIndex>,
}

impl SearchManager {
  pub fn ensure_index(&mut self, root_path: &Path) -> Result<SearchIndexReady, SearchError> {
    let normalized = normalized_root_path(root_path)?;
    if let Some(handle) = self.roots.get(&normalized).copied() {
      return Ok(SearchIndexReady {
        index_id:  handle.get(),
        root_path: normalized.to_string_lossy().into_owned(),
      });
    }

    let db_dir = tempfile::tempdir()?;
    let shared_picker = SharedFilePicker::default();
    let shared_frecency = SharedFrecency::default();
    let shared_query_tracker = SharedQueryTracker::default();

    shared_frecency.init(FrecencyTracker::open(db_dir.path().join("frecency"))?)?;
    shared_query_tracker.init(QueryTracker::open(db_dir.path().join("queries"))?)?;

    FilePicker::new_with_shared_state(
      shared_picker.clone(),
      shared_frecency.clone(),
      FilePickerOptions {
        base_path: normalized.to_string_lossy().into_owned(),
        mode: FFFMode::default(),
        ..Default::default()
      },
    )?;

    let handle = SearchIndexHandle::next(&mut self.next_index_id);
    self.roots.insert(normalized.clone(), handle);
    self.indices.insert(handle, SearchIndex {
      handle,
      root_path: normalized.clone(),
      shared_picker,
      shared_frecency,
      shared_query_tracker,
      _db_dir: db_dir,
    });

    Ok(SearchIndexReady {
      index_id:  handle.get(),
      root_path: normalized.to_string_lossy().into_owned(),
    })
  }

  pub fn status(&self, index: SearchIndexHandle) -> Result<SearchIndexStatus, SearchError> {
    let index = self.index(index)?;
    let picker_guard = index.shared_picker.read()?;
    let picker = picker_guard.as_ref().ok_or(SearchError::UnknownIndex(index.handle))?;
    let progress = picker.get_scan_progress();
    Ok(SearchIndexStatus {
      index_id:        index.handle.get(),
      root_path:       index.root_path.to_string_lossy().into_owned(),
      scanning:        progress.is_scanning,
      watcher_ready:   progress.is_watcher_ready,
      warmup_complete: progress.is_warmup_complete,
      indexed_files:   picker.get_files().len(),
    })
  }

  pub fn refresh(&self, index: SearchIndexHandle) -> Result<(), SearchError> {
    let index = self.index(index)?;
    index
      .shared_picker
      .trigger_full_rescan_async(&index.shared_frecency)?;
    Ok(())
  }

  pub fn query_files(
    &self,
    index: SearchIndexHandle,
    query: &str,
    offset: usize,
    limit: usize,
    current_path: Option<&str>,
  ) -> Result<FileSearchResult, SearchError> {
    let index = self.index(index)?;
    let picker_guard = index.shared_picker.read()?;
    let picker = picker_guard.as_ref().ok_or(SearchError::UnknownIndex(index.handle))?;
    let query_tracker = index.shared_query_tracker.read()?;
    let parser = QueryParser::new(FileSearchConfig);
    let parsed = parser.parse(query);
    let results = picker.fuzzy_search(
      &parsed,
      query_tracker.as_ref(),
      fuzzy_search_options(&index.root_path, current_path, offset, limit),
    );
    let progress = picker.get_scan_progress();

    Ok(FileSearchResult {
      index_id:        index.handle.get(),
      root_path:       index.root_path.to_string_lossy().into_owned(),
      query:           query.to_string(),
      total_matched:   results.total_matched,
      total_files:     results.total_files,
      scanning:        progress.is_scanning,
      watcher_ready:   progress.is_watcher_ready,
      warmup_complete: progress.is_warmup_complete,
      items:           results
        .items
        .into_iter()
        .zip(results.scores)
        .map(|(item, score)| file_search_item(&picker, item, score.total, score.match_type, score.exact_match))
        .collect(),
    })
  }

  pub fn query_dirs(
    &self,
    index: SearchIndexHandle,
    query: &str,
    offset: usize,
    limit: usize,
    current_path: Option<&str>,
  ) -> Result<DirSearchResult, SearchError> {
    let index = self.index(index)?;
    let picker_guard = index.shared_picker.read()?;
    let picker = picker_guard.as_ref().ok_or(SearchError::UnknownIndex(index.handle))?;
    let parser = QueryParser::new(FileSearchConfig);
    let parsed = parser.parse(query);
    let results = picker.fuzzy_search_directories(
      &parsed,
      fuzzy_search_options(&index.root_path, current_path, offset, limit),
    );
    let progress = picker.get_scan_progress();

    Ok(DirSearchResult {
      index_id:        index.handle.get(),
      root_path:       index.root_path.to_string_lossy().into_owned(),
      query:           query.to_string(),
      total_matched:   results.total_matched,
      total_dirs:      results.total_dirs,
      scanning:        progress.is_scanning,
      watcher_ready:   progress.is_watcher_ready,
      warmup_complete: progress.is_warmup_complete,
      items:           results
        .items
        .into_iter()
        .zip(results.scores)
        .map(|(item, score)| dir_search_item(&picker, item, score.total, score.match_type, score.exact_match))
        .collect(),
    })
  }

  pub fn query_mixed(
    &self,
    index: SearchIndexHandle,
    query: &str,
    offset: usize,
    limit: usize,
    current_path: Option<&str>,
  ) -> Result<MixedSearchResult, SearchError> {
    let index = self.index(index)?;
    let picker_guard = index.shared_picker.read()?;
    let picker = picker_guard.as_ref().ok_or(SearchError::UnknownIndex(index.handle))?;
    let query_tracker = index.shared_query_tracker.read()?;
    let parser = QueryParser::new(FileSearchConfig);
    let parsed = parser.parse(query);
    let results = picker.fuzzy_search_mixed(
      &parsed,
      query_tracker.as_ref(),
      fuzzy_search_options(&index.root_path, current_path, offset, limit),
    );
    let progress = picker.get_scan_progress();

    Ok(MixedSearchResult {
      index_id:        index.handle.get(),
      root_path:       index.root_path.to_string_lossy().into_owned(),
      query:           query.to_string(),
      total_matched:   results.total_matched,
      total_files:     results.total_files,
      total_dirs:      results.total_dirs,
      scanning:        progress.is_scanning,
      watcher_ready:   progress.is_watcher_ready,
      warmup_complete: progress.is_warmup_complete,
      items:           results
        .items
        .into_iter()
        .zip(results.scores)
        .map(|(item, score)| mixed_search_item(&picker, item, score.total, score.match_type, score.exact_match))
        .collect(),
    })
  }

  pub fn query_grep(
    &self,
    index: SearchIndexHandle,
    query: &str,
    offset: usize,
    limit: usize,
    _current_path: Option<&str>,
    before_context: usize,
    after_context: usize,
  ) -> Result<GrepSearchResult, SearchError> {
    let index = self.index(index)?;
    let picker_guard = index.shared_picker.read()?;
    let picker = picker_guard.as_ref().ok_or(SearchError::UnknownIndex(index.handle))?;
    let parser = QueryParser::new(GrepConfig);
    let parsed = parser.parse(query);
    let results = picker.grep(&parsed, &GrepSearchOptions {
      file_offset: offset,
      page_limit: limit.max(1),
      before_context,
      after_context,
      ..Default::default()
    });
    let progress = picker.get_scan_progress();

    let items = results
      .matches
      .into_iter()
      .filter_map(|matched| {
        let file = results.files.get(matched.file_index)?;
        let path = file.absolute_path(picker, picker.base_path());
        Some(GrepSearchItem {
          kind: "grep",
          path: path.to_string_lossy().into_owned(),
          relative_path: file.relative_path(picker),
          file_name: file.file_name(picker),
          parent_path: file.dir_str(picker),
          line: matched.line_number,
          column: matched.col,
          text: matched.line_content,
          score: i32::from(matched.fuzzy_score.unwrap_or(0)),
          spans: matched
            .match_byte_offsets
            .into_iter()
            .map(|(start, end)| GrepSearchSpan { start, end })
            .collect(),
        })
      })
      .collect();

    Ok(GrepSearchResult {
      index_id: index.handle.get(),
      root_path: index.root_path.to_string_lossy().into_owned(),
      query: query.to_string(),
      total_files: results.total_files,
      total_files_searched: results.total_files_searched,
      files_with_matches: results.files_with_matches,
      next_offset: results.next_file_offset,
      regex_fallback_error: results.regex_fallback_error,
      scanning: progress.is_scanning,
      watcher_ready: progress.is_watcher_ready,
      warmup_complete: progress.is_warmup_complete,
      items,
    })
  }

  pub fn record_selection(
    &self,
    index: SearchIndexHandle,
    query: &str,
    path: &Path,
  ) -> Result<(), SearchError> {
    let index = self.index(index)?;
    let normalized_path = normalize_file_path(path)?;

    if let Some(tracker) = index.shared_frecency.read()?.as_ref() {
      tracker.track_access(&normalized_path)?;
    }
    if let Some(tracker) = index.shared_query_tracker.write()?.as_mut() {
      tracker.track_query_completion(query, &index.root_path, &normalized_path)?;
    }
    if let Some(tracker) = index.shared_frecency.read()?.as_ref() {
      let mut picker = index.shared_picker.write()?;
      if let Some(picker) = picker.as_mut() {
        picker.update_single_file_frecency(&normalized_path, tracker)?;
      }
    }

    Ok(())
  }

  pub fn read_text_preview(
    &self,
    path: &Path,
    max_bytes: usize,
    max_lines: usize,
    around_line: Option<usize>,
    context_lines: usize,
  ) -> Result<TextPreviewResult, SearchError> {
    let normalized = normalize_file_path(path)?;
    let bytes = fs::read(&normalized)?;
    let binary = bytes.contains(&0);
    if binary {
      return Ok(TextPreviewResult {
        path: normalized.to_string_lossy().into_owned(),
        text: String::new(),
        start_line: 0,
        end_line: 0,
        truncated: false,
        binary: true,
        rows: None,
        syntax: None,
      });
    }

    let text = String::from_utf8_lossy(&bytes).into_owned();
    let lines = text.lines().collect::<Vec<_>>();
    let total_lines = lines.len();
    let max_lines = max_lines.max(1);

    let (start_index, end_index) = if let Some(line) = around_line.filter(|line| *line > 0) {
      let center = line.saturating_sub(1).min(total_lines.saturating_sub(1));
      let start = center.saturating_sub(context_lines);
      let end = (center + context_lines + 1).min(total_lines);
      (start, end.max(start + 1).min(total_lines))
    } else {
      (0, total_lines.min(max_lines))
    };

    let mut snippet = if start_index < end_index {
      lines[start_index..end_index].join("\n")
    } else {
      String::new()
    };

    let mut truncated = start_index > 0 || end_index < total_lines;
    if snippet.len() > max_bytes {
      truncate_string_at_char_boundary(&mut snippet, max_bytes);
      truncated = true;
    }

    Ok(TextPreviewResult {
      path: normalized.to_string_lossy().into_owned(),
      text: snippet,
      start_line: if total_lines == 0 { 0 } else { start_index + 1 },
      end_line: if total_lines == 0 {
        0
      } else if start_index < end_index {
        end_index
      } else {
        start_index + 1
      },
      truncated,
      binary: false,
      rows: None,
      syntax: None,
    })
  }

  fn index(&self, handle: SearchIndexHandle) -> Result<&SearchIndex, SearchError> {
    self.indices.get(&handle).ok_or(SearchError::UnknownIndex(handle))
  }
}

fn normalized_root_path(path: &Path) -> Result<PathBuf, SearchError> {
  let normalized = normalize_file_path(path)?;
  Ok(if normalized.is_file() {
    normalized.parent().unwrap_or(&normalized).to_path_buf()
  } else {
    normalized
  })
}

fn fuzzy_search_options<'a>(
  root_path: &'a Path,
  current_path: Option<&'a str>,
  offset: usize,
  limit: usize,
) -> FuzzySearchOptions<'a> {
  FuzzySearchOptions {
    max_threads: 0,
    current_file: current_path,
    project_path: Some(root_path),
    combo_boost_score_multiplier: 12,
    min_combo_count: 2,
    pagination: PaginationArgs {
      offset,
      limit: limit.max(1),
    },
  }
}

fn file_search_item(
  picker: &FilePicker,
  item: &fff_search::FileItem,
  score: i32,
  match_type: &str,
  exact_match: bool,
) -> FileSearchItem {
  let path = item.absolute_path(picker, picker.base_path());
  FileSearchItem {
    kind: "file",
    path: path.to_string_lossy().into_owned(),
    relative_path: item.relative_path(picker),
    file_name: item.file_name(picker),
    parent_path: item.dir_str(picker),
    score,
    match_type: match_type.to_string(),
    exact_match,
  }
}

fn dir_search_item(
  picker: &FilePicker,
  item: &fff_search::DirItem,
  score: i32,
  match_type: &str,
  exact_match: bool,
) -> DirSearchItem {
  let path = item.absolute_path(picker, picker.base_path());
  DirSearchItem {
    kind: "dir",
    path: path.to_string_lossy().into_owned(),
    relative_path: item.relative_path(picker),
    name: item.dir_name(picker),
    score,
    match_type: match_type.to_string(),
    exact_match,
  }
}

fn mixed_search_item(
  picker: &FilePicker,
  item: MixedItemRef<'_>,
  score: i32,
  match_type: &str,
  exact_match: bool,
) -> MixedSearchItem {
  match item {
    MixedItemRef::File(file) => MixedSearchItem::File {
      path: file.absolute_path(picker, picker.base_path()).to_string_lossy().into_owned(),
      relative_path: file.relative_path(picker),
      file_name: file.file_name(picker),
      parent_path: file.dir_str(picker),
      score,
      match_type: match_type.to_string(),
      exact_match,
    },
    MixedItemRef::Dir(dir) => MixedSearchItem::Dir {
      path: dir.absolute_path(picker, picker.base_path()).to_string_lossy().into_owned(),
      relative_path: dir.relative_path(picker),
      name: dir.dir_name(picker),
      score,
      match_type: match_type.to_string(),
      exact_match,
    },
  }
}

fn truncate_string_at_char_boundary(text: &mut String, max_bytes: usize) {
  if text.len() <= max_bytes {
    return;
  }
  let mut boundary = max_bytes.min(text.len());
  while boundary > 0 && !text.is_char_boundary(boundary) {
    boundary -= 1;
  }
  text.truncate(boundary);
}

