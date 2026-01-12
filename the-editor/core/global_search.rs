use std::{
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use anyhow::{Result, anyhow};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{BinaryDetection, SearcherBuilder, sinks};
use ignore::{DirEntry, WalkBuilder, WalkState};
use ropey::Rope;
use the_editor_loader as loader;

use crate::editor::FilePickerConfig;

pub(crate) fn filter_picker_entry(entry: &DirEntry, root: &Path, dedup_symlinks: bool) -> bool {
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
      .map(|path| !path.starts_with(root))
      .unwrap_or(true);
  }

  true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResult {
  pub path: PathBuf,
  pub line_num: usize,
  pub line_text: String,
}

impl FileResult {
  #[inline]
  pub fn new(path: &Path, line_num: usize, line_text: &str) -> Self {
    Self {
      path: path.to_path_buf(),
      line_num,
      line_text: line_text.to_string(),
    }
  }
}

#[derive(Clone)]
pub struct SearchOptions {
  pub smart_case: bool,
  pub file_picker: FilePickerConfig,
  pub documents: Arc<Vec<(Option<PathBuf>, Rope)>>,
}

pub enum MatchControl {
  Continue,
  Stop,
}

pub(crate) fn walk_workspace_matches(
  query: &str,
  search_root: &Path,
  options: &SearchOptions,
  handler: Arc<dyn Fn(FileResult) -> MatchControl + Send + Sync>,
) -> Result<()> {
  if query.is_empty() {
    return Ok(());
  }

  if !search_root.exists() {
    return Err(anyhow!("search root does not exist"));
  }

  let matcher = RegexMatcherBuilder::new()
    .case_smart(options.smart_case)
    .build(query)?;

  let search_root = search_root.to_path_buf();
  let absolute_root = search_root
    .canonicalize()
    .unwrap_or_else(|_| search_root.clone());

  let searcher = SearcherBuilder::new()
    .binary_detection(BinaryDetection::quit(b'\x00'))
    .build();

  WalkBuilder::new(&search_root)
    .hidden(options.file_picker.hidden)
    .parents(options.file_picker.parents)
    .ignore(options.file_picker.ignore)
    .follow_links(options.file_picker.follow_symlinks)
    .git_ignore(options.file_picker.git_ignore)
    .git_global(options.file_picker.git_global)
    .git_exclude(options.file_picker.git_exclude)
    .max_depth(options.file_picker.max_depth)
    .filter_entry({
      let absolute_root = absolute_root.clone();
      let dedup_symlinks = options.file_picker.deduplicate_links;
      move |entry| filter_picker_entry(entry, &absolute_root, dedup_symlinks)
    })
    .add_custom_ignore_filename(loader::config_dir().join("ignore"))
    .add_custom_ignore_filename(".helix/ignore")
    .build_parallel()
    .run(|| {
      let matcher = matcher.clone();
      let mut searcher = searcher.clone();
      let handler = Arc::clone(&handler);
      let documents = Arc::clone(&options.documents);

      Box::new(move |entry: Result<DirEntry, ignore::Error>| -> WalkState {
        let entry = match entry {
          Ok(entry) => entry,
          Err(err) => {
            log::warn!("Global search walker error: {}", err);
            return WalkState::Continue;
          },
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
          return WalkState::Continue;
        }

        let path = entry.path();
        let path_buf = path.to_path_buf();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let sink_path = path_buf.clone();
        let handler_for_sink = Arc::clone(&handler);
        let stop_for_sink = Arc::clone(&stop_flag);
        let sink = sinks::UTF8(move |line_num, line_content| {
          let line = line_num.saturating_sub(1) as usize;
          let result = FileResult::new(&sink_path, line, line_content);
          let keep_running = matches!((handler_for_sink)(result), MatchControl::Continue);
          if !keep_running {
            stop_for_sink.store(true, Ordering::Relaxed);
          }
          Ok(keep_running)
        });

        let result = if let Some((_, doc)) = documents.iter().find(|(doc_path, _)| {
          doc_path
            .as_ref()
            .is_some_and(|doc_path| doc_path == &path_buf)
        }) {
          let text = doc.to_string();
          searcher.search_slice(&matcher, text.as_bytes(), sink)
        } else {
          searcher.search_path(&matcher, &path_buf, sink)
        };

        if let Err(err) = result {
          log::error!("Global search error: {}, {}", path_buf.display(), err);
        }

        if stop_flag.load(Ordering::Relaxed) {
          WalkState::Quit
        } else {
          WalkState::Continue
        }
      })
    });

  Ok(())
}

#[cfg(test)]
mod tests;
