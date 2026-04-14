use std::{
  collections::{
    HashMap,
    hash_map::DefaultHasher,
  },
  hash::{
    Hash,
    Hasher,
  },
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    Mutex,
    OnceLock,
    atomic::AtomicBool,
  },
  time::Duration,
};

use fff_search::{
  FFFMode,
  FilePicker as FffFilePicker,
  FilePickerOptions as FffFilePickerOptions,
  FrecencyTracker,
  FuzzySearchOptions,
  GrepMode,
  GrepSearchOptions,
  Location,
  PaginationArgs,
  QueryParser,
  QueryTracker,
  SharedFrecency,
  SharedPicker,
  SharedQueryTracker,
  grep_search,
  parse_grep_query,
};

const SEARCH_READY_TIMEOUT: Duration = Duration::from_millis(200);
const FILE_SEARCH_COMBO_BOOST_SCORE_MULTIPLIER: i32 = 100;
const FILE_SEARCH_MIN_COMBO_COUNT: u32 = 3;
const FILE_SEARCH_OVERFETCH_MULTIPLIER: usize = 8;

static WORKSPACE_BACKENDS: OnceLock<Mutex<HashMap<PathBuf, Arc<FffWorkspaceBackend>>>> =
  OnceLock::new();

#[derive(Debug, Clone)]
pub(crate) struct FffFileSearchHit {
  pub path:     PathBuf,
  pub location: Option<Location>,
}

#[derive(Debug, Clone)]
pub(crate) struct FffFileSearchResponse {
  pub hits:          Vec<FffFileSearchHit>,
  pub total_matched: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct FffGrepMatch {
  pub path:                 PathBuf,
  pub line_number_one_based: usize,
  pub line_text:            String,
  pub match_bytes:          Vec<(usize, usize)>,
}

#[derive(Debug, Clone)]
pub(crate) struct FffGrepResponse {
  pub matches: Vec<FffGrepMatch>,
  pub indexing: bool,
  pub error:   Option<String>,
}

#[derive(Debug)]
struct FffWorkspaceBackend {
  root:                 PathBuf,
  shared_picker:        SharedPicker,
  shared_frecency:      SharedFrecency,
  shared_query_tracker: SharedQueryTracker,
}

pub(crate) fn file_search_overfetch_limit(limit: usize) -> usize {
  limit
    .max(1)
    .saturating_mul(FILE_SEARCH_OVERFETCH_MULTIPLIER)
}

pub(crate) fn search_files(
  root: &Path,
  query: &str,
  current_file: Option<&Path>,
  limit: usize,
) -> Result<FffFileSearchResponse, String> {
  workspace_backend(root)?.search_files(query, current_file, limit)
}

pub(crate) fn search_grep(
  root: &Path,
  query: &str,
  smart_case: bool,
  limit: usize,
  cancel: &AtomicBool,
) -> Result<FffGrepResponse, String> {
  search_grep_with_mode(root, query, smart_case, limit, GrepMode::Regex, cancel)
}

pub(crate) fn search_grep_with_mode(
  root: &Path,
  query: &str,
  smart_case: bool,
  limit: usize,
  mode: GrepMode,
  cancel: &AtomicBool,
) -> Result<FffGrepResponse, String> {
  workspace_backend(root)?.search_grep(query, smart_case, limit, mode, cancel)
}

pub(crate) fn track_file_selection(root: &Path, query: &str, path: &Path) {
  if let Ok(backend) = workspace_backend(root) {
    backend.track_file_selection(query, path);
  }
}

pub(crate) fn track_grep_selection(root: &Path, query: &str, path: &Path) {
  if let Ok(backend) = workspace_backend(root) {
    backend.track_grep_selection(query, path);
  }
}

pub(crate) fn track_grep_query(root: &Path, query: &str) {
  if let Ok(backend) = workspace_backend(root) {
    backend.track_grep_query(query);
  }
}

fn workspace_backend(root: &Path) -> Result<Arc<FffWorkspaceBackend>, String> {
  let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
  let cache = WORKSPACE_BACKENDS.get_or_init(|| Mutex::new(HashMap::new()));

  if let Some(existing) = cache
    .lock()
    .map_err(|_| "fff backend cache lock poisoned".to_string())?
    .get(&root)
    .cloned()
  {
    return Ok(existing);
  }

  let backend = Arc::new(FffWorkspaceBackend::new(root.clone())?);
  cache
    .lock()
    .map_err(|_| "fff backend cache lock poisoned".to_string())?
    .insert(root, backend.clone());
  Ok(backend)
}

impl FffWorkspaceBackend {
  fn new(root: PathBuf) -> Result<Self, String> {
    let db_root = backend_db_root(&root);
    std::fs::create_dir_all(&db_root)
      .map_err(|err| format!("failed to create fff cache dir '{}': {err}", db_root.display()))?;

    let shared_picker = SharedPicker::default();
    let shared_frecency = SharedFrecency::default();
    let shared_query_tracker = SharedQueryTracker::default();

    let frecency = FrecencyTracker::new(db_root.join("frecency"), false)
      .map_err(|err| format!("failed to initialize fff frecency db: {err}"))?;
    shared_frecency
      .init(frecency)
      .map_err(|err| format!("failed to initialize shared fff frecency: {err}"))?;

    let query_tracker = QueryTracker::new(db_root.join("queries"), false)
      .map_err(|err| format!("failed to initialize fff query db: {err}"))?;
    shared_query_tracker
      .init(query_tracker)
      .map_err(|err| format!("failed to initialize shared fff query tracker: {err}"))?;

    FffFilePicker::new_with_shared_state(
      shared_picker.clone(),
      shared_frecency.clone(),
      FffFilePickerOptions {
        base_path:         root.display().to_string(),
        warmup_mmap_cache: false,
        mode:              FFFMode::Ai,
        cache_budget:      None,
        watch:             true,
      },
    )
    .map_err(|err| format!("failed to initialize fff file picker: {err}"))?;

    Ok(Self {
      root,
      shared_picker,
      shared_frecency,
      shared_query_tracker,
    })
  }

  fn search_files(
    &self,
    query: &str,
    current_file: Option<&Path>,
    limit: usize,
  ) -> Result<FffFileSearchResponse, String> {
    let _ = self.shared_picker.wait_for_scan(SEARCH_READY_TIMEOUT);
    let picker_guard = self
      .shared_picker
      .read()
      .map_err(|err| format!("failed to lock fff picker: {err}"))?;
    let picker = picker_guard
      .as_ref()
      .ok_or_else(|| "fff picker was not initialized".to_string())?;
    let query_tracker_guard = self
      .shared_query_tracker
      .read()
      .map_err(|err| format!("failed to lock fff query tracker: {err}"))?;
    let query_tracker = query_tracker_guard.as_ref();

    let parser = QueryParser::default();
    let parsed = parser.parse(query);
    let result = FffFilePicker::fuzzy_search(
      picker.get_files(),
      &parsed,
      query_tracker,
      FuzzySearchOptions {
        max_threads:                  0,
        current_file:                 current_file.and_then(|path| path.to_str()),
        project_path:                 Some(self.root.as_path()),
        combo_boost_score_multiplier: FILE_SEARCH_COMBO_BOOST_SCORE_MULTIPLIER,
        min_combo_count:              FILE_SEARCH_MIN_COMBO_COUNT,
        pagination:                   PaginationArgs { offset: 0, limit },
      },
    );

    Ok(FffFileSearchResponse {
      hits: result
        .items
        .into_iter()
        .map(|item| FffFileSearchHit {
          path:     item.as_path().to_path_buf(),
          location: result.location,
        })
        .collect(),
      total_matched: result.total_matched,
    })
  }

  fn search_grep(
    &self,
    query: &str,
    smart_case: bool,
    limit: usize,
    mode: GrepMode,
    cancel: &AtomicBool,
  ) -> Result<FffGrepResponse, String> {
    let _ = self.shared_picker.wait_for_scan(SEARCH_READY_TIMEOUT);
    let picker_guard = self
      .shared_picker
      .read()
      .map_err(|err| format!("failed to lock fff picker: {err}"))?;
    let picker = picker_guard
      .as_ref()
      .ok_or_else(|| "fff picker was not initialized".to_string())?;
    let parsed = parse_grep_query(query);
    let options = GrepSearchOptions {
      smart_case,
      page_limit: limit.max(1),
      max_matches_per_file: 200,
      time_budget_ms: 250,
      mode,
      ..GrepSearchOptions::default()
    };
    let overlay_guard = picker.bigram_overlay().map(|overlay| overlay.read());
    let result = grep_search(
      picker.get_files(),
      &parsed,
      &options,
      picker.cache_budget(),
      picker.bigram_index(),
      overlay_guard.as_deref(),
      Some(cancel),
    );

    Ok(FffGrepResponse {
      matches: result
        .matches
        .into_iter()
        .filter_map(|matched| {
          let file = result.files.get(matched.file_index)?;
          Some(FffGrepMatch {
            path:                 file.as_path().to_path_buf(),
            line_number_one_based: matched.line_number as usize,
            line_text:            matched.line_content,
            match_bytes:          matched
              .match_byte_offsets
              .into_iter()
              .map(|(start, end)| (start as usize, end as usize))
              .collect(),
          })
        })
        .collect(),
      indexing: picker.get_scan_progress().is_scanning,
      error:   result.regex_fallback_error,
    })
  }

  fn track_file_selection(&self, query: &str, path: &Path) {
    if let Ok(frecency_guard) = self.shared_frecency.read()
      && let Some(frecency) = frecency_guard.as_ref()
    {
      let _ = frecency.track_access(path);
    }
    if query.trim().is_empty() {
      return;
    }
    if let Ok(mut query_tracker_guard) = self.shared_query_tracker.write()
      && let Some(query_tracker) = query_tracker_guard.as_mut()
    {
      let _ = query_tracker.track_query_completion(query, self.root.as_path(), path);
    }
  }

  fn track_grep_selection(&self, query: &str, path: &Path) {
    if let Ok(frecency_guard) = self.shared_frecency.read()
      && let Some(frecency) = frecency_guard.as_ref()
    {
      let _ = frecency.track_access(path);
    }
    self.track_grep_query(query);
  }

  fn track_grep_query(&self, query: &str) {
    if query.trim().is_empty() {
      return;
    }
    if let Ok(mut query_tracker_guard) = self.shared_query_tracker.write()
      && let Some(query_tracker) = query_tracker_guard.as_mut()
    {
      let _ = query_tracker.track_grep_query(query, self.root.as_path());
    }
  }
}

fn backend_db_root(root: &Path) -> PathBuf {
  let mut hasher = DefaultHasher::new();
  root.hash(&mut hasher);
  let root_hash = format!("{:016x}", hasher.finish());
  the_loader::cache_dir().join("fff-search").join(root_hash)
}
